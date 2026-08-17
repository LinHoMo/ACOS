# ACOS P0：控制语义 + 失败恢复 + Benchmark — 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让 ACOS 运行时真正执行 Conditional/LoopMap/Retry 控制语义，支持 RetryPolicy → RuleReplanner → ModelReplanner 的失败恢复状态机，并用独立 `acos-bench` crate（fixture 即行为契约）做可回归验证。

**Architecture:** 控制语义以 `CirNode.control: Option<ControlSpec>` 表达（与业务 inputs 分离）；运行时 `run_node` 按 control 解释条件/循环/重试；失败时经 `execute_with_recovery` 进入事务式恢复管道（RecoveryProposal 校验通过才提交，替换根保留原 node id）；`acos-bench` 提供确定性 fixture 与三态报告（PASS/FAIL/SKIP）。

**Tech Stack:** Rust 1.81 workspace、tokio、serde/serde_json/serde_yaml、async-trait；现有 crates：acos-core/compiler/runtime/plugin/state/verify/llm/cli。

**Spec:** `docs/superpowers/specs/2026-08-17-control-semantics-recovery-benchmark-design.md`（status: approved, v0.1, scope P0）

## Spec 澄清（实现中明确的 5 处工程决策）

1. **重试安全门公式修正**（spec §4.3）：门 = `primitive.idempotent() || 所有效果均为纯读(FsRead|NetworkRead)`。不采用 spec 中"或 reversible"的字面公式——因为 5 个 builtin 原语全部声明 `reversible: true`，若按字面公式 write_file/execute_python 也可自动重试，与 spec 示例（write_file 禁止自动重试）矛盾。MVP 取保守：只有纯读或显式声明 idempotent 才可自动重试。
2. **fixture 增加 `compiler: inline|rules|model` 字段**（spec §7.2 扩展）：`mode: run` 需要确定性编译控制语义程序，而 RuleCompiler 不生成 control 节点、无 key 时 ModelCompiler 不可用。`inline` = 校验内嵌 CIR 后直接执行（Compile 列显示 PASS），使 control semantics benchmark 无需 API key 即可确定性运行。`mode: cir` 语义不变（只校验不执行）。
3. **FailureClassifier 实现为 `AcosError::classify(&self) -> FailureClass` 方法**（spec §4.4），集中唯一分类入口；运行时内部错误传播用 `Result<_, (String, AcosError)>` 元组（String = 最深失败节点 id），避免新增错误变体。
4. **condition 静态标识符校验只对 node outputs**（spec §3 "任务级保留绑定" 暂缓）：RuleCompiler 不把 goal 放入 env，保留绑定无实际语义，YAGNI 暂不做。
5. **Proposal 效果检查**：subgraph 中各 capability 声明的效果 kind 必须 ⊆ `program.effects` 中已声明的 kind（registry-aware，运行时提交门）。

## 全局约束 / Global Constraints

- Rust 1.81+，edition 2021；workspace lints：`unsafe_code = deny`、`missing_docs = warn`、`rust_2018_idioms = deny`、clippy pedantic warn（每个 pub 项都要有文档注释）
- serde 一律 `rename_all = "camelCase"`（类型）/ `"snake_case"`（kind/class 枚举）
- 条件表达式禁止模糊引用：`acos-expr` 标识符从 env 精确匹配；编译期静态校验标识符必须命中某节点 output
- `CirNodeKind::Retry` 标记 `#[deprecated(note = "use ControlSpec.retry")]`；Compiler 不得生成 `kind=retry`；Runtime 的 Retry 分支仅按 `control.retry` 解释
- While = 先求值后执行；Until = 先执行后求值（off-by-one 必须按此实现）
- 循环上限：While/Until `max_iterations` 必填（≥1）；ForEach `None` = 数组长度自然上限
- 重试禁止对非幂等副作用盲目重试（安全门）；`ExternalIrreversible` + retry → 编译/校验拒绝
- 恢复管道：RecoveryProposal 必须过 结构→能力→效果 校验才提交；subgraph 根节点必须复用原 node id
- 报告三态 PASS/FAIL/SKIP；`--require-model` 把 SKIP 视为 FAIL
- 每个任务结束必须 `cargo test -p <crate>` 通过（必要时 `--workspace`）并提交

## 文件结构 / File Map

```text
crates/acos-core/src/
├── types.rs            # 修改：ControlSpec/ConditionSpec/LoopSpec/RetryPolicy/FailureClass/CirNode.control/else_children/CirNodeKind::Retry deprecated/FailureContext/RecoveryProposal
├── expr.rs             # 新增：acos-expr 最小条件表达式（parse/evaluate/collect_identifiers）
├── error.rs            # 修改：PrimitiveFailure.class + classify()
├── traits.rs           # 修改：Primitive::idempotent() + Replanner/ModelReplanner/RecoveryContext
└── lib.rs              # 修改：导出 expr、新类型
crates/acos-compiler/src/
├── lib.rs              # 修改：validate_cir 公开 + 控制语义校验 + PLANNER_SYSTEM_PROMPT 更新
└── replan.rs           # 新增：ModelRecoveryPlanner + RECOVERY_SYSTEM_PROMPT
crates/acos-runtime/src/
├── lib.rs              # 修改：run_node 控制语义 + 错误对 (String, AcosError) + execute_with_recovery + 事务提交门
└── replan.rs           # 新增：RuleReplanner + RecoveryRule + OfflineFallbackRule
crates/acos-llm/src/lib.rs   # 修改：LongCatClient::dummy() 测试辅助
crates/acos-bench/      # 新增 crate
├── Cargo.toml
├── src/{lib,main,runner,report,registry}.rs
├── fixtures/{condition,loop,retry,recovery,negative}/*.yaml
└── tests/{condition,loop,retry,recovery}.rs
crates/acos-cli/src/main.rs  # 修改：bench 子命令
schemas/cir/cir.proto        # 修改：control 字段同步
docs/                        # 修改：HANDOFF.md / README.md / PROJECT_STATUS.md / specs/cir_spec.md / CHANGELOG.md
```

---

### Task 1: acos-core 控制语义类型 + CirNode 扩展 + proto 同步

**Files:**
- Modify: `crates/acos-core/src/types.rs`
- Modify: `schemas/cir/cir.proto`
- Test: `crates/acos-core/src/types.rs`（内嵌 tests）

**Interfaces:**
- Consumes: 无（类型层）
- Produces: `ControlSpec{condition,loop_spec,retry}`、`ConditionSpec{expression}`、`LoopKind{While,Until,ForEach}`、`LoopSpec{kind,condition,max_iterations,input,item_var}`、`RetryStrategy{Fixed}`、`RetryPolicy{max_attempts,backoff_ms,strategy,retry_on}`、`FailureClass{Timeout,RateLimit,TransientNetworkError,InvalidInput,PermissionDenied,SyntaxError,Unknown}`、`CirNode.control: Option<ControlSpec>`、`CirNode.else_children: Vec<String>`

- [ ] **Step 1: 写失败测试（类型往返 + 序列化）**

在 `types.rs` 末尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn control_spec_roundtrips_to_json() {
        let control = ControlSpec {
            condition: Some(ConditionSpec { expression: "exists(doc)".into() }),
            loop_spec: None,
            retry: Some(RetryPolicy {
                max_attempts: 3,
                backoff_ms: 100,
                strategy: RetryStrategy::Fixed,
                retry_on: vec![FailureClass::Timeout],
            }),
        };
        let json = serde_json::to_string(&control).unwrap();
        let back: ControlSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, control);
    }

    #[test]
    fn cir_node_control_and_else_children_default_to_empty() {
        let json = r#"{"kind":"primitive_invocation","nodeId":"a","capability":"read_file","output":null,"children":[],"inputs":{}}"#;
        let node: CirNode = serde_json::from_str(json).unwrap();
        assert_eq!(node.control, None);
        assert!(node.else_children.is_empty());
    }

    #[test]
    fn loop_spec_serializes_camel_case() {
        let spec = LoopSpec {
            kind: LoopKind::ForEach,
            condition: None,
            max_iterations: None,
            input: Some("${files}".into()),
            item_var: Some("item".into()),
        };
        let json = serde_json::to_value(&spec).unwrap();
        assert_eq!(json["kind"], "for_each");
        assert_eq!(json["maxIterations"], serde_json::Value::Null);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-core control_spec_roundtrips -- --nocapture`
Expected: FAIL（`ControlSpec`/`LoopSpec`/`FailureClass` 等未定义）

- [ ] **Step 3: 实现类型**

在 `crates/acos-core/src/types.rs` 的 `CirNodeKind` 之后、`CirNode` 之前插入：

```rust
// ── Control semantics ────────────────────────────────────────────────────────

/// Condition expression attached to a `Conditional` node.
///
/// Uses the safe `acos-expr` subset (`acos_core::expr`); no arbitrary code
/// and no fuzzy reference resolution.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConditionSpec {
    /// Expression, e.g. `exists(doc)` or `test.exit_code != 0`.
    pub expression: String,
}

/// Loop kind of a `LoopMap` node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoopKind {
    /// Evaluate condition first; run body while true.
    While,
    /// Run body first; exit when condition true.
    Until,
    /// Iterate over an env list binding `item_var` each round.
    ForEach,
}

/// Loop configuration of a `LoopMap` node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LoopSpec {
    /// Loop kind.
    pub kind: LoopKind,
    /// While/Until condition expression.
    pub condition: Option<String>,
    /// While/Until: required, >= 1. ForEach: `None` = whole input list.
    pub max_iterations: Option<u32>,
    /// ForEach: env reference to the input list (e.g. `"${files}"`).
    pub input: Option<String>,
    /// ForEach: name of the iteration variable bound in env.
    pub item_var: Option<String>,
}

/// Retry strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RetryStrategy {
    /// Fixed delay between attempts.
    Fixed,
}

/// Failure class used to decide retry/recovery behavior.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum FailureClass {
    /// Operation timed out.
    Timeout,
    /// Rate limited by an external system.
    RateLimit,
    /// Transient network error.
    TransientNetworkError,
    /// Input was invalid.
    InvalidInput,
    /// Permission denied.
    PermissionDenied,
    /// Syntax error in user-provided code.
    SyntaxError,
    /// Unclassifiable.
    Unknown,
}

/// Retry policy attached via [`ControlSpec::retry`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RetryPolicy {
    /// Total attempts including the first; must be >= 1 (0 rejected at compile).
    pub max_attempts: u32,
    /// Delay between attempts in milliseconds.
    pub backoff_ms: u64,
    /// Retry strategy (MVP: fixed delay only).
    pub strategy: RetryStrategy,
    /// Failure classes to retry; empty = all retryable classes.
    #[serde(default)]
    pub retry_on: Vec<FailureClass>,
}

/// Control semantics attached to a node — distinct from business `inputs`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ControlSpec {
    /// Condition for `Conditional` nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionSpec>,
    /// Loop config for `LoopMap` nodes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_spec: Option<LoopSpec>,
    /// Retry policy for any executable node.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,
}
```

修改 `CirNodeKind::Retry` 变体为 deprecated：

```rust
    /// Retry node.
    ///
    /// Deprecated: retry semantics are expressed via [`ControlSpec::retry`]
    /// since P0. Kept for wire compatibility with CIR v0.1; removed in CIR 2.0.
    #[deprecated(note = "use ControlSpec.retry")]
    Retry,
