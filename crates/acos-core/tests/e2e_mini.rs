/// ACOS Mini end-to-end test skeleton.
///
/// This is the **first** acceptance test. It proves that a structured task
/// can be compiled into a CIR program, executed, and produce verifiable
/// artifacts with evidence references.
///
/// See docs/specs/mvp_spec.md — "必需演示 / Required demo".

use acos_core::id::TaskId;
use acos_core::schema::{from_json, to_json, validate_task_spec};
use acos_core::types::{TaskInput, TaskOutput, TaskSpec};

/// Builds the ACOS Mini "required demo" task: analyze five CSV files,
/// detect malformed columns, clean, compute stats, aggregate, generate a
/// markdown report, and request approval before any external send.
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
        outputs: vec![TaskOutput {
            output_type: "Report".into(),
            format: Some("markdown".into()),
        }],
        constraints: None,
        optimization: None,
        approval: None,
    }
}

#[test]
fn task_spec_roundtrips_to_json() {
    let task = mini_csv_task();
    validate_task_spec(&task).expect("valid task spec");

    let json = to_json(&task).expect("serialize");
    println!("Task JSON:\n{json}");

    // Round-trip
    let parsed: TaskSpec = from_json(&json).expect("deserialize");
    assert_eq!(parsed.id.0, task.id.0);
    assert_eq!(parsed.inputs.len(), 5);
    assert!(parsed.goal.contains("CSV"));
}

// The following tests are the M2 acceptance skeleton. They will compile
// once `acos-compiler` and `acos-runtime` are implemented.

#[test]
#[ignore = "M2: requires acos-compiler"]
fn mini_csv_pipeline_compiles() {
    let _task = mini_csv_task();
    // let program = acos_compiler::compile(task).unwrap();
    // assert!(!program.nodes.is_empty());
    todo!("wire acos-compiler");
}

#[test]
#[ignore = "M2: requires acos-runtime"]
fn mini_csv_pipeline_executes_and_produces_report() {
    // let task = mini_csv_task();
    // let program = acos_compiler::compile(task).unwrap();
    // let report = acos_runtime::execute(program).unwrap();
    // assert!(report.artifacts().contains("report.md"));
    // assert!(report.evidence().iter().all(|e| e.is_logged()));
    todo!("wire acos-runtime");
}
