//! P1-5B Discovery Probe — 3-run cognitive program discovery experiment.
//!
//! Runs the flagship task through `ModelCompiler::compile_traced` to capture
//! the full LLM exchange trace (raw responses, repair attempts, timing).
//! Saves each run's trace to `experiments/p1-5b-cognitive-program-discovery/`.
//!
//! Usage (from workspace root):
//!   cargo run -p acos-cli --bin p1-5b-probe -- [--runs 3] [--task PATH] [--gt PATH]
//!
//! Requires `LONGCAT_API_KEY` in the environment or `.env`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use acos_compiler::ModelCompiler;
use acos_core::schema::from_yaml;
use acos_core::traits::{ArtifactStore, EventStore};
use acos_core::types::TaskSpec;
use acos_runtime::RuntimeImpl;
use acos_state::InMemoryStore;
use acos_verify::{GroundTruth, verify_run_full};

const MAX_REPAIR_ATTEMPTS: u32 = 3;
const DEFAULT_RUNS: usize = 3;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let task_path = get_arg(&args, "--task")
        .unwrap_or_else(|| "tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml".into());
    let gt_path = get_arg(&args, "--gt")
        .unwrap_or_else(|| "tests/benchmarks/p1/flagship_csv_quality/expected/ground_truth.yaml".into());
    let runs: usize = get_arg(&args, "--runs")
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_RUNS);

    // Load .env if present.
    if let Err(e) = try_load_env() {
        eprintln!("[warn] .env not loaded: {e}");
    }

    let task_spec = read_task_spec(&task_path).await?;
    let ground_truth = GroundTruth::from_yaml(&gt_path)
        .map_err(|e| format!("failed to load ground truth from {gt_path}: {e}"))?;

    println!("P1-5B Discovery Probe");
    println!("  task: {task_path}");
    println!("  ground_truth: {gt_path}");
    println!("  runs: {runs}");
    println!("  max_repair_attempts: {MAX_REPAIR_ATTEMPTS}");
    println!();

    let out_dir = PathBuf::from("experiments/p1-5b-cognitive-program-discovery/probe-results");
    tokio::fs::create_dir_all(&out_dir).await.ok();

    for run_idx in 1..=runs {
        let run_start = Instant::now();
        println!("── Run {run_idx}/{runs} ──");

        let compiler = ModelCompiler::from_env()
            .map_err(|e| format!("ModelCompiler::from_env failed (set LONGCAT_API_KEY?): {e}"))?;
        let traced = compiler.compile_traced(&task_spec, MAX_REPAIR_ATTEMPTS).await;

        let compile_ok = traced.result.is_ok();

        println!(
            "  compile: {} ({}ms, {} repair attempts)",
            if compile_ok { "OK" } else { "FAIL" },
            traced.trace.timing.total_ms,
            traced.trace.repair_attempts.len(),
        );

        if let Some(ref err) = traced.trace.final_error {
            println!("  final error: {err}");
        }

        // Save the trace as JSON
        let trace_path = out_dir.join(format!("run-{run_idx:03}.trace.json"));
        let trace_json = build_trace_json(
            run_idx,
            &task_path,
            &traced,
            compile_ok,
            run_start.elapsed().as_millis() as u64,
        );
        tokio::fs::write(&trace_path, trace_json).await?;
        println!("  trace saved: {}", trace_path.display());

        // Execution attempt (only if compile succeeded)
        if let Ok(result) = &traced.result {
            let program = &result.program;
            let exec_start = Instant::now();
            let event_store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
            let artifact_store: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
            let runtime = RuntimeImpl::new(event_store.clone(), artifact_store.clone());

            match runtime.execute(program.clone()).await {
                Ok(report) => {
                    let exec_ms = exec_start.elapsed().as_millis() as u64;
                    println!(
                        "  execution: OK (run_id={}, {} artifacts, {}ms)",
                        report.run_id.0,
                        report.artifacts.len(),
                        exec_ms
                    );
                    println!("  status: {:?}", report.status);

                    // Verification against ground truth
                    let artifact_content = if !report.artifacts.is_empty() {
                        artifact_store
                            .get_by_name(report.run_id, &report.artifacts[0])
                            .await
                            .ok()
                    } else {
                        None
                    };
                    match verify_run_full(
                        &*event_store,
                        artifact_content,
                        report.run_id,
                        &ground_truth,
                    )
                    .await
                    {
                        Ok(verification) => {
                            println!(
                                "  verification: {}",
                                if verification.all_passed() {
                                    "PASSED"
                                } else {
                                    "FAILED"
                                }
                            );
                        }
                        Err(e) => {
                            eprintln!("  verification error: {e}");
                        }
                    }
                }
                Err(e) => {
                    println!("  execution: FAIL ({e})");
                }
            }
        }

        println!();
    }

    println!(
        "P1-5B Discovery Probe complete. Results in {}",
        out_dir.display()
    );
    Ok(())
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn get_arg(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|a| a == key)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn try_load_env() -> Result<(), Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(".env")?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            std::env::set_var(k.trim(), v.trim());
        }
    }
    Ok(())
}

