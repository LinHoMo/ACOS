//! Loop control-semantics suite (fixtures-as-contracts).

use acos_bench::{BenchArgs, run};

#[tokio::test]
async fn loop_suite_passes() {
    let mut dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.push("fixtures");
    let report = run(BenchArgs {
        fixtures_dir: dir,
        suite: Some("loop".into()),
        case: None,
        require_model: false,
    })
    .await;
    assert_eq!(report.failed(), 0, "loop suite: {:?}", report.cases);
    assert!(report.passed() >= 1);
}
