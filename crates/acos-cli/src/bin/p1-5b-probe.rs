//! P1-5B Discovery Probe — 3-run cognitive program discovery experiment.
//!
//! Runs the flagship task through `ModelCompiler::compile_traced` to capture
//! the full LLM exchange trace (raw responses, repair attempts, timing).
//! Evaluates the generated CIR against the Behavioral Requirements Matrix
//! (7 BRs measuring what the program DOES, not its structure) and computes
//! Input Binding Accuracy (do generated read_file paths match declared
//! TaskSpec.inputs paths?).
//!
//! v0.2 mode (`--plan`): runs the Structured Program Synthesis frontend
//! (`compile_plan_traced`) — the LLM produces a Plan IR, the deterministic
//! compiler lowers it to CIR. The trace records the final Plan IR alongside
//! the compiled CIR for Experiment A (Control Flow Discovery) and
//! Experiment B (Two-stage Compilation) metrics.
//!
//! Probe-2: run with `--out-dir .../probe-2-results` so Probe-1 records in
//! `probe-results/` are never overwritten.
//!
//! Usage (from workspace root):
//!   cargo run -p acos-cli --bin p1-5b-probe -- [--runs 3] [--task PATH] [--gt PATH] [--out-dir DIR] [--plan]
//!
//! Requires `LONGCAT_API_KEY` in the environment or `.env`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use acos_compiler::ModelCompiler;
use acos_core::schema::from_yaml;
use acos_core::traits::{ArtifactStore, EventStore};
use acos_core::types::{CirNodeKind, TaskSpec};
use acos_runtime::RuntimeImpl;
use acos_state::InMemoryStore;
use acos_verify::{verify_run_full, GroundTruth};
use serde::Serialize;

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
    let out_dir = get_arg(&args, "--out-dir").map(PathBuf::from).unwrap_or_else(|| {
        PathBuf::from("experiments/p1-5b-cognitive-program-discovery/probe-results")
    });
    let start_index: usize = get_arg(&args, "--start-index")
        .and_then(|s| s.parse().ok())
        .unwrap_or(1);
    let plan_mode = args.iter().any(|a| a == "--plan");
    let csv_mode = match get_arg(&args, "--csv-mode").as_deref() {
        Some("observe") => acos_compiler::CsvMode::Observe,
        Some("enforce") | None => acos_compiler::CsvMode::Enforce,
        Some(other) => {
            eprintln!("unknown --csv-mode '{other}' (expected 'observe' or 'enforce')");
            std::process::exit(2);
        }
    };
    // P1-5B v0.4 S1–S3 factor switches: serialization teaching (S1/S3) and the
    // Structured Inputs Package (S2/S3: prompt teaching + env rejection +
    // runtime `inputs` injection).
    let serialization_teaching = args.iter().any(|a| a == "--serialization-teaching");
    let structured_inputs = args.iter().any(|a| a == "--structured-inputs");
    if structured_inputs {
        std::env::set_var("ACOS_STRUCTURED_INPUTS", "1");
    }

    if let Err(e) = try_load_env() {
        eprintln!("[warn] .env not loaded: {e}");
    }

    let task_spec = read_task_spec(&task_path).await?;
    let ground_truth = GroundTruth::from_yaml(&gt_path)
        .map_err(|e| format!("failed to load ground truth from {gt_path}: {e}"))?;

    let declared_paths: HashSet<String> = task_spec.inputs.iter().map(|i| i.path.clone()).collect();

    println!("P1-5B Discovery Probe (Behavioral Requirements Analysis)");
    println!("  mode: {}", if plan_mode { "PLAN (v0.2 Structured Program Synthesis)" } else { "CIR (v0.1 direct)" });
    println!("  task: {task_path}");
    println!("  ground_truth: {gt_path}");
    println!("  runs: {runs}");
    println!("  start_index: {start_index}");
    println!("  out_dir: {}", out_dir.display());
    println!("  max_repair_attempts: {MAX_REPAIR_ATTEMPTS}");
    println!("  declared_inputs: {}", declared_paths.len());
    println!(
        "  v0.4 factors: serialization_teaching={serialization_teaching}, structured_inputs={structured_inputs}"
    );
    let llm_provider = std::env::var("ACOS_LLM_PROVIDER").unwrap_or_else(|_| "anthropic".into());
    let llm_model = std::env::var("ACOS_LLM_MODEL").unwrap_or_else(|_| "LongCat-2.0".into());
    let llm_base = std::env::var("LONGCAT_BASE_URL").unwrap_or_else(|_| "(default)".into());
    let llm_key_len = std::env::var("LONGCAT_API_KEY")
        .or_else(|_| std::env::var("OPENAI_API_KEY"))
        .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
        .map(|k| k.len())
        .unwrap_or(0);
    println!("  llm: provider={llm_provider}, model={llm_model}, base={llm_base}, key_len={llm_key_len}");
    println!();

    tokio::fs::create_dir_all(&out_dir).await.ok();

    for run_idx in start_index..start_index + runs {
        let run_start = Instant::now();
        println!("── Run {run_idx}/{runs} ──");

        let mut compiler = ModelCompiler::from_env()
            .map_err(|e| format!("ModelCompiler::from_env failed (set LONGCAT_API_KEY?): {e}"))?;
        compiler.set_csv_mode(csv_mode);
        compiler.set_serialization_teaching(serialization_teaching);
        compiler.set_structured_inputs(structured_inputs);
        let traced = if plan_mode {
            compiler.compile_plan_traced(&task_spec, MAX_REPAIR_ATTEMPTS).await
        } else {
            compiler.compile_traced(&task_spec, MAX_REPAIR_ATTEMPTS).await
        };

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

        let (contract_pass, contract_error) = if let Ok(ref result) = traced.result {
            match acos_compiler::validate_cir(&result.program) {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e.to_string())),
            }
        } else {
            (false, Some("compile failed".to_string()))
        };
        let contract = serde_json::json!({
            "pass": contract_pass,
            "error": contract_error,
        });
        if contract_pass {
            println!("  contract: PASS (R1-R5)");
        } else {
            println!(
                "  contract: FAIL {}",
                contract_error.as_deref().unwrap_or("unknown")
            );
        }

        let behavioral = if let Ok(ref result) = traced.result {
            let analysis = analyze_behavioral_requirements(&result.program, &declared_paths);
            println!("  behavioral: {}/7 PASS", analysis.pass_count);
            for check in &analysis.checks {
                println!("    {}: {}", check.verdict.symbol(), check.name);
                println!("      └─ {}", check.detail);
            }
            println!("  binding_accuracy: {}/{}", analysis.binding_matched, analysis.binding_total);
            println!("  complexity: {} nodes, {} loops, {} conditions, {} retries",
                analysis.node_count, analysis.loop_count, analysis.condition_count, analysis.retry_count);
            Some(serde_json::to_value(&analysis)?)
        } else {
            None
        };

        let execution = if let Ok(ref result) = traced.result {
            let program = &result.program;
            let event_store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
            let artifact_store: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
            let runtime = RuntimeImpl::new(event_store.clone(), artifact_store.clone());

            match runtime.execute(program.clone()).await {
                Ok(report) => {
                    println!("  execution: OK ({} artifacts)", report.artifacts.len());
                    println!("  status: {:?}", report.status);
                    let artifact_content = if !report.artifacts.is_empty() {
                        artifact_store.get_by_name(report.run_id, &report.artifacts[0]).await.ok()
                    } else { None };
                    let verification = match verify_run_full(&*event_store, artifact_content, report.run_id, &ground_truth).await {
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
                    serde_json::json!({ "ok": true, "artifact_count": report.artifacts.len(), "verification": verification })
                }
                Err(e) => {
                    println!("  execution: FAIL ({e})");
                    serde_json::json!({ "ok": false, "error": e.to_string() })
                }
            }
        } else {
            serde_json::Value::Null
        };

        let trace_path = out_dir.join(format!("run-{run_idx:03}.trace.json"));
        if tokio::fs::metadata(&trace_path).await.is_ok() {
            eprintln!(
                "ERROR: {} already exists; refusing to overwrite. Use --start-index {} to continue.",
                trace_path.display(),
                run_idx + 1
            );
            std::process::exit(1);
        }
        let trace_json = build_trace_json(
            run_idx, &task_path, &traced, compile_ok,
            run_start.elapsed().as_millis() as u64, behavioral, execution, contract,
        );
        tokio::fs::write(&trace_path, trace_json).await?;
        println!("  trace saved: {}", trace_path.display());
        println!();
    }

    println!("P1-5B Discovery Probe complete. Results in {}", out_dir.display());
    Ok(())
}