```

修改 `CirNode` 结构体（加 `else_children` 与 `control`）：

```rust
pub struct CirNode {
    /// Node kind.
    pub kind: CirNodeKind,
    /// Node id within the program.
    pub node_id: String,
    /// Invoked primitive capability id (e.g. `"read_file"`), if any.
    pub capability: Option<String>,
    /// Named output binding, if any.
    pub output: Option<String>,
    /// Child node ids (for sequence/parallel/conditional/loop).
    pub children: Vec<String>,
    /// False branch for `Conditional` nodes (true branch is `children`).
    #[serde(default)]
    pub else_children: Vec<String>,
    /// Input bindings for primitive invocations: param name -> literal or
    /// `$output_ref`.
    #[serde(default)]
    pub inputs: std::collections::HashMap<String, serde_json::Value>,
    /// Control semantics (condition/loop/retry), separate from `inputs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlSpec>,
}
```

- [ ] **Step 4: 修复所有 `CirNode {` 构造点**

Run: `rg -n "CirNode \{" crates/`
Expected: `acos-compiler/src/lib.rs`（RuleCompiler 5 处）等。给每个构造追加 `else_children: vec![],` 与 `control: None,`。若 `acos-server` / `e2e_mini.rs` 也有构造点，一并修复。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-core`
Expected: 3 个新测试 PASS + 既有测试 PASS

- [ ] **Step 6: 同步 cir.proto**

`schemas/cir/cir.proto` 中 `CirNode` 增加：

```proto
  repeated string else_children = 6;
  optional ControlSpec control = 7;
```

并在文件尾追加消息定义（注释注明 Retry deprecated）：

```proto
// Control semantics attached to a node (P0). Retry via CirNodeKind::RETRY is
// deprecated; use control.retry instead.
message ControlSpec {
  optional ConditionSpec condition = 1;
  optional LoopSpec loop_spec = 2;
  optional RetryPolicy retry = 3;
}

message ConditionSpec {
  string expression = 1;
}

enum LoopKind {
  LOOP_KIND_UNSPECIFIED = 0;
  LOOP_KIND_WHILE = 1;
  LOOP_KIND_UNTIL = 2;
  LOOP_KIND_FOR_EACH = 3;
}

message LoopSpec {
  LoopKind kind = 1;
  optional string condition = 2;
  optional uint32 max_iterations = 3;
  optional string input = 4;
  optional string item_var = 5;
}

enum RetryStrategy {
  RETRY_STRATEGY_UNSPECIFIED = 0;
  RETRY_STRATEGY_FIXED = 1;
}

enum FailureClass {
  FAILURE_CLASS_UNSPECIFIED = 0;
  FAILURE_CLASS_TIMEOUT = 1;
  FAILURE_CLASS_RATE_LIMIT = 2;
  FAILURE_CLASS_TRANSIENT_NETWORK_ERROR = 3;
  FAILURE_CLASS_INVALID_INPUT = 4;
  FAILURE_CLASS_PERMISSION_DENIED = 5;
  FAILURE_CLASS_SYNTAX_ERROR = 6;
  FAILURE_CLASS_UNKNOWN = 7;
}

message RetryPolicy {
  uint32 max_attempts = 1;
  uint64 backoff_ms = 2;
  RetryStrategy strategy = 3;
  repeated FailureClass retry_on = 4;
}
```

- [ ] **Step 7: 提交**

```bash
git add crates/acos-core/src/types.rs schemas/cir/cir.proto
git commit -m "feat(core): ControlSpec/LoopSpec/RetryPolicy/FailureClass 类型 + CirNode.control"
```

---

### Task 2: acos-core `expr` 模块（acos-expr）

**Files:**
- Create: `crates/acos-core/src/expr.rs`
- Modify: `crates/acos-core/src/lib.rs`
- Test: `crates/acos-core/src/expr.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 1 的 `TypedValue`、`AcosError`
- Produces: `expr::parse(&str) -> Result<Expr, AcosError>`、`expr::evaluate(&Expr, &HashMap<String, TypedValue>) -> Result<bool, AcosError>`、`expr::collect_identifiers(&Expr) -> Vec<String>`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{TypedValue, ValueType};

    fn env(pairs: &[(&str, Value)]) -> HashMap<String, TypedValue> {
        pairs.iter().map(|(k, v)| {
            (k.to_string(), TypedValue { value_type: ValueType::Scalar, payload: v.clone() })
        }).collect()
    }

    fn eval_str(input: &str, e: &HashMap<String, TypedValue>) -> bool {
        evaluate(&parse(input).unwrap(), e).unwrap()
    }

    #[test]
    fn evaluates_comparison_on_nested_field() {
        let e = env(&[("test", serde_json::json!({"exit_code": 1}))]);
        assert!(eval_str("test.exit_code != 0", &e));
        assert!(!eval_str("test.exit_code == 0", &e));
        assert!(eval_str("test.exit_code > 0 && test.exit_code <= 2", &e));
    }

    #[test]
    fn evaluates_exists_and_not_exists() {
        let e = env(&[("doc", serde_json::json!({"content": "x"}))]);
        assert!(eval_str("exists(doc)", &e));
        assert!(!eval_str("not_exists(doc)", &e));
        assert!(eval_str("not_exists(missing)", &e));
        assert!(eval_str("exists(doc.content)", &e));
    }

    #[test]
    fn literal_conditions_work() {
        let e = env(&[]);
        assert!(eval_str("1 == 1", &e));
        assert!(!eval_str("1 == 2", &e));
        assert!(eval_str("'ok' == 'ok'", &e));
        assert!(eval_str("true && !false", &e));
    }

    #[test]
    fn unknown_binding_is_an_error_not_false() {
        let e = env(&[]);
        let err = evaluate(&parse("undefined > 1").unwrap(), &e).unwrap_err();
        assert!(err.to_string().contains("undefined"));
    }

    #[test]
    fn collect_identifiers_returns_roots() {
        let expr = parse("exists(a) && b.x > 1 && !exists(c)").unwrap();
        let mut ids = collect_identifiers(&expr);
        ids.sort();
        assert_eq!(ids, vec!["a", "b", "c"]);
    }

    #[test]
    fn rejects_bare_identifier_condition() {
        assert!(parse("doc").is_err());
        assert!(parse("doc > ").is_err());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-core expr::tests -- --nocapture`
Expected: FAIL（`expr` 模块不存在）

- [ ] **Step 3: 实现 expr.rs**

```rust
//! Minimal condition expression language (`acos-expr`).
//!
//! Safe subset: identifier paths, literals, comparisons, existence checks,
//! and boolean combinators. **No arbitrary code execution** and **no fuzzy
//! reference resolution** — identifiers must resolve exactly in the env.

use std::collections::HashMap;

use serde_json::Value;

use crate::error::AcosError;
use crate::types::TypedValue;

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    /// Equal.
    Eq,
    /// Not equal.
    Ne,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
    /// Greater than or equal.
    Ge,
    /// Less than or equal.
    Le,
}

/// An operand: literal or identifier path.
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// Literal value.
    Literal(Value),
    /// Identifier path into the env.
    Path(Path),
}

/// Identifier path, e.g. `test.exit_code`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    /// Path segments; first is the env binding name.
    pub segments: Vec<String>,
}

/// Parsed expression tree.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// Logical and.
    And(Box<Expr>, Box<Expr>),
    /// Logical or.
    Or(Box<Expr>, Box<Expr>),
    /// Logical not.
    Not(Box<Expr>),
    /// Comparison.
    Cmp(Operand, CmpOp, Operand),
    /// Path resolves to a value.
    Exists(Path),
    /// Path does not resolve to a value.
    NotExists(Path),
}

/// Parses an expression string.
pub fn parse(input: &str) -> Result<Expr, AcosError> {
    let tokens = tokenize(input)?;
    let mut parser = Parser { tokens, pos: 0 };
    let expr = parser.parse_or()?;
    if !matches!(parser.peek(), Some(Token::Eof)) {
        return Err(err("unexpected token after expression"));
    }
    Ok(expr)
}

/// Evaluates an expression against the environment (exact identifier lookup).
pub fn evaluate(expr: &Expr, env: &HashMap<String, TypedValue>) -> Result<bool, AcosError> {
    match expr {
        Expr::And(a, b) => Ok(evaluate(a, env)? && evaluate(b, env)?),
        Expr::Or(a, b) => Ok(evaluate(a, env)? || evaluate(b, env)?),
        Expr::Not(a) => Ok(!evaluate(a, env)?),
        Expr::Cmp(l, op, r) => {
            let lv = resolve_operand(l, env)?;
            let rv = resolve_operand(r, env)?;
            compare(&lv, *op, &rv)
        }
        Expr::Exists(p) => Ok(resolve_path(p, env)?.is_some()),
        Expr::NotExists(p) => Ok(resolve_path(p, env)?.is_none()),
    }
}

/// Collects the root identifier of every path in the expression (for
/// compile-time validation).
pub fn collect_identifiers(expr: &Expr) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::And(a, b) | Expr::Or(a, b) => {
                walk(a, out);
                walk(b, out);
            }
            Expr::Not(a) => walk(a, out),
            Expr::Cmp(l, _, r) => {
                for op in [l, r] {
                    if let Operand::Path(p) = op {
                        out.push(p.segments[0].clone());
                    }
                }
            }
            Expr::Exists(p) | Expr::NotExists(p) => out.push(p.segments[0].clone()),
        }
    }
    walk(expr, &mut out);
    out
}

fn err(message: impl Into<String>) -> AcosError {
    AcosError::ValidationFailure { message: message.into() }
}

fn resolve_operand(op: &Operand, env: &HashMap<String, TypedValue>) -> Result<Value, AcosError> {
    match op {
        Operand::Literal(v) => Ok(v.clone()),
        Operand::Path(p) => resolve_path(p, env)?.ok_or_else(|| {
            err(format!("condition referenced unknown binding '{}'", p.segments[0]))
        }),
    }
}

fn resolve_path(p: &Path, env: &HashMap<String, TypedValue>) -> Result<Option<Value>, AcosError> {
    let root = p.segments.first().ok_or_else(|| err("empty path in condition"))?;
    let Some(tv) = env.get(root) else {
        return Ok(None);
    };
    let mut current = tv.payload.clone();
    for seg in &p.segments[1..] {
        match current {
            Value::Object(map) => {
                let Some(v) = map.get(seg) else { return Ok(None); };
                current = v.clone();
            }
            _ => return Ok(None),
        }
    }
    Ok(Some(current))
}

fn compare(l: &Value, op: CmpOp, r: &Value) -> Result<bool, AcosError> {
    match op {
        CmpOp::Eq => Ok(l == r),
        CmpOp::Ne => Ok(l != r),
        CmpOp::Gt | CmpOp::Lt | CmpOp::Ge | CmpOp::Le => {
            let (Some(ln), Some(rn)) = (l.as_f64(), r.as_f64()) else {
                return Err(err(format!(
                    "ordering comparison requires numbers, got {l} and {r}"
                )));
            };
            Ok(match op {
                CmpOp::Gt => ln > rn,
                CmpOp::Lt => ln < rn,
                CmpOp::Ge => ln >= rn,
                CmpOp::Le => ln <= rn,
                CmpOp::Eq | CmpOp::Ne => unreachable!(),
            })
        }
    }
}

// ── tokenizer ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String),
    Number(f64),
    Str(String),
    True,
    False,
    Eq,
    Ne,
    Gt,
    Lt,
    Ge,
    Le,
    And,
    Or,
    Not,
    LParen,
    RParen,
    Dot,
    Eof,
}

fn tokenize(input: &str) -> Result<Vec<Token>, AcosError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\n' => {
                chars.next();
            }
            '(' => {
                tokens.push(Token::LParen);
                chars.next();
            }
            ')' => {
                tokens.push(Token::RParen);
                chars.next();
            }
            '.' => {
                tokens.push(Token::Dot);
                chars.next();
            }
            '=' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Eq);
                } else {
                    return Err(err("expected '=='"));
                }
            }
            '!' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ne);
                } else {
                    tokens.push(Token::Not);
                }
            }
            '>' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Ge);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    tokens.push(Token::Le);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else {
                    return Err(err("expected '&&'"));
                }
            }
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    return Err(err("expected '||'"));
                }
            }
            '\'' => {
                chars.next();
                let mut s = String::new();
                for c in chars.by_ref() {
                    if c == '\'' {
                        break;
                    }
                    s.push(c);
                }
                tokens.push(Token::Str(s));
            }
            c if c.is_ascii_digit() => {
                let mut s = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_ascii_digit() || d == '.' {
                        s.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                let n: f64 = s.parse().map_err(|_| err(format!("invalid number '{s}'")))?;
                tokens.push(Token::Number(n));
            }
            c if c.is_alphabetic() || c == '_' => {
                let mut s = String::new();
                while let Some(&d) = chars.peek() {
                    if d.is_alphanumeric() || d == '_' {
                        s.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(match s.as_str() {
                    "true" => Token::True,
                    "false" => Token::False,
                    _ => Token::Ident(s),
                });
            }
            other => return Err(err(format!("unexpected character '{other}'"))),
        }
    }
    tokens.push(Token::Eof);
    Ok(tokens)
}

// ── parser ───────────────────────────────────────────────────────────────────

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Token> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expect(&mut self, t: &Token) -> Result<(), AcosError> {
        if self.peek() == Some(t) {
            self.pos += 1;
            Ok(())
        } else {
            Err(err(format!("expected {t:?}, found {:?}", self.peek())))
        }
    }

    fn parse_or(&mut self) -> Result<Expr, AcosError> {
        let mut left = self.parse_and()?;
        while self.peek() == Some(&Token::Or) {
            self.pos += 1;
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, AcosError> {
        let mut left = self.parse_not()?;
        while self.peek() == Some(&Token::And) {
            self.pos += 1;
            let right = self.parse_not()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<Expr, AcosError> {
        if self.peek() == Some(&Token::Not) {
            self.pos += 1;
            Ok(Expr::Not(Box::new(self.parse_not()?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, AcosError> {
        match self.peek() {
            Some(Token::LParen) => {
                self.pos += 1;
                let e = self.parse_or()?;
                self.expect(&Token::RParen)?;
                Ok(e)
            }
            Some(Token::Ident(name)) if name == "exists" || name == "not_exists" => {
                let not = name == "not_exists";
                self.pos += 1;
                self.expect(&Token::LParen)?;
                let path = self.parse_path()?;
                self.expect(&Token::RParen)?;
                if not {
                    Ok(Expr::NotExists(path))
                } else {
                    Ok(Expr::Exists(path))
                }
            }
            Some(Token::Ident(_)) => {
                let operand = self.parse_operand()?;
                self.finish_comparison(operand)
            }
            Some(Token::Number(_))
            | Some(Token::Str(_))
            | Some(Token::True)
            | Some(Token::False) => {
                let operand = self.parse_operand()?;
                self.finish_comparison(operand)
            }
            other => Err(err(format!("unexpected token {other:?}"))),
        }
    }

    fn finish_comparison(&mut self, left: Operand) -> Result<Expr, AcosError> {
        let op = match self.peek() {
            Some(Token::Eq) => CmpOp::Eq,
            Some(Token::Ne) => CmpOp::Ne,
            Some(Token::Gt) => CmpOp::Gt,
            Some(Token::Lt) => CmpOp::Lt,
            Some(Token::Ge) => CmpOp::Ge,
            Some(Token::Le) => CmpOp::Le,
            Some(Token::Eof) | Some(Token::RParen) | Some(Token::And) | Some(Token::Or) => {
                return Err(err("bare operand in condition; use exists(...) or a comparison"));
            }
            _ => return Err(err("expected comparison operator")),
        };
        self.pos += 1;
        let right = self.parse_operand()?;
        Ok(Expr::Cmp(left, op, right))
    }

    fn parse_operand(&mut self) -> Result<Operand, AcosError> {
        match self.next() {
            Some(Token::Number(n)) => Ok(Operand::Literal(Value::from(n))),
            Some(Token::Str(s)) => Ok(Operand::Literal(Value::String(s))),
            Some(Token::True) => Ok(Operand::Literal(Value::Bool(true))),
            Some(Token::False) => Ok(Operand::Literal(Value::Bool(false))),
            Some(Token::Ident(name)) => {
                let mut segments = vec![name];
                while self.peek() == Some(&Token::Dot) {
                    self.pos += 1;
                    match self.next() {
                        Some(Token::Ident(seg)) => segments.push(seg),
                        _ => return Err(err("expected identifier after '.'")),
                    }
                }
                Ok(Operand::Path(Path { segments }))
            }
            other => Err(err(format!("expected operand, found {other:?}"))),
        }
    }

    fn parse_path(&mut self) -> Result<Path, AcosError> {
        match self.parse_operand()? {
            Operand::Path(p) => Ok(p),
            _ => Err(err("exists() requires an identifier path")),
        }
    }
}
```

- [ ] **Step 4: lib.rs 导出**

`crates/acos-core/src/lib.rs` 增加：

```rust
pub mod expr;
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-core expr::tests`
Expected: 全部 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/acos-core/src/expr.rs crates/acos-core/src/lib.rs
git commit -m "feat(core): acos-expr 最小条件表达式（parse/evaluate/collect_identifiers）"
```

---

### Task 3: acos-core 失败分类 + `Primitive::idempotent()`

**Files:**
- Modify: `crates/acos-core/src/error.rs`
- Modify: `crates/acos-core/src/traits.rs`
- Modify: `crates/acos-plugin/src/primitives.rs`
- Test: `crates/acos-core/src/error.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 1 的 `FailureClass`
- Produces: `AcosError::PrimitiveFailure{message, primitive_id, class}`（新必填字段）、`AcosError::classify(&self) -> FailureClass`、`Primitive::idempotent(&self) -> bool`（默认 false）

- [ ] **Step 1: 写失败测试**

`error.rs` 追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FailureClass;

    #[test]
    fn classifies_primitive_failure_by_declared_class() {
        let e = AcosError::PrimitiveFailure {
            message: "timed out".into(),
            primitive_id: Some("search".into()),
            class: FailureClass::Timeout,
        };
        assert_eq!(e.classify(), FailureClass::Timeout);
    }

    #[test]
    fn classifies_provider_failure_as_transient() {
        let e = AcosError::ProviderFailure {
            message: "provider down".into(),
            provider: "x".into(),
        };
        assert_eq!(e.classify(), FailureClass::TransientNetworkError);
    }

    #[test]
    fn unknown_errors_classify_as_unknown() {
        let e = AcosError::Internal { message: "boom".into() };
        assert_eq!(e.classify(), FailureClass::Unknown);
    }

    #[test]
    fn primitive_failure_serializes_class_with_default_unknown() {
        let json = r#"{"message":"m","primitive_id":null}"#;
        let e: AcosError = serde_json::from_str(json).unwrap();
        assert_eq!(e.classify(), FailureClass::Unknown);
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-core error::tests`
Expected: FAIL（`class` 字段不存在）

- [ ] **Step 3: 实现**

`error.rs` 修改 `PrimitiveFailure` 变体：

```rust
    /// A primitive operation failed (e.g. `read_file` could not read).
    #[error("primitive failure: {message} (primitive={primitive_id:?}, class={class:?})")]
    PrimitiveFailure {
        /// Human-readable message.
        message: String,
        /// Which primitive failed, if known.
        primitive_id: Option<String>,
        /// Failure class driving retry/recovery decisions.
        #[serde(default)]
        class: crate::types::FailureClass,
    },
```

`error.rs` 的 `impl AcosError` 增加：

```rust
    /// Classifies this error for retry/recovery decisions.
    ///
    /// This is the single classification entry point; runtime code must not
    /// pattern-match on error strings.
    pub fn classify(&self) -> crate::types::FailureClass {
        use crate::types::FailureClass;
        match self {
            AcosError::PrimitiveFailure { class, .. } => class.clone(),
            AcosError::ProviderFailure { .. } => FailureClass::TransientNetworkError,
            _ => FailureClass::Unknown,
        }
    }
```

- [ ] **Step 4: 修复所有 `PrimitiveFailure {` 构造点**

Run: `rg -n "PrimitiveFailure \{" crates/`
Expected: `acos-plugin/src/primitives.rs` 多处。每处追加 `class: FailureClass::Unknown,`（import 补充 `FailureClass`）。若 server/其他 crate 有构造点一并修复。

`traits.rs` 的 `Primitive` trait 增加（默认方法，无需改实现）：

```rust
    /// Returns whether repeating this primitive is safe (idempotent).
    ///
    /// Defaults to `false`. Primitives that can be safely re-invoked without
    /// duplicated side effects (e.g. pure reads) may override this.
    fn idempotent(&self) -> bool {
        false
    }
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-core && cargo test -p acos-plugin`
Expected: 全部 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/acos-core/src/error.rs crates/acos-core/src/traits.rs crates/acos-plugin/src/primitives.rs
git commit -m "feat(core): FailureClass 分类入口 + Primitive::idempotent()"
```

---

### Task 4: acos-core 恢复类型与 Trait

**Files:**
- Modify: `crates/acos-core/src/types.rs`
- Modify: `crates/acos-core/src/traits.rs`
- Modify: `crates/acos-core/src/lib.rs`
- Test: `crates/acos-core/src/types.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 1/3 类型
- Produces: `FailureContext{run_id,node_id,error_class,error_message,attempts,recent_events}`、`RecoveryProposal{replace_node,subgraph,reason}`、`trait Replanner::propose(&FailureContext,&CirProgram)->Option<RecoveryProposal>`、`#[async_trait] trait ModelReplanner::propose(...)->Result<Option<RecoveryProposal>,AcosError>`、`RecoveryContext<'a>{rule,model}`

- [ ] **Step 1: 写失败测试**

`types.rs` tests 追加：

```rust
    #[test]
    fn recovery_proposal_roundtrips() {
        let p = RecoveryProposal {
            replace_node: "B".into(),
            subgraph: vec![],
            reason: "fallback".into(),
        };
        let json = serde_json::to_string(&p).unwrap();
        let back: RecoveryProposal = serde_json::from_str(&json).unwrap();
        assert_eq!(back, p);
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-core recovery_proposal_roundtrips`
Expected: FAIL

- [ ] **Step 3: 实现 types.rs 追加**

```rust
// ── Failure recovery ─────────────────────────────────────────────────────────

/// Context describing a runtime failure, passed to replanners.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct FailureContext {
    /// The failing run.
    pub run_id: RunId,
    /// Id of the deepest failing node.
    pub node_id: String,
    /// Classified failure class.
    pub error_class: FailureClass,
    /// Human-readable error message.
    pub error_message: String,
    /// Recovery attempts already consumed.
    pub attempts: u32,
    /// Most recent events of the run (newest first, up to 5).
    pub recent_events: Vec<crate::traits::Event>,
}

/// A recovery patch proposal produced by a replanner.
///
/// The runtime validates and commits it transactionally; the subgraph root
/// MUST reuse [`Self::replace_node`] as its node id so upstream/downstream
/// references stay intact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryProposal {
    /// Id of the failing node being replaced.
    pub replace_node: String,
    /// Replacement subgraph; root keeps `replace_node`'s id.
    pub subgraph: Vec<CirNode>,
    /// Human-readable reason.
    pub reason: String,
}
```

- [ ] **Step 4: traits.rs 追加（文件末尾）**

```rust
// ── Recovery replanners ──────────────────────────────────────────────────────

/// Deterministic failure recovery planner (rule-based, no external deps).
pub trait Replanner: Send + Sync + std::fmt::Debug {
    /// Proposes a recovery patch for a failure, or `None` if this replanner
    /// cannot handle it.
    fn propose(&self, failure: &FailureContext, program: &CirProgram)
        -> Option<RecoveryProposal>;
}

/// Model-driven recovery planner (LLM generates recovery subgraphs).
#[async_trait]
pub trait ModelReplanner: Send + Sync + std::fmt::Debug {
    /// Proposes a recovery patch for a failure, or `None` if unavailable.
    async fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Result<Option<RecoveryProposal>, AcosError>;
}

/// Recovery strategies wired into one execution.
#[derive(Debug, Default)]
pub struct RecoveryContext<'a> {
    /// Deterministic rule replanner (tried first).
    pub rule: Option<&'a dyn Replanner>,
    /// Model replanner (tried when rules cannot fix).
    pub model: Option<&'a dyn ModelReplanner>,
}
```

`traits.rs` 顶部 import 补充：

```rust
use crate::types::{
    CirProgram, EffectDecl, FailureContext, RecoveryProposal, TaskSpec, TypedValue,
};
```

- [ ] **Step 5: lib.rs 导出**

```rust
pub use types::{
    CirProgram, EffectDecl, EffectKind, Task, TaskSpec, TypedValue, ValueType, ConditionSpec,
    ControlSpec, FailureClass, FailureContext, LoopKind, LoopSpec, RecoveryProposal, RetryPolicy,
    RetryStrategy,
};
```

- [ ] **Step 6: 运行测试**

Run: `cargo test -p acos-core`
Expected: 全部 PASS

- [ ] **Step 7: 提交**

```bash
git add crates/acos-core/src/types.rs crates/acos-core/src/traits.rs crates/acos-core/src/lib.rs
git commit -m "feat(core): FailureContext/RecoveryProposal + Replanner/ModelReplanner traits"
```

---

### Task 5: acos-compiler 校验扩展 + 提示词更新

**Files:**
- Modify: `crates/acos-compiler/src/lib.rs`
- Test: `crates/acos-compiler/src/lib.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 1/2 类型与 `expr::parse/collect_identifiers`
- Produces: `pub fn validate_cir(&CirProgram) -> Result<(), AcosError>`（语义校验：conditional/loop/retry/else_children 规则）

- [ ] **Step 1: 写失败测试**

`lib.rs` tests 追加：

```rust
    fn program_with(nodes: Vec<CirNode>) -> CirProgram {
        CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: nodes.iter().map(|n| n.node_id.clone()).collect::<Vec<_>>(),
            nodes,
            effects: vec![],
        }
    }

    fn primitive_node(id: &str) -> CirNode {
        CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: id.into(),
            capability: Some("search".into()),
            output: Some(format!("out_{id}")),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        }
    }

    #[test]
    fn validate_accepts_valid_conditional() {
        let cond = CirNode {
            kind: CirNodeKind::Conditional,
            node_id: "check".into(),
            capability: None,
            output: None,
            children: vec!["then".into()],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: Some(ConditionSpec { expression: "exists(out_search)".into() }),
                loop_spec: None,
                retry: None,
            }),
        };
        let program = program_with(vec![primitive_node("search"), primitive_node("then"), cond]);
        assert!(validate_cir(&program).is_ok());
    }

    #[test]
    fn validate_rejects_loop_without_max_iterations() {
        let loop_node = CirNode {
            kind: CirNodeKind::LoopMap,
            node_id: "loop".into(),
            capability: None,
            output: None,
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: Some(LoopSpec {
                    kind: LoopKind::While,
                    condition: Some("1 == 1".into()),
                    max_iterations: None,
                    input: None,
                    item_var: None,
                }),
                retry: None,
            }),
        };
        let err = validate_cir(&program_with(vec![loop_node])).unwrap_err();
        assert!(err.to_string().contains("max_iterations"));
    }

    #[test]
    fn validate_rejects_retry_zero_attempts() {
        let node = CirNode {
            control: Some(ControlSpec {
                condition: None,
                loop_spec: None,
                retry: Some(RetryPolicy {
                    max_attempts: 0,
                    backoff_ms: 1,
                    strategy: RetryStrategy::Fixed,
                    retry_on: vec![],
                }),
            }),
            ..primitive_node("p")
        };
        let err = validate_cir(&program_with(vec![node])).unwrap_err();
        assert!(err.to_string().contains("max_attempts"));
    }

    #[test]
    fn validate_rejects_condition_with_undeclared_identifier() {
        let cond = CirNode {
            kind: CirNodeKind::Conditional,
            node_id: "check".into(),
            capability: None,
            output: None,
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: Some(ConditionSpec { expression: "exists(test.exit_cod)".into() }),
                loop_spec: None,
                retry: None,
            }),
        };
        let err = validate_cir(&program_with(vec![primitive_node("search"), cond])).unwrap_err();
        assert!(err.to_string().contains("undeclared identifier"));
    }

    #[test]
    fn validate_rejects_else_children_on_non_conditional() {
        let node = CirNode {
            else_children: vec!["x".into()],
            ..primitive_node("p")
        };
        let err = validate_cir(&program_with(vec![node])).unwrap_err();
        assert!(err.to_string().contains("else_children"));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-compiler validate_`
Expected: FAIL（`ControlSpec` 未导入 / `validate_cir` 无语义校验）

- [ ] **Step 3: 实现**

import 更新：

```rust
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, ConditionSpec, ControlSpec, EffectDecl, EffectKind,
    LoopKind, LoopSpec, RetryPolicy, TaskSpec,
};
```

`validate_cir` 改为 `pub`，并在结构检查后调用新函数：

```rust
/// Validates structural and control-semantic invariants of a CIR program.
///
/// Structural: entry/children references exist. Control-semantic: every
/// Conditional has `control.condition` with statically resolvable
/// identifiers, every LoopMap has a valid `control.loop_spec`, retry
/// policies are sane, and `else_children` is only used on Conditional nodes.
pub fn validate_cir(program: &CirProgram) -> Result<(), AcosError> {
    // ...existing structural checks unchanged...
    validate_control_semantics(program)?;
    Ok(())
}

