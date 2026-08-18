//! Run metrics for baseline agents.
//!
//! All baselines and ACOS record the same metrics for fair comparison.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Metrics collected during a baseline run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMetrics {
    /// Agent type identifier.
    pub agent_type: String,
    /// Task identifier.
    pub task_id: String,
    /// When the run started.
    pub started_at: DateTime<Utc>,
    /// When the run finished.
    pub finished_at: Option<DateTime<Utc>>,
    /// Total wall-clock duration in milliseconds.
    pub duration_ms: u64,
    /// Number of LLM API calls made.
    pub llm_calls: u32,
    /// Number of tokens estimated (if available).
    pub estimated_tokens: u64,
    /// Number of tool calls executed.
    pub tool_calls: u32,
    /// Number of tool calls that failed.
    pub tool_failures: u32,
    /// Number of retries (0 for baseline — no retry logic).
    pub retries: u32,
    /// Number of replans (0 for baseline — no replan logic).
    pub replans: u32,
    /// Number of primitives/tools used (baseline: distinct tool types used).
    pub distinct_tools_used: u32,
    /// Final artifact count.
    pub artifact_count: u32,
    /// Whether the agent reported success.
    pub reported_success: bool,
    /// Whether the verifier passed.
    pub verification_passed: Option<bool>,
    /// Final verification score (fraction of findings that passed).
    pub verification_score: Option<f64>,
    /// Estimated cost in USD (if model pricing known).
    pub estimated_cost_usd: Option<f64>,
    /// Total output characters produced.
    pub output_chars: u64,
    /// Final report content (if produced).
    pub final_report: Option<String>,
}

impl RunMetrics {
    /// Creates a new metrics record for the given agent and task.
    pub fn new(agent_type: &str, task_id: &str) -> Self {
        Self {
            agent_type: agent_type.to_string(),
            task_id: task_id.to_string(),
            started_at: Utc::now(),
            finished_at: None,
            duration_ms: 0,
            llm_calls: 0,
            estimated_tokens: 0,
            tool_calls: 0,
            tool_failures: 0,
            retries: 0,
            replans: 0,
            distinct_tools_used: 0,
            artifact_count: 0,
            reported_success: false,
            verification_passed: None,
            verification_score: None,
            estimated_cost_usd: None,
            output_chars: 0,
            final_report: None,
        }
    }

    /// Marks the run as complete and records final duration.
    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
        self.duration_ms = (self.finished_at.unwrap() - self.started_at).num_milliseconds().max(0) as u64;
    }

    /// Returns a summary string for display.
    pub fn summary(&self) -> String {
        format!(
            "Agent: {}\nTask: {}\nDuration: {}ms\nLLM calls: {}\nTool calls: {} ({} failed)\nRetries: {} | Replans: {}\nArtifacts: {}\nReported success: {}\nVerification: {}\nEstimated cost: ${:.4}",
            self.agent_type,
            self.task_id,
            self.duration_ms,
            self.llm_calls,
            self.tool_calls,
            self.tool_failures,
            self.retries,
            self.replans,
            self.artifact_count,
            self.reported_success,
            self.verification_passed.map(|p| if p { "PASS" } else { "FAIL" }).unwrap_or("N/A"),
            self.estimated_cost_usd.unwrap_or(0.0),
        )
    }
}