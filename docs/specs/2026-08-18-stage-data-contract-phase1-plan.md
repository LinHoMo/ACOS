# Stage Data Contract Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在编译期实现 CIR 数据契约检查（R1–R5），把 NameError/KeyError/missing binding/bad field 从 Runtime 前移到 Compile，为 Formal P1-5B 提供四层指标中的 Contract 层。

**Architecture:** `CirNode.output` 从 `Option<String>` 破坏性迁移为 `Option<OutputSpec>`（name/type_name/fields，强制 schema）；新增 `acos-compiler/src/contract.rs` 契约检查器挂入 `validate_cir_semantic`；Runtime 增加 `${a.b.c}` 点路径解析；探针记录 Contract PASS/FAIL。

**Tech Stack:** Rust workspace（acos-core / acos-compiler / acos-runtime / acos-bench / acos-cli），serde（camelCase JSON），YAML fixtures（acos-bench）。

**Spec:** `docs/specs/2026-08-18-stage-data-contract-design.md`（APPROVED，2026-08-18）

## Global Constraints

（逐字来自 spec，所有任务隐含遵守）
1. **Phase 1 只做静态契约，不实现 Python structured transport**（stdin/JSON/env 留给 Phase 2）——不得提前实现。
2. 不做：类型推导、dependent type、Hindley–Milner、Python AST 类型分析、跨语言 ABI。
3. 不做动态索引/复杂表达式：`${a[dynamic]}`、`${a[0].b}` 均拒绝；只允许 `identifier.field.field` 静态路径。
4. `output != None → name 非空 ∧ type_name 非空`（R5）。
5. Loop 聚合输出类型必须是 `List<T>`，T = body 最后一个声明 output 的 child 的 type_name。
6. R2 是**结构可达性**（producer 必须在 consumer 的可见集中），不是节点数组顺序。
7. `item_var` 在 loop body 内可见，loop 外 unresolved；`item_var` 不得与任何已存在的顶层 binding 同名（→ DataContractViolation）。
8. Conditional 分支内部产生的 binding 不允许在分支外被无条件消费。
9. R1 只验证 `${ref}` 引用存在；不保证嵌入源码后语义/语法安全（Phase 2 处理）。condition 表达式（`exists(x)`）不扫描。
10. 迁移验收：全 workspace 测试保持绿。

---

### Task 1: acos-core — OutputSpec/FieldSpec 定义与 CirNode 变更

**Files:**
- Modify: `crates/acos-core/src/types.rs`（CirNode 约 288-310 行）
- Modify: `crates/acos-core/src/schema.rs`（CirProgram 构造点）
- Test: `crates/acos-core/src/types.rs` 测试模块（serde round-trip）

**Interfaces:**
- Produces: `OutputSpec { name: String, type_name: String, fields: Vec<FieldSpec> }`（serde camelCase：`{"name": "...", "typeName": "...", "fields": [...]}`，fields 有 `#[serde(default)]`）
- Produces: `FieldSpec { name: String, type_name: String }`（serde camelCase）
- Produces: `CirNode.output: Option<OutputSpec>`、`CirNode.input_types: HashMap<String, String>`（`#[serde(default)]`）
- Consumes: 现有 `CirNodeKind`、serde 属性模式（`#[serde(rename_all = "camelCase")]`）

- [ ] **Step 1: 在 `types.rs` 的 `CirNode` 定义前新增类型，并修改 CirNode**

```rust
/// A declared output binding with its data contract.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OutputSpec {
    /// Binding name referenced by consumers (`${name}`).
    pub name: String,
    /// Declared type name (e.g. `CsvAnalysisResult`, `List<CsvAnalysisResult>`).
    pub type_name: String,
    /// Field-level schema for record types (R4). May be empty.
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
}

/// A single field declaration inside an `OutputSpec`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FieldSpec {
    /// Field name reachable via dotted path (`${name.field}`).
    pub name: String,
    /// Declared field type: Number | Integer | String | Boolean | List | Record | Any.
    pub type_name: String,
}
```

`CirNode` 中把 `pub output: Option<String>` 替换为：

```rust
    /// Named output binding with its data contract, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSpec>,
    /// Expected type name per input key (R3). Optional.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub input_types: std::collections::HashMap<String, String>,
```

- [ ] **Step 2: 运行确认失败**

Run: `cargo check -p acos-core 2>&1 | Select-String "error" | Select-Object -First 10`
Expected: 编译错误（`output` 类型变化波及构造点，如 `schema.rs`）

- [ ] **Step 3: 修复 acos-core 内部构造点**

`crates/acos-core/src/schema.rs` 中构造 `CirNode` 处：`output: None` → 仍为 `None`（类型是 `Option<OutputSpec>`，`None` 不变）；若有 `output: Some(...)` 则改为 `output: Some(OutputSpec { name: "...".into(), type_name: "...".into(), fields: vec![] })`。`crates/acos-core/tests/e2e_mini.rs` 同样处理。