fn vf(message: impl Into<String>) -> AcosError {
    AcosError::ValidationFailure { message: message.into() }
}

fn validate_control_semantics(program: &CirProgram) -> Result<(), AcosError> {
    let outputs: std::collections::HashSet<&str> = program
        .nodes
        .iter()
        .filter_map(|n| n.output.as_deref())
        .collect();

    for node in &program.nodes {
        if !matches!(node.kind, CirNodeKind::Conditional) && !node.else_children.is_empty() {
            return Err(vf(format!(
                "node '{}' uses else_children but is not conditional",
                node.node_id
            )));
        }
        match node.kind {
            CirNodeKind::Conditional => {
                let cond = node
                    .control
                    .as_ref()
                    .and_then(|c| c.condition.as_ref())
                    .ok_or_else(|| {
                        vf(format!("conditional node '{}' has no control.condition", node.node_id))
                    })?;
                let expr = acos_core::expr::parse(&cond.expression)
                    .map_err(|e| vf(format!("conditional node '{}': {e}", node.node_id)))?;
                for id in acos_core::expr::collect_identifiers(&expr) {
                    if !outputs.contains(id.as_str()) {
                        return Err(vf(format!(
                            "conditional node '{}' references undeclared identifier '{id}'",
                            node.node_id
                        )));
                    }
                }
            }
            CirNodeKind::LoopMap => {
                let spec = node
                    .control
                    .as_ref()
                    .and_then(|c| c.loop_spec.as_ref())
                    .ok_or_else(|| {
                        vf(format!("loop node '{}' has no control.loop_spec", node.node_id))
                    })?;
                match spec.kind {
                    LoopKind::While | LoopKind::Until => {
                        if spec.condition.is_none() {
                            return Err(vf(format!(
                                "loop node '{}' must set control.loop_spec.condition for {:?}",
                                node.node_id, spec.kind
                            )));
                        }
                        if spec.max_iterations.is_none() {
                            return Err(vf(format!(
                                "loop node '{}' must set max_iterations for {:?} (termination guarantee)",
                                node.node_id, spec.kind
                            )));
                        }
                    }
                    LoopKind::ForEach => {
                        if spec.input.is_none() || spec.item_var.is_none() {
                            return Err(vf(format!(
                                "loop node '{}' must set input and item_var for for_each",
                                node.node_id
                            )));
                        }
                    }
                }
                if spec.max_iterations == Some(0) {
                    return Err(vf(format!(
                        "loop node '{}' max_iterations must be >= 1",
                        node.node_id
                    )));
                }
            }
            _ => {}
        }
        if let Some(retry) = node.control.as_ref().and_then(|c| c.retry.as_ref()) {
            if retry.max_attempts == 0 {
                return Err(vf(format!(
                    "node '{}' retry.max_attempts must be >= 1",
                    node.node_id
                )));
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: 更新 PLANNER_SYSTEM_PROMPT**

`lib.rs` 常量 `PLANNER_SYSTEM_PROMPT` 中：
- 规则 2 的 kind 列表改为：`sequence, parallel, conditional, loop_map, primitive_invocation`
- 追加新规则：

```
10. Control semantics: conditions, loops, and retries are expressed via the
    node-level `control` object, NEVER via extra `inputs` keys and NEVER via a
    node kind of `retry`:
    - conditional: { "kind": "conditional", "control": { "condition": { "expression": "exists(doc)" } }, "children": [then...], "elseChildren": [else...] }
    - loop_map: { "kind": "loop_map", "control": { "loopSpec": { "kind": "while|until|for_each", "condition": "...", "maxIterations": 5, "input": "${files}", "itemVar": "item" } }, "children": [body...] }
      while/until MUST set maxIterations; for_each uses input + itemVar.
    - retry: attach "control": { "retry": { "maxAttempts": 3, "backoffMs": 200, "strategy": "fixed", "retryOn": ["timeout", "rate_limit", "transient_network_error"] } } to the executable node.
    Expression language (acos-expr): exists(name), not_exists(name), field paths
    like `test.exit_code`, comparisons == != > < >= <=, && || !, string literals
    in single quotes, numbers, true/false. Only reference `output` names that
    other nodes in the graph declare.
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-compiler`
Expected: 全部 PASS（含 5 个新校验测试）

- [ ] **Step 6: 提交**

```bash
git add crates/acos-compiler/src/lib.rs
git commit -m "feat(compiler): validate_cir 控制语义校验 + 提示词教学 control 语法"
```

---

### Task 6: acos-runtime 控制语义执行

**Files:**
- Modify: `crates/acos-runtime/src/lib.rs`
- Test: `crates/acos-runtime/src/lib.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 1/2/3（`expr::parse/evaluate`、`ControlSpec`、`e.classify()`）
- Produces: `run_nodes/run_node` 错误签名改为 `Result<_, (String, AcosError)>`（String = 最深失败节点 id）；控制语义（conditional/loop/retry）+ 事件 `iteration.started/completed`、`retry.started/exhausted`；`retry_safe` 安全门

- [ ] **Step 1: 写失败测试**

`lib.rs` tests 追加（注意：tests 已有 `csv_task` 与既有执行测试）：

```rust
    fn primitive_node(id: &str, capability: &str, output: Option<&str>) -> CirNode {
        CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: id.into(),
            capability: Some(capability.into()),
            output: output.map(String::from),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        }
    }

    fn program_from(nodes: Vec<CirNode>) -> CirProgram {
        CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["root".into()],
            nodes,
            effects: vec![],
        }
    }

    async fn events_for(program: &CirProgram) -> Vec<String> {
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let report = runtime.execute(program.clone()).await.unwrap();
        store
            .query(report.run_id)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect()
    }

    #[tokio::test]
    async fn conditional_selects_then_branch() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                children: vec!["search".into(), "check".into(), "then_summarize".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            primitive_node("search", "search", Some("results")),
            CirNode {
                kind: CirNodeKind::Conditional,
                node_id: "check".into(),
                capability: None,
                output: None,
                children: vec!["then_summarize".into()],
                else_children: vec!["else_write".into()],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: Some(ConditionSpec { expression: "exists(results)".into() }),
                    loop_spec: None,
                    retry: None,
                }),
            },
            primitive_node("then_summarize", "summarize", Some("summary")),
            primitive_node("else_write", "write_file", Some("ref")),
        ];
        let events = events_for(&program_from(nodes)).await;
        let then = events.iter().filter(|t| *t == "primitive.end").count();
        assert!(then >= 2, "then branch should run summarize (+search)");
    }

    #[tokio::test]
    async fn conditional_selects_else_branch_on_false_condition() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                children: vec!["check".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::Conditional,
                node_id: "check".into(),
                capability: None,
                output: None,
                children: vec!["then_summarize".into()],
                else_children: vec!["else_write".into()],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: Some(ConditionSpec { expression: "1 == 2".into() }),
                    loop_spec: None,
                    retry: None,
                }),
            },
            primitive_node("then_summarize", "summarize", Some("summary")),
            primitive_node("else_write", "write_file", Some("ref")),
        ];
        let dir = std::env::temp_dir().join(format!("acos-else-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut nodes = nodes;
        let write = nodes.iter_mut().find(|n| n.node_id == "else_write").unwrap();
        write.inputs.insert(
            "path".into(),
            serde_json::Value::String(dir.join("out.txt").to_string_lossy().to_string()),
        );
        write.inputs.insert("content".into(), serde_json::Value::String("fallback".into()));
        let program = program_from(nodes);
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let report = runtime.execute(program).await.unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        assert!(
            report.artifacts.contains(&dir.join("out.txt").to_string_lossy().to_string()),
            "else branch write_file must produce the artifact"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn for_each_loop_over_empty_list_is_ok() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                children: vec!["search".into(), "loop".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            primitive_node("search", "search", Some("items")),
            CirNode {
                kind: CirNodeKind::LoopMap,
                node_id: "loop".into(),
                capability: None,
                output: None,
                children: vec!["body".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: None,
                    loop_spec: Some(LoopSpec {
                        kind: LoopKind::ForEach,
                        condition: None,
                        max_iterations: None,
                        input: Some("${items}".into()),
                        item_var: Some("item".into()),
                    }),
                    retry: None,
                }),
            },
            primitive_node("body", "summarize", Some("summary")),
        ];
        let events = events_for(&program_from(nodes)).await;
        assert_eq!(events.iter().filter(|t| *t == "iteration.started").count(), 0);
    }

    #[tokio::test]
    async fn while_loop_hits_limit_and_fails() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::LoopMap,
                node_id: "root".into(),
                capability: None,
                output: None,
                children: vec!["body".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: None,
                    loop_spec: Some(LoopSpec {
                        kind: LoopKind::While,
                        condition: Some("1 == 1".into()),
                        max_iterations: Some(2),
                        input: None,
                        item_var: None,
                    }),
                    retry: None,
                }),
            },
            primitive_node("body", "search", Some("r")),
        ];
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let err = runtime.execute(program_from(nodes)).await.unwrap_err();
        assert!(err.to_string().contains("max_iterations"));
    }

    #[tokio::test]
    async fn retry_recovers_transient_failure_then_succeeds() {
        let nodes = vec![CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "root".into(),
            capability: Some("flaky_search".into()),
            output: Some("r".into()),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: None,
                retry: Some(RetryPolicy {
                    max_attempts: 3,
                    backoff_ms: 1,
                    strategy: RetryStrategy::Fixed,
                    retry_on: vec![FailureClass::Timeout],
                }),
            }),
        }];
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let report = runtime.execute(program_from(nodes)).await.unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        let events = store.query(report.run_id).await.unwrap();
        assert!(events.iter().any(|e| e.event_type == "retry.started"));
    }
