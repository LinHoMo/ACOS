//! Recovery control-semantics suite (fixtures-as-contracts).

use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn rule_replan_recovers() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("recovery".into()),
        case: Some("recovery_rule_replan".into()),
        require_model: false,
    })
    .await;
    let case = &report.cases[0];
    assert_eq!(case.status.to_string(), "PASS", "{case:?}");
    assert_eq!(case.recovery.as_deref(), Some("rule"));
}

#[tokio::test]
async fn recovery_suite_passes_with_model_skip() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    // Without an LLM key the model-expected case is SKIP, not FAIL.
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("recovery".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "recovery suite: {:?}", report.cases);
    assert!(report.passed() >= 1);
    assert!(report.skipped() >= 1, "model case should be skipped without a key");
}
