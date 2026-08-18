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
    /// Estimated input tokens (rough: chars / 4).
    pub input_tokens: u64,
    /// Estimated output tokens (rough: chars / 4).
    pub output_tokens: u64,
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
    /// Whether the agent self-reported success (stopped calling tools).
    pub self_reported_success: bool,
    /// Whether the verifier passed (set externally after verification).
    pub verified_success: Option<bool>,
    /// Task success = verifier passed (alias for verified_success).
    pub task_success: Option<bool>,
    /// Final verification score (fraction of findings that passed).
    pub verification_score: Option<f64>,
    /// Estimated cost in USD (None if unknown).
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
            input_tokens: 0,
            output_tokens: 0,
            tool_calls: 0,
            tool_failures: 0,
            retries: 0,
            replans: 0,
            distinct_tools_used: 0,
            artifact_count: 0,
            self_reported_success: false,
            verified_success: None,
            task_success: None,
            verification_score: None,
            estimated_cost_usd: None,
            output_chars: 0,
            final_report: None,
        }
    }

    /// Marks the run as complete and records final duration.
    pub fn finish(&mut self) {
        self.finished_at = Some(Utc::now());
        self.duration_ms = (self.finished_at.unwrap() - self.started_at)
            .num_milliseconds()
            .max(0) as u64;
    }

    /// Returns a summary string for display.
    pub fn summary(&self) -> String {
        let total_tokens = self.input_tokens + self.output_tokens;
        format!(
            "Agent: {}\n\
             Task: {}\n\
             Duration: {}ms\n\
             LLM calls: {}\n\
             Tokens: {} (in: {}, out: {})\n\
             Tool calls: {} ({} failed)\n\
             Retries: {} | Replans: {}\n\
             Distinct tools: {}\n\
             Artifacts: {}\n\
             Self-reported success: {}\n\
             Verified success: {}\n\
             Task success: {}\n\
             Verification score: {}\n\
             Estimated cost: {}\n\
             Output chars: {}",
            self.agent_type,
            self.task_id,
            self.duration_ms,
            self.llm_calls,
            total_tokens,
            self.input_tokens,
            self.output_tokens,
            self.tool_calls,
            self.tool_failures,
            self.retries,
            self.replans,
            self.distinct_tools_used,
            self.artifact_count,
            self.self_reported_success,
            self.verified_success.map(|p| if p { "PASS" } else { "FAIL" }).unwrap_or("N/A"),
            self.task_success.map(|p| if p { "PASS" } else { "FAIL" }).unwrap_or("N/A"),
            self.verification_score.map(|s| format!("{:.2}", s)).unwrap_or("N/A".into()),
            self.estimated_cost_usd.map(|c| format!("${:.4}", c)).unwrap_or("N/A".into()),
            self.output_chars,
        )
    }
}