```

在 tests 模块内定义 flaky 原语与注册表（本任务内联，Task 10 正式引入 bench registry）：

```rust
    use acos_core::traits::{CapabilityDesc, PluginRegistry, Primitive, PrimitiveManifest};
    use acos_core::id::PrimitiveId;

    #[derive(Debug)]
    struct FlakySearchPrimitive {
        remaining: std::sync::atomic::AtomicUsize,
    }

    impl FlakySearchPrimitive {
        fn new(failures: usize) -> Self {
            Self { remaining: std::sync::atomic::AtomicUsize::new(failures) }
        }
    }

    #[async_trait]
    impl Primitive for FlakySearchPrimitive {
        fn capability(&self) -> CapabilityDesc {
            CapabilityDesc {
                id: "flaky_search".into(),
                name: "Flaky Search".into(),
                input_type: "SearchQuery".into(),
                output_type: "DocumentList".into(),
            }
        }

        fn effects(&self) -> Vec<EffectDecl> {
            vec![EffectDecl {
                kind: EffectKind::NetworkRead,
                description: "network read".into(),
                reversible: true,
            }]
        }

        async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
            if self.remaining.load(std::sync::atomic::Ordering::SeqCst) > 0 {
                self.remaining.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                return Err(AcosError::PrimitiveFailure {
                    message: "simulated timeout".into(),
                    primitive_id: Some("flaky_search".into()),
                    class: FailureClass::Timeout,
                });
            }
            Ok(TypedValue {
                value_type: ValueType::List,
                payload: serde_json::json!([]),
            })
        }

        fn has_compensation(&self, _e: &EffectDecl) -> bool {
            false
        }

        async fn compensate(&self, _e: &EffectDecl, _i: TypedValue) -> Result<(), AcosError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FlakyRegistry;

    impl FlakyRegistry {
        fn new() -> Self {
            Self
        }
    }

    #[async_trait]
    impl PluginRegistry for FlakyRegistry {
        fn list(&self) -> Vec<CapabilityDesc> {
            vec![FlakySearchPrimitive::new(0).capability()]
        }

        async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError> {
            if capability_id == "flaky_search" {
                Ok(Box::new(FlakySearchPrimitive::new(1)))
            } else {
                Err(AcosError::ValidationFailure {
                    message: format!("unknown: {capability_id}"),
                })
            }
        }

        async fn load(&self, _m: PrimitiveManifest) -> Result<PrimitiveId, AcosError> {
            Ok(PrimitiveId::new())
        }

        async fn unload(&self, _id: PrimitiveId) -> Result<(), AcosError> {
            Ok(())
        }
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-runtime`
Expected: FAIL（新类型/新语义未实现）

- [ ] **Step 3: 实现控制语义**

import 更新：

```rust
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, ConditionSpec, ControlSpec, EffectDecl, EffectKind,
    FailureClass, LoopKind, LoopSpec, RetryPolicy, RetryStrategy, TypedValue, ValueType,
};
use acos_core::expr;
```

`run_nodes` 签名改为错误对（内部逻辑不变，`?` 现在传播 `(String, AcosError)`）：

```rust
    async fn run_nodes(
        &self,
        node_map: &HashMap<String, CirNode>,
        entries: &[String],
        run_id: RunId,
        env: Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> Result<(Vec<String>, Vec<Evidence>), (String, AcosError)> {
        // 内部逻辑与原来一致
    }
```

`run_node` 改为：先走 retry 包装，再分发（新增 `run_node_inner`）：

```rust
    /// Executes a single node (with its `control.retry` policy applied).
    #[allow(clippy::too_many_arguments)]
    async fn run_node(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        let retry = node.control.as_ref().and_then(|c| c.retry.clone());
        if let Some(policy) = retry {
            let mut attempt = 0u32;
            loop {
                attempt += 1;
                match self
                    .run_node_inner(node, run_id, env, artifacts, evidence, node_map)
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err((failing_id, e)) => {
                        let class = e.classify();
                        let retryable =
                            policy.retry_on.is_empty() || policy.retry_on.contains(&class);
                        let safe = self.retry_safe(node).await;
                        if attempt >= policy.max_attempts || !retryable || !safe {
                            if attempt > 1 {
                                self.event_store
                                    .append(
                                        run_id,
                                        "retry.exhausted".into(),
                                        serde_json::json!({
                                            "node_id": &node.node_id,
                                            "attempts": attempt,
                                        }),
                                    )
                                    .await
                                    .ok();
                            }
                            return Err((failing_id, e));
                        }
                        self.event_store
                            .append(
                                run_id,
                                "retry.started".into(),
                                serde_json::json!({
                                    "node_id": &node.node_id,
                                    "attempt": attempt,
                                    "class": format!("{class:?}"),
                                }),
                            )
                            .await
                            .ok();
                        tokio::time::sleep(std::time::Duration::from_millis(policy.backoff_ms))
                            .await;
                    }
                }
            }
        }
        self.run_node_inner(node, run_id, env, artifacts, evidence, node_map).await
    }

    /// Retry-safety gate: only pure-read effects or explicitly idempotent
    /// primitives may be auto-retried (conservative MVP rule).
    async fn retry_safe(&self, node: &CirNode) -> bool {
        if node.kind != CirNodeKind::PrimitiveInvocation {
            return true;
        }
        let Some(capability) = node.capability.as_deref() else {
            return true;
        };
        let Ok(primitive) = self.registry.resolve(capability).await else {
            return false;
        };
        primitive.idempotent()
            || primitive.effects().iter().all(|e| {
                matches!(e.kind, EffectKind::FsRead | EffectKind::NetworkRead)
            })
    }

    /// Executes a single node by kind (no retry policy applied).
    #[allow(clippy::too_many_arguments)]
    async fn run_node_inner(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        match node.kind {
            CirNodeKind::Sequence | CirNodeKind::Parallel => {
                self.event_store
                    .append(
                        run_id,
                        "node.start".into(),
                        serde_json::json!({
                            "node_id": &node.node_id,
                            "kind": format!("{:?}", node.kind),
                        }),
                    )
                    .await
                    .map_err(|e| (node.node_id.clone(), e))?;
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        (
                            node.node_id.clone(),
                            AcosError::RuntimeInfrastructureFailure {
                                message: format!("child node {child_id} not found"),
                            },
                        )
                    })?;
                    Box::pin(
                        self.run_node(child, run_id, env, artifacts, evidence, node_map),
                    )
                    .await?;
                }
                Ok(())
            }
            CirNodeKind::PrimitiveInvocation => {
                self.run_primitive(node, run_id, env, artifacts, evidence)
                    .await
                    .map_err(|e| (node.node_id.clone(), e))
            }
            CirNodeKind::Conditional => {
                self.run_conditional(node, run_id, env, artifacts, evidence, node_map)
                    .await
            }
            CirNodeKind::LoopMap => {
                self.run_loop(node, run_id, env, artifacts, evidence, node_map).await
            }
            _ => {
                // Checkpoint / Verification / ArtifactRef / Retry (deprecated):
                // passthrough children, unchanged.
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        (
                            node.node_id.clone(),
                            AcosError::RuntimeInfrastructureFailure {
                                message: format!("child node {child_id} not found"),
                            },
                        )
                    })?;
                    Box::pin(
                        self.run_node(child, run_id, env, artifacts, evidence, node_map),
                    )
                    .await?;
                }
                Ok(())
            }
        }
    }
```

新增 `run_conditional` 与 `run_loop`（放在 `run_primitive` 之后）：

```rust
    /// Executes a conditional node: evaluates `control.condition` and walks
    /// `children` (true) or `else_children` (false).
    #[allow(clippy::too_many_arguments)]
    async fn run_conditional(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        let cond = node
            .control
            .as_ref()
            .and_then(|c| c.condition.as_ref())
            .ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::ValidationFailure {
                        message: format!(
                            "conditional node {} has no control.condition",
                            node.node_id
                        ),
                    },
                )
            })?;
        let expr = expr::parse(&cond.expression).map_err(|e| (node.node_id.clone(), e))?;
        let branch = {
            let guard = env.lock().await;
            expr::evaluate(&expr, &guard).map_err(|e| (node.node_id.clone(), e))?
        };
        self.event_store
            .append(
                run_id,
                "node.start".into(),
                serde_json::json!({
                    "node_id": &node.node_id,
                    "kind": "conditional",
                    "branch": if branch { "then" } else { "else" },
                }),
            )
            .await
            .map_err(|e| (node.node_id.clone(), e))?;
        let branch_children: Vec<String> = if branch {
            node.children.clone()
        } else {
            node.else_children.clone()
        };
        for child_id in &branch_children {
            let child = node_map.get(child_id).ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::RuntimeInfrastructureFailure {
                        message: format!("child node {child_id} not found"),
                    },
                )
            })?;
            Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map)).await?;
        }
        Ok(())
    }

    /// Executes a loop node per `control.loop_spec` semantics.
    ///
    /// While: evaluate condition first, then body. Until: run body first,
    /// then evaluate condition. ForEach: bind `item_var` per item.
    #[allow(clippy::too_many_arguments)]
    async fn run_loop(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        let spec = node
            .control
            .as_ref()
            .and_then(|c| c.loop_spec.as_ref())
            .ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::ValidationFailure {
                        message: format!("loop node {} has no control.loop_spec", node.node_id),
                    },
                )
            })?;
        match spec.kind {
            LoopKind::While | LoopKind::Until => {
                let condition = spec.condition.as_deref().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: "while/until loop missing condition".into(),
                        },
                    )
                })?;
                let expr = expr::parse(condition).map_err(|e| (node.node_id.clone(), e))?;
                let max = spec.max_iterations.ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: "while/until loop missing max_iterations".into(),
                        },
                    )
                })?;
                let mut iteration = 0u32;
                loop {
                    if iteration >= max {
                        return Err((
                            node.node_id.clone(),
                            AcosError::RuntimeInfrastructureFailure {
                                message: format!(
                                    "loop '{}' exceeded max_iterations ({max})",
                                    node.node_id
                                ),
                            },
                        ));
                    }
                    if matches!(spec.kind, LoopKind::While) {
                        let done = {
                            let guard = env.lock().await;
                            !expr::evaluate(&expr, &guard).map_err(|e| (node.node_id.clone(), e))?
                        };
                        if done {
                            break;
                        }
                    }
                    iteration += 1;
                    self.event_store
                        .append(
                            run_id,
                            "iteration.started".into(),
                            serde_json::json!({ "node_id": &node.node_id, "index": iteration }),
                        )
                        .await
                        .ok();
                    for child_id in &node.children {
                        let child = node_map.get(child_id).ok_or_else(|| {
                            (
                                node.node_id.clone(),
                                AcosError::RuntimeInfrastructureFailure {
                                    message: format!("child node {child_id} not found"),
                                },
                            )
                        })?;
                        Box::pin(
                            self.run_node(child, run_id, env, artifacts, evidence, node_map),
                        )
                        .await?;
                    }
                    self.event_store
                        .append(
                            run_id,
                            "iteration.completed".into(),
                            serde_json::json!({ "node_id": &node.node_id, "index": iteration }),
                        )
                        .await
                        .ok();
                    if matches!(spec.kind, LoopKind::Until) {
                        let done = {
                            let guard = env.lock().await;
                            expr::evaluate(&expr, &guard).map_err(|e| (node.node_id.clone(), e))?
                        };
                        if done {
                            break;
                        }
                    }
                }
                Ok(())
            }
            LoopKind::ForEach => {
                let input_ref = spec.input.as_deref().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: "for_each loop missing input".into(),
                        },
                    )
                })?;
                let item_var = spec.item_var.as_deref().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: "for_each loop missing item_var".into(),
                        },
                    )
                })?;
                let input_name = strip_ref(input_ref);
                let items = {
                    let guard = env.lock().await;
                    guard.get(input_name).map(|tv| tv.payload.clone())
                }
                .ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: format!(
                                "loop {} input '{input_name}' not found in env",
                                node.node_id
                            ),
                        },
                    )
                })?;
                let items = match items {
                    Value::Array(arr) => arr,
                    other => {
                        return Err((
                            node.node_id.clone(),
                            AcosError::ValidationFailure {
                                message: format!(
                                    "loop {} input '{input_name}' is not a list: {other}",
                                    node.node_id
                                ),
                            },
                        ));
                    }
                };
                let limit = spec
                    .max_iterations
                    .map(|n| (n as usize).min(items.len()))
                    .unwrap_or(items.len());
                for (i, item) in items.iter().take(limit).enumerate() {
                    env.lock().await.insert(
                        item_var.to_string(),
                        TypedValue {
                            value_type: ValueType::Scalar,
                            payload: item.clone(),
                        },
                    );
                    self.event_store
                        .append(
                            run_id,
                            "iteration.started".into(),
                            serde_json::json!({ "node_id": &node.node_id, "index": i }),
                        )
                        .await
                        .ok();
                    for child_id in &node.children {
                        let child = node_map.get(child_id).ok_or_else(|| {
                            (
                                node.node_id.clone(),
                                AcosError::RuntimeInfrastructureFailure {
                                    message: format!("child node {child_id} not found"),
                                },
                            )
                        })?;
                        Box::pin(
                            self.run_node(child, run_id, env, artifacts, evidence, node_map),
                        )
                        .await?;
                    }
                    self.event_store
                        .append(
                            run_id,
                            "iteration.completed".into(),
                            serde_json::json!({ "node_id": &node.node_id, "index": i }),
                        )
                        .await
                        .ok();
                }
                Ok(())
            }
        }
    }
