//! ACOS P1 Baseline Agents.
//!
//! Direct Tool-Loop: LLM + Tools + Loop. No planner, no compiler, no ACOS.
//!
//! Scope: empirical comparison with ACOS on the same flagship task.

#![warn(missing_docs)]

pub mod agent;
pub mod metrics;
pub mod tools;
pub mod evidence;

pub use agent::{ToolLoopAgent, AgentConfig};
pub use metrics::RunMetrics;
pub use evidence::EvidenceItem;