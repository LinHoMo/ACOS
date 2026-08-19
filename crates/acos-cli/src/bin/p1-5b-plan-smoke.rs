//! P1-5B v0.2 pipeline smoke test (no LLM).
//!
//! Loads the hand-authored golden Plan (`plan-smoke.json`, test infrastructure
//! only — never injected into prompts), compiles it deterministically to CIR,
//! executes it against the flagship dataset, and verifies the artifact.
//!
//! This exercises the total-function contract end to end:
//!   valid Plan -> compile_plan -> valid CIR -> runtime -> verified artifact.
//!
//! Usage (from workspace root):
//!   cargo run -p acos-cli --bin p1-5b-plan-smoke

use std::path::PathBuf;
use std::sync::Arc;

use acos_compiler::plan::PlanIR;
use acos_core::id::ProgramId;
use acos_core::schema::from_yaml;
use acos_core::traits::{ArtifactStore, EventStore};
use acos_runtime::RuntimeImpl;
use acos_state::InMemoryStore;
use acos_verify::{verify_run_full, GroundTruth};

const PLAN_PATH: &str = "experiments/p1-5b-cognitive-program-discovery/plan-smoke.json";
const TASK_PATH: &str = "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml";
const GT_PATH: &str = "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let plan_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| PLAN_PATH.to_string());
    let plan_raw = std::fs::read_to_string(&plan_path)?;
    let plan: PlanIR = serde_json::from_str(&plan_raw)
        .map_err(|e| format!("failed to parse {plan_path}: {e}"))?;

    let task_yaml = std::fs::read_to_string(TASK_PATH)?;
    let task = from_yaml(&task_yaml)?;
    let ground_truth = GroundTruth::from_yaml(GT_PATH)?;

    println!("P1-5B v0.2/v0.3 pipeline smoke");
    println!("  plan: {plan_path}");
    println!("  task: {TASK_PATH}");

    // 1. Plan validation (must pass; the plan is authored valid).
    acos_compiler::plan::validate_plan(&plan)
        .map_err(|e| format!("plan validation failed: {e}"))?;
    println!("[1] plan validation: OK");

    // 2. Deterministic compilation (total function).
    let program = acos_compiler::plan::compile_plan(&plan, &task, task.id, ProgramId::new())
        .map_err(|e| format!("plan compilation failed: {e}"))?;
    println!(
        "[2] compiled: {} nodes ({} primitives, {} loops, {} conditionals, {} retries)",
        program.nodes.len(),
        program.nodes.iter().filter(|n| n.capability.is_some()).count(),
        program.nodes.iter().filter(|n| matches!(n.kind, acos_core::types::CirNodeKind::LoopMap)).count(),
        program.nodes.iter().filter(|n| matches!(n.kind, acos_core::types::CirNodeKind::Conditional)).count(),
        program.nodes.iter().filter(|n| n.control.as_ref().and_then(|c| c.retry.as_ref()).is_some()).count(),
    );

    // 3. Contract (R1-R5) — already enforced inside compile_plan; assert here.
    acos_compiler::validate_cir(&program)?;
    println!("[3] contract (R1-R5): OK");

    // 4. Execution.
    let event_store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
    let artifact_store: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
    let runtime = RuntimeImpl::new(event_store.clone(), artifact_store.clone());
    let report = runtime.execute(program.clone()).await?;
    println!("[4] execution: OK ({} artifacts)", report.artifacts.len());
    let artifact_content = if !report.artifacts.is_empty() {
        artifact_store.get_by_name(report.run_id, &report.artifacts[0]).await.ok()
    } else {
        None
    };
    if artifact_content.is_none() {
        return Err("no artifact produced".into());
    }
    let text = String::from_utf8_lossy(artifact_content.as_deref().unwrap_or(&[]));
    println!("  artifact: {} bytes", text.len());
    for line in text.lines().take(12) {
        println!("    | {line}");
    }

    // 5. Verification (structural + semantic + evidence).
    let verification = verify_run_full(&*event_store, artifact_content, report.run_id, &ground_truth).await?;
    let mut ok = true;
    for layer in [acos_verify::VerificationLayer::Structural, acos_verify::VerificationLayer::Semantic, acos_verify::VerificationLayer::Evidence] {
        let finds = verification.layer_findings(layer.clone());
        let passed = !finds.is_empty() && finds.iter().all(|f| f.passed);
        if !passed {
            ok = false;
        }
        let label = match layer {
            acos_verify::VerificationLayer::Structural => "structural",
            acos_verify::VerificationLayer::Semantic => "semantic",
            acos_verify::VerificationLayer::Evidence => "evidence",
        };
        println!(
            "[5] verification {label}: {} ({}/{})",
            if passed { "PASS" } else { "FAIL" },
            finds.iter().filter(|f| f.passed).count(),
            finds.len()
        );
        for f in finds.iter().filter(|f| !f.passed) {
            println!("      ✗ {}", f.message);
        }
    }
    if !ok {
        return Err("verification failed".into());
    }

    // 6. Report file check on disk (write_file side effect).
    let disk_path = PathBuf::from("p1_flagship_report.md");
    if disk_path.exists() {
        println!("[6] report artifact on disk: {}", disk_path.display());
        let _ = std::fs::remove_file(&disk_path);
    }

    println!("\nP1-5B v0.2 pipeline smoke: ALL PASSED");
    Ok(())
}