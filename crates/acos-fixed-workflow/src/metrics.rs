//! P1-4 Fixed Workflow metrics (four-layer model + engineering cost).

use serde::Serialize;

/// Four-layer result model shared with the other experimental systems.
#[derive(Debug, Clone, Serialize)]
pub struct LayerOutcomes {
    pub contract: bool,
    pub execute: bool,
    pub adequacy: bool,
}

/// Engineering cost — the human-authored program's price tag.
#[derive(Debug, Clone, Serialize)]
pub struct EngineeringCost {
    /// Lines of Python in the fixed workflow script.
    pub loc: usize,
    /// Estimated author time (human estimate, minutes).
    pub author_time_minutes: u32,
    /// No CIR nodes; the program is authored directly.
    pub nodes: Option<u32>,
}

/// Full fixed-workflow run record.
#[derive(Debug, Clone, Serialize)]
pub struct FixedWorkflowMetrics {
    pub system: String,
    pub task: String,
    pub execution_time_ms: u64,
    pub layers: LayerOutcomes,
    pub engineering_cost: EngineeringCost,
    /// Number of per-file processing steps observed from script output.
    pub steps: usize,
}

impl FixedWorkflowMetrics {
    pub fn all_passed(&self) -> bool {
        self.layers.contract && self.layers.execute && self.layers.adequacy
    }
}

/// Count lines in the embedded workflow script source (engineering cost).
pub fn count_script_loc(source: &str) -> usize {
    source.lines().filter(|l| !l.trim().is_empty()).count()
}