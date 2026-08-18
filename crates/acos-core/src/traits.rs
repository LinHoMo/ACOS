//! Public ACOS traits — the interface contracts between layers.
//!
//! These traits define the seams between compiler, runtime, state, verify,
//! and plugin layers. Each layer depends only on these traits (from
//! `acos-core`), never on another layer's concrete type.
//!
//! All traits use `#[async_trait]` so they remain object-safe (usable as
//! `dyn Trait`). See `docs/internal/architecture.md`.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::AcosError;
use crate::id::{ArtifactId, PrimitiveId, RunId};
use crate::types::{CirProgram, EffectDecl, FailureContext, RecoveryProposal, TaskSpec, TypedValue};

// ── Primitive ────────────────────────────────────────────────────────────────

/// Description of a primitive's capability.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityDesc {
    /// Capability id (e.g. `"information.summarization"`).
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Input type name.
    pub input_type: String,
    /// Output type name.
    pub output_type: String,
}

/// A primitive invoke request (runtime RPC / in-process).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokeRequest {
    /// Primitive id.
    pub primitive_id: String,
    /// JSON-encoded input.
    pub input: serde_json::Value,
}

/// A primitive invoke response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct InvokeResponse {
    /// JSON-encoded output.
    pub output: serde_json::Value,
}

/// A cognitive primitive — the smallest unit of schedulable cognitive work.
///
/// See `docs/specs/cognitive_primitive_spec.md`.
#[async_trait]
pub trait Primitive: Send + Sync + std::fmt::Debug {
    /// Returns this primitive's capability description.
    fn capability(&self) -> CapabilityDesc;

    /// Returns the effects this primitive may have.
    fn effects(&self) -> Vec<EffectDecl>;

    /// Invokes the primitive with the given input value.
    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError>;

    /// Returns whether this primitive has a compensation for the given effect.
    fn has_compensation(&self, effect: &EffectDecl) -> bool;

    /// Executes the compensation for a performed effect.
    async fn compensate(&self, effect: &EffectDecl, input: TypedValue) -> Result<(), AcosError>;

    /// Returns whether repeating this primitive is safe (idempotent).
    ///
    /// Defaults to `false`. Primitives that can be safely re-invoked without
    /// duplicated side effects (e.g. pure reads) may override this.
    fn idempotent(&self) -> bool {
        false
    }
}

// ── Compiler ─────────────────────────────────────────────────────────────────

/// The result of compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileResult {
    /// The compiled CIR program.
    pub program: CirProgram,
    /// Compile-time diagnostics (warnings, notes).
    pub diagnostics: Vec<Diagnostic>,
}

/// A compile-time diagnostic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Diagnostic {
    /// Severity.
    pub level: DiagnosticLevel,
    /// Human-readable message.
    pub message: String,
}

/// Diagnostic severity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticLevel {
    /// Informational.
    Note,
    /// Warning.
    Warning,
    /// Error.
    Error,
}

/// The cognitive compiler: turns a task spec into a validated CIR program.
///
/// See `docs/specs/cir_spec.md` and `docs/internal/compiler_design.md`.
#[async_trait]
pub trait Compiler: Send + Sync + std::fmt::Debug {
    /// Compiles a task specification into a CIR program.
    async fn compile(&self, task: TaskSpec) -> Result<CompileResult, AcosError>;
}

// ── Runtime ──────────────────────────────────────────────────────────────────

/// Handle to a submitted run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RunHandle(pub RunId);

/// Status of a run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    /// Waiting to be scheduled.
    Pending,
    /// Currently executing.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed.
    Failed,
    /// Compensating (rolling back).
    Compensating,
    /// Compensated (rolled back).
    Compensated,
}

/// The cognitive runtime: executes CIR programs durably.
///
/// See `docs/specs/runtime_model.md` and `docs/specs/execution_model.md`.
#[async_trait]
pub trait Runtime: Send + Sync + std::fmt::Debug {
    /// Submits a program for execution, returning a handle.
    async fn submit(&self, program: CirProgram) -> Result<RunHandle, AcosError>;

    /// Polls the status of a run.
    async fn poll(&self, handle: RunHandle) -> Result<RunStatus, AcosError>;

    /// Requests a checkpoint for a run.
    async fn checkpoint(&self, handle: RunHandle) -> Result<(), AcosError>;

    /// Triggers compensation for a run's effects.
    async fn compensate(&self, handle: RunHandle) -> Result<(), AcosError>;
}

// ── Event store ──────────────────────────────────────────────────────────────

/// A persisted event in the event log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Monotonic sequence number.
    pub seq: u64,
    /// Run this event belongs to.
    pub run_id: RunId,
    /// Event type tag.
    pub event_type: String,
    /// JSON payload.
    pub payload: serde_json::Value,
}

/// The append-only event store — the source of truth for state.
///
/// See `docs/specs/state_and_event_model.md`.
#[async_trait]
pub trait EventStore: Send + Sync + std::fmt::Debug {
    /// Appends an event to the log.
    async fn append(
        &self,
        run_id: RunId,
        event_type: String,
        payload: serde_json::Value,
    ) -> Result<Event, AcosError>;

    /// Queries events for a run.
    async fn query(&self, run_id: RunId) -> Result<Vec<Event>, AcosError>;

    /// Replays all events for a run in order.
    async fn replay(&self, run_id: RunId) -> Result<Vec<Event>, AcosError>;
}

// ── Artifact store ───────────────────────────────────────────────────────────

/// The artifact store — persists produced outputs.
#[async_trait]
pub trait ArtifactStore: Send + Sync + std::fmt::Debug {
    /// Stores an artifact and returns its id.
    async fn put(&self, run_id: RunId, name: String, content: Vec<u8>)
        -> Result<ArtifactId, AcosError>;

    /// Retrieves an artifact by id.
    async fn get(&self, id: ArtifactId) -> Result<Vec<u8>, AcosError>;
}

// ── Plugin registry ──────────────────────────────────────────────────────────

/// The plugin registry — manages primitive provider lifecycle.
///
/// See `docs/specs/plugin_system.md`.
#[async_trait]
pub trait PluginRegistry: Send + Sync + std::fmt::Debug {
    /// Lists all registered primitives.
    fn list(&self) -> Vec<CapabilityDesc>;

    /// Finds a primitive by capability id.
    async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError>;

    /// Hot-loads a primitive provider.
    async fn load(&self, manifest: PrimitiveManifest) -> Result<PrimitiveId, AcosError>;

    /// Hot-unloads a primitive provider.
    async fn unload(&self, id: PrimitiveId) -> Result<(), AcosError>;
}

/// A primitive provider manifest (MVP wire format).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrimitiveManifest {
    /// API version.
    pub api_version: String,
    /// Primitive id.
    pub id: String,
    /// Semantic version.
    pub version: String,
    /// Capability id.
    pub capability: String,
    /// Provider runtime (e.g. `"process"`).
    pub runtime: String,
    /// Provider command.
    pub command: String,
}

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
