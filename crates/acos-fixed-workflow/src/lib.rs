//! ACOS P1-4 Fixed Workflow Baseline.
//!
//! A deterministic, human-authored program executed directly (no CIR, no
//! ACOS runtime/compiler) and verified with the same oracle as the other
//! experimental systems. See `docs/specs/2026-08-19-p1-4-fixed-workflow-design.md`.

pub mod metrics;
pub mod tools;

use acos_core::id::RunId;
use acos_core::traits::EventStore;
use acos_state::InMemoryStore;
use acos_verify::{verify_run_full, GroundTruth};
use metrics::{count_script_loc, FixedWorkflowMetrics, LayerOutcomes};
use serde_json::json;

/// Embedded human-authored fixed program (the experiment object).
const FLAGSHIP_SCRIPT: &str = include_str!("../workflows/flagship.py");

/// Flagship task input CSV files (declared in `acos_task.yaml`).
pub const FLAGSHIP_INPUTS: [&str; 4] = [
    "sales_q1.csv",
    "sales_q2.csv",
    "sales_q3.csv",
    "sales_q4.csv",
];

/// Run the P1-FLAGSHIP-001 fixed workflow and verify its artifact.
///
/// - `dataset_dir`: directory containing the 4 input CSVs.
/// - `report_path`: where the script writes its markdown report (artifact).
/// - `gt_path`: Ground Truth YAML used by the shared verification oracle.
/// - `author_time_minutes`: engineering cost estimate (human input).
pub async fn run_flagship(
    dataset_dir: &str,
    report_path: &str,
    gt_path: &str,
    author_time_minutes: u32,
) -> Result<FixedWorkflowMetrics, String> {
    let store = InMemoryStore::new();
    let run_id = RunId::new();

    let script_path = std::env::temp_dir().join("acos-fixed-workflow-flagship.py");
    std::fs::write(&script_path, FLAGSHIP_SCRIPT)
        .map_err(|e| format!("failed to stage workflow script: {e}"))?;

    store
        .append(run_id, "run.started".to_string(), json!({"system": "fixed_workflow", "task": "P1-FLAGSHIP-001"}))
        .await
        .ok();

    let (stdout, elapsed_ms) = tools::run_script(
        script_path.to_str().ok_or("temp path invalid")?,
        &[dataset_dir, report_path],
    )
    .await?;

    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| format!("script stdout is not valid JSON: {e}"))?;
    let files = parsed["files"].as_array().cloned().unwrap_or_default();
    for (i, f) in files.iter().enumerate() {
        let name = f["file"].as_str().unwrap_or("unknown");
        store
            .append(
                run_id,
                "primitive.end".to_string(),
                json!({"primitive_id": format!("fixed_workflow.step{}", i + 1), "file": name}),
            )
            .await
            .ok();
    }
    let steps = files.len();
    let grand_total = parsed["summary"]["grand_total_revenue"].as_f64();
    store
        .append(
            run_id,
            "run.finished".to_string(),
            json!({"status": "completed", "grand_total_revenue": grand_total}),
        )
        .await
        .ok();

    let artifact = std::fs::read(report_path).map_err(|e| format!("artifact missing after run: {e}"))?;
    let ground_truth = GroundTruth::from_yaml(gt_path).map_err(|e| format!("ground truth load: {e}"))?;
    let report = verify_run_full(&store, Some(artifact), run_id, &ground_truth)
        .await
        .map_err(|e| format!("verification failed: {e}"))?;

    Ok(FixedWorkflowMetrics {
        system: "fixed_workflow".into(),
        task: "P1-FLAGSHIP-001".into(),
        execution_time_ms: elapsed_ms,
        layers: LayerOutcomes {
            contract: report.layer_passed(acos_verify::VerificationLayer::Structural),
            execute: true,
            adequacy: report.all_passed(),
        },
        engineering_cost: metrics::EngineeringCost {
            loc: count_script_loc(FLAGSHIP_SCRIPT),
            author_time_minutes,
            nodes: None,
        },
        steps,
    })
}

/// Serialize the metrics record as the unified CLI JSON output.
pub fn metrics_to_json(m: &FixedWorkflowMetrics) -> String {
    serde_json::to_string_pretty(m).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_loc_counts_source() {
        let loc = count_script_loc(FLAGSHIP_SCRIPT);
        assert!(loc > 50, "workflow script should be a real program, got {loc} lines");
    }

    #[test]
    fn inputs_declared_cover_flagship() {
        assert_eq!(FLAGSHIP_INPUTS.len(), 4);
        assert!(FLAGSHIP_INPUTS.iter().all(|f| f.starts_with("sales_q")));
    }
}