//! Unified error type covering all seven ACOS failure domains.
//!
//! See `docs/internal/architecture.md` § 失败域 / Failure domains.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// The top-level error type for all ACOS operations.
///
/// Every failure domain in the architecture maps to a variant here so that
/// callers can recover explicitly per domain.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum AcosError {
    /// A primitive operation failed (e.g. `read_file` could not read).
    #[error("primitive failure: {message} (primitive={primitive_id:?})")]
    PrimitiveFailure {
        /// Human-readable message.
        message: String,
        /// Which primitive failed, if known.
        primitive_id: Option<String>,
    },

    /// A primitive provider failed (process down, unhealthy, etc.).
    #[error("provider failure: {message} (provider={provider:?})")]
    ProviderFailure {
        /// Human-readable message.
        message: String,
        /// Provider identity.
        provider: String,
    },

    /// The compiler could not produce a valid program.
    #[error("compiler failure: {message}")]
    CompilerFailure {
        /// Human-readable message.
        message: String,
    },

    /// Pre-execution validation of a program failed.
    #[error("validation failure: {message}")]
    ValidationFailure {
        /// Human-readable message.
        message: String,
    },

    /// A compensation (rollback) operation itself failed.
    ///
    /// This is distinct from the original failure: it must be flagged for
    /// human intervention.
    #[error("compensation failure: {message} (effect={effect:?})")]
    CompensationFailure {
        /// Human-readable message.
        message: String,
        /// The effect whose compensation failed.
        effect: Option<String>,
    },

    /// Runtime infrastructure failed (scheduler, event store, IPC).
    #[error("runtime infrastructure failure: {message}")]
    RuntimeInfrastructureFailure {
        /// Human-readable message.
        message: String,
    },

    /// An external system (network, filesystem, API) failed.
    #[error("external system failure: {message} (system={system:?})")]
    ExternalSystemFailure {
        /// Human-readable message.
        message: String,
        /// Which external system.
        system: Option<String>,
    },

    /// The operation was rejected by user policy.
    #[error("user-policy rejection: {message}")]
    UserPolicyRejection {
        /// Human-readable message.
        message: String,
    },

    /// A catch-all for errors that do not fit other domains.
    #[error("internal error: {message}")]
    Internal {
        /// Human-readable message.
        message: String,
    },
}

impl AcosError {
    /// Returns `true` if this error represents a failure that requires
    /// human intervention (compensation failures always do).
    pub fn requires_human_intervention(&self) -> bool {
        matches!(self, AcosError::CompensationFailure { .. })
    }
}
