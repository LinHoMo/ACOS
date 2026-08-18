//! Condition control-semantics suite (fixtures-as-contracts).

use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn condition_suite_passes() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("condition".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "condition suite: {:?}", report.cases);
    assert_eq!(report.skipped(), 0);
    assert!(report.passed() >= 2);
}