```

新增模块级辅助函数：

```rust
/// Strips `$name` / `${name}` reference syntax, returning the bare name.
fn strip_ref(s: &str) -> &str {
    if let Some(inner) = s.strip_prefix("${").and_then(|x| x.strip_suffix('}')) {
        inner
    } else if let Some(inner) = s.strip_prefix('$') {
        inner
    } else {
        s
    }
}
```

- [ ] **Step 4: 修复既有测试**

`runtime_executes_pipeline_and_produces_report_artifact` 应不受影响（execute 签名不变）。若 `run_nodes` 私有签名变化引发编译错误，仅调整调用点。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-runtime`
Expected: 新测试 PASS + 既有测试 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/acos-runtime/src/lib.rs
git commit -m "feat(runtime): conditional/loop/retry 控制语义 + 重试安全门 + 错误对传播"
```

---

### Task 7: acos-runtime `execute_with_recovery` + 事务式提交门

**Files:**
- Modify: `crates/acos-runtime/src/lib.rs`
- Test: `crates/acos-runtime/src/lib.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 4（`RecoveryContext/FailureContext/RecoveryProposal/Replanner`）、Task 6
- Produces: `RuntimeImpl::execute_with_recovery(&self, CirProgram, Option<&RecoveryContext<'_>>) -> Result<RunReport, AcosError>`；`execute()` 委托 `execute_with_recovery(program, None)`；事件 `replan.started/completed/rejected`；`MAX_RECOVERY_ATTEMPTS = 3`；`pub async fn validate_proposal(&self, &CirProgram, &RecoveryProposal) -> Result<(), AcosError>`

- [ ] **Step 1: 写失败测试**

tests 追加（`FlakyRegistry` 来自 Task 6）：

```rust
    #[derive(Debug)]
    struct FixedPathRule(String);

    impl Replanner for FixedPathRule {
        fn propose(
            &self,
            failure: &FailureContext,
            program: &CirProgram,
        ) -> Option<RecoveryProposal> {
            let failing = program.nodes.iter().find(|n| n.node_id == failure.node_id)?;
            let mut root = failing.clone();
            root.kind = CirNodeKind::PrimitiveInvocation;
            root.capability = Some("read_file".into());
            root.children = vec![];
            root.control = None;
            root.inputs = vec![(
                "path".into(),
                serde_json::Value::String(self.0.clone()),
            )]
            .into_iter()
            .collect();
            Some(RecoveryProposal {
                replace_node: failure.node_id.clone(),
                subgraph: vec![root],
                reason: "fallback to local read".into(),
            })
        }
    }

    #[tokio::test]
    async fn recovery_replaces_failing_node_and_completes() {
        let dir = std::env::temp_dir().join(format!("acos-recover-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fallback.txt"), "cached").unwrap();
        let fallback_path = dir.join("fallback.txt").to_string_lossy().to_string();

        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                children: vec!["search".into(), "write".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "search".into(),
                capability: Some("flaky_search".into()),
                output: Some("results".into()),
                children: vec![],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "write".into(),
                capability: Some("write_file".into()),
                output: Some("ref".into()),
                children: vec![],
                else_children: vec![],
                inputs: vec![
                    ("path".into(), serde_json::Value::String(dir.join("out.txt").to_string_lossy().to_string())),
                    ("content".into(), serde_json::Value::String("${results}".into())),
                ]
                .into_iter()
                .collect(),
                control: None,
            },
        ];
        let mut program = program_from(nodes);
        program.effects = vec![
            EffectDecl { kind: EffectKind::NetworkRead, description: "search".into(), reversible: true },
            EffectDecl { kind: EffectKind::FsRead, description: "read".into(), reversible: true },
            EffectDecl { kind: EffectKind::FsWrite, description: "write".into(), reversible: true },
        ];
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let rule = FixedPathRule(fallback_path);
        let ctx = RecoveryContext { rule: Some(&rule), model: None };
        let report = runtime.execute_with_recovery(program, Some(&ctx)).await.unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        let events = store.query(report.run_id).await.unwrap();
        assert!(events.iter().any(|e| e.event_type == "replan.started"));
        assert!(events.iter().any(|e| e.event_type == "replan.completed"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn proposal_must_reuse_replace_node_id() {
        let program = program_from(vec![primitive_node("a", "search", None)]);
        let bad = RecoveryProposal {
            replace_node: "a".into(),
            subgraph: vec![primitive_node("b", "search", None)],
            reason: "bad root id".into(),
        };
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let err = runtime.validate_proposal(&program, &bad).await.unwrap_err();
        assert!(err.to_string().contains("reuse replace_node"));
    }
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-runtime recovery_`
Expected: FAIL（`execute_with_recovery`/`validate_proposal` 不存在）

- [ ] **Step 3: 实现**

import 补充：

```rust
use std::collections::HashSet;

use acos_core::traits::{
    ArtifactStore, Event, EventStore, PluginRegistry, Primitive, RecoveryContext, Replanner,
    RunHandle, RunStatus,
};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, ConditionSpec, ControlSpec, EffectDecl, EffectKind,
    FailureClass, FailureContext, LoopKind, LoopSpec, RecoveryProposal, RetryPolicy,
    RetryStrategy, TypedValue, ValueType,
};
```

`execute` 委托 + 新方法（`execute` 之后插入）：

```rust
    /// Maximum recovery (replan) attempts per run before failing.
    pub const MAX_RECOVERY_ATTEMPTS: u32 = 3;

    /// Executes a program and returns a report.
    pub async fn execute(&self, program: CirProgram) -> Result<RunReport, AcosError> {
        self.execute_with_recovery(program, None).await
    }

    /// Executes a program with optional failure recovery.
    ///
    /// On failure: rule replanner first, then model replanner. A proposal is
    /// only committed after [`Self::validate_proposal`] passes; the failing
    /// node is replaced with the subgraph root (which keeps the failing
    /// node's id) and the whole program is re-run from `entry`. The env
    /// carries over across attempts so produced bindings survive.
    pub async fn execute_with_recovery(
        &self,
        program: CirProgram,
        recovery: Option<&RecoveryContext<'_>>,
    ) -> Result<RunReport, AcosError> {
        let run_id = RunId::new();
        self.event_store
            .append(
                run_id,
                "run.started".into(),
                serde_json::json!({ "program_id": program.id.0 }),
            )
            .await?;

        let env = Arc::new(Mutex::new(HashMap::<String, TypedValue>::new()));
        let mut program = program;
        let mut attempts = 0u32;

        loop {
            let node_map: HashMap<String, CirNode> = program
                .nodes
                .iter()
                .map(|n| (n.node_id.clone(), n.clone()))
                .collect();

            match self
                .run_nodes(&node_map, &program.entry, run_id, env.clone())
                .await
            {
                Ok((produced, evidence)) => {
                    self.event_store
                        .append(
                            run_id,
                            "run.finished".into(),
                            serde_json::json!({ "status": "Completed" }),
                        )
                        .await
                        .ok();
                    return Ok(RunReport {
                        run_id,
                        status: RunStatus::Completed,
                        artifacts: produced,
                        evidence,
                    });
                }
                Err((node_id, e)) => {
                    let mut recovered = false;
                    if attempts < Self::MAX_RECOVERY_ATTEMPTS {
                        if let Some(ctx) = recovery {
                            let class = e.classify();
                            let recent_events: Vec<Event> = self
                                .event_store
                                .query(run_id)
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .rev()
                                .take(5)
                                .collect();
                            let failure = FailureContext {
                                run_id,
                                node_id: node_id.clone(),
                                error_class: class,
                                error_message: e.to_string(),
                                attempts,
                                recent_events,
                            };
                            if let Some(rule) = ctx.rule {
                                if let Some(proposal) = rule.propose(&failure, &program) {
                                    recovered = self
                                        .try_commit(&run_id, &mut program, &proposal, "rule")
                                        .await;
                                }
                            }
                            if !recovered {
                                if let Some(model) = ctx.model {
                                    if let Ok(Some(proposal)) =
                                        model.propose(&failure, &program).await
                                    {
                                        recovered = self
                                            .try_commit(&run_id, &mut program, &proposal, "model")
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    if !recovered {
                        self.event_store
                            .append(
                                run_id,
                                "run.finished".into(),
                                serde_json::json!({ "status": "Failed" }),
                            )
                            .await
                            .ok();
                        return Err(e);
                    }
                    attempts += 1;
                }
            }
        }
    }

    /// Validates a recovery proposal and, if valid, commits it into
    /// `program` (transactional patch). Emits `replan.started` /
    /// `replan.completed` / `replan.rejected`.
    async fn try_commit(
        &self,
        run_id: &RunId,
        program: &mut CirProgram,
        proposal: &RecoveryProposal,
        planner: &str,
    ) -> bool {
        self.event_store
            .append(
                *run_id,
                "replan.started".into(),
                serde_json::json!({
                    "planner": planner,
                    "node_id": proposal.replace_node,
                    "reason": proposal.reason,
                }),
            )
            .await
            .ok();
        match self.validate_proposal(program, proposal).await {
            Ok(()) => {
                program.nodes.retain(|n| n.node_id != proposal.replace_node);
                program.nodes.extend(proposal.subgraph.clone());
                self.event_store
                    .append(
                        *run_id,
                        "replan.completed".into(),
                        serde_json::json!({
                            "planner": planner,
                            "node_id": proposal.replace_node,
                            "subgraph_nodes": proposal
                                .subgraph
                                .iter()
                                .map(|n| &n.node_id)
                                .collect::<Vec<_>>(),
                        }),
                    )
                    .await
                    .ok();
                true
            }
            Err(ve) => {
                self.event_store
                    .append(
                        *run_id,
                        "replan.rejected".into(),
                        serde_json::json!({
                            "planner": planner,
                            "node_id": proposal.replace_node,
                            "error": ve.to_string(),
                        }),
                    )
                    .await
                    .ok();
                false
            }
        }
    }

    /// Transactional commit gate for a recovery proposal.
    ///
    /// 1. Subgraph root must reuse `replace_node`'s id.
    /// 2. Node ids unique within program ∪ subgraph.
    /// 3. Every child reference resolves within subgraph ∪ program.
    /// 4. Every capability resolves via the registry.
    /// 5. Every effect kind declared by the subgraph primitives is already
    ///    declared in `program.effects`.
    pub async fn validate_proposal(
        &self,
        program: &CirProgram,
        proposal: &RecoveryProposal,
    ) -> Result<(), AcosError> {
        let root = proposal.subgraph.first().ok_or_else(|| {
            AcosError::ValidationFailure {
                message: "recovery proposal has empty subgraph".into(),
            }
        })?;
        if root.node_id != proposal.replace_node {
            return Err(AcosError::ValidationFailure {
                message: format!(
                    "recovery subgraph root '{}' must reuse replace_node id '{}'",
                    root.node_id, proposal.replace_node
                ),
            });
        }
        let mut known: HashSet<&str> =
            program.nodes.iter().map(|n| n.node_id.as_str()).collect();
        for node in &proposal.subgraph {
            if node.node_id != proposal.replace_node && !known.insert(node.node_id.as_str()) {
                return Err(AcosError::ValidationFailure {
                    message: format!(
                        "recovery subgraph introduces duplicate node id '{}'",
                        node.node_id
                    ),
                });
            }
        }
        for node in &proposal.subgraph {
            for child in &node.children {
                let in_subgraph = proposal.subgraph.iter().any(|s| &s.node_id == child);
                if !in_subgraph && !known.contains(child.as_str()) {
                    return Err(AcosError::ValidationFailure {
                        message: format!("recovery subgraph references unknown child '{child}'"),
                    });
                }
            }
        }
        for node in &proposal.subgraph {
            if let Some(capability) = &node.capability {
                let primitive = self
                    .registry
                    .resolve(capability)
                    .await
                    .map_err(|_| AcosError::ValidationFailure {
                        message: format!(
                            "recovery subgraph capability '{capability}' is unavailable"
                        ),
                    })?;
                for effect in primitive.effects() {
                    if !program.effects.iter().any(|d| d.kind == effect.kind) {
                        return Err(AcosError::ValidationFailure {
                            message: format!(
                                "recovery subgraph effect {:?} not declared in program.effects",
                                effect.kind
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }
```

- [ ] **Step 4: 运行测试**

Run: `cargo test -p acos-runtime`
Expected: 全部 PASS（含既有 pipeline 测试）

- [ ] **Step 5: 提交**

```bash
git add crates/acos-runtime/src/lib.rs
git commit -m "feat(runtime): execute_with_recovery 恢复管道 + 事务式 proposal 提交门"
```

---

### Task 8: acos-runtime RuleReplanner

**Files:**
- Create: `crates/acos-runtime/src/replan.rs`
- Modify: `crates/acos-runtime/src/lib.rs`（`pub mod replan;` + re-export）
- Test: `crates/acos-runtime/src/replan.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 4 traits、Task 1/3 类型
- Produces: `pub trait RecoveryRule { fn matches(&FailureContext)->bool; fn propose(&FailureContext,&CirProgram)->Option<RecoveryProposal> }`、`pub struct RuleReplanner { rules: Vec<Box<dyn RecoveryRule>> }`（`new()` + `with_rule()` + `impl Replanner`）、`pub struct OfflineFallbackRule { fallback_path: String }`

- [ ] **Step 1: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_core::types::{CirNode, CirNodeKind, CirProgram, FailureClass};

    fn program_with_search() -> CirProgram {
        CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["root".into()],
            nodes: vec![CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "root".into(),
                capability: Some("search".into()),
                output: Some("results".into()),
                children: vec![],
                else_children: vec![],
                inputs: Default::default(),
                control: None,
            }],
            effects: vec![],
        }
    }

    fn failure(node_id: &str, class: FailureClass) -> FailureContext {
        FailureContext {
            run_id: RunId::new(),
            node_id: node_id.into(),
            error_class: class,
            error_message: "boom".into(),
            attempts: 0,
            recent_events: vec![],
        }
    }

    #[test]
    fn offline_fallback_rule_matches_transient_classes() {
        let rule = OfflineFallbackRule { fallback_path: "fallback.txt".into() };
        assert!(rule.matches(&failure("root", FailureClass::Timeout)));
        assert!(rule.matches(&failure("root", FailureClass::TransientNetworkError)));
        assert!(!rule.matches(&failure("root", FailureClass::Unknown)));
        assert!(!rule.matches(&failure("root", FailureClass::InvalidInput)));
    }

    #[test]
    fn offline_fallback_rule_proposes_read_file_replacement() {
        let rule = OfflineFallbackRule { fallback_path: "fallback.txt".into() };
        let program = program_with_search();
        let proposal = rule.propose(&failure("root", FailureClass::Timeout), &program).unwrap();
        assert_eq!(proposal.replace_node, "root");
        let root = proposal.subgraph.first().unwrap();
        assert_eq!(root.node_id, "root");
        assert_eq!(root.capability.as_deref(), Some("read_file"));
        assert_eq!(root.inputs["path"], serde_json::Value::String("fallback.txt".into()));
        assert_eq!(root.output.as_deref(), Some("results"));
    }

    #[test]
    fn rule_replanner_returns_none_when_no_rule_matches() {
        let replanner = RuleReplanner::new()
            .with_rule(Box::new(OfflineFallbackRule { fallback_path: "x".into() }));
        let program = program_with_search();
        let proposal = replanner.propose(&failure("root", FailureClass::Unknown), &program);
        assert!(proposal.is_none());
    }

    #[test]
    fn rule_replanner_returns_none_when_node_missing() {
        let replanner = RuleReplanner::new()
            .with_rule(Box::new(OfflineFallbackRule { fallback_path: "x".into() }));
        let program = program_with_search();
        let proposal = replanner.propose(&failure("missing", FailureClass::Timeout), &program);
        assert!(proposal.is_none());
    }
}
```

