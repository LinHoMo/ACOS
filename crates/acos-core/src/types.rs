//! Core ACOS types: tasks, CIR, programs, values, effects, evidence.
//!
//! These are the value types shared across compiler, runtime, state, and
//! verify. They are kept dependency-light (serde only) so every crate can
//! depend on them.

use serde::{Deserialize, Serialize};

use crate::id::{ArtifactId, ProgramId, RunId, TaskId};

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
    Retry,
    /// Checkpoint.
    Checkpoint,
    /// Verification obligation.
    Verification,
    /// Artifact reference.
    ArtifactRef,
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
    /// Named output binding, if any.
    pub output: Option<String>,
    /// Child node ids (for sequence/parallel/conditional/loop).
    pub children: Vec<String>,
    /// Input bindings for primitive invocations: param name -> literal or
    /// `$output_ref`. The runtime resolves `$ref` against the environment.
    #[serde(default)]
    pub inputs: std::collections::HashMap<String, serde_json::Value>,
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

// ── Convenience aliases ──────────────────────────────────────────────────────

/// A task paired with its spec (used at the compiler boundary).
pub type Task = TaskSpec;

/// A task identifier alias for readability at call sites.
pub use crate::id::TaskId as TaskIdAlias;

/// A run identifier alias for readability at call sites.
pub use crate::id::RunId as RunIdAlias;