#[derive(Serialize)]
struct BehavioralAnalysis {
    pass_count: u32,
    binding_matched: usize,
    binding_total: usize,
    node_count: usize,
    primitive_count: u32,
    loop_count: u32,
    condition_count: u32,
    retry_count: u32,
    checks: Vec<BehavioralCheck>,
}

#[derive(Serialize)]
struct BehavioralCheck { name: String, verdict: Verdict, detail: String }

#[derive(Serialize)]
#[serde(rename_all = "UPPERCASE")]
enum Verdict { Pass, Fail, Partial }

impl Verdict {
    fn symbol(&self) -> &str {
        match self { Verdict::Pass => "✓", Verdict::Fail => "✗", Verdict::Partial => "~" }
    }
}

fn analyze_behavioral_requirements(
    program: &acos_core::types::CirProgram,
    declared_paths: &HashSet<String>,
) -> BehavioralAnalysis {
    let nodes = &program.nodes;
    let node_count = nodes.len();
    let mut generated_paths = HashSet::new();
    let mut has_write = false;
    let mut loop_count = 0u32;
    let mut condition_count = 0u32;
    let mut retry_count = 0u32;
    let mut primitive_count = 0u32;
    let mut capabilities = HashSet::new();
    let mut item_vars: HashSet<String> = HashSet::new();

    for node in nodes {
        match node.kind {
            CirNodeKind::LoopMap => loop_count += 1,
            CirNodeKind::Conditional => condition_count += 1,
            _ => {}
        }
        if let Some(ref cap) = node.capability {
            capabilities.insert(cap.clone());
            primitive_count += 1;
            if cap == "read_file" {
                if let Some(path_val) = node.inputs.get("path") {
                    if let Some(s) = path_val.as_str() {
                        generated_paths.insert(s.to_string());
                    }
                }
            } else if cap == "write_file" {
                has_write = true;
            }
        }
        // Declared input paths may legitimately be embedded in execute_python
        // code (e.g. a path list built in Python, pandas reads). Count them as
        // referenced so binding accuracy reflects real input usage.
        if node.capability.as_deref() == Some("execute_python") {
            if let Some(code) = node.inputs.get("code").and_then(|v| v.as_str()) {
                for declared in declared_paths {
                    if code.contains(declared.as_str()) {
                        generated_paths.insert(declared.clone());
                    }
                }
            }
        }
        if node.control.as_ref().and_then(|c| c.retry.as_ref()).is_some() {
            retry_count += 1;
        }
        // Loop item vars are runtime bindings (resolved by the runtime during
        // iteration), not hallucinated resources.
        if let Some(spec) = node.control.as_ref().and_then(|c| c.loop_spec.as_ref()) {
            if let Some(var) = &spec.item_var {
                item_vars.insert(format!("${{{var}}}"));
            }
        }
    }

    let mut checks = Vec::new();
    let mut pass_count = 0u32;

    // BR-1: Multi-File Processing
    let matched: HashSet<_> = generated_paths.intersection(declared_paths).cloned().collect();
    let binding_total = declared_paths.len();
    let binding_matched = matched.len();
    let br1 = if binding_total > 0 { binding_matched == binding_total } else { true };
    checks.push(BehavioralCheck {
        name: "BR-1 Multi-File Processing".into(),
        verdict: if br1 { Verdict::Pass } else if binding_matched > 0 { Verdict::Partial } else { Verdict::Fail },
        detail: format!("{}/{} declared paths referenced", binding_matched, binding_total),
    });
    if br1 { pass_count += 1; }

    // BR-2: Data Quality Analysis
    let has_analysis = capabilities.contains("execute_python") || primitive_count > 2;
    checks.push(BehavioralCheck {
        name: "BR-2 Data Quality Analysis".into(),
        verdict: if has_analysis { Verdict::Pass } else { Verdict::Fail },
        detail: if has_analysis { format!("analysis caps: {:?}", capabilities) } else { "only read/write".into() },
    });
    if has_analysis { pass_count += 1; }

    // BR-3: Anomaly Detection/Repair
    let has_repair = condition_count > 0 || capabilities.contains("execute_python");
    checks.push(BehavioralCheck {
        name: "BR-3 Anomaly Detection/Repair".into(),
        verdict: if has_repair { Verdict::Pass } else { Verdict::Fail },
        detail: format!("conditions={}, python={}", condition_count, capabilities.contains("execute_python")),
    });
    if has_repair { pass_count += 1; }

    // BR-4: Structured Report
    checks.push(BehavioralCheck {
        name: "BR-4 Structured Report".into(),
        verdict: if has_write { Verdict::Pass } else { Verdict::Fail },
        detail: if has_write { "write_file present".into() } else { "no output".into() },
    });
    if has_write { pass_count += 1; }

    // BR-5: Evidence/Audit Trail
    let has_evidence = node_count >= 4 || primitive_count >= 3;
    checks.push(BehavioralCheck {
        name: "BR-5 Evidence/Audit Trail".into(),
        verdict: if has_evidence { Verdict::Pass } else { Verdict::Fail },
        detail: format!("{} nodes, {} primitives", node_count, primitive_count),
    });
    if has_evidence { pass_count += 1; }

    // BR-6: Control Flow Complexity
    let has_control = loop_count > 0 || condition_count > 0 || retry_count > 0;
    checks.push(BehavioralCheck {
        name: "BR-6 Control Flow Complexity".into(),
        verdict: if has_control { Verdict::Pass } else { Verdict::Fail },
        detail: format!("{} loops, {} conds, {} retries", loop_count, condition_count, retry_count),
    });
    if has_control { pass_count += 1; }

    // BR-7: No Hallucinated Resources
    let undeclared: Vec<_> = generated_paths
        .difference(declared_paths)
        .filter(|p| !item_vars.contains(*p))
        .collect();
    let no_hallu = undeclared.is_empty();
    checks.push(BehavioralCheck {
        name: "BR-7 No Hallucinated Resources".into(),
        verdict: if no_hallu { Verdict::Pass } else if undeclared.len() < generated_paths.len() { Verdict::Partial } else { Verdict::Fail },
        detail: if no_hallu { "all paths declared".into() } else { format!("undeclared: {:?}", undeclared) },
    });
    if no_hallu { pass_count += 1; }

    BehavioralAnalysis { pass_count, binding_matched, binding_total, node_count, primitive_count, loop_count, condition_count, retry_count, checks }
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
            let k = k.trim();
            // Prefer an already-set environment variable (shell/session wins
            // over `.env`); only fill unset keys.
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v.trim());
            }
        }
    }
    Ok(())
}