- [ ] **Step 2: 运行测试确认失败**

Run: `cargo test -p acos-runtime replan::tests`
Expected: FAIL（模块不存在）

- [ ] **Step 3: 实现 replan.rs**

```rust
//! Deterministic rule-based failure recovery.

use acos_core::traits::Replanner;
use acos_core::types::{
    CirNodeKind, CirProgram, FailureClass, FailureContext, RecoveryProposal,
};

/// A recovery rule (capability-specific or capability-agnostic).
///
/// Rules are tried in registration order; the first rule whose
/// [`RecoveryRule::matches`] returns `true` and whose `propose` returns
/// `Some` wins.
pub trait RecoveryRule: Send + Sync + std::fmt::Debug {
    /// Whether this rule applies to the failure.
    fn matches(&self, failure: &FailureContext) -> bool;
    /// Proposes a recovery patch, or `None` if it cannot produce one.
    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal>;
}

/// Deterministic replanner: tries registered rules in order.
#[derive(Debug, Default)]
pub struct RuleReplanner {
    rules: Vec<Box<dyn RecoveryRule>>,
}

impl RuleReplanner {
    /// Creates an empty replanner (no rules).
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a recovery rule.
    pub fn with_rule(mut self, rule: Box<dyn RecoveryRule>) -> Self {
        self.rules.push(rule);
        self
    }
}

impl Replanner for RuleReplanner {
    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal> {
        for rule in &self.rules {
            if rule.matches(failure) {
                if let Some(proposal) = rule.propose(failure, program) {
                    return Some(proposal);
                }
            }
        }
        None
    }
}

/// Falls back to reading a local file when a node fails with a transient
/// class (timeout / rate limit / transient network error).
///
/// The failing node is replaced in place: the subgraph root keeps the failing
/// node's id and becomes a `read_file` invocation.
#[derive(Debug)]
pub struct OfflineFallbackRule {
    /// Path of the local fallback file.
    pub fallback_path: String,
}

impl RecoveryRule for OfflineFallbackRule {
    fn matches(&self, failure: &FailureContext) -> bool {
        matches!(
            failure.error_class,
            FailureClass::Timeout
                | FailureClass::RateLimit
                | FailureClass::TransientNetworkError
        )
    }

    fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Option<RecoveryProposal> {
        let failing = program.nodes.iter().find(|n| n.node_id == failure.node_id)?;
        let mut root = failing.clone();
        root.kind = CirNodeKind::PrimitiveInvocation;
        root.capability = Some("read_file".into());
        root.children = vec![];
        root.control = None;
        root.inputs = vec![(
            "path".to_string(),
            serde_json::Value::String(self.fallback_path.clone()),
        )]
        .into_iter()
        .collect();
        Some(RecoveryProposal {
            replace_node: failure.node_id.clone(),
            subgraph: vec![root],
            reason: format!(
                "{}: falling back to local file '{}'",
                failure.error_message, self.fallback_path
            ),
        })
    }
}
```

- [ ] **Step 4: lib.rs 挂载**

`crates/acos-runtime/src/lib.rs` 顶部追加：

```rust
pub mod replan;

pub use replan::{OfflineFallbackRule, RecoveryRule, RuleReplanner};
```

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-runtime`
Expected: 全部 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/acos-runtime/src/replan.rs crates/acos-runtime/src/lib.rs
git commit -m "feat(runtime): RuleReplanner + OfflineFallbackRule"
```

---

### Task 9: acos-compiler ModelRecoveryPlanner

**Files:**
- Create: `crates/acos-compiler/src/replan.rs`
- Modify: `crates/acos-compiler/src/lib.rs`（`pub mod replan;` + re-export）
- Modify: `crates/acos-llm/src/lib.rs`（`LongCatClient::dummy()` 测试辅助）
- Test: `crates/acos-compiler/src/replan.rs`（内嵌 tests）

**Interfaces:**
- Consumes: Task 4 traits、`extract_json_object`（lib.rs 改 `pub(crate)`）
- Produces: `pub struct ModelRecoveryPlanner { llm: LongCatClient }`（`from_env() -> Result<Self, AcosError>`、`parse_proposal(&str) -> Result<RecoveryProposal, AcosError>`、`impl acos_core::traits::ModelReplanner`）

- [ ] **Step 1: 准备 `LongCatClient::dummy()`**

先确认字段：Run: `rg -n "struct LongCatClient" crates/acos-llm/src/lib.rs` 并读取 struct 定义。在 `impl LongCatClient` 内追加：

```rust
    /// Creates a client that always fails on `complete` (for tests that only
    /// exercise parsing).
    pub fn dummy() -> Self {
        Self {
            api_key: String::new(),
            base_url: "http://127.0.0.1:1".into(),
            model: "dummy".into(),
        }
    }
```

（字段名以实际 struct 为准。）

- [ ] **Step 2: 写失败测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_core::types::FailureClass;

    fn failure() -> FailureContext {
        FailureContext {
            run_id: RunId::new(),
            node_id: "B".into(),
            error_class: FailureClass::Unknown,
            error_message: "boom".into(),
            attempts: 0,
            recent_events: vec![],
        }
    }

    #[test]
    fn parses_recovery_subgraph_json() {
        let raw = r#"{
            "replaceNode": "B",
            "reason": "use a local read instead",
            "subgraph": [
                { "kind": "primitive_invocation", "nodeId": "B", "capability": "read_file", "output": "results", "children": [], "inputs": { "path": "fallback.txt" } }
            ]
        }"#;
        let planner = ModelRecoveryPlanner { llm: acos_llm::LongCatClient::dummy() };
        let proposal = planner.parse_proposal(raw).unwrap();
        assert_eq!(proposal.replace_node, "B");
        assert_eq!(proposal.subgraph.len(), 1);
        assert_eq!(proposal.subgraph[0].node_id, "B");
    }

    #[test]
    fn parses_proposal_wrapped_in_markdown() {
        let raw = "```json\n{\"replaceNode\":\"B\",\"reason\":\"r\",\"subgraph\":[]}\n```";
        let planner = ModelRecoveryPlanner { llm: acos_llm::LongCatClient::dummy() };
        let proposal = planner.parse_proposal(raw).unwrap();
        assert_eq!(proposal.replace_node, "B");
    }

    #[test]
    fn rejects_invalid_proposal_json() {
        let planner = ModelRecoveryPlanner { llm: acos_llm::LongCatClient::dummy() };
        assert!(planner.parse_proposal("not json").is_err());
    }

    #[test]
    fn builds_prompt_containing_failure_and_program() {
        let planner = ModelRecoveryPlanner { llm: acos_llm::LongCatClient::dummy() };
        let program = CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["B".into()],
            nodes: vec![CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "B".into(),
                capability: Some("search".into()),
                output: Some("results".into()),
                children: vec![],
                else_children: vec![],
                inputs: Default::default(),
                control: None,
            }],
            effects: vec![],
        };
        let prompt = planner.build_user_prompt(&failure(), &program);
        assert!(prompt.contains("RecoverySubgraph"));
        assert!(prompt.contains("B"));
    }
}
```

（`build_user_prompt` 需为 `pub(crate)` 或 `pub` 供测试使用——按仓库惯例 tests 在同模块，`fn build_user_prompt` 即可。）

- [ ] **Step 3: 运行测试确认失败**

Run: `cargo test -p acos-compiler replan::tests`
Expected: FAIL

- [ ] **Step 4: 实现 replan.rs**

```rust
//! Model-driven recovery planning: asks Claude (via LongCat) to produce a
//! `RecoverySubgraph` patch for a runtime failure.

use async_trait::async_trait;

use acos_core::error::AcosError;
use acos_core::traits::ModelReplanner;
use acos_core::types::{CirProgram, FailureContext, RecoveryProposal};

use crate::extract_json_object;

/// System prompt teaching the model the RecoverySubgraph JSON format.
const RECOVERY_SYSTEM_PROMPT: &str = r#"You are the ACOS Recovery Planner. A cognitive program failed at runtime and you must produce a **RecoverySubgraph**: a minimal patch that replaces ONE failing node.

# Available primitives (capabilities)
- `search` ({ "query": "..." } -> DocumentList, network read)
- `read_file` ({ "path": "..." } -> Document, fs read)
- `write_file` ({ "path": "...", "content": "..." } -> ArtifactRef, fs write)
- `execute_python` ({ "code": "..." } -> ExecutionResult, process spawn)
- `summarize` ({ "document": "..." } or { "documents": [...] } -> Summary)

# Rules
1. Respond with ONLY valid JSON (no markdown, no commentary):
   { "replaceNode": "<failing node id>", "reason": "<why>", "subgraph": [ <CIR nodes> ] }
2. The subgraph ROOT node MUST reuse replaceNode as its nodeId — do not introduce a new id for the root.
3. Only reference node outputs that already exist in the provided program.
4. Only use the capabilities listed above.
5. Do not declare new top-level effects; reuse the program's existing effect kinds.
6. Keep the patch minimal: prefer replacing a primitive with another primitive over adding containers.
7. If the failing node is `execute_python` with a missing-module error, you may emit a sequence root whose children are: an install step (execute_python with pip-install code, new nodeId) then the retry of the original node (new nodeId).
"#;

/// Model-assisted recovery planner backed by LongCat.
#[derive(Debug, Clone)]
pub struct ModelRecoveryPlanner {
    llm: acos_llm::LongCatClient,
}

impl ModelRecoveryPlanner {
    /// Creates a planner from environment configuration.
    pub fn from_env() -> Result<Self, AcosError> {
        Ok(Self {
            llm: acos_llm::LongCatClient::from_env()?,
        })
    }

    /// Parses a RecoverySubgraph JSON response into a proposal.
    ///
    /// Tolerates markdown fences and surrounding commentary.
    pub fn parse_proposal(&self, raw: &str) -> Result<RecoveryProposal, AcosError> {
        let json_str = extract_json_object(raw);
        let value: serde_json::Value =
            serde_json::from_str(&json_str).map_err(|e| AcosError::CompilerFailure {
                message: format!(
                    "model returned invalid RecoverySubgraph JSON: {e}\n--- raw ---\n{raw}"
                ),
            })?;
        serde_json::from_value(value).map_err(|e| AcosError::CompilerFailure {
            message: format!("RecoverySubgraph does not match schema: {e}\n--- raw ---\n{raw}"),
        })
    }

    /// Builds the user prompt with failure context and the current program.
    fn build_user_prompt(&self, failure: &FailureContext, program: &CirProgram) -> String {
        let program_json = serde_json::to_string_pretty(program)
            .unwrap_or_else(|_| format!("{program:?}"));
        let failure_json = serde_json::to_string_pretty(failure)
            .unwrap_or_else(|_| format!("{failure:?}"));
        format!(
            "The runtime failed. Produce a RecoverySubgraph.\n\n## Failure\n```json\n{failure_json}\n```\n\n## Current program\n```json\n{program_json}\n```"
        )
    }
}

#[async_trait]
impl ModelReplanner for ModelRecoveryPlanner {
    async fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Result<Option<RecoveryProposal>, AcosError> {
        let prompt = self.build_user_prompt(failure, program);
        let raw = self.llm.complete(RECOVERY_SYSTEM_PROMPT, &prompt).await?;
        Ok(Some(self.parse_proposal(&raw)?))
    }
}
```

`lib.rs` 修改：
- `fn extract_json_object` → `pub(crate) fn extract_json_object`
- 顶部追加 `pub mod replan;` 与 `pub use replan::ModelRecoveryPlanner;`
- tests 中 `build_user_prompt` 调用（同 crate 模块可直接访问私有 fn，无需 pub）

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-compiler && cargo test -p acos-llm`
Expected: 全部 PASS

- [ ] **Step 6: 提交**

```bash
git add crates/acos-compiler/src/replan.rs crates/acos-compiler/src/lib.rs crates/acos-llm/src/lib.rs
git commit -m "feat(compiler): ModelRecoveryPlanner 生成 RecoverySubgraph"
```

### Task 10: acos-bench crate 骨架 + BenchRegistry + condition/loop/retry 套件

**Files:**
- Create: `crates/acos-bench/Cargo.toml`
- Create: `crates/acos-bench/src/lib.rs`（`pub mod fixtures; pub mod registry; pub mod report; pub mod runner;` + `pub async fn run(args: BenchArgs) -> BenchReport`）
- Create: `crates/acos-bench/src/fixtures.rs`
- Create: `crates/acos-bench/src/registry.rs`
- Create: `crates/acos-bench/src/runner.rs`
- Create: `crates/acos-bench/src/report.rs`
- Create: `crates/acos-bench/tests/condition.rs`、`crates/acos-bench/tests/loop.rs`、`crates/acos-bench/tests/retry.rs`
- Create: `crates/acos-bench/fixtures/condition/basic.yaml`、`crates/acos-bench/fixtures/condition/repair_branch.yaml`、`crates/acos-bench/fixtures/loop/foreach.yaml`、`crates/acos-bench/fixtures/loop/while_limit.yaml`、`crates/acos-bench/fixtures/retry/timeout.yaml`
- Test: 上述三个集成测试文件

**Interfaces:**
- Consumes: `acos-core`（types/expr/events）、`acos-compiler::validate_cir`（Task 5 起 pub）、`acos-runtime::RuntimeImpl`、`acos-plugin::BuiltinRegistry`、`acos-verify::verify_run`
- Produces: `BenchArgs { fixtures_dir, suite: Option<String>, case: Option<String>, require_model: bool }`、`Fixture`/`FixtureMode`/`CompilerKind`、`BenchRegistry`、`BenchReport`

> **为什么提前到 Replanner 之前**（用户批准的顺序）：先立契约，后实现恢复；Rule/Model Replanner 完成时必须通过本 Task 的 fixtures 才能算合标。

- [ ] **Step 1: 新建 crate**

```toml
[package]
name = "acos-bench"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
description = "ACOS benchmark harness: fixtures-as-contracts regression suite."

[dependencies]
acos-core = { path = "../acos-core" }
acos-compiler = { path = "../acos-compiler" }
acos-runtime = { path = "../acos-runtime" }
acos-state = { path = "../acos-state" }
acos-plugin = { path = "../acos-plugin" }
acos-verify = { path = "../acos-verify" }
async-trait = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
tokio = { version = "1", features = ["rt-multi-thread", "macros", "fs"] }
uuid = { version = "1", features = ["v4"] }

[features]
default = ["in-memory"]
in-memory = ["acos-runtime/in-memory", "acos-state/in-memory", "acos-plugin/in-memory", "acos-verify/in-memory"]

[[bin]]
name = "acos-bench"
path = "src/main.rs"
```

Run: `cargo check -p acos-bench` → 报 "no targets specified"，正常（下一步补 lib 后消失）。workspace members 为 `crates/*`，新目录自动纳入，无需改根 Cargo.toml。

- [ ] **Step 2: `fixtures.rs`（fixture 模型 + 加载）**

