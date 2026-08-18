//! Stage Data Contract validation (P1-5B Formal).
//!
//! Phase 1 scope (see docs/specs/2026-08-18-stage-data-contract-design.md):
//! static contract checks only. Python structured transport is Phase 2.

use std::collections::HashMap;

use acos_core::types::{CirNode, CirNodeKind, CirProgram, OutputSpec};

use crate::CompilerError;

/// Extracts every `${...}` reference (including dotted paths) from a JSON value.
pub fn extract_refs(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    match value {
        serde_json::Value::String(s) => {
            let bytes = s.as_bytes();
            let mut i = 0;
            while i + 1 < bytes.len() {
                if bytes[i] == b'$' && bytes[i + 1] == b'{' {
                    if let Some(end) = s[i + 2..].find('}') {
                        out.push(s[i + 2..i + 2 + end].to_string());
                        i += 2 + end;
                    }
                }
                i += 1;
            }
        }
        serde_json::Value::Array(items) => for v in items { out.extend(extract_refs(v)); },
        serde_json::Value::Object(map) => for v in map.values() { out.extend(extract_refs(v)); },
        _ => {}
    }
    out
}

/// Validates R1 (binding existence), R2 (structural reachability), R3 (type
/// alignment), R4 (field paths), R5 (output completeness) and item-var rules.
pub fn validate_data_contract(program: &CirProgram) -> Result<(), CompilerError> {
    // R5: every declared output must be complete.
    for n in &program.nodes {
        if let Some(o) = &n.output {
            if o.name.trim().is_empty() || o.type_name.trim().is_empty() {
                return Err(CompilerError::DataContractViolation {
                    node_id: n.node_id.clone(),
                    message: format!("output schema incomplete (name='{}', type_name='{}')", o.name, o.type_name),
                });
            }
        }
    }

    // Top-level binding names (R1 resolution + item-var shadowing check).
    let mut producers: HashMap<String, (String, OutputSpec)> = HashMap::new();
    for n in &program.nodes {
        if let Some(o) = &n.output {
            producers.insert(o.name.clone(), (n.node_id.clone(), o.clone()));
        }
    }

    // Walk the graph structurally, threading a visible-binding set per scope.
    let mut entry_nodes: Vec<&CirNode> = Vec::new();
    let by_id: HashMap<&str, &CirNode> = program.nodes.iter().map(|n| (n.node_id.as_str(), n)).collect();
    for e in &program.entry {
        if let Some(n) = by_id.get(e.as_str()) { entry_nodes.push(n); }
    }

    // R1 + R2: walk with scope threading; `walk` returns the bindings its
    // children produce for the enclosing scope.
    fn walk<'a>(node: &'a CirNode, by_id: &HashMap<&str, &'a CirNode>, scope: &mut HashMap<String, OutputSpec>, producers: &HashMap<String, (String, OutputSpec)>) -> Result<HashMap<String, OutputSpec>, CompilerError> {
        // Deviation from plan skeleton: check_node mount point (plan Task 6
        // documents "check_node 对每个进入 walk 的节点" — the plan's own test
        // rejects_unresolved_binding requires non-entry refs to be checked).
        check_node(node, scope)?;
        let mut produced: HashMap<String, OutputSpec> = HashMap::new();
        match node.kind {
            CirNodeKind::Sequence => {
                if node.output.is_some() {
                    return Err(CompilerError::DataContractViolation {
                        node_id: node.node_id.clone(),
                        message: "container node 'sequence' cannot declare output (runtime binds only primitive and loop outputs)".into(),
                    });
                }
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    // children see current scope + earlier siblings' outputs
                    let mut child_scope = scope.clone();
                    for (k, v) in &produced { child_scope.insert(k.clone(), v.clone()); }
                    let child_produced = walk(child, by_id, &mut child_scope, producers)?;
                    for (k, v) in child_produced { produced.insert(k, v); }
                }
                // sequence else_children: branch-local scope, outputs do not escape
                for child_id in &node.else_children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    let mut child_scope = scope.clone();
                    walk(child, by_id, &mut child_scope, producers)?;
                }
            }
            CirNodeKind::Parallel => {
                if node.output.is_some() {
                    return Err(CompilerError::DataContractViolation {
                        node_id: node.node_id.clone(),
                        message: "container node 'parallel' cannot declare output (runtime binds only primitive and loop outputs)".into(),
                    });
                }
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    // parallel branches share nothing; each gets the incoming scope only,
                    // but once the block completes, branch outputs are visible to siblings (R2)
                    let mut child_scope = scope.clone();
                    let child_produced = walk(child, by_id, &mut child_scope, producers)?;
                    for (k, v) in child_produced { produced.insert(k, v); }
                }
            }
            CirNodeKind::Conditional => {
                if node.output.is_some() {
                    return Err(CompilerError::DataContractViolation {
                        node_id: node.node_id.clone(),
                        message: "container node 'conditional' cannot declare output (runtime binds only primitive and loop outputs)".into(),
                    });
                }
                for child_id in node.children.iter().chain(node.else_children.iter()) {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    let mut branch_scope = scope.clone();
                    walk(child, by_id, &mut branch_scope, producers)?;
                    // branch-produced bindings do NOT escape (constraint 8)
                }
            }
            CirNodeKind::LoopMap => {
                let spec = node.control.as_ref().and_then(|c| c.loop_spec.as_ref());
                let mut body_scope = scope.clone();
                if let Some(item_var) = spec.and_then(|s| s.item_var.clone()) {
                    if producers.contains_key(&item_var) {
                        return Err(CompilerError::DataContractViolation {
                            node_id: node.node_id.clone(),
                            message: format!("loop item_var '{item_var}' shadows an existing top-level binding"),
                        });
                    }
                    body_scope.insert(item_var.clone(), OutputSpec { name: item_var.clone(), type_name: "Any".into(), fields: vec![] });
                }
                let mut body_produced: HashMap<String, OutputSpec> = HashMap::new();
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    let mut child_scope = body_scope.clone();
                    for (k, v) in &body_produced { child_scope.insert(k.clone(), v.clone()); }
                    let child_produced = walk(child, by_id, &mut child_scope, producers)?;
                    for (k, v) in child_produced { body_produced.insert(k, v); }
                }
                // loop aggregate output: List<T> where T = last body child output type
                if let Some(o) = &node.output {
                    let last_type = node.children.iter().rev().find_map(|cid| by_id.get(cid.as_str()).and_then(|c| c.output.as_ref()).map(|o| o.type_name.clone()));
                    match last_type {
                        Some(t) => {
                            let expected = format!("List<{t}>");
                            if o.type_name != expected {
                                return Err(CompilerError::DataContractViolation {
                                    node_id: node.node_id.clone(),
                                    message: format!("loop aggregate output type '{}' must be '{}'", o.type_name, expected),
                                });
                            }
                        }
                        None => return Err(CompilerError::DataContractViolation {
                            node_id: node.node_id.clone(),
                            message: format!("loop output '{}' declared but no body child produces a value", o.name),
                        }),
                    }
                }
            }
            _ => {}
        }
        // R2: a node's own output is visible to its enclosing scope — but only
        // kinds whose outputs the runtime actually binds (primitive invocations
        // and loop aggregates; containers are rejected above).
        if node.kind == CirNodeKind::PrimitiveInvocation {
            if let Some(o) = &node.output {
                produced.insert(o.name.clone(), o.clone());
            }
        }
        Ok(produced)
    }

    // check references against the visible set
    fn check_node(node: &CirNode, scope: &HashMap<String, OutputSpec>) -> Result<(), CompilerError> {
        for (key, val) in &node.inputs {
            for raw in extract_refs(val) {
                let spec = check_ref(node, &raw, scope)?;
                if let Some(expected) = node.input_types.get(key) {
                    let same = expected.eq_ignore_ascii_case(&spec.type_name);
                    let num_int = (expected.eq_ignore_ascii_case("number") && spec.type_name.eq_ignore_ascii_case("integer"))
                        || (expected.eq_ignore_ascii_case("integer") && spec.type_name.eq_ignore_ascii_case("number"));
                    if !same && !num_int {
                        return Err(CompilerError::DataContractViolation {
                            node_id: node.node_id.clone(),
                            message: format!("input '{key}' expects '{expected}' but producer '{}' declares '{}'", spec.name, spec.type_name),
                        });
                    }
                }
            }
        }
        // R1 also covers control references (loop_spec.input, condition
        // expressions, retry config): scan the serialized control spec the
        // same way as inputs. input_types do not apply to control fields.
        if let Some(control) = &node.control {
            if let Ok(value) = serde_json::to_value(control) {
                for raw in extract_refs(&value) {
                    check_ref(node, &raw, scope)?;
                }
            }
        }
        Ok(())
    }

    // R1 resolution + R4 field paths for a single `${...}` reference.
    // Returns the resolved producer spec (used by the caller for R3 checks).
    fn check_ref<'a>(node: &CirNode, raw: &str, scope: &'a HashMap<String, OutputSpec>) -> Result<&'a OutputSpec, CompilerError> {
        let mut parts = raw.split('.');
        let name = parts.next().unwrap_or("");
        if name.contains('[') || name.contains(']') {
            return Err(CompilerError::DataContractViolation {
                node_id: node.node_id.clone(),
                message: format!("binding '{name}' uses dynamic indexing, not supported in Phase 1"),
            });
        }
        let spec = scope.get(name).ok_or_else(|| CompilerError::UnresolvedBinding { node_id: node.node_id.clone(), binding: name.to_string() })?;
        for field in parts {
            if field.contains('[') || field.contains(']') {
                return Err(CompilerError::DataContractViolation {
                    node_id: node.node_id.clone(),
                    message: format!("field '{field}' of '{name}' uses dynamic indexing, not supported in Phase 1"),
                });
            }
            let f = spec.fields.iter().find(|f| f.name == field).ok_or_else(|| CompilerError::DataContractViolation {
                node_id: node.node_id.clone(),
                message: format!("binding '{name}' has no field '{field}'"),
            })?;
            if f.type_name == "List" || f.type_name == "Record" {
                return Err(CompilerError::DataContractViolation {
                    node_id: node.node_id.clone(),
                    message: format!("field '{field}' of '{name}' requires indexing (Phase 2)"),
                });
            }
        }
        Ok(spec)
    }

    // Sibling entries share one top-level scope (declared entry order, not
    // node array order): earlier entries' outputs are visible to later
    // entries' own references (e.g. a loop entry whose input comes from a
    // producer entry).
    let mut top_scope: HashMap<String, OutputSpec> = HashMap::new();
    for e in &entry_nodes {
        check_node(e, &top_scope)?;
        let produced = walk(e, &by_id, &mut top_scope, &producers)?;
        top_scope.extend(produced);
    }
    Ok(())
}

