//! ACOS core types, traits, errors, and schema primitives.
//!
//! This crate is the shared foundation for all other ACOS crates. It defines
//! the public traits (`Primitive`, `Compiler`, `Runtime`, `EventStore`, …),
//! the core value types (`Cir`, `Task`, `Program`, `Evidence`, …), and the
//! unified `AcosError` type covering all seven failure domains.
//!
//! See `docs/internal/architecture.md` and `docs/specs/` for the design.

#![warn(missing_docs)]

pub mod error;
pub mod id;
pub mod schema;
pub mod traits;
pub mod types;

pub use error::AcosError;
pub use id::{ArtifactId, PrimitiveId, ProgramId, RunId, TaskId};
pub use types::{CirProgram, EffectDecl, EffectKind, Task, TaskSpec, TypedValue, ValueType};