```rust
//! Fixture-as-contract loading. See docs/superpowers/specs/2026-08-17-...-design.md §5.

use acos_core::types::CirProgram;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// How a fixture's program is produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FixtureMode {
    /// Run the inline `cir` (validated, then executed). Compile column = PASS.
    Cir,
    /// Run the compiler pipeline (`compiler` decides rules vs model).
    Run,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompilerKind {
    Rules,
    Model,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Fixture {
    pub id: String,
    /// Compiler mode: inline cir (FixtureMode::Cir) vs full pipeline.
    #[serde(default)]
    pub mode: FixtureMode,
    /// Which compiler backend to use; only meaningful when mode = run.
    pub compiler: Option<CompilerKind>,
    pub goal: String,
    pub cir: Option<CirProgram>,
    /// Files written into the scratch workspace; `{workspace}` is substituted
    /// into `inputs` before execution.
    #[serde(default)]
    pub files: HashMap<String, String>,
    pub expected: Expected,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Expected {
    /// Pass when compile errors (validation) are expected.
    #[serde(default)]
    pub compile: Option<bool>,
    /// Pass when the run is expected to complete successfully.
    #[serde(default)]
    pub execution: Option<bool>,
    /// Pass when verification (acos-verify) is expected to pass.
    #[serde(default)]
    pub verification: Option<bool>,
    /// Expected recovery label, e.g. `retry`, `rule`, `model`.
    pub recovery: Option<String>,
    /// Expected final status string of the run, e.g. `success`, `failed`.
    pub final_status: Option<String>,
    /// Expected validation rejection reason substring (negative fixtures).
    pub validation: Option<String>,
}

/// Loads `fixtures_dir/**/*.yaml`, grouped by top-level directory (suite).
pub fn load_fixtures(fixtures_dir: &Path) -> Vec<(String, Fixture)> {
    let mut out = Vec::new();
    let mut stack = vec![fixtures_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "yaml") {
                let suite = path
                    .parent()
                    .and_then(Path::file_name)
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| "misc".into());
                let fixture: Fixture = serde_yaml::from_str(
                    &std::fs::read_to_string(&path).expect("read fixture"),
                )
                .expect("parse fixture");
                out.push((suite, fixture));
            }
        }
    }
    out.sort_by(|a, b| a.1.id.cmp(&b.1.id));
    out
}

/// Creates a scratch workspace under a temp dir, writes `files`, returns the
/// workspace path. The runtime must be given a `FileStore` rooted there.
pub async fn prepare_workspace(fixture: &Fixture) -> std::io::Result<(tempdir::TempDir, PathBuf)>
// NOTE: no extra dep needed — reuse `std::env::temp_dir()` + unique suffix:
```

（避免引入 tempfile 依赖：用 `std::env::temp_dir()` 拼接 `acos-bench-<uuid>`，结束后 `remove_dir_all` 清理。）

```rust
pub fn prepare_workspace(fixture: &Fixture) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("acos-bench-{}", uuid::Uuid::new_v4()));
    for (name, content) in &fixture.files {
        let path = dir.join(name);
        std::fs::create_dir_all(path.parent().expect("file has parent")).expect("mkdir");
        std::fs::write(path, content).expect("write fixture file");
    }
    dir
}

/// Substitutes `{workspace}` tokens in string inputs.
pub fn substitute_workspace(input: &str, workspace: &str) -> String {
    input.replace("{workspace}", workspace)
}

/// Convenience accessor for runner internals.
impl Fixture {
    pub fn files_in(&self, workspace: &str) -> HashMap<String, String> {
        self.files
            .iter()
            .map(|(k, v)| (k.clone(), substitute_workspace(v, workspace)))
            .collect()
    }
}
```

（`mode`/`compiler` 的 serde 行为：`mode` 缺省 `Cir`（内嵌 CIR 为主）；`compiler: None` + `mode: Run` 视为编译错误。Runner 内补该校验。）

- [ ] **Step 3: `registry.rs`（BenchRegistry + 测试原语）**

```rust
//! Bench-only primitives and the composite registry.

use acos_core::error::AcosError;
use acos_core::traits::{Capability, Primitive, PrimitiveContext, PrimitiveManifest, EffectDecl};
use acos_core::types::{CapabilityDesc, EffectKind, FailureClass};
use acos_plugin::BuiltinRegistry;
use async_trait::async_trait;
use std::collections::HashMap;

/// Fails the first `failures` invocations with `class`, then returns `[]`.
pub struct FlakySearchPrimitive {
    failures: usize,
    class: FailureClass,
    count: std::sync::atomic::AtomicUsize,
}

impl FlakySearchPrimitive {
    pub fn new(failures: usize, class: FailureClass) -> Self {
        Self { failures, class, count: std::sync::atomic::AtomicUsize::new(0) }
    }
}

#[async_trait]
impl Primitive for FlakySearchPrimitive {
    fn manifest(&self) -> PrimitiveManifest {
        PrimitiveManifest {
            name: "search".into(),
            description: "benchmark stub: fails N times then returns empty results".into(),
            capability: Capability::Search,
            idempotent: false,
            effects: vec![EffectDecl {
                kind: EffectKind::NetworkRead,
                description: "network search read".into(),
            }],
        }
    }

    async fn execute(&self, ctx: PrimitiveContext) -> Result<String, AcosError> {
        if self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst) < self.failures {
            Err(AcosError::PrimitiveFailure {
                message: format!("stub failure ({:?})", self.class),
                primitive_id: "search".into(),
                class: self.class,
            })
        } else {
            Ok("[]".into())
        }
    }
}

/// Returns a fixed list of items for foreach sources.
pub struct ListSourcePrimitive {
    items: Vec<String>,
}

impl ListSourcePrimitive {
    pub fn new(items: Vec<String>) -> Self {
        Self { items }
    }
}

#[async_trait]
impl Primitive for ListSourcePrimitive {
    fn manifest(&self) -> PrimitiveManifest {
        PrimitiveManifest {
            name: "list_source".into(),
            description: "benchmark stub: fixed item list".into(),
            capability: Capability::Search,
            idempotent: true,
            effects: vec![EffectDecl {
                kind: EffectKind::NetworkRead,
                description: "stub read".into(),
            }],
        }
    }

    async fn execute(&self, _ctx: PrimitiveContext) -> Result<String, AcosError> {
        Ok(serde_json::to_string(&self.items).expect("serialize items"))
    }
}

/// Rejects retry-on-failure (ExternalIrreversible effect) — used by negative
/// retry fixtures.
pub struct IrreversiblePrimitive;

#[async_trait]
impl Primitive for IrreversiblePrimitive {
    fn manifest(&self) -> PrimitiveManifest {
        PrimitiveManifest {
            name: "irreversible".into(),
            description: "benchmark stub: non-retryable external effect".into(),
            capability: Capability::ExecutePython,
            idempotent: false,
            effects: vec![EffectDecl {
                kind: EffectKind::ExternalIrreversible,
                description: "irreversible external side effect".into(),
            }],
        }
    }

    async fn execute(&self, _ctx: PrimitiveContext) -> Result<String, AcosError> {
        Ok("done".into())
    }
}

/// BuiltinRegistry plus bench stubs. `search` fails exactly once (timeout)
/// so fixtures can rely on deterministic first-attempt failure.
pub struct BenchRegistry {
    inner: BuiltinRegistry,
    search: FlakySearchPrimitive,
    list_source: ListSourcePrimitive,
    irreversible: IrreversiblePrimitive,
}

impl BenchRegistry {
    pub fn new() -> Self {
        Self {
            inner: BuiltinRegistry::new(),
            search: FlakySearchPrimitive::new(1, FailureClass::Timeout),
            list_source: ListSourcePrimitive::new(
                vec!["alpha".into(), "beta".into(), "gamma".into()],
            ),
            irreversible: IrreversiblePrimitive,
        }
    }

    /// Overrides the stub failure class (used by recovery suites).
    pub fn with_search_failure_class(mut self, class: FailureClass) -> Self {
        self.search = FlakySearchPrimitive::new(1, class);
        self
    }
}

impl Default for BenchRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl acos_core::traits::Registry for BenchRegistry {
    fn resolve(&self, name: &str) -> Option<&dyn Primitive> {
        match name {
            "search" => Some(&self.search),
            "list_source" => Some(&self.list_source),
            "irreversible" => Some(&self.irreversible),
            _ => self.inner.resolve(name),
        }
    }

    fn all(&self) -> Vec<PrimitiveManifest> {
        let mut v = self.inner.all();
        v.push(self.search.manifest());
        v.push(self.list_source.manifest());
        v.push(self.irreversible.manifest());
        v
    }
}
```

（确认 `acos-core::traits` 中 Registry trait 的实际签名——以 `crates/acos-core/src/traits.rs` 为准，方法名若为 `get`/`manifests` 则同步改名。`Capability` enum 变体以实际为准，`Search`/`ExecutePython` 已存在；若无 `Search` 变体则用最近的等价变体并注明。）

- [ ] **Step 4: `report.rs`（结果模型 + 打印）**

```rust
//! Case outcomes and the human-readable report table.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStatus {
    Pass,
    Fail,
    Skip,
}

impl fmt::Display for CaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Skip => "SKIP",
        })
    }
}

#[derive(Debug, Clone)]
pub struct CaseResult {
    pub id: String,
    pub suite: String,
    pub status: CaseStatus,
    /// None when the compile step was skipped (mode = cir).
    pub compile: Option<bool>,
    pub execution: Option<bool>,
    pub recovery: Option<String>,
    pub verification: Option<bool>,
    pub note: String,
}

#[derive(Debug, Default)]
pub struct BenchReport {
    pub cases: Vec<CaseResult>,
}

impl BenchReport {
    pub fn total(&self) -> usize { self.cases.len() }
    pub fn passed(&self) -> usize { self.cases.iter().filter(|c| c.status == CaseStatus::Pass).count() }
    pub fn failed(&self) -> usize { self.cases.iter().filter(|c| c.status == CaseStatus::Fail).count() }
    pub fn skipped(&self) -> usize { self.cases.iter().filter(|c| c.status == CaseStatus::Skip).count() }

    pub fn print(&self) {
        println!("ACOS Benchmark v0.1");
        println!("{:<28} {:<6} {:<8} {:<8} {:<8}", "Case", "Result", "Compile", "Execute", "Recover");
        println!("{}", "-".repeat(58));
        for case in &self.cases {
            let compile = case.compile.map(|b| if b { "PASS" } else { "FAIL" }).unwrap_or("-");
            let execution = case.execution.map(|b| if b { "PASS" } else { "FAIL" }).unwrap_or("-");
            let recovery = case.recovery.as_deref().unwrap_or("-");
            println!("{:<28} {:<6} {:<8} {:<8} {:<8} {}", case.id, case.status, compile, execution, recovery, case.note);
        }
        println!("{}", "-".repeat(58));
        println!("{} cases / {} passed / {} failed / {} skipped",
            self.total(), self.passed(), self.failed(), self.skipped());
    }
}
```

- [ ] **Step 5: `runner.rs` + `lib.rs` + `main.rs`**

runner 核心流程（`pub async fn run_case(args: &BenchArgs, suite: &str, fixture: &Fixture) -> CaseResult`）：

1. **compile 阶段**：
   - `mode = Cir`：调用 `acos_compiler::validate_cir(&cir)`（Task 5 起 pub）。期望 `compile: Some(true)`（缺省）时失败 → `Fail`；若 `expected.compile == Some(false)` 但校验通过 → 也 `Fail`（契约反了）。
   - `mode = Run`：`compiler = Some(Rules)` 走 `RuleCompiler`（外部临时文件 + `acos_compiler::compile`），`Some(Model)` 走 ModelCompiler（无 key → `Skip` + note "model not configured"）。
   - 校验失败（validate_cir 返回 Err）：若 `expected.validation` 存在且包含子串 → `Pass`（负例）；否则 `Fail`，note = 错误消息。**validation 负例直接短路返回，不执行。**
2. **execution 阶段**（compile 通过后）：`prepare_workspace` → `RuntimeImpl::new(Arc<BenchRegistry>, EventStore::in_memory(), ArtifactStore::in_memory())` → 对 `inputs` 做 `{workspace}` 替换 → `execute_with_recovery(program, None)`。
   - 期望 `execution: true`（缺省）但执行失败 → `Fail`（除非 `expected.recovery` 存在且恢复成功——见下）。
   - 恢复观测：从 EventStore 拉事件，取最后 `recovery_kind`：`retry`（retry.started 出现）| `rule`（replan.started 且 payload.planner = "rule"）| `model`（payload.planner = "model"）。若 `expected.recovery` 存在但观测不到 → `Fail`；观测到但执行最终失败 → `Fail`；都满足 → `Pass`（recovery 成功不需要 verification）。
   - `expected.final_status` 存在时核对最终状态字符串。
3. **verification 阶段**（执行成功且未短路）：`verify_run(events)`（acos-verify）→ 与 `expected.verification`（缺省 true）比对。

`lib.rs`：

```rust
//! ACOS benchmark harness — fixtures-as-contracts.

pub mod fixtures;
pub mod registry;
pub mod report;
pub mod runner;

use fixtures::Fixture;
use report::{BenchReport, CaseResult};
use std::path::PathBuf;

/// Top-level CLI-facing arguments.
#[derive(Debug, Clone)]
pub struct BenchArgs {
    pub fixtures_dir: PathBuf,
    /// Restrict to one suite (top-level fixture dir name).
    pub suite: Option<String>,
    /// Restrict to one case by id.
    pub case: Option<String>,
    /// Turn SKIP into FAIL (used by CI to require model-backed recovery).
    pub require_model: bool,
}

/// Runs all selected fixtures and returns the aggregated report.
pub async fn run(args: BenchArgs) -> BenchReport {
    let fixtures = fixtures::load_fixtures(&args.fixtures_dir);
    let mut report = BenchReport::default();
    for (suite, fixture) in fixtures {
        if let Some(s) = &args.suite {
            if suite != *s { continue; }
        }
        if let Some(c) = &args.case {
            if fixture.id != *c { continue; }
        }
        report.cases.push(runner::run_case(&args, &suite, &fixture).await);
    }
    report
}
```

`main.rs`：

```rust
use acos_bench::{BenchArgs, run};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let mut fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures_dir.push("fixtures");
    let mut suite = None;
    let mut case = None;
    let mut require_model = false;
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--suite" => { suite = args.get(i + 1).cloned(); i += 1; }
            "--case" => { case = args.get(i + 1).cloned(); i += 1; }
            "--require-model" => require_model = true,
            "--fixtures" => { fixtures_dir = args.get(i + 1).map(PathBuf::from).unwrap_or(fixtures_dir); i += 1; }
            other => { eprintln!("unknown argument: {other}"); std::process::exit(2); }
        }
        i += 1;
    }
    let report = run(BenchArgs { fixtures_dir, suite, case, require_model }).await;
    report.print();
    std::process::exit(if report.failed() == 0 { 0 } else { 1 });
}
```

- [ ] **Step 6: fixtures + 集成测试**

`crates/acos-bench/fixtures/condition/basic.yaml`：

```yaml
id: condition_basic
mode: cir
goal: "search, then summarize when results exist"
cir:
  program_id: bench_condition_basic
  nodes:
    - node_id: "A"
      kind: primitive
      capability: search
      output: search_results
      inputs:
        query: "acos"
    - node_id: "B"
      kind: primitive
      capability: summarize
      output: summary
      inputs:
        text: "{$search_results}"
      control:
        condition:
          identifier: search_results
          operator: exists
          operand: null
expected:
  execution: true
  final_status: success
```

（YAML 中的 `{$search_results}` 引用串、Conditional 的 `control.condition` 结构、以及 CirProgram/CirNode 的 serde 字段名，均以 Task 1 中最终实现的 `types.rs` 为准——本计划 Task 1 已定：`condition: { identifier, operator, operand }`、`loop_spec: { kind: while|until|foreach, condition?, list_source?, item_var?, max_iterations? }`、`retry: { max_attempts, retry_on: [FailureClass], backoff_ms }`。编写 fixture 前先 `rg -n "pub struct (CirProgram|CirNode|ControlSpec)" crates/acos-core/src/types.rs` 核对字段名。）

`crates/acos-bench/fixtures/condition/repair_branch.yaml`（else 分支写文件）：

```yaml
id: condition_repair_branch
mode: cir
goal: "take the else branch and write a marker file"
cir:
  program_id: bench_condition_repair
  nodes:
    - node_id: "A"
      kind: primitive
      capability: write_file
      output: marker
      inputs:
        path: "{workspace}/marker.txt"
        content: "repair"
      control:
        condition:
          identifier: search_results
          operator: exists
          operand: null
      else_children:
        - node_id: "C"
          kind: primitive
          capability: write_file
          output: marker2
          inputs:
            path: "{workspace}/else.txt"
            content: "else-branch"
files:
  workspace-marker: ""
expected:
  execution: true
```

