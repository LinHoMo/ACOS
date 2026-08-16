//! ACOS command-line interface.
//!
//! MVP commands:
//!   acos compile <task.yaml>   — compile a task spec to CIR and print it
//!   acos run <task.yaml>       — compile and execute a task

use acos_compiler::RuleCompiler;
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
    match args.get(1).map(String::as_str) {
        Some("compile") => {
            let path = args.get(2).expect("usage: acos compile <task.yaml>");
            let yaml = tokio::fs::read_to_string(path).await.map_err(|e| {
                acos_core::error::AcosError::ValidationFailure {
                    message: format!("failed to read {path}: {e}"),
                }
            })?;
            let task: TaskSpec = from_yaml(&yaml)?;
            let result = RuleCompiler::new().compile(task).await?;
            println!(
                "Compiled program {} ({} nodes, {} diagnostics)",
                result.program.id.0,
                result.program.nodes.len(),
                result.diagnostics.len()
            );
            println!(
                "{}",
                serde_json::to_string_pretty(&result.program).unwrap_or_default()
            );
            Ok(())
        }
        Some("run") => {
            let path = args.get(2).expect("usage: acos run <task.yaml>");
            let yaml = tokio::fs::read_to_string(path).await.map_err(|e| {
                acos_core::error::AcosError::ValidationFailure {
                    message: format!("failed to read {path}: {e}"),
                }
            })?;
            let task: TaskSpec = from_yaml(&yaml)?;
            let result = RuleCompiler::new().compile(task).await?;
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
                if verification.all_passed() {
                    "PASSED"
                } else {
                    "FAILED"
                }
            );
            Ok(())
        }
        _ => {
            eprintln!("usage: acos <compile|run> <task.yaml>");
            std::process::exit(1);
        }
    }
}
