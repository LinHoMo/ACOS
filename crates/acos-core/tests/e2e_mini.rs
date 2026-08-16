//! ACOS Mini end-to-end test — the MVP acceptance test.
//!
//! Proves the full pipeline: parse task YAML → compile to CIR → execute →
//! produce a verifiable artifact with logged evidence.
//!
//! See docs/specs/mvp_spec.md — "必需演示 / Required demo".

use acos_core::id::TaskId;
use acos_core::schema::{from_yaml, validate_task_spec};
use acos_core::traits::Compiler;
use acos_core::types::{TaskInput, TaskSpec};
use acos_state::InMemoryStore;

/// Builds the ACOS Mini "required demo" task: analyze five CSV files,
/// detect malformed columns, clean, compute stats, aggregate, generate a
/// markdown report, and request approval before sending externally.
fn mini_csv_task() -> TaskSpec {
    TaskSpec {
        api_version: "acos.io/v1".into(),
        id: TaskId::new(),
        goal: "Analyze five CSV files. For each, detect malformed columns, clean them if needed, compute key statistics, aggregate results, generate a markdown report, and request approval before sending externally.".into(),
        inputs: (1..=5)
            .map(|i| TaskInput {
                input_type: "File".into(),
                path: format!("./data/sales_{i}.csv"),
                format: Some("csv".into()),
            })
            .collect(),
        outputs: vec![],
        constraints: None,
        optimization: None,
        approval: None,
    }
}

#[test]
fn task_spec_roundtrips_to_json() {
    let task = mini_csv_task();
    validate_task_spec(&task).expect("valid task spec");

    let json = serde_json::to_string_pretty(&task).expect("serialize");
    let parsed: TaskSpec = acos_core::schema::from_json(&json).expect("deserialize");
    assert_eq!(parsed.id.0, task.id.0);
    assert_eq!(parsed.inputs.len(), 5);
    assert!(parsed.goal.contains("CSV"));
}

#[test]
fn task_spec_parses_from_yaml_fixture() {
    let yaml = include_str!("../../../tests/fixtures/mini_csv_task.yaml");
    let task: TaskSpec = from_yaml(yaml).expect("parse yaml");
    assert_eq!(task.inputs.len(), 5);
    assert!(task.goal.contains("CSV"));
}

/// The core MVP acceptance test: compile + execute + verify.
#[tokio::test]
async fn mvp_mini_csv_pipeline_compiles_executes_and_verifies() {
    // Set up a temp workspace with 5 CSV files.
    let dir = std::env::temp_dir().join(format!("acos-mvp-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).expect("create dir");
    for i in 1..=5 {
        let content = format!(
            "product,units,price\nitem_a,{},10.0\nitem_b,{},20.0\n",
            i * 10,
            i * 5
        );
        std::fs::write(dir.join(format!("sales_{i}.csv")), content).expect("write csv");
    }

    // Build task pointing at the temp CSVs.
    let mut task = mini_csv_task();
    for (i, input) in task.inputs.iter_mut().enumerate() {
        let n = i + 1;
        input.path = dir.join(format!("sales_{n}.csv")).to_string_lossy().to_string();
    }

    // Ensure the deterministic pipeline is tested without external LLM.
    std::env::remove_var("LONGCAT_API_KEY");
    std::env::remove_var("ANTHROPIC_API_KEY");

    // 1. Compile.
    let compiler = acos_compiler::RuleCompiler::new();
    let result = compiler.compile(task).await.expect("compile");
    let program = result.program;
    assert!(!program.nodes.is_empty(), "program has nodes");
    assert!(!program.entry.is_empty(), "program has entry");

    // 2. Execute.
    let event_store: std::sync::Arc<dyn acos_core::traits::EventStore + Send + Sync> =
        std::sync::Arc::new(InMemoryStore::new());
    let artifact_store: std::sync::Arc<dyn acos_core::traits::ArtifactStore + Send + Sync> =
        std::sync::Arc::new(InMemoryStore::new());
    let runtime = acos_runtime::Runtime::new(event_store.clone(), artifact_store);
    let report = runtime.execute(program).await.expect("execute");

    assert_eq!(
        report.status,
        acos_core::traits::RunStatus::Completed,
        "run completed"
    );

    // 3. Verify artifacts.
    assert!(
        report.artifacts().contains(&"report.md".to_string()),
        "report.md artifact produced; got {:?}",
        report.artifacts()
    );

    // 4. Verify evidence logged.
    assert!(!report.evidence.is_empty(), "evidence collected");
    assert!(
        report.evidence().iter().all(|e| e.is_logged()),
        "all evidence logged"
    );

    // 5. Verify via the verify pipeline.
    let verification = acos_verify::verify_run(&*event_store, report.run_id)
        .await
        .expect("verify");
    assert!(verification.all_passed(), "verification passed");

    // 6. Event log reconstructs the run.
    let events = acos_core::traits::EventStore::replay(&*event_store, report.run_id).await.expect("replay");
    assert!(
        events.iter().any(|e| e.event_type == "run.started"),
        "run.started logged"
    );
    assert!(
        events.iter().any(|e| e.event_type == "run.finished"),
        "run.finished logged"
    );

    // Cleanup.
    std::fs::remove_dir_all(&dir).ok();
}