（注意：`search` stub 第一次失败 → 若本 fixture 期望走 condition 分支，需保证 search 已成功——即该 fixture 不在依赖 search 成功的路径上，或 fixture 直接省略 search 节点让 A 的条件引用不存在的输出。**简化：condition 系列 fixture 不依赖 search stub，条件引用 WriteFile 的固定输出 `marker`（exists 恒真），else 分支由不存在的引用驱动。** 具体组合在实现时按 validate_cir 的"标识符必须命中某节点 output"规则设计，plan 只定骨架。）

`crates/acos-bench/fixtures/loop/foreach.yaml`：

```yaml
id: loop_foreach
mode: cir
goal: "summarize each list item"
cir:
  program_id: bench_loop_foreach
  nodes:
    - node_id: "src"
      kind: primitive
      capability: list_source
      output: items
      inputs: {}
    - node_id: "loop"
      kind: primitive
      capability: summarize
      output: per_item
      inputs:
        text: "{$item}"
      control:
        loop_spec:
          kind: foreach
          list_source: "{$items}"
          item_var: "item"
expected:
  execution: true
```

`crates/acos-bench/fixtures/loop/while_limit.yaml`（负例：超限失败）：

```yaml
id: loop_while_limit
mode: cir
goal: "a while loop that must terminate via max_iterations"
cir:
  program_id: bench_loop_while
  nodes:
    - node_id: "A"
      kind: primitive
      capability: summarize
      output: out
      inputs:
        text: "{$out}"
      control:
        loop_spec:
          kind: while
          condition:
            identifier: out
            operator: ne
            operand: null
          max_iterations: 2
expected:
  execution: false
  final_status: failed
```

`crates/acos-bench/fixtures/retry/timeout.yaml`：

```yaml
id: retry_timeout
mode: cir
goal: "transient search failure is retried"
cir:
  program_id: bench_retry_timeout
  nodes:
    - node_id: "A"
      kind: primitive
      capability: search
      output: results
      inputs:
        query: "acos"
      control:
        retry:
          max_attempts: 3
          retry_on: [timeout]
          backoff_ms: 5
expected:
  recovery: retry
  execution: true
```

集成测试（`tests/condition.rs`）：

```rust
use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn condition_suite_passes() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("condition".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "condition suite: {:?}", report.cases);
    assert_eq!(report.skipped(), 0);
    assert!(report.passed() >= 2);
}
```

`tests/loop.rs`：`suite: Some("loop")`，断言 `failed() == 0`、`passed() >= 1`（while_limit 为负例 → 通过的方式是 `expected.execution: false` 命中，仍算 Pass）。`tests/retry.rs`：`suite: Some("retry")`，断言 `passed() == 1` 且该 case `recovery == Some("retry")`。

- [ ] **Step 7: 运行测试**

Run: `cargo test -p acos-bench`；`cargo run -p acos-bench -- --suite condition`
Expected: 全部 PASS；表格输出 3+ 行且 `0 failed / 0 skipped`

- [ ] **Step 8: 提交**

```bash
git add crates/acos-bench Cargo.lock
git commit -m "feat(bench): acos-bench harness + condition/loop/retry fixtures-as-contracts"
```

### Task 11: acos-bench recovery 套件 + 负例套件 + --require-model

**Files:**
- Modify: `crates/acos-bench/src/runner.rs`（--require-model、SKIP→FAIL、recovery 观测完善）
- Modify: `crates/acos-bench/src/lib.rs`（`run` 内应用 require_model 转换）
- Modify: `crates/acos-bench/src/registry.rs`（`with_search_failure_class` 已备；如需 key 驱动 model 跳过则保持原样）
- Create: `crates/acos-bench/fixtures/recovery/rule_replan.yaml`
- Create: `crates/acos-bench/fixtures/recovery/model_replan.yaml`（期望 model，无 key → SKIP）
- Create: `crates/acos-bench/fixtures/negative/loop_no_max.yaml`
- Create: `crates/acos-bench/fixtures/negative/retry_zero.yaml`
- Create: `crates/acos-bench/fixtures/negative/retry_irreversible.yaml`
- Create: `crates/acos-bench/tests/recovery.rs`、`crates/acos-bench/tests/negative.rs`
- Test: 上述两个集成测试文件

**Interfaces:**
- Consumes: Task 8 `RuleReplanner`、Task 9 `ModelRecoveryPlanner`、`execute_with_recovery`（Task 7）
- Produces: recovery/negative 两套契约 fixture

- [ ] **Step 1: 负例契约（先写，复现失败）**

`fixtures/negative/loop_no_max.yaml`：

```yaml
id: negative_loop_no_max
mode: cir
goal: "while loop without max_iterations must be rejected"
cir:
  program_id: bench_neg_loop
  nodes:
    - node_id: "A"
      kind: primitive
      capability: summarize
      output: out
      inputs:
        text: "hi"
      control:
        loop_spec:
          kind: while
          condition:
            identifier: out
            operator: ne
            operand: null
          max_iterations: null
expected:
  compile: false
  validation: "max_iterations"
```

`fixtures/negative/retry_zero.yaml`：

```yaml
id: negative_retry_zero
mode: cir
goal: "retry with max_attempts 0 must be rejected"
cir:
  program_id: bench_neg_retry0
  nodes:
    - node_id: "A"
      kind: primitive
      capability: search
      output: results
      inputs:
        query: "acos"
      control:
        retry:
          max_attempts: 0
          retry_on: [timeout]
          backoff_ms: 1
expected:
  compile: false
  validation: "max_attempts"
```

`fixtures/negative/retry_irreversible.yaml`（registry-aware 校验，Task 6/7 已实现）：

```yaml
id: negative_retry_irreversible
mode: cir
goal: "retry on an irreversible primitive must be rejected"
cir:
  program_id: bench_neg_irrev
  nodes:
    - node_id: "A"
      kind: primitive
      capability: irreversible
      output: done
      inputs: {}
      control:
        retry:
          max_attempts: 2
          retry_on: [timeout]
          backoff_ms: 1
expected:
  compile: false
  validation: "irreversible"
```

（`negative_retry_irreversible` 的校验在 runtime 侧（registry-aware `check_retry_irreversible(program, registry)`，Task 6 已加）。为让该负例在 compile 阶段短路，runner 对 `mode: cir` 的 fixture 依次做 `validate_cir`（Task 5）+ registry 检查，任一失败且命中 `expected.validation` 子串 → Pass。）

- [ ] **Step 2: recovery 契约**

`fixtures/recovery/rule_replan.yaml`（依赖 Task 8 OfflineFallbackRule；`search` stub 用 `FailureClass::Timeout`）：

```yaml
id: recovery_rule_replan
mode: cir
goal: "timeout on search falls back to local read via rule replanner"
cir:
  program_id: bench_rec_rule
  nodes:
    - node_id: "search"
      kind: primitive
      capability: search
      output: results
      inputs:
        query: "acos"
files:
  fallback.txt: "static answer"
expected:
  recovery: rule
  execution: true
```

（本 fixture 的关键：`search` stub 恒失败一次（Timeout），RuleReplanner 检测到 `FailureClass::Timeout` + 候选节点 `read_file` → 生成子图替换 `search` 为 `read_file {workspace}/fallback.txt`。runner 需在 `execute_with_recovery` 时传入 `RecoveryContext { planner: Some(Arc<RuleReplanner>), model: None }`，具体字段以 Task 4/7 定义为准。）

`fixtures/recovery/model_replan.yaml`（无 key → SKIP；`--require-model` 下变 FAIL）：

```yaml
id: recovery_model_replan
mode: cir
goal: "unknown-class failure is delegated to the model replanner"
cir:
  program_id: bench_rec_model
  nodes:
    - node_id: "A"
      kind: primitive
      capability: search
      output: results
      inputs:
        query: "acos"
expected:
  recovery: model
  execution: true
```

（runner 实现：model 路径需要 LLM key。`LongCatClient::from_env()` 失败 → 记 `Skip`，note = "model not configured"；`--require-model` 时 runner 把该 Skip 翻转为 Fail 并在 note 中注明 `REQUIRE-MODEL`。）

- [ ] **Step 3: runner 支持 --require-model + recovery 观测**

`runner.rs` 追加：

```rust
/// Post-processes the report: --require-model turns skipped cases that
/// expected model recovery into failures.
pub fn apply_require_model(report: &mut BenchReport) {
    for case in &mut report.cases {
        if case.status == report::CaseStatus::Skip
            && case.recovery.as_deref() == Some("model")
            && case.note.contains("model not configured")
        {
            case.status = report::CaseStatus::Fail;
            case.note.push_str(" (REQUIRE-MODEL)");
        }
    }
}
```

`lib.rs` 的 `run()` 在 `report` 返回前调用 `apply_require_model`（仅当 `args.require_model`）。recovery 观测在 Step 5 已实现（EventStore 事件过滤 `retry.started`/`replan.started`，`replan.started` payload 含 `planner: "rule"|"model"`——字段名以 Task 7 事件定义为准，必要时在 runner 内 `match` 提取）。

- [ ] **Step 4: 集成测试**

`tests/recovery.rs`：

```rust
use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn rule_replan_recovers() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("recovery".into()),
        case: Some("recovery_rule_replan".into()),
        require_model: false,
    })
    .await;
    let case = &report.cases[0];
    assert_eq!(case.status.to_string(), "PASS", "{case:?}");
    assert_eq!(case.recovery.as_deref(), Some("rule"));
}
```

`tests/negative.rs`：断言 negative 套件 `failed() == 0`、`skipped() == 0`、passed >= 3。

- [ ] **Step 5: 运行测试**

Run: `cargo test -p acos-bench && cargo run -p acos-bench -- --suite recovery && cargo run -p acos-bench -- --suite recovery --require-model`
Expected: 无 --require-model 时 recovery 套件 = 1 PASS + 1 SKIP；加 --require-model 后 = 1 PASS + 1 FAIL，退出码 1（main.rs 已按 failed() 退出）。

- [ ] **Step 6: 提交**

```bash
git add crates/acos-bench
git commit -m "feat(bench): recovery + negative suites, --require-model gate"
```

### Task 12: acos-cli `bench` 子命令

**Files:**
- Modify: `crates/acos-cli/Cargo.toml`（+ `acos-bench` 依赖）
- Modify: `crates/acos-cli/src/main.rs`（解析 `bench` 子命令）
- Test: `crates/acos-cli/tests/`（如已有测试则追加；无则手测）

**Interfaces:**
- Consumes: `acos_bench::{BenchArgs, run}`
- Produces: `acos bench [--suite S] [--case C] [--require-model]`

- [ ] **Step 1: 加依赖**

`crates/acos-cli/Cargo.toml` 的 `[dependencies]` 追加：

```toml
acos-bench = { path = "../acos-bench" }
```

- [ ] **Step 2: 子命令解析**

`main.rs` 顶部追加：

```rust
use std::path::PathBuf;
```

在 `fn main` 内、现有子命令分发之前插入：

```rust
if args.get(1).is_some_and(|a| a == "bench") {
    // acos bench [--suite S] [--case C] [--require-model]
    let mut suite = None;
    let mut case = None;
    let mut require_model = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--suite" => { suite = args.get(i + 1).cloned(); i += 1; }
            "--case" => { case = args.get(i + 1).cloned(); i += 1; }
            "--require-model" => require_model = true,
            other => {
                eprintln!("usage: acos bench [--suite S] [--case C] [--require-model]");
                eprintln!("unknown argument: {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
    let mut fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures_dir.push("../acos-bench/fixtures");
    let rt = tokio::runtime::Runtime::new().expect("tokio runtime");
    let report = rt.block_on(acos_bench::run(acos_bench::BenchArgs {
        fixtures_dir,
        suite,
        case,
        require_model,
    }));
    report.print();
    std::process::exit(if report.failed() == 0 { 0 } else { 1 });
}
```

（`env!("CARGO_MANIFEST_DIR")` 是 `crates/acos-cli`，相对路径 `../acos-bench/fixtures` 在发布二进制中不存在——仅限开发用。更稳的做法是允许 `--fixtures <dir>` 覆盖，默认值同上并注释说明。已有 `compile`/`run` 子命令的解析方式以 main.rs 现状为准，风格保持一致：若现状用 match 分发则并入 match。）

- [ ] **Step 3: 手测**

Run: `cargo run -p acos-cli -- bench --suite condition`
Expected: 输出 bench 表格，退出码 0。
Run: `cargo run -p acos-cli -- bench --suite recovery --require-model`（无 key）
Expected: 退出码 1（model case FAIL）。

- [ ] **Step 4: 提交**

```bash
git add crates/acos-cli
git commit -m "feat(cli): acos bench subcommand"
```

### Task 13: 文档收尾

**Files:**
- Modify: `HANDOFF.md`（P0 状态、已知限制更新）
- Modify: `README.md`（acos bench 用法）
- Modify: `PROJECT_STATUS.md`（P0 完成标记、下一阶段）
- Modify: `docs/specs/cir_spec.md`（control 字段同步）
- Modify: `CHANGELOG.md`（记录 P0 变更）

- [ ] **Step 1: cir_spec.md 同步**

对照 `schemas/cir/cir.proto`（Task 1 已同步）把 `docs/specs/cir_spec.md` 中 CirNode 定义补充：

```markdown
### CirNode.control（P0 新增）

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `condition` | ConditionSpec | while 条件或分支条件 |
| `loop_spec` | LoopSpec | 循环语义 |
| `retry` | RetryPolicy | 失败重试（需 retry-safe） |

语义规则：
- `While` 先求值条件再执行；`Until` 先执行再求值（避免 off-by-one）。
- `ForEach` 无 `max_iterations` 时以数组长度自然终止；`While`/`Until` 必须显式提供 `max_iterations >= 1`。
- `Retry` 仅对暂态类（`timeout`/`ratelimit`/`transient`）生效，且节点原语必须 retry-safe（`idempotent()` 或全部效果为纯读）；`ExternalIrreversible` 效果禁止重试。
```

（具体措辞与 Task 1 的 proto/validate_cir 实现保持一致。）

- [ ] **Step 2: README.md + PROJECT_STATUS.md + HANDOFF.md + CHANGELOG.md**

- README：新增小节：

```markdown
## Benchmark

fixture-as-contract 回归套件（P0）：

```bash
cargo run -p acos-cli -- bench --suite condition
cargo run -p acos-cli -- bench                    # 全量
cargo run -p acos-cli -- bench --require-model    # CI 严格模式
```
```

- PROJECT_STATUS：P0 完成 → 勾选 `[x]`，新增"已实现"摘要（控制语义 / 恢复状态机 / bench）与已知限制（ModelReplanner 无 key 跳过、expr 禁止模糊引用、ForEach 无并发）。
- HANDOFF：更新"当前进度"为 P0 完成；已知限制同步；把 `MAX_RECOVERY_ATTEMPTS=3`、`--require-model` 等操作细节写进交接注意事项。
- CHANGELOG：追加 `[0.1.0] - <日期>`（或现有版本结构）条目，列出 P0 全部变更点（控制语义、恢复、bench、CLI）。

- [ ] **Step 3: 全量回归**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets`
Expected: 全部 PASS；clippy 无新 warning（存量 pedantic warning 忽略）。

- [ ] **Step 4: 提交**

```bash
git add HANDOFF.md README.md PROJECT_STATUS.md docs/specs/cir_spec.md CHANGELOG.md
git commit -m "docs: P0 收尾 — 控制语义/恢复/bench 文档同步"
```

---

## 收尾清单（所有任务完成后）

- [ ] `cargo test --workspace` 全绿
- [ ] `cargo clippy --workspace --all-targets` 无新 warning
- [ ] `cargo run -p acos-cli -- bench` 全量表格输出，0 failed
- [ ] P0 三个目标（控制语义 / 失败恢复 / benchmark 契约）均已落地并有测试
- [ ] 按需推进 P1（expr 增强、任务级保留绑定、ForEach 并发、恢复事件可视化）