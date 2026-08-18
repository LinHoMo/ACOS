//! ACOS command-line interface.
//!
//! MVP commands:
//!   acos compile <task.yaml>   — compile a task spec to CIR and print it
//!   acos run <task.yaml>       — compile and execute a task
//!   acos run-cir <cir.json>    — execute a pre-compiled CIR program directly
//!
//! Planner selection:
//!   - Default: **ModelCompiler** (Claude via LongCat). Set `LONGCAT_API_KEY`
//!     (or `ANTHROPIC_API_KEY`) to enable. Configure the model with
//!     `ACOS_LLM_MODEL` (default `claude-sonnet-4-5-20250929`).
//!   - Fallback: `--rules` flag forces the deterministic `RuleCompiler`.
//!
//! Benchmark harness:
//!   acos bench [--suite S] [--case C] [--require-model] [--fixtures DIR]
//!     Runs the fixtures-as-contracts regression suite (see `acos_bench`).

use acos_bench::{run, BenchArgs};
use acos_compiler::{ModelCompiler, RuleCompiler};
use acos_core::error::AcosError;
use acos_core::schema::from_yaml;
use acos_core::traits::Compiler;
use acos_core::types::{CirProgram, TaskSpec, TypedValue, ValueType};
use acos_runtime::Runtime;
use acos_state::InMemoryStore;
use acos_verify::{verify_run, verify_run_full, GroundTruth};
use std::collections::HashMap;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), AcosError> {
    let args: Vec<String> = std::env::args().collect();
    let use_rules = args.iter().any(|a| a == "--rules");
    let positional: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).map(String::as_str).collect();

    match positional.get(1).copied() {
        Some("bench") => {
            // acos bench [--suite S] [--case C] [--require-model] [--fixtures DIR]
            let mut suite: Option<String> = None;
            let mut case: Option<String> = None;
            let mut require_model = false;
            let mut fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            fixtures_dir.push("../acos-bench/fixtures");
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--suite" => {
                        suite = args.get(i + 1).cloned();
                        i += 1;
                    }
                    "--case" => {
                        case = args.get(i + 1).cloned();
                        i += 1;
                    }
                    "--fixtures" => {
                        if let Some(p) = args.get(i + 1) {
                            fixtures_dir = PathBuf::from(p);
                        }
                        i += 1;
                    }
                    "--require-model" => require_model = true,
                    other => {
                        eprintln!(
                            "usage: acos bench [--suite S] [--case C] [--require-model] [--fixtures DIR]"
                        );
                        eprintln!("unknown argument: {other}");
                        std::process::exit(2);
                    }
                }
                i += 1;
            }
            let report = run(BenchArgs {
                fixtures_dir,
                suite,
                case,
                require_model,
            })
            .await;
            report.print();
            std::process::exit(if report.failed() == 0 { 0 } else { 1 });
        }
        Some("run-cir") => {
            // acos run-cir <cir.json> [--env <env.json>] [--verify <ground_truth.yaml>]
            let path = positional.get(2).expect("usage: acos run-cir <cir.json> [--env <env.json>] [--verify <ground_truth.yaml>]");
            let env_path = match args.iter().position(|a| a == "--env") {
                Some(idx) => args.get(idx + 1).map(|s| s.as_str()),
                None => None,
            };
            let verify_path = match args.iter().position(|a| a == "--verify") {
                Some(idx) => args.get(idx + 1).map(|s| s.as_str()),
                None => None,
            };
            run_cir(path, env_path, verify_path).await
        }
        Some("compile") => {
            let path = positional.get(2).expect("usage: acos compile <task.yaml>");
            let task = read_task(path).await?;
            let result = compile(&task, use_rules).await?;
            println!(
                "Compiled program {} ({} nodes, {} diagnostics)",
                result.program.id.0,
                result.program.nodes.len(),
                result.diagnostics.len()
            );
            for d in &result.diagnostics {
                println!("  [{:?}] {}", d.level, d.message);
            }
            println!("{}", serde_json::to_string_pretty(&result.program).unwrap_or_default());
            Ok(())
        }
        Some("run") => {
            let path = positional.get(2).expect("usage: acos run <task.yaml>");
            let task = read_task(path).await?;
            let result = compile(&task, use_rules).await?;
            let event_store: std::sync::Arc<dyn acos_core::traits::EventStore + Send + Sync> =
                std::sync::Arc::new(InMemoryStore::new());
            let artifact_store: std::sync::Arc<dyn acos_core::traits::ArtifactStore + Send + Sync> =
                std::sync::Arc::new(InMemoryStore::new());
            let runtime = Runtime::new(event_store.clone(), artifact_store);
            let report = runtime.execute(result.program).await?;

            println!("Run {}: {:?}", report.run_id.0, report.status);
            println!("Artifacts: {:?}", report.artifacts);
            println!("Evidence: {} items", report.evidence.len());

            let verification = verify_run(&*event_store, report.run_id).await?;
            println!(
                "Verification: {}",
                if verification.all_passed() { "PASSED" } else { "FAILED" }
            );
            Ok(())
        }
        _ => {
            eprintln!("usage: acos <compile|run|run-cir|bench> [args]");
            std::process::exit(1);
        }
    }
}

async fn read_task(path: &str) -> Result<TaskSpec, AcosError> {
    let yaml = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AcosError::ValidationFailure {
            message: format!("failed to read {path}: {e}"),
        })?;
    from_yaml(&yaml)
}

async fn compile(task: &TaskSpec, use_rules: bool) -> Result<acos_core::traits::CompileResult, AcosError> {
    if use_rules {
        return RuleCompiler::new().compile(task.clone()).await;
    }
    match ModelCompiler::from_env() {
        Ok(model) => {
            println!("[planner] model-assisted (Claude via LongCat)");
            model.compile(task.clone()).await
        }
        Err(_) => {
            println!("[planner] LONGCAT_API_KEY not set; falling back to rule-first planner");
            RuleCompiler::new().compile(task.clone()).await
        }
    }
}

/// Loads a CIR JSON file and runs it directly, bypassing the compiler.
async fn run_cir(path: &str, env_path: Option<&str>, verify_path: Option<&str>) -> Result<(), AcosError> {
    let json = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| AcosError::ValidationFailure {
            message: format!("failed to read {path}: {e}"),
        })?;
    let program: CirProgram = serde_json::from_str(&json).map_err(|e| {
        AcosError::ValidationFailure {
            message: format!("failed to parse CIR JSON: {e}"),
        }
    })?;

    let seed_env = match env_path {
        Some(env_path) => {
            let env_json = tokio::fs::read_to_string(env_path)
                .await
                .map_err(|e| AcosError::ValidationFailure {
                    message: format!("failed to read {env_path}: {e}"),
                })?;
            parse_env_json(&env_json)?
        }
        None => HashMap::new(),
    };

    let event_store: std::sync::Arc<dyn acos_core::traits::EventStore + Send + Sync> =
        std::sync::Arc::new(InMemoryStore::new());
    let artifact_store: std::sync::Arc<dyn acos_core::traits::ArtifactStore + Send + Sync> =
        std::sync::Arc::new(InMemoryStore::new());
    let runtime = Runtime::new(event_store.clone(), artifact_store.clone());
    let report = runtime.execute_with_env(program, None, seed_env).await?;

    println!("Run {}: {:?}", report.run_id.0, report.status);
    println!("Artifacts: {:?}", report.artifacts);
    println!("Evidence: {} items", report.evidence.len());

    // Read artifact content for verification
    let artifact_content = if !report.artifacts.is_empty() {
        let artifact_name = &report.artifacts[0];
        artifact_store.get_by_name(report.run_id, artifact_name).await.ok()
    } else {
        None
    };

    if let Some(gt_path) = verify_path {
        // Full three-layer verification against ground truth
        let ground_truth = GroundTruth::from_yaml(gt_path)?;
        let verification = verify_run_full(&*event_store, artifact_content, report.run_id, &ground_truth).await?;
        println!("\n=== Semantic Verification ===");
        println!("Overall: {}", if verification.all_passed() { "PASSED" } else { "FAILED" });
        println!("\n[Structural]");
        for f in verification.layer_findings(acos_verify::VerificationLayer::Structural) {
            println!("  [{}] {}", if f.passed { "PASS" } else { "FAIL" }, f.message);
        }
        println!("\n[Semantic]");
        for f in verification.layer_findings(acos_verify::VerificationLayer::Semantic) {
            println!("  [{}] {}", if f.passed { "PASS" } else { "FAIL" }, f.message);
        }
        println!("\n[Evidence]");
        for f in verification.layer_findings(acos_verify::VerificationLayer::Evidence) {
            println!("  [{}] {}", if f.passed { "PASS" } else { "FAIL" }, f.message);
        }
    } else {
        // Legacy MVP verification
        let verification = verify_run(&*event_store, report.run_id).await?;
        println!(
            "Verification: {}",
            if verification.all_passed() { "PASSED" } else { "FAILED" }
        );
    }
    Ok(())
}

/// Parses a JSON object of {name: {valueType, payload}} into a TypedValue map.
fn parse_env_json(json: &str) -> Result<HashMap<String, TypedValue>, AcosError> {
    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(json).map_err(|e| AcosError::ValidationFailure {
            message: format!("failed to parse env JSON: {e}"),
        })?;
    let mut env = HashMap::new();
    for (key, val) in raw {
        let tv = if let Some(obj) = val.as_object() {
            let vt = match obj.get("valueType").and_then(|v| v.as_str()) {
                Some("List") => ValueType::List,
                Some("Record") => ValueType::Record,
                Some("Optional") => ValueType::Optional,
                _ => ValueType::Scalar,
            };
            TypedValue {
                value_type: vt,
                payload: obj.get("payload").cloned().unwrap_or(serde_json::Value::Null),
            }
        } else {
            TypedValue {
                value_type: ValueType::Scalar,
                payload: val,
            }
        };
        env.insert(key, tv);
    }
    Ok(env)
}
