//! P1-5B Trace Replay — execute + verify saved probe CIRs without recompiling.
//!
//! Loads each `run-NNN.trace.json` from a probe results dir, deserializes the
//! saved `final_cir`, executes it on the runtime, verifies against ground
//! truth, and patches the trace file with the execution result.
//!
//! Usage (from workspace root):
//!   cargo run -p acos-cli --bin p1-5b-replay -- [--dir DIR] [--gt PATH]

use std::path::PathBuf;
use std::sync::Arc;

use acos_core::traits::{ArtifactStore, EventStore};
use acos_core::types::CirProgram;
use acos_runtime::RuntimeImpl;
use acos_state::InMemoryStore;
use acos_verify::{verify_run_full, GroundTruth};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let dir = get_arg(&args, "--dir").unwrap_or_else(|| {
        "experiments/p1-5b-cognitive-program-discovery/probe-2c-results".into()
    });
    let gt_path = get_arg(&args, "--gt")
        .unwrap_or_else(|| "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml".into());

    if let Err(e) = try_load_env() {
        eprintln!("[warn] .env not loaded: {e}");
    }

    let ground_truth = GroundTruth::from_yaml(&gt_path)
        .map_err(|e| format!("failed to load ground truth from {gt_path}: {e}"))?;

    let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("run-") && n.ends_with(".trace.json"))
                .unwrap_or(false)
        })
        .collect();
    files.sort();

    println!("P1-5B Trace Replay");
    println!("  dir: {dir}");
    println!("  ground_truth: {gt_path}");
    println!("  traces: {}", files.len());
    println!();

    for path in &files {
        let name = path.file_name().unwrap().to_string_lossy();
        println!("── {name} ──");
        let raw = std::fs::read_to_string(path)?;
        let mut trace: serde_json::Value = serde_json::from_str(&raw)?;

        let Some(cir) = trace
            .pointer("/output/final_cir")
            .and_then(|v| serde_json::from_value::<CirProgram>(v.clone()).ok())
        else {
            println!("  no final_cir — skipping");
            continue;
        };

        let event_store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let artifact_store: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(event_store.clone(), artifact_store.clone());

        let execution = match runtime.execute(cir.clone()).await {
            Ok(report) => {
                println!("  execution: OK ({} artifacts)", report.artifacts.len());
                let artifact_content = if !report.artifacts.is_empty() {
                    artifact_store
                        .get_by_name(report.run_id, &report.artifacts[0])
                        .await
                        .ok()
                } else {
                    None
                };
                let preview = artifact_content
                    .as_deref()
                    .map(|c| {
                        let s = String::from_utf8_lossy(c);
                        let truncated: String = s.chars().take(4000).collect();
                        if truncated.len() < s.len() { format!("{truncated}…[truncated]") } else { truncated }
                    });
                let verification =
                    match verify_run_full(&*event_store, artifact_content, report.run_id, &ground_truth)
                        .await
                    {
                        Ok(v) => {
                            let passed = v.all_passed();
                            println!("  verification: {}", if passed { "PASSED" } else { "FAILED" });
                            serde_json::json!({ "ok": true, "passed": passed })
                        }
                        Err(e) => {
                            eprintln!("  verification error: {e}");
                            serde_json::json!({ "ok": false, "error": e.to_string() })
                        }
                    };
                serde_json::json!({ "ok": true, "artifact_count": report.artifacts.len(), "artifact_preview": preview, "verification": verification })
            }
            Err(e) => {
                println!("  execution: FAIL ({e})");
                serde_json::json!({ "ok": false, "error": e.to_string() })
            }
        };

        trace["execution"] = execution;
        std::fs::write(path, serde_json::to_string_pretty(&trace)?)?;
        println!();
    }

    println!("Replay complete.");
    Ok(())
}

fn get_arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}

fn try_load_env() -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(".env")?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') { continue; }
        if let Some((k, v)) = line.split_once('=') {
            std::env::set_var(k.trim(), v.trim());
        }
    }
    Ok(())
}