async fn read_task_spec(path: &str) -> Result<TaskSpec, Box<dyn std::error::Error>> {
    let yaml = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| format!("failed to read {path}: {e}"))?;
    Ok(from_yaml(&yaml)?)
}

/// Builds the full trace JSON for a single run.
fn build_trace_json(
    run_idx: usize,
    task_path: &str,
    traced: &acos_compiler::TracedCompile,
    compile_success: bool,
    wall_ms: u64,
) -> String {
    let t = &traced.trace;
    let program = traced.result.as_ref().ok().map(|r| &r.program);

    // Program metrics (only if compile succeeded)
    let metrics_json = match program {
        Some(p) => {
            let caps: Vec<String> = {
                let mut c: Vec<String> =
                    p.nodes.iter().filter_map(|n| n.capability.clone()).collect();
                c.sort();
                c.dedup();
                c
            };
            serde_json::json!({
                "node_count": p.nodes.len(),
                "primitive_count": p.nodes.iter().filter(|n| n.capability.is_some()).count(),
                "control_node_count": p.nodes.iter().filter(|n| n.control.is_some()).count(),
                "loop_count": p.nodes.iter().filter(|n| matches!(n.kind, acos_core::types::CirNodeKind::LoopMap)).count(),
                "condition_count": p.nodes.iter().filter(|n| matches!(n.kind, acos_core::types::CirNodeKind::Conditional)).count(),
                "retry_count": p.nodes.iter().filter(|n| n.control.as_ref().and_then(|c| c.retry.as_ref()).is_some()).count(),
                "capability_types": caps,
            })
        }
        None => serde_json::Value::Null,
    };

    // Final CIR (only if compile succeeded)
    let cir_json = match program {
        Some(p) => serde_json::to_value(p).ok(),
        None => None,
    };

    let record = serde_json::json!({
        "run": {
            "run_index": run_idx,
            "timestamp": iso_timestamp(),
            "model": "LongCat-2.0",
            "compile_success": compile_success,
            "final_error": t.final_error,
        },
        "input": {
            "task_spec_path": task_path,
            "prompt_sent": t.initial_prompt,
        },
        "output": {
            "initial_raw_response": t.initial_response,
            "initial_parse_error": t.initial_error,
            "repair_count": t.repair_attempts.len(),
            "repair_traces": t.repair_attempts.iter().map(|r| {
                serde_json::json!({
                    "attempt": r.attempt,
                    "prompt": r.prompt,
                    "raw_response": r.response,
                    "validation_error": r.validation_error,
                })
            }).collect::<Vec<_>>(),
            "final_cir": cir_json,
        },
        "program_metrics": metrics_json,
        "timing": {
            "initial_llm_ms": t.timing.initial_llm_ms,
            "repair_llm_ms": t.timing.repair_llm_ms,
            "total_compile_ms": t.timing.total_ms,
            "total_wall_ms": wall_ms,
        },
        "repair_tax": {
            "first_pass_success": t.initial_error.is_none(),
            "repair_attempts_used": t.repair_attempts.len(),
            "repair_latency_ms": t.timing.repair_llm_ms,
        },
    });

    serde_json::to_string_pretty(&record).unwrap_or_default()
}

/// Simple ISO-8601 timestamp (no chrono dependency needed).
fn iso_timestamp() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    // RFC 3339 format: YYYY-MM-DDTHH:MM:SSZ (approximate, good enough for trace)
    let secs = dur.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    // Approximate date from days since epoch (good enough for trace ordering)
    format!("{}T{h:02}:{m:02}:{s:02}Z", days_to_date(days))
}

/// Converts days since UNIX epoch to an approximate date string.
fn days_to_date(days: u64) -> String {
    // 1970-01-01 + days. This is approximate but good enough for trace files.
    let year = 1970u64 + days / 365;
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{year}-{month:02}-{day:02}")
}
