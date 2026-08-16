//! Strongly-typed newtypes for ACOS identifiers.
//!
//! Using newtypes prevents mixing up `RunId` with `ProgramId` at compile time.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A unique run (execution instance) identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RunId(pub Uuid);

/// A unique compiled program identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProgramId(pub Uuid);

/// A unique task specification identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaskId(pub Uuid);

/// A unique cognitive primitive identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PrimitiveId(pub Uuid);

/// A unique artifact (produced output) identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ArtifactId(pub Uuid);

impl RunId {
    /// Generate a fresh `RunId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl ProgramId {
    /// Generate a fresh `ProgramId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl TaskId {
    /// Generate a fresh `TaskId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl PrimitiveId {
    /// Generate a fresh `PrimitiveId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl ArtifactId {
    /// Generate a fresh `ArtifactId`.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RunId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ProgramId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for PrimitiveId {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ArtifactId {
    fn default() -> Self {
        Self::new()
    }
}
