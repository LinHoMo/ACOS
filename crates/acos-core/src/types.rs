//! Core ACOS types: tasks, CIR, programs, values, effects, evidence.
//!
//! These are the value types shared across compiler, runtime, state, and
//! verify. They are kept dependency-light (serde only) so every crate can
//! depend on them.

use serde::{Deserialize, Serialize};

use crate::id::{ProgramId, RunId, TaskId};

// ── Task specification (compiler front-end input) ────────────────────────────

/// A cognitive task specification — the stable front-end input to the compiler.
///
/// See `docs/specs/task_spec.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskSpec {
    /// API/version discriminator (e.g. `"acos.io/v1"`).
    pub api_version: String,
    /// Stable task identifier.
    pub id: TaskId,
    /// Natural-language goal.
    pub goal: String,
    /// Declared inputs.
    pub inputs: Vec<TaskInput>,
    /// Declared outputs.
    pub outputs: Vec<TaskOutput>,
    /// Execution constraints.
    pub constraints: Option<TaskConstraints>,
    /// Optimization intent.
    pub optimization: Option<OptimizationGoal>,
    /// Approval policy for external side effects.
    pub approval: Option<ApprovalPolicy>,
}

/// A single task input.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    /// Input type (e.g. `File`).
    #[serde(rename = "type")]
    pub input_type: String,
    /// Path or reference.
    pub path: String,
    /// Format (e.g. `csv`).
    pub format: Option<String>,
}

/// A single task output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskOutput {
    /// Output type (e.g. `Report`).
    #[serde(rename = "type")]
    pub output_type: String,
    /// Format (e.g. `markdown`).
    pub format: Option<String>,
}

/// Execution constraints for a task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TaskConstraints {
    /// Wall-clock timeout in seconds.
    pub timeout_seconds: Option<u64>,
    /// Maximum allowed cost (provider-defined units).
    pub max_cost: Option<f64>,
    /// Whether network access is allowed.
    pub allowed_network: Option<bool>,
}

/// Optimization intent.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OptimizationGoal {
    /// Primary objective.
    pub primary: String,
    /// Secondary objective.
    pub secondary: Option<String>,
}

/// Approval policy for external side effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalPolicy {
    /// Whether external side effects require approval.
    pub external_side_effects: String,
}

// ── Typed values ─────────────────────────────────────────────────────────────

/// The type of a typed value.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ValueType {
    /// Primitive scalar.
    Scalar,
    /// Record / struct.
    Record,
    /// List.
    List,
    /// Optional value.
    Optional,
    /// Result / error union.
    Result,
    /// Nominal semantic type.
    Nominal,
}

/// A typed, opaque value flowing through the execution graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TypedValue {
    /// The value's type.
    pub value_type: ValueType,
    /// JSON-encoded payload (MVP representation).
    pub payload: serde_json::Value,
}

// ── Effects ──────────────────────────────────────────────────────────────────

/// The kind of effect a primitive may have.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum EffectKind {
    /// Filesystem read.
    FsRead,
    /// Filesystem write.
    FsWrite,
    /// Network read.
    NetworkRead,
    /// Network write.
    NetworkWrite,
    /// Process spawn.
    ProcessSpawn,
    /// Secret read.
    SecretRead,
    /// Irreversible external effect (requires approval, no compensation).
    ExternalIrreversible,
}

/// A declared effect with its compensation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct EffectDecl {
    /// The effect kind.
    pub kind: EffectKind,
    /// Human-readable description.
    pub description: String,
    /// Whether this effect is reversible (and thus has a compensation).
    pub reversible: bool,
}

// ── CIR (Cognitive Intermediate Representation) ─────────────────────────────

/// The kind of a CIR node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum CirNodeKind {
    /// A primitive invocation.
    PrimitiveInvocation,
    /// Sequential composition.
    Sequence,
    /// Parallel composition.
    Parallel,
    /// Conditional branch.
    Conditional,
    /// Loop / map.
    LoopMap,
    /// Retry node.
    ///
    /// Deprecated: retry semantics are expressed via [`ControlSpec::retry`]
    /// since P0. Kept for wire compatibility with CIR v0.1; removed in CIR 2.0.
    #[deprecated(note = "use ControlSpec.retry")]
    Retry,
    /// Checkpoint.
    Checkpoint,
    /// Verification obligation.
    Verification,
    /// Artifact reference.
    ArtifactRef,
}

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
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
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
    #[default]
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

/// A single node in the CIR graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CirNode {
    /// Node kind.
    pub kind: CirNodeKind,
    /// Node id within the program.
    pub node_id: String,
    /// Invoked primitive capability id (e.g. `"read_file"`), if any.
    /// This is how the runtime resolves the primitive via the registry.
    pub capability: Option<String>,
    /// Named output binding with its data contract, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSpec>,
    /// Expected type name per input key (R3). Optional.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub input_types: std::collections::HashMap<String, String>,
    /// Child node ids (for sequence/parallel/conditional/loop).
    pub children: Vec<String>,
    /// False branch for `Conditional` nodes (true branch is `children`).
    #[serde(default)]
    pub else_children: Vec<String>,
    /// Input bindings for primitive invocations: param name -> literal or
    /// `$output_ref`. The runtime resolves `$ref` against the environment.
    #[serde(default)]
    pub inputs: std::collections::HashMap<String, serde_json::Value>,
    /// Control semantics (condition/loop/retry), separate from `inputs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlSpec>,
}

/// A compiled cognitive program (the CIR).
///
/// See `docs/specs/cir_spec.md`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CirProgram {
    /// Program identifier.
    pub id: ProgramId,
    /// Source task identifier.
    pub task_id: TaskId,
    /// Entry node ids.
    pub entry: Vec<String>,
    /// All nodes, keyed by node_id.
    pub nodes: Vec<CirNode>,
    /// Top-level declared effects.
    pub effects: Vec<EffectDecl>,
}

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

// ── Convenience aliases ──────────────────────────────────────────────────────

/// A task paired with its spec (used at the compiler boundary).
pub type Task = TaskSpec;

/// A task identifier alias for readability at call sites.
pub use crate::id::TaskId as TaskIdAlias;

/// A run identifier alias for readability at call sites.
pub use crate::id::RunId as RunIdAlias;

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
}
