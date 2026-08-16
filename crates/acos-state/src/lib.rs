//! ACOS state stores.
//!
//! Provides the in-memory and (future) SQLite implementations of the
//! `EventStore` and `ArtifactStore` traits.

#![warn(missing_docs)]

pub mod memory;

pub use memory::InMemoryStore;