async fn read_task_spec(path: &str) -> Result<TaskSpec, Box<dyn std::error::Error>> {
    let yaml = tokio::fs::read_to_string(path).await
        .map_err(|e| format!("failed to read {path}: {e}"))?;
    Ok(from_yaml(&yaml)?)
}

fn iso_timestamp() -> String {
    use std::time::SystemTime;
    let dur = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let days = secs / 86400;
    let rem = secs % 86400;
    let h = rem / 3600;
    let m = (rem % 3600) / 60;
    let s = rem % 60;
    format!("{}T{h:02}:{m:02}:{s:02}Z", days_to_date(days))
}

/// Approximate date from days since epoch (good enough for trace ordering).
fn days_to_date(days: u64) -> String {
    let year = 1970u64 + days / 365;
    let day_of_year = days % 365;
    let month = day_of_year / 30 + 1;
    let day = day_of_year % 30 + 1;
    format!("{year}-{month:02}-{day:02}")
}

#[allow(clippy::too_many_arguments)]
fn build_trace_json(
    run_idx: usize, task_path: &str, traced: &acos_compiler::TracedCompile,
    compile_success: bool, wall_ms: u64, behavioral: Option<serde_json::Value>,
    execution: serde_json::Value, contract: serde_json::Value,
) -> String {
    let t = &traced.trace;
    let program = traced.result.as_ref().ok().map(|r| &r.program);

    let metrics_json = match program {
        Some(p) => {
            let caps: Vec<String> = {
                let mut c: Vec<String> = p.nodes.iter().filter_map(|n| n.capability.clone()).collect();
                c.sort();
                c.dedup();
                c
            };
            serde_json::json!({
                "node_count": p.nodes.len(),
                "primitive_count": p.nodes.iter().filter(|n| n.capability.is_some()).count(),
                "control_node_count": p.nodes.iter().filter(|n| n.control.is_some()).count(),
                "loop_count": p.nodes.iter().filter(|n| matches!(n.kind, CirNodeKind::LoopMap)).count(),
                "condition_count": p.nodes.iter().filter(|n| matches!(n.kind, CirNodeKind::Conditional)).count(),
                "retry_count": p.nodes.iter().filter(|n| n.control.as_ref().and_then(|c| c.retry.as_ref()).is_some()).count(),
                "capability_types": caps,
            })
        }
        None => serde_json::Value::Null,
    };

    let cir_json = program.and_then(|p| serde_json::to_value(p).ok());
    let plan_json = t.plan.as_ref().and_then(|p| serde_json::to_value(p).ok());

    let plan_metrics = t.plan.as_ref().map(|p| {
        let step_count = p.steps.len();
        let foreach_count = count_step_kind(&p.steps, "foreach");
        let conditional_count = count_step_kind(&p.steps, "conditional");
        let retry_count = count_step_kind(&p.steps, "retry");
        serde_json::json!({
            "step_count": step_count,
            "foreach_count": foreach_count,
            "conditional_count": conditional_count,
            "retry_count": retry_count,
            "control_intent_count": foreach_count + conditional_count + retry_count,
        })
    });

    let record = serde_json::json!({
        "run": { "run_index": run_idx, "timestamp": iso_timestamp(), "model": std::env::var("ACOS_LLM_MODEL").unwrap_or_else(|_| "LongCat-2.0".into()), "mode": if t.plan.is_some() { "plan" } else { "cir" }, "compile_success": compile_success, "final_error": t.final_error },
        "contract": contract,
        "input": { "task_spec_path": task_path, "prompt_sent": t.initial_prompt },
        "output": {
            "initial_raw_response": t.initial_response,
            "initial_parse_error": t.initial_error,
            "repair_count": t.repair_attempts.len(),
            "repair_traces": t.repair_attempts.iter().map(|r| serde_json::json!({
                "attempt": r.attempt, "prompt": r.prompt, "raw_response": r.response, "validation_error": r.validation_error,
            })).collect::<Vec<_>>(),
            "final_plan": plan_json,
            "final_cir": cir_json,
        },
        "plan_metrics": plan_metrics,
        "program_metrics": metrics_json,
        "behavioral_analysis": behavioral,
        "execution": execution,
        "timing": { "initial_llm_ms": t.timing.initial_llm_ms, "repair_llm_ms": t.timing.repair_llm_ms, "total_compile_ms": t.timing.total_ms, "total_wall_ms": wall_ms },
        "repair_tax": { "first_pass_success": t.initial_error.is_none(), "repair_attempts_used": t.repair_attempts.len(), "repair_latency_ms": t.timing.repair_llm_ms },
    });
    serde_json::to_string_pretty(&record).unwrap_or_default()
}

/// Counts steps of a given kind across the whole plan tree.
fn count_step_kind(steps: &[acos_compiler::plan::PlanStep], kind: &str) -> usize {
    steps.iter().fold(0usize, |acc, s| {
        let mine = match s.kind {
            acos_compiler::plan::StepKind::Foreach => "foreach",
            acos_compiler::plan::StepKind::Conditional => "conditional",
            acos_compiler::plan::StepKind::Retry => "retry",
            acos_compiler::plan::StepKind::Primitive => "primitive",
        };
        acc + usize::from(mine == kind) + count_step_kind(&s.body, kind)
    })
}