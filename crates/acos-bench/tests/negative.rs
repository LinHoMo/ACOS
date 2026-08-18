//! Negative (validation-rejection) suite (fixtures-as-contracts).

use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn negative_suite_rejects() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("negative".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "negative suite: {:?}", report.cases);
    assert_eq!(report.skipped(), 0);
    assert!(report.passed() >= 3, "all negative fixtures should pass: {:?}", report.cases);
}