#[cfg(test)]
#[allow(unused_mut)]
mod tests {
    use super::*;
    use acos_core::id::{ProgramId, TaskId};
    use acos_core::types::{ConditionSpec, ControlSpec, FieldSpec, LoopKind, LoopSpec};

    type OutputArgs = Option<(&'static str, &'static str, Vec<(&'static str, &'static str)>)>;

    fn node(id: &str, output: OutputArgs) -> CirNode {
        CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: id.into(),
            capability: Some("execute_python".into()),
            output: output.map(|(name, ty, fields)| OutputSpec {
                name: name.into(),
                type_name: ty.into(),
                fields: fields.into_iter().map(|(n, t)| FieldSpec { name: n.into(), type_name: t.into() }).collect(),
            }),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            input_types: HashMap::new(),
            control: None,
        }
    }

    fn program(entry: Vec<&str>, nodes: Vec<CirNode>) -> CirProgram {
        CirProgram { id: ProgramId::new(), task_id: TaskId(uuid::Uuid::new_v4()), entry: entry.into_iter().map(String::from).collect(), nodes, effects: vec![] }
    }

    fn seq_root(children: Vec<&str>) -> CirNode {
        CirNode { kind: CirNodeKind::Sequence, node_id: "root".into(), capability: None, output: None,
            children: children.into_iter().map(String::from).collect(), else_children: vec![],
            inputs: HashMap::new(), input_types: HashMap::new(), control: None }
    }

    #[test]
    fn parallel_outputs_visible_after_block() {
        let mut a = node("a", Some(("doc_a", "Document", vec![])));
        let mut c = node("c", None);
        c.inputs.insert("text".into(), serde_json::Value::String("${doc_a}".into()));
        let mut par = CirNode { kind: CirNodeKind::Parallel, node_id: "par".into(), capability: None,
            output: None, children: vec!["a".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: None };
        let mut root = seq_root(vec!["par", "c"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root, par, a, c])).is_ok());
    }

    #[test]
    fn dotted_field_path_must_exist_in_schema() {
        let mut a = node("a", Some(("vr", "ValidationResult",
            vec![("total_issues", "Integer"), ("issues", "List")])));
        let mut b = node("b", None);
        b.inputs.insert("code".into(), serde_json::Value::String("n = ${vr.total_issues}".into()));
        let mut root = seq_root(vec!["a", "b"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root.clone(), a.clone(), b.clone()])).is_ok());
        b.inputs.insert("code".into(), serde_json::Value::String("n = ${vr.missing_field}".into()));
        let err = validate_data_contract(&program(vec!["root"], vec![root, a, b])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    }

    #[test]
    fn dynamic_index_paths_rejected_in_phase1() {
        let mut a = node("a", Some(("all", "List<String>", vec![])));
        let mut b = node("b", None);
        b.inputs.insert("code".into(), serde_json::Value::String("x = ${all[0]}".into()));
        let mut root = seq_root(vec!["a", "b"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root, a, b])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    }

    #[test]
    fn loop_input_reference_must_resolve() {
        let mut body = node("body", Some(("per", "String", vec![])));
        let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
            output: None, children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
                loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                    input: Some("${missing_items}".into()), item_var: Some("item".into()) }), retry: None }) };
        let mut root = seq_root(vec!["loop"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root, loop_node, body])).unwrap_err();
        assert!(matches!(err, CompilerError::UnresolvedBinding { ref binding, .. } if binding == "missing_items"));
    }

    #[test]
    fn item_var_visible_inside_loop_body_not_outside() {
        let src = node("src", Some(("items", "List<String>", vec![])));
        let mut body = node("body", Some(("per", "String", vec![])));
        body.inputs.insert("text".into(), serde_json::Value::String("${item}".into()));
        let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
            output: None, children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
                loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                    input: Some("${items}".into()), item_var: Some("item".into()) }), retry: None }) };
        let mut root = seq_root(vec!["src", "loop"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root.clone(), src.clone(), loop_node.clone(), body.clone()])).is_ok());
        let mut after = node("after", None);
        after.inputs.insert("code".into(), serde_json::Value::String("x = ${item}".into()));
        let mut root2 = seq_root(vec!["src", "loop", "after"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root2, src, loop_node, body, after])).unwrap_err();
        assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
    }

    #[test]
    fn item_var_shadowing_top_level_binding_rejected() {
        let files = node("files_src", Some(("files", "List<String>", vec![])));
        let mut src = node("src", Some(("file_path", "String", vec![])));
        let mut body = node("body", Some(("per", "String", vec![])));
        let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
            output: None, children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
                loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                    input: Some("${files}".into()), item_var: Some("file_path".into()) }), retry: None }) };
        let mut root = seq_root(vec!["files_src", "src", "loop"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root, files, src, loop_node, body])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    }

    #[test]
    fn loop_aggregate_type_must_be_list_of_last_child_type() {
        let src = node("src", Some(("items", "List<String>", vec![])));
        let mut body = node("body", Some(("vr", "ValidationResult", vec![])));
        let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
            output: Some(OutputSpec { name: "all_results".into(), type_name: "List<ValidationResult>".into(), fields: vec![] }),
            children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
                loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                    input: Some("${items}".into()), item_var: Some("item".into()) }), retry: None }) };
        let mut root = seq_root(vec!["src", "loop"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root.clone(), src.clone(), loop_node.clone(), body.clone()])).is_ok());
        loop_node.output.as_mut().unwrap().type_name = "ValidationResult".into();
        let err = validate_data_contract(&program(vec!["root"], vec![root, src, loop_node, body])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    }

    #[test]
    fn incomplete_output_schema_rejected() {
        let mut a = node("a", Some(("x", "", vec![])));
        let mut root = seq_root(vec!["a"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root, a])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    }

    #[test]
    fn container_node_output_rejected() {
        let mut child = node("a", Some(("x", "String", vec![])));
        let root = CirNode { kind: CirNodeKind::Sequence, node_id: "root".into(), capability: None,
            output: Some(OutputSpec { name: "container_out".into(), type_name: "String".into(), fields: vec![] }),
            children: vec!["a".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: None };
        let err = validate_data_contract(&program(vec!["root"], vec![root, child])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { ref message, .. } if message.contains("cannot declare output")));
    }

    #[test]
    fn type_match_is_case_insensitive() {
        let a = node("a", Some(("stats", "Number", vec![])));
        let mut b = node("b", None);
        b.inputs.insert("stats".into(), serde_json::Value::String("${stats}".into()));
        b.input_types.insert("stats".into(), "number".into());
        let root = seq_root(vec!["a", "b"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root, a, b])).is_ok());
    }

    #[test]
    fn rejects_unresolved_binding() {
        let mut consumer = node("cons", None);
        consumer.inputs.insert("code".into(), serde_json::Value::String("data = ${processed_data}".into()));
        let root = CirNode { kind: CirNodeKind::Sequence, node_id: "root".into(), capability: None,
            output: None, children: vec!["cons".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: None };
        let p = program(vec!["root"], vec![root, consumer]);
        let err = validate_data_contract(&p).unwrap_err();
        assert!(matches!(err, CompilerError::UnresolvedBinding { ref binding, .. } if binding == "processed_data"));
    }

    #[test]
    fn sequence_allows_earlier_sibling_output() {
        let a = node("a", Some(("doc", "Document", vec![])));
        let mut b = node("b", None);
        b.inputs.insert("text".into(), serde_json::Value::String("${doc}".into()));
        let root = seq_root(vec!["a", "b"]);
        assert!(validate_data_contract(&program(vec!["root"], vec![root, a, b])).is_ok());
    }

    #[test]
    fn parallel_branches_do_not_share_outputs() {
        let a = node("a", Some(("doc", "Document", vec![])));
        let mut b = node("b", None);
        b.inputs.insert("text".into(), serde_json::Value::String("${doc}".into()));
        let root = CirNode { kind: CirNodeKind::Parallel, node_id: "root".into(), capability: None,
            output: None, children: vec!["a".into(), "b".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: None };
        let err = validate_data_contract(&program(vec!["root"], vec![root, a, b])).unwrap_err();
        assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
    }

    #[test]
    fn conditional_branch_output_unusable_outside() {
        let a = node("a", Some(("branch_result", "String", vec![])));
        let cond = CirNode { kind: CirNodeKind::Conditional, node_id: "cond".into(), capability: None,
            output: None, children: vec!["a".into()], else_children: vec![], inputs: HashMap::new(),
            input_types: HashMap::new(), control: Some(ControlSpec { condition: Some(ConditionSpec { expression: "true".into() }), loop_spec: None, retry: None }) };
        let mut after = node("after", None);
        after.inputs.insert("code".into(), serde_json::Value::String("x = ${branch_result}".into()));
        let root = seq_root(vec!["cond", "after"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root, cond, a, after])).unwrap_err();
        assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
    }

    #[test]
    fn type_mismatch_rejected_but_number_integer_compatible() {
        let a = node("a", Some(("stats", "CsvAnalysisResult", vec![])));
        let mut b = node("b", None);
        b.inputs.insert("stats".into(), serde_json::Value::String("${stats}".into()));
        b.input_types.insert("stats".into(), "OtherType".into());
        let root = seq_root(vec!["a", "b"]);
        let err = validate_data_contract(&program(vec!["root"], vec![root.clone(), a.clone(), b.clone()])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
        b.input_types.insert("stats".into(), "number".into());
        // producer declares CsvAnalysisResult — not numeric: still violation
        let err = validate_data_contract(&program(vec!["root"], vec![root.clone(), a.clone(), b.clone()])).unwrap_err();
        assert!(matches!(err, CompilerError::DataContractViolation { .. }));
        // exact match passes
        b.input_types.insert("stats".into(), "CsvAnalysisResult".into());
        assert!(validate_data_contract(&program(vec!["root"], vec![root, a, b])).is_ok());
    }
}