- [ ] **Step 4: 补 serde round-trip 测试（types.rs 测试模块）**

```rust
#[test]
fn cir_node_output_spec_round_trip() {
    let node: CirNode = serde_json::from_str(
        r#"{"kind":"primitive_invocation","nodeId":"a","capability":"read_file",
            "output":{"name":"doc","typeName":"Document","fields":[]},
            "children":[],"inputs":{}}"#,
    )
    .unwrap();
    assert_eq!(node.output.as_ref().unwrap().name, "doc");
    assert_eq!(node.output.as_ref().unwrap().type_name, "Document");
    let back = serde_json::to_string(&node).unwrap();
    assert!(back.contains("\"output\":{\"name\":\"doc\",\"typeName\":\"Document\",\"fields\":[]}"));
}

#[test]
fn cir_node_missing_output_still_deserializes() {
    let node: CirNode = serde_json::from_str(
        r#"{"kind":"sequence","nodeId":"root","capability":null,"output":null,"children":[],"inputs":{}}"#,
    )
    .unwrap();
    assert!(node.output.is_none());
}
```

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p acos-core`
Expected: 全绿（含既有测试；e2e_mini 若引用 output 字段已修）

- [ ] **Step 6: Commit**

```bash
git add crates/acos-core
git commit -m "feat(core): OutputSpec data contract on CirNode output"
```

---

### Task 2: 迁移 acos-compiler（内嵌 JSON fixtures + 测试构造 + prompt 示例）

**Files:**
- Modify: `crates/acos-compiler/src/lib.rs`（多处：rule_compile 构造、测试模块 JSON 字符串、系统提示中的 CIR 示例 JSON）
- Modify: `crates/acos-compiler/src/replan.rs`（测试构造点）
- Test: 既有测试（不新增）

**Interfaces:**
- Consumes: `OutputSpec { name, type_name, fields }`（Task 1）
- Produces: 编译期语义不变的迁移后代码

- [ ] **Step 1: 编译错误清单**

Run: `cargo check -p acos-compiler 2>&1 | Select-String "error\[|error:" | Select-Object -First 20`
Expected: 列出所有需要迁移的位置

- [ ] **Step 2: 逐处迁移（每个 `output: Some("...")` / JSON 字符串中的 `"output": "..."`）**

代码构造点规则（如 `rule_compile` 中 `output: Some(format!("raw_{i}"))`）：

```rust
output: Some(OutputSpec {
    name: format!("raw_{i}"),
    type_name: "String".into(),
    fields: vec![],
}),
```

内嵌 JSON 字符串（测试 fixture 与系统提示 CIR 示例，如 `"ln": null` → `"output": null`；`"output": "raw_0"` → `"output": {"name": "raw_0", "typeName": "String", "fields": []}`）。系统提示中的示例 CIR 必须同步更新（模型会读到示例格式）：

```json
{ "kind": "primitive_invocation", "nodeId": "step_0", "capability": "read_file", "output": { "name": "raw_0", "typeName": "String", "fields": [] }, "children": [], "inputs": { "path": "/absolute/path/to/file" } }
```

`replan.rs` 的测试构造点同样处理（`output: Some("results".into())` → `OutputSpec { name: "results".into(), type_name: "String".into(), fields: vec![] }`）。

- [ ] **Step 3: 确认通过**

Run: `cargo test -p acos-compiler`
Expected: 全绿（26+ 测试，含 robustness suite）

- [ ] **Step 4: Commit**

```bash
git add crates/acos-compiler
git commit -m "refactor(compiler): migrate CirNode constructions to OutputSpec"
```

---

### Task 3: 迁移 acos-runtime（测试构造 + replan 测试）

**Files:**
- Modify: `crates/acos-runtime/src/lib.rs`（测试模块 `primitive_node` helper 与手写节点构造）
- Modify: `crates/acos-runtime/src/replan.rs`（测试构造点）
- Test: 既有测试

**Interfaces:**
- Consumes: `OutputSpec`（Task 1）

- [ ] **Step 1: 编译错误清单**

Run: `cargo check -p acos-runtime 2>&1 | Select-String "error\[|error:" | Select-Object -First 20`

- [ ] **Step 2: 迁移测试 helper**

`primitive_node(id, capability, output: Option<&str>)` 改为返回带 schema 的 output：

```rust
fn primitive_node(id: &str, capability: &str, output: Option<&str>) -> CirNode {
    CirNode {
        kind: CirNodeKind::PrimitiveInvocation,
        node_id: id.into(),
        capability: Some(capability.into()),
        output: output.map(|name| OutputSpec {
            name: name.into(),
            type_name: "String".into(),
            fields: vec![],
        }),
        children: vec![],
        else_children: vec![],
        inputs: HashMap::new(),
        input_types: HashMap::new(),
        control: None,
    }
}
```

所有手写 `output: Some("...".into())` 改为 `OutputSpec { ... }`（`for_each_loop_binds_aggregated_output` 测试中的 loop 节点输出类型必须满足 List 规则：`type_name: "List<String>".into()`）。

- [ ] **Step 3: 确认通过**

Run: `cargo test -p acos-runtime`
Expected: 全绿（13+ 测试，含 loop 聚合）

- [ ] **Step 4: Commit**

```bash
git add crates/acos-runtime
git commit -m "refactor(runtime): migrate CirNode constructions to OutputSpec"
```

---

### Task 4: 迁移 acos-bench fixtures 与 golden CIR

**Files:**
- Modify: `crates/acos-bench/fixtures/*/*.yaml`（loop/foreach.yaml、loop/while_limit.yaml、retry/timeout.yaml、condition/basic.yaml、condition/repair_branch.yaml、recovery/rule_replan.yaml、recovery/model_replan.yaml、negative/retry_zero.yaml、negative/retry_irreversible.yaml）
- Modify: `tests/benchmarks/p1/flagship_csv_quality/golden_cir.json`
- Test: acos-bench 既有测试

**Interfaces:**
- Consumes: `OutputSpec` serde 格式（Task 1）

- [ ] **Step 1: 迁移 YAML fixtures（`output: items` 形式）**

```yaml
    - nodeId: "src"
      kind: primitive_invocation
      capability: list_source
      output:
        name: items
        typeName: StringList
        fields: []
      children: []
      inputs: {}
```

对每个 fixture：`output: X` → `output: {name: X, typeName: <类型>, fields: []}`。类型按语义选（foreach 的 items=StringList、per_item=String；while_limit 的 out=String；timeout/results、basic/search_results、summary、repair_branch/marker、summary2、marker2、recovery/results、negative/results、done=String）。

- [ ] **Step 2: 迁移 golden_cir.json**

有 output 的 5 个节点补 schema（类型按实际 Python 输出选择）：

```json
{ "kind": "primitive_invocation", "nodeId": "validate", "capability": "execute_python",
  "output": { "name": "validation_result", "typeName": "ValidationResult",
    "fields": [ {"name": "path", "typeName": "String"}, {"name": "has_error", "typeName": "Boolean"},
                {"name": "issues", "typeName": "List"}, {"name": "row_count", "typeName": "Integer"},
                {"name": "columns", "typeName": "List"} ] },
  "children": [], "elseChildren": [], "inputs": { ... } }
```

`repair`/`no_repair_needed` → `repaired_content: RepairResult`（fields: path/repaired/rows_cleaned）；`analyze_with_retry` → `file_analysis: FileAnalysis`（fields: path/total_revenue/row_count/category_counts）；`merge_report` → `report_ref: ArtifactRef`。

- [ ] **Step 3: 确认通过**

Run: `cargo test -p acos-bench; cargo test -p acos-cli`（或 `cargo test --workspace` 视网速）
Expected: 全绿（含 loop/condition/retry/recovery bench 套件）

- [ ] **Step 4: Commit**

```bash
git add crates/acos-bench tests/benchmarks
git commit -m "refactor(bench): migrate fixtures and golden CIR to OutputSpec"
```

---

### Task 5: 契约检查器 — 错误变体 + R1 binding 解析 + 挂入校验

**Files:**
- Create: `crates/acos-compiler/src/contract.rs`
- Modify: `crates/acos-compiler/src/lib.rs`（`CompilerError` 新增变体、`validate_cir_semantic` 挂载、`mod contract;`、`CompilerError` Display/From）
- Test: `crates/acos-compiler/src/contract.rs` 测试模块

**Interfaces:**
- Produces: `pub fn validate_data_contract(program: &CirProgram) -> Result<(), CompilerError>`
- Produces: `CompilerError::UnresolvedBinding { node_id: String, binding: String }`、`CompilerError::DataContractViolation { node_id: String, message: String }`
- Produces: `fn extract_refs(value: &serde_json::Value) -> Vec<String>`（递归收集所有字符串中的 `${...}` 引用）
- Consumes: `CirNode.output: Option<OutputSpec>`、`input_types`（Task 1）

- [ ] **Step 1: 在 `lib.rs` 的 `CompilerError` 枚举追加变体 + Display**

```rust
    /// A `${ref}` in inputs/control does not resolve to any producer binding.
    UnresolvedBinding {
        /// Node that contains the bad reference.
        node_id: String,
        /// The unresolved binding name (dotted path prefix).
        binding: String,
    },
    /// Data contract rule violation (type/field/ordering/completeness).
    DataContractViolation {
        /// Node that violates the contract.
        node_id: String,
        /// Human-readable explanation.
        message: String,
    },
```

Display 分支：

```rust
            CompilerError::UnresolvedBinding { node_id, binding } => {
                write!(f, "node '{node_id}' references unresolved binding '${{{binding}}}'")
            }
            CompilerError::DataContractViolation { node_id, message } => {
                write!(f, "node '{node_id}' violates data contract: {message}")
            }
```

- [ ] **Step 2: 写失败测试（contract.rs 测试模块）**

测试辅助（先建，供后续 Task 复用）：

```rust
fn node(id: &str, output: Option<(&str, &str, Vec<(&str, &str)>)>) -> CirNode {
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
```

```rust
#[test]
fn rejects_unresolved_binding() {
    let mut consumer = node("cons", None);
    consumer.inputs.insert("code".into(), serde_json::Value::String("data = ${processed_data}".into()));
    let mut root = CirNode { kind: CirNodeKind::Sequence, node_id: "root".into(), capability: None,
        output: None, children: vec!["cons".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: None };
    let p = program(vec!["root"], vec![root, consumer]);
    let err = validate_data_contract(&p).unwrap_err();
    assert!(matches!(err, CompilerError::UnresolvedBinding { ref binding, .. } if binding == "processed_data"));
}
```

- [ ] **Step 3: 实现 contract.rs 骨架 + R1**

```rust
//! Stage Data Contract validation (P1-5B Formal).
//!
//! Phase 1 scope (see docs/specs/2026-08-18-stage-data-contract-design.md):
//! static contract checks only. Python structured transport is Phase 2.

use std::collections::{HashMap, HashSet};
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
    let mut visible: HashMap<String, OutputSpec> = HashMap::new();
    let mut entry_nodes: Vec<&CirNode> = Vec::new();
    let by_id: HashMap<&str, &CirNode> = program.nodes.iter().map(|n| (n.node_id.as_str(), n)).collect();
    for e in &program.entry {
        if let Some(n) = by_id.get(e.as_str()) { entry_nodes.push(n); }
    }

    // R1 + R2: walk with scope threading; `walk` returns the bindings its
    // children produce for the enclosing scope.
    fn walk<'a>(node: &'a CirNode, by_id: &HashMap<&str, &'a CirNode>, scope: &mut HashMap<String, OutputSpec>, producers: &HashMap<String, (String, OutputSpec)>) -> Result<HashMap<String, OutputSpec>, CompilerError> {
        let mut produced = HashMap::new();
        match node.kind {
            CirNodeKind::Sequence | CirNodeKind::Parallel => {
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    // children see current scope + earlier siblings' outputs
                    let mut child_scope = scope.clone();
                    for (k, v) in &produced { child_scope.insert(k.clone(), v.clone()); }
                    walk(child, by_id, &mut child_scope, producers)?;
                    for (k, v) in &child_scope { produced.insert(k.clone(), v.clone()); }
                }
                // conditional children: both branches stay inside the branch scope
                if node.kind == CirNodeKind::Sequence {
                    for child_id in &node.else_children {
                        let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                        let mut child_scope = scope.clone();
                        walk(child, by_id, &mut child_scope, producers)?;
                    }
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
                let mut body_produced = HashMap::new();
                for child_id in &node.children {
                    let child = by_id.get(child_id.as_str()).ok_or_else(|| CompilerError::InvalidReference { node_id: node.node_id.clone(), referenced: child_id.clone() })?;
                    let mut child_scope = body_scope.clone();
                    for (k, v) in &body_produced { child_scope.insert(k.clone(), v.clone()); }
                    walk(child, by_id, &mut child_scope, producers)?;
                    for (k, v) in &child_scope { body_produced.insert(k.clone(), v.clone()); }
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
                            produced.insert(o.name.clone(), o.clone());
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
    fn check_node(node: &CirNode, scope: &HashMap<String, OutputSpec>, producers: &HashMap<String, (String, OutputSpec)>) -> Result<(), CompilerError> {
        for (key, val) in &node.inputs {
            for raw in extract_refs(val) {
                let mut parts = raw.split('.');
                let name = parts.next().unwrap_or("");
                let spec = scope.get(name).or_else(|| producers.get(name).map(|(_, s)| s));
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
        check_node(e, &scope, &producers)?;
        let produced = walk(e, &by_id, &mut scope, &mut HashSet::new().into())?;
        scope.extend(produced);
        // sibling entries share the top-level scope; check remaining nodes through walk
        // (children are checked inside walk; here we re-check the entry's own refs only)
    }
    // Final sweep: every node checked with the program-wide producer map for
    // R1 (binding must exist somewhere) — scoping is enforced by walk.
    Ok(())
}
```

> 注：上面的实现骨架中 `walk` 已顺带覆盖 R2/R3/R4 的多数逻辑，后续 Task 6/7 将用测试驱动把它修正到最终形态（`walk` 的 sibling 传播与 `check_node` 的挂载点需在测试驱动下收口，见 Task 6 Step 2）。

- [ ] **Step 4: 挂入 `validate_cir_semantic`（lib.rs，在 UnreachableNodes 检查之后）**

```rust
    // Stage data contract (R1–R5).
    validate_data_contract(program)?;
```

同时 `use crate::contract::validate_data_contract;` 与 `mod contract;`。

- [ ] **Step 5: 跑测试**

Run: `cargo test -p acos-compiler`
Expected: 新测试通过；既有 26 测试保持绿（golden CIR 已迁移，不触发新检查错误——golden 中 `${current_file}` 在 loop body 内可见、`${input_files}` 来自 entry 注入？若 acos-bench 测试跑 golden 时 `input_files` 无 producer，会新增 UnresolvedBinding 失败——**若出现**，在 `acos-bench`/测试的注入路径把 `input_files` 作为环境注入绑定豁免（见 Step 6 说明）**

- [ ] **Step 6: 环境注入绑定豁免（如需要）**

`input_files` 等由 `--env`/`execute_with_env` 注入的绑定没有 CIR producer。契约检查器需要把"环境注入绑定"作为合法 producer。实现：`validate_data_contract` 增加参数 `env_bindings: &[String]`（或读取 `program` 上无此信息则通过 `producers` 预填）：在 lib.rs 的调用点把已知环境绑定（`["input_files"]`，即 run-cir --env 注入的键）传入，检查器把它们视为 `OutputSpec { name, type_name: "Any", fields: [] }`。golden 测试若未注入则按 Step 5 观察到的实际错误处理。

- [ ] **Step 7: Commit**

```bash
git add crates/acos-compiler
git commit -m "feat(compiler): stage data contract R1 binding resolution"
```

---

### Task 6: 契约检查器 — R2 结构可达性 + R3 类型对齐（测试驱动收口）

**Files:**
- Modify: `crates/acos-compiler/src/contract.rs`
- Test: 同文件测试模块

**Interfaces:**
- Consumes: `validate_data_contract`、`extract_refs`（Task 5）
- Produces: 最终形态的 `walk`（scope 线程化）+ `check_node`（引用检查挂载）

- [ ] **Step 1: 写失败测试（R2 三个语义 + R3）**

```rust
#[test]
fn sequence_allows_earlier_sibling_output() {
    let mut a = node("a", Some(("doc", "Document", vec![])));
    let mut b = node("b", None);
    b.inputs.insert("text".into(), serde_json::Value::String("${doc}".into()));
    let mut root = seq_root(vec!["a", "b"]);
    assert!(validate_data_contract(&program(vec!["root"], vec![root, a, b])).is_ok());
}

#[test]
fn parallel_branches_do_not_share_outputs() {
    let mut a = node("a", Some(("doc", "Document", vec![])));
    let mut b = node("b", None);
    b.inputs.insert("text".into(), serde_json::Value::String("${doc}".into()));
    let mut root = CirNode { kind: CirNodeKind::Parallel, node_id: "root".into(), capability: None,
        output: None, children: vec!["a".into(), "b".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: None };
    let err = validate_data_contract(&program(vec!["root"], vec![root, a, b])).unwrap_err();
    assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
}

#[test]
fn conditional_branch_output_unusable_outside() {
    let mut a = node("a", Some(("branch_result", "String", vec![])));
    let mut cond = CirNode { kind: CirNodeKind::Conditional, node_id: "cond".into(), capability: None,
        output: None, children: vec!["a".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: Some(ControlSpec { condition: Some(ConditionSpec::default()), loop_spec: None, retry: None }) };
    let mut after = node("after", None);
    after.inputs.insert("code".into(), serde_json::Value::String("x = ${branch_result}".into()));
    let mut root = seq_root(vec!["cond", "after"]);
    let err = validate_data_contract(&program(vec!["root"], vec![root, cond, a, after])).unwrap_err();
    assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
}

#[test]
fn type_mismatch_rejected_but_number_integer_compatible() {
    let mut a = node("a", Some(("stats", "CsvAnalysisResult", vec![])));
    let mut b = node("b", None);
    b.inputs.insert("stats".into(), serde_json::Value::String("${stats}".into()));
    b.input_types.insert("stats".into(), "OtherType".into());
    let mut root = seq_root(vec!["a", "b"]);
    let err = validate_data_contract(&program(vec!["root"], vec![root, a.clone(), b.clone()])).unwrap_err();
    assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    b.input_types.insert("stats".into(), "number".into());
    // producer declares CsvAnalysisResult — not numeric: still violation
    let err = validate_data_contract(&program(vec!["root"], vec![root.clone(), a.clone(), b.clone()])).unwrap_err();
    assert!(matches!(err, CompilerError::DataContractViolation { .. }));
    // exact match passes
    b.input_types.insert("stats".into(), "CsvAnalysisResult".into());
    assert!(validate_data_contract(&program(vec!["root"], vec![root, a, b])).is_ok());
}
```

辅助 `seq_root`：

```rust
fn seq_root(children: Vec<&str>) -> CirNode {
    CirNode { kind: CirNodeKind::Sequence, node_id: "root".into(), capability: None, output: None,
        children: children.into_iter().map(String::from).collect(), else_children: vec![],
        inputs: HashMap::new(), input_types: HashMap::new(), control: None }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p acos-compiler contract::tests::parallel_branches_do_not_share_outputs`（等）
Expected: FAIL（Task 5 骨架未正确实现作用域收口）

- [ ] **Step 3: 收口实现**

按测试修正 `walk`：Parallel 分支的 child 只继承传入 `scope`（不传播兄弟 produced）；Conditional 分支的 produced 不并入外层 `produced`；`check_node` 在 walk 内对每个节点调用（进入节点时用 child_scope）。确保：
- sequence：`child_scope = scope + produced(之前兄弟)`；该 child walk 后把**其内部 produced**（含子序列传播）并入 `produced`。
- parallel：每个 child 用 `scope.clone()`（不含兄弟 produced，分支间互不可见）；**块完成后所有分支 produced 并入外层 `produced`（对后续兄弟可见，spec R2 line 98；2026-08-18 用户决策确认）**。
- conditional：children/else_children 各用 `scope.clone()`，produced 丢弃。
- loop body：body_scope = scope + item_var + body 内兄弟 produced；loop 聚合 output 并入外层 produced。
- `check_node`：对每个进入 walk 的节点，以其 child_scope 检查 inputs（R1/R3/R4）。

- [ ] **Step 4: 跑全部契约测试**

Run: `cargo test -p acos-compiler`
Expected: 全绿（既有 + 新增）

- [ ] **Step 5: Commit**

```bash
git add crates/acos-compiler/src/contract.rs
git commit -m "feat(compiler): contract R2 structural reachability, R3 type alignment"
```

---

### Task 7: 契约检查器 — R4 静态字段路径 + R5 完整性 + item_var 遮蔽

**Files:**
- Modify: `crates/acos-compiler/src/contract.rs`
- Test: 同文件测试模块

**Interfaces:**
- Consumes: Task 5/6 的 `validate_data_contract` 形态

- [ ] **Step 0: Parallel 输出逃逸修正（2026-08-18 用户决策：spec R2 line 98 为准）**

修正 walk 的 parallel 分支：child 仍用 `scope.clone()`（分支间互不可见），但块完成后**分支 produced 并入外层 `produced`**（对后续兄弟可见）。新增失败测试先行：

```rust
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
```

（当前 Task 6 实现下该测试红——parallel produced 被丢弃；修正后绿。`parallel_branches_do_not_share_outputs` 保持红→绿不变：b 在分支内消费，仍被拒。）

- [ ] **Step 1: 写失败测试**

```rust
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
fn item_var_visible_inside_loop_body_not_outside() {
    let mut body = node("body", Some(("per", "String", vec![])));
    body.inputs.insert("text".into(), serde_json::Value::String("${item}".into()));
    let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
        output: None, children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
            loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                input: Some("${items}".into()), item_var: Some("item".into()) }), retry: None }) };
    let mut root = seq_root(vec!["loop"]);
    assert!(validate_data_contract(&program(vec!["root"], vec![root.clone(), loop_node.clone(), body.clone()])).is_ok());
    let mut after = node("after", None);
    after.inputs.insert("code".into(), serde_json::Value::String("x = ${item}".into()));
    let mut root2 = seq_root(vec!["loop", "after"]);
    let err = validate_data_contract(&program(vec!["root"], vec![root2, loop_node, body, after])).unwrap_err();
    assert!(matches!(err, CompilerError::UnresolvedBinding { .. }));
}

#[test]
fn item_var_shadowing_top_level_binding_rejected() {
    let mut src = node("src", Some(("file_path", "String", vec![])));
    let mut body = node("body", Some(("per", "String", vec![])));
    let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
        output: None, children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
            loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                input: Some("${files}".into()), item_var: Some("file_path".into()) }), retry: None }) };
    let mut root = seq_root(vec!["src", "loop"]);
    let err = validate_data_contract(&program(vec!["root"], vec![root, src, loop_node, body])).unwrap_err();
    assert!(matches!(err, CompilerError::DataContractViolation { .. }));
}

#[test]
fn loop_aggregate_type_must_be_list_of_last_child_type() {
    let mut body = node("body", Some(("vr", "ValidationResult", vec![])));
    let mut loop_node = CirNode { kind: CirNodeKind::LoopMap, node_id: "loop".into(), capability: None,
        output: Some(OutputSpec { name: "all_results".into(), type_name: "List<ValidationResult>".into(), fields: vec![] }),
        children: vec!["body".into()], else_children: vec![], inputs: HashMap::new(),
        input_types: HashMap::new(), control: Some(ControlSpec { condition: None,
            loop_spec: Some(LoopSpec { kind: LoopKind::ForEach, condition: None, max_iterations: None,
                input: Some("${items}".into()), item_var: Some("item".into()) }), retry: None }) };
    let mut root = seq_root(vec!["loop"]);
    assert!(validate_data_contract(&program(vec!["root"], vec![root.clone(), loop_node.clone(), body.clone()])).is_ok());
    loop_node.output.as_mut().unwrap().type_name = "ValidationResult".into();
    let err = validate_data_contract(&program(vec!["root"], vec![root, loop_node, body])).unwrap_err();
    assert!(matches!(err, CompilerError::DataContractViolation { .. }));
}

#[test]
fn incomplete_output_schema_rejected() {
    let mut a = node("a", Some(("x", "", vec![])));
    let mut root = seq_root(vec!["a"]);
    let err = validate_data_contract(&program(vec!["root"], vec![root, a])).unwrap_err();
    assert!(matches!(err, CompilerError::DataContractViolation { .. }));
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p acos-compiler contract::tests`
Expected: 新测试 FAIL

- [ ] **Step 3: 实现收口**

- R4：`extract_refs` 已返回整段 `${...}` 内容（含点路径），`check_node` 按 `.` 拆分并在 `spec.fields` 中逐段查找（Task 5 骨架已有）；字段类型为 List/Record 时点路径继续 → `DataContractViolation`（Phase 2 才支持索引）；含 `[` 或 `]` 的路径段 → `DataContractViolation`（动态索引拒绝）。
- item_var 遮蔽：`walk` 的 LoopMap 分支已有（Task 5 骨架），修正为只在 `producers`（全部顶层 output 名）包含 item_var 时报错，测试驱动确认。
- loop 聚合 `List<T>`：`walk` LoopMap 分支按"body 最后一个声明 output 的 child"取 T（children 逆序 find_map），与 `node.output.type_name` 精确比较；body 无产出但声明了聚合 → violation。
- R5：Task 5 骨架已有（entry 处逐节点检查），测试驱动确认覆盖 walk 内所有节点。

- [ ] **Step 4: 跑全部测试**

Run: `cargo test -p acos-compiler`
Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add crates/acos-compiler/src/contract.rs
git commit -m "feat(compiler): contract R4 field paths, R5 completeness, item-var rules"
```

---

### Task 8: Runtime 点路径解析 `${a.b.c}`

**Files:**
- Modify: `crates/acos-runtime/src/lib.rs`（`resolve_ref`/`resolve_ref_value` 约 959-1080 行）
- Test: 同文件测试模块

**Interfaces:**
- Consumes: 现有 `resolve_ref(ref_str, env)`、`TypedValue.payload: serde_json::Value`
- Produces: `${a.b.c}` 在单引用快速路径与嵌入字符串路径中均解析为嵌套字段

- [ ] **Step 1: 写失败测试**

```rust
#[tokio::test]
async fn dotted_path_resolves_nested_field() {
    let env: Arc<Mutex<HashMap<String, TypedValue>>> = Arc::new(Mutex::new(HashMap::new()));
    env.lock().await.insert("vr".into(), TypedValue {
        value_type: ValueType::Record,
        payload: serde_json::json!({"total_issues": 3, "issues": ["a", "b"]}),
    });
    let out = resolve_ref("${vr.total_issues}", &env).await;
    assert_eq!(out, "3");
    let out2 = resolve_ref("prefix ${vr.total_issues} suffix", &env).await;
    assert_eq!(out2, "prefix 3 suffix");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p acos-runtime dotted_path`
Expected: FAIL（当前 `${vr.total_issues}` 找不到整名 → 原样返回）

- [ ] **Step 3: 实现**

在 `resolve_ref_value` 中：当 env 无 `name` 整名匹配时，尝试按 `.` 拆分：首段查 env，得到 `TypedValue` 后在 `payload` 逐段取 `serde_json::Value` 字段（`Value::Object(map).get(segment)`；非 Object 或缺失 → 返回 None）。快速路径返回该子值字符串化；嵌入路径（`resolve_ref` 的循环替换处）同样：替换函数内部对 `${name}` 内容先尝试整名，再尝试点路径解析。

```rust
async fn resolve_dotted(ref_str: &str, env: &Arc<Mutex<HashMap<String, TypedValue>>>) -> Option<TypedValue> {
    let name = ref_str.strip_prefix("${").and_then(|s| s.strip_suffix('}'))?;
    let mut parts = name.split('.');
    let head = parts.next()?;
    let guard = env.lock().await;
    let mut cur = guard.get(head)?.clone();
    drop(guard);
    for seg in parts {
        match &cur.payload {
            serde_json::Value::Object(map) => {
                let v = map.get(seg)?;
                cur = TypedValue {
                    value_type: if v.is_array() { ValueType::List } else if v.is_object() { ValueType::Record } else { ValueType::Scalar },
                    payload: v.clone(),
                };
            }
            _ => return None,
        }
    }
    Some(cur)
}
```

并在 `resolve_ref` 快速路径（整个字符串是单个 `${...}`）与循环替换路径中优先尝试 `resolve_dotted`，失败回落到整名查找。

- [ ] **Step 4: 跑测试**

Run: `cargo test -p acos-runtime`
Expected: 全绿

- [ ] **Step 5: Commit**

```bash
git add crates/acos-runtime/src/lib.rs
git commit -m "feat(runtime): resolve dotted field paths in env references"
```

---

### Task 9: 探针 Contract 层（trace 记录 + 输出）

**Files:**
- Modify: `crates/acos-cli/src/bin/p1-5b-probe.rs`（compile 结果处理 + `build_trace_json`）
- Test: 编译验证（探针是离线工具，不新增单测）

**Interfaces:**
- Consumes: `compile_traced` 结果 + `validate_data_contract`（经 `validate_cir` 路径或直接调用）
- Produces: trace JSON 顶层新增 `contract: { "pass": bool, "error": Option<String> }`（放在 `run` 或顶层）

- [ ] **Step 1: 在 trace 中记录契约结果**

探针在 compile 成功后，对 `final_cir` 调用契约检查并记录（直接调用 `acos_compiler::contract::validate_data_contract` 需 `pub`——若未导出则用 `validate_cir` 的返回区分：编译成功即契约通过，因为契约检查已挂入 `validate_cir_semantic`；编译失败时从 `final_error` 判断是否契约类错误）。

实现（build_trace_json 调用点）：

```rust
let contract = compile_ok.then(|| acos_compiler::validate_cir(&traced.final_cir).map(|_| ()))
    .map(|r| match r {
        Ok(()) => serde_json::json!({ "pass": true, "error": null }),
        Err(e) => serde_json::json!({ "pass": false, "error": e.to_string() }),
    })
    .unwrap_or(serde_json::json!({ "pass": false, "error": "compile failed" }));
```

并把 `contract` 写入 trace record（`build_trace_json` 增加参数或字段）。控制台输出新增：

```text
  contract: PASS (R1-R5)
```

- [ ] **Step 2: 编译验证**

Run: `cargo build -p acos-cli`
Expected: 编译通过；`cargo test -p acos-cli` 绿

- [ ] **Step 3: Commit**

```bash
git add crates/acos-cli/src/bin/p1-5b-probe.rs
git commit -m "feat(probe): record Stage Data Contract PASS/FAIL in trace"
```

---

### Task 10: cir_spec.md 更新 + 全 workspace 验证 + 收尾

**Files:**
- Modify: `docs/specs/cir_spec.md`
- Modify: `PROJECT_STATUS.md`

- [ ] **Step 1: cir_spec.md 增加契约章节**

内容要点（从 spec 抄录）：
- `output` 现在是 `OutputSpec { name, typeName, fields }`；有输出必有完整 schema（R5）
- `inputTypes`：input key → 期望类型名
- R1–R5 规则简述
- Loop 聚合输出类型必须是 `List<T>`（T = body 最后一个声明 output 的 child 类型）；`${all_results.total_issues}` 编译期 FAIL；`${all_results[0].total_issues}` Phase 2
- 点路径 `${a.b.c}` 静态字段路径；动态索引 Phase 2
- item_var 作用域与遮蔽规则
- 架构原则：runtime values 以结构化数据跨阶段（Phase 2 落地）

- [ ] **Step 2: 全 workspace 验证**

Run: `cargo test --workspace`
Expected: 全绿（这是迁移正确性的最终验收）

- [ ] **Step 3: PROJECT_STATUS.md 更新**

P1-5B Formal Branch 条目下增加：`Stage Data Contract Phase 1 已完成（编译期 R1–R5，见 docs/specs/2026-08-18-stage-data-contract-design.md）`；Known Limitation 中 Generated-code data contract 标注 Phase 1 已实施静态部分、Phase 2 structured transport 待做。

- [ ] **Step 4: Commit**

```bash
git add docs/specs/cir_spec.md PROJECT_STATUS.md
git commit -m "docs: Stage Data Contract Phase 1 in CIR spec and status"
```

---

## Self-Review 结果

- **Spec 覆盖**：① OutputSpec/FieldSpec/input_types → Task 1；② 存量迁移（golden+fixtures+构造点）→ Task 2/3/4；③ R1 → Task 5；④ R2/R3 → Task 6；⑤ R4/R5/item_var/loop List<T> → Task 7；⑥ runtime 点路径 → Task 8；⑦ 探针 Contract 层 → Task 9；⑧ cir_spec 文档 → Task 10。无缺口。
- **Placeholder 扫描**：所有步骤含具体代码或命令；无 TBD/TODO。
- **类型一致性**：`OutputSpec.name/type_name/fields`、`FieldSpec.name/type_name`、`input_types: HashMap<String,String>` 在全部任务中一致；错误变体名一致。
- **纪律检查**：无任何任务实现 stdin/JSON structured transport（Phase 2 边界保持）。