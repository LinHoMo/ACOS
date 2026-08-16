//! ACOS command-line interface.
//!
//! MVP commands:
//!   acos compile <task.yaml>   — compile a task spec to CIR and print it
//!   acos run <task.yaml>       — compile and execute a task
//!
//! Planner selection:
//!   - Default: **ModelCompiler** (Claude via LongCat). Set `LONGCAT_API_KEY`
//!     (or `ANTHROPIC_API_KEY`) to enable. Configure the model with
//!     `ACOS_LLM_MODEL` (default `claude-sonnet-4-5-20250929`).
//!   - Fallback: `--rules` flag forces the deterministic `RuleCompiler`.

use acos_compiler::{ModelCompiler, RuleCompiler};
use acos_core::error::AcosError;
use acos_core::schema::from_yaml;
use acos_core::traits::Compiler;
use acos_core::types::TaskSpec;
use acos_runtime::Runtime;
use acos_state::InMemoryStore;
use acos_verify::verify_run;

#[tokio::main]
async fn main() -> Result<(), AcosError> {
    let args: Vec<String> = std::env::args().collect();
    let use_rules = args.iter().any(|a| a == "--rules");
    let positional: Vec<&str> = args.iter().filter(|a| !a.starts_with('-')).map(String::as_str).collect();

    match positional.get(1).map(|s| *s) {
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
            eprintln!("usage: acos <compile|run> <task.yaml> [--rules]");
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
