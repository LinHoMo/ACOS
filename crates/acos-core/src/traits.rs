//! Public ACOS traits — the interface contracts between layers.
//!
//! These traits define the seams between compiler, runtime, state, verify,
//! and plugin layers. Each layer depends only on these traits (from
//! `acos-core`), never on another layer's concrete type.
//!
//! See `docs/internal/architecture.md` § 设计规则：机制与策略分离.

use serde::{Deserialize, Serialize};

use crate::error::AcosError;
use crate::id::{ArtifactId, PrimitiveId, RunId};
use crate::types::{EffectDecl, TaskSpec, TypedValue};

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

/// A cognitive primitive — the smallest unit of schedulable cognitive work.
///
/// See `docs/specs/cognitive_primitive_spec.md`.
pub trait Primitive: Send + Sync + std::fmt::Debug {
    /// Returns this primitive's capability description.
    fn capability(&self) -> CapabilityDesc;

    /// Returns the effects this primitive may have.
    fn effects(&self) -> Vec<EffectDecl>;

    /// Invokes the primitive with the given input.
    ///
    /// Returns the output value, or a [`AcosError::PrimitiveFailure`].
    fn invoke(
        &self,
        input: TypedValue,
    ) -> impl std::future::Future<Output = Result<TypedValue, AcosError>> + Send;

    /// Returns the compensation for an effect, if reversible.
    ///
    /// Irreversible effects (`external.irreversible`) return `None` and must
    /// be gated by approval instead.
    fn compensation(&self, effect: &EffectDecl) -> Option<CompensationFn>;
}

/// A compensation (rollback) function for a reversible effect.
pub type CompensationFn = Box<dyn Fn() -> Result<(), AcosError> + Send + Sync>;

// ── Compiler ─────────────────────────────────────────────────────────────────

/// The result of compilation.
#[derive(Debug, Clone, PartialEq)]
pub struct CompileResult {
    /// The compiled CIR program.
    pub program: crate::types::CirProgram,
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
pub trait Compiler: Send + Sync + std::fmt::Debug {
    /// Compiles a task specification into a CIR program.
    fn compile(
        &self,
        task: TaskSpec,
    ) -> impl std::future::Future<Output = Result<CompileResult, AcosError>> + Send;
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
pub trait Runtime: Send + Sync + std::fmt::Debug {
    /// Submits a program for execution, returning a handle.
    fn submit(
        &self,
        program: crate::types::CirProgram,
    ) -> impl std::future::Future<Output = Result<RunHandle, AcosError>> + Send;

    /// Polls the status of a run.
    fn poll(
        &self,
        handle: RunHandle,
    ) -> impl std::future::Future<Output = Result<RunStatus, AcosError>> + Send;

    /// Requests a checkpoint for a run.
    fn checkpoint(
        &self,
        handle: RunHandle,
    ) -> impl std::future::Future<Output = Result<(), AcosError>> + Send;

    /// Triggers compensation for a run's effects.
    fn compensate(
        &self,
        handle: RunHandle,
    ) -> impl std::future::Future<Output = Result<(), AcosError>> + Send;
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
pub trait EventStore: Send + Sync + std::fmt::Debug {
    /// Appends an event to the log.
    fn append(
        &self,
        run_id: RunId,
        event_type: String,
        payload: serde_json::Value,
    ) -> impl std::future::Future<Output = Result<Event, AcosError>> + Send;

    /// Queries events for a run.
    fn query(
        &self,
        run_id: RunId,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, AcosError>> + Send;

    /// Replays all events for a run in order.
    fn replay(
        &self,
        run_id: RunId,
    ) -> impl std::future::Future<Output = Result<Vec<Event>, AcosError>> + Send;
}

// ── Artifact store ───────────────────────────────────────────────────────────

/// The artifact store — persists produced outputs.
pub trait ArtifactStore: Send + Sync + std::fmt::Debug {
    /// Stores an artifact and returns its id.
    fn put(
        &self,
        run_id: RunId,
        name: String,
        content: Vec<u8>,
    ) -> impl std::future::Future<Output = Result<ArtifactId, AcosError>> + Send;

    /// Retrieves an artifact by id.
    fn get(
        &self,
        id: ArtifactId,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, AcosError>> + Send;
}

// ── Plugin registry ──────────────────────────────────────────────────────────

/// The plugin registry — manages primitive provider lifecycle.
///
/// See `docs/specs/plugin_system.md`.
pub trait PluginRegistry: Send + Sync + std::fmt::Debug {
    /// Lists all registered primitives.
    fn list(&self) -> Vec<CapabilityDesc>;

    /// Finds a primitive by capability id.
    fn resolve(
        &self,
        capability_id: &str,
    ) -> impl std::future::Future<Output = Result<Box<dyn Primitive>, AcosError>> + Send;

    /// Hot-loads a primitive provider.
    fn load(
        &self,
        manifest: PrimitiveManifest,
    ) -> impl std::future::Future<Output = Result<PrimitiveId, AcosError>> + Send;

    /// Hot-unloads a primitive provider.
    fn unload(
        &self,
        id: PrimitiveId,
    ) -> impl std::future::Future<Output = Result<(), AcosError>> + Send;
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
