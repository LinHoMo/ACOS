//! Built-in cognitive primitives and the plugin registry.
//!
//! Implements the five MVP primitives: `search`, `read_file`, `write_file`,
//! `execute_python`, `summarize`.

#![warn(missing_docs)]

pub mod primitives;
pub mod registry;

pub use registry::BuiltinRegistry;
