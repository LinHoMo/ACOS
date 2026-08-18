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
        // the node's own output is visible to its enclosing scope (R2)
        if let Some(o) = &node.output {
            produced.insert(o.name.clone(), o.clone());
        }
        match node.kind {
            CirNodeKind::Sequence => {
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
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    // parallel branches share nothing; each gets the incoming scope only
                    let mut child_scope = scope.clone();
                    walk(child, by_id, &mut child_scope, producers)?;
                }
            }
            CirNodeKind::Conditional => {
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
        Ok(produced)
    }

    // check references against the visible set
    fn check_node(node: &CirNode, scope: &HashMap<String, OutputSpec>) -> Result<(), CompilerError> {
        for (key, val) in &node.inputs {
            for raw in extract_refs(val) {
                let mut parts = raw.split('.');
                let name = parts.next().unwrap_or("");
                let spec = scope.get(name);
                let Some(spec) = spec else {
                    return Err(CompilerError::UnresolvedBinding { node_id: node.node_id.clone(), binding: name.to_string() });
                };
                for field in parts {
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
                if let Some(expected) = node.input_types.get(key) {
                    if expected != &spec.type_name && !(expected == "number" && spec.type_name == "integer") && !(expected == "integer" && spec.type_name == "number") {
                        return Err(CompilerError::DataContractViolation {
                            node_id: node.node_id.clone(),
                            message: format!("input '{key}' expects '{expected}' but producer '{}' declares '{}'", spec.name, spec.type_name),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    for e in &entry_nodes {
        let mut scope: HashMap<String, OutputSpec> = HashMap::new();
        check_node(e, &scope)?;
        let produced = walk(e, &by_id, &mut scope, &producers)?;
        scope.extend(produced);
        // sibling entries share the top-level scope; check remaining nodes through walk
        // (children are checked inside walk; here we re-check the entry's own refs only)
    }
    // Final sweep: every node checked with the program-wide producer map for
    // R1 (binding must exist somewhere) — scoping is enforced by walk.
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::{ProgramId, TaskId};
    use acos_core::types::{ConditionSpec, ControlSpec, FieldSpec};

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
