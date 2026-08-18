//! Evidence collection for baseline agents.
//!
//! Mirrors the ACOS event model so the same verifier can process both.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single evidence item (analogous to an ACOS event).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceItem {
    /// Sequence number.
    pub seq: u64,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
    /// Event type (mirrors ACOS event types).
    pub event_type: String,
    /// Event payload.
    pub payload: serde_json::Value,
}

impl EvidenceItem {
    /// Creates a new evidence item.
    pub fn new(seq: u64, event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            seq,
            timestamp: Utc::now(),
            event_type: event_type.to_string(),
            payload,
        }
    }

    /// Creates a "run.started" event.
    pub fn run_started(task_id: &str) -> Self {
        Self::new(1, "run.started", serde_json::json!({ "task_id": task_id }))
    }

    /// Creates a "run.finished" event.
    pub fn run_finished(status: &str) -> Self {
        Self::new(0, "run.finished", serde_json::json!({ "status": status }))
    }

    /// Creates a "llm.call" event.
    pub fn llm_call(model: &str, input_tokens: u64, output_chars: u64) -> Self {
        Self::new(0, "llm.call", serde_json::json!({
            "model": model,
            "input_tokens": input_tokens,
            "output_chars": output_chars,
        }))
    }

    /// Creates a "tool.call" event.
    pub fn tool_call(name: &str, success: bool, output_chars: u64) -> Self {
        Self::new(0, "tool.call", serde_json::json!({
            "tool": name,
            "success": success,
            "output_chars": output_chars,
        }))
    }

    /// Creates an "artifact.stored" event.
    pub fn artifact_stored(name: &str, path: &str, size_bytes: u64) -> Self {
        Self::new(0, "artifact.stored", serde_json::json!({
            "name": name,
            "path": path,
            "size_bytes": size_bytes,
        }))
    }
}

/// Collects evidence during a run.
#[derive(Debug, Clone, Default)]
pub struct EvidenceLog {
    items: Vec<EvidenceItem>,
    next_seq: u64,
}

impl EvidenceLog {
    /// Creates a new evidence log with a run.started event.
    pub fn new(task_id: &str) -> Self {
        let mut log = Self { items: vec![], next_seq: 1 };
        log.add(EvidenceItem::run_started(task_id));
        log
    }

    /// Adds an evidence item with auto-incrementing seq.
    pub fn add(&mut self, mut item: EvidenceItem) {
        item.seq = self.next_seq;
        self.next_seq += 1;
        self.items.push(item);
    }

    /// Returns all evidence items.
    pub fn items(&self) -> &[EvidenceItem] {
        &self.items
    }

    /// Adds a run.finished event and returns the full log.
    pub fn finish(mut self, status: &str) -> Vec<EvidenceItem> {
        let mut item = EvidenceItem::run_finished(status);
        item.seq = self.next_seq;
        self.items.push(item);
        self.items
    }
}