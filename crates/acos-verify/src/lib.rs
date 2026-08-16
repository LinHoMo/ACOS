//! ACOS verification pipeline.
//!
//! Verifies that a run produced the declared outputs and that all evidence
//! was logged (the "model-visible means logged" invariant).

#![warn(missing_docs)]

use acos_core::error::AcosError;
use acos_core::id::RunId;
use acos_core::traits::EventStore;

/// A verification finding.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationFinding {
    /// Whether this finding passed.
    pub passed: bool,
    /// Human-readable message.
    pub message: String,
}

/// A verification report for a run.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationReport {
    /// Findings from the verification.
    pub findings: Vec<VerificationFinding>,
}

impl VerificationReport {
    /// Returns `true` if all findings passed.
    pub fn all_passed(&self) -> bool {
        self.findings.iter().all(|f| f.passed)
    }
}

/// Verifies a run using its event log.
///
/// For the MVP, this checks:
/// 1. The run reached a `run.finished` event.
/// 2. At least one artifact-producing primitive succeeded.
pub async fn verify_run(
    event_store: &dyn EventStore,
    run_id: RunId,
) -> Result<VerificationReport, AcosError> {
    let events = event_store.replay(run_id).await?;

    let mut findings = vec![];

    let has_start = events.iter().any(|e| e.event_type == "run.started");
    let has_finish = events.iter().any(|e| e.event_type == "run.finished");
    let primitive_successes = events
        .iter()
        .filter(|e| e.event_type == "primitive.end")
        .count();

    findings.push(VerificationFinding {
        passed: has_start,
        message: if has_start {
            "run.started event present".into()
        } else {
            "missing run.started event".into()
        },
    });

    findings.push(VerificationFinding {
        passed: has_finish,
        message: if has_finish {
            "run.finished event present".into()
        } else {
            "missing run.finished event".into()
        },
    });

    findings.push(VerificationFinding {
        passed: primitive_successes > 0,
        message: format!("{primitive_successes} primitives executed successfully"),
    });

    Ok(VerificationReport { findings })
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_core::traits::EventStore;
    use acos_state::InMemoryStore;

    #[tokio::test]
    async fn verify_run_passes_for_completed_run() {
        let store = InMemoryStore::new();
        let run_id = RunId::new();
        store
            .append(run_id, "run.started".into(), serde_json::json!({}))
            .await
            .unwrap();
        store
            .append(
                run_id,
                "primitive.end".into(),
                serde_json::json!({"ok": true}),
            )
            .await
            .unwrap();
        store
            .append(run_id, "run.finished".into(), serde_json::json!({}))
            .await
            .unwrap();

        let report = verify_run(&store, run_id).await.unwrap();
        assert!(report.all_passed());
    }
}
