//! Retry control-semantics suite (fixtures-as-contracts).

use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn retry_suite_passes() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("retry".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "retry suite: {:?}", report.cases);
    assert_eq!(report.passed(), 1);
    assert_eq!(report.cases[0].recovery.as_deref(), Some("retry"));
}
