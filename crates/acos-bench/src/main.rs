//! `acos-bench` CLI entry point.

use acos_bench::{BenchArgs, run};
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    let mut fixtures_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fixtures_dir.push("fixtures");
    let mut suite = None;
    let mut case = None;
    let mut require_model = false;

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--suite" => {
                if let Some(v) = args.get(i + 1) {
                    suite = Some(v.clone());
                }
                i += 1;
            }
            "--case" => {
                if let Some(v) = args.get(i + 1) {
                    case = Some(v.clone());
                }
                i += 1;
            }
            "--require-model" => require_model = true,
            "--fixtures" => {
                if let Some(v) = args.get(i + 1) {
                    fixtures_dir = PathBuf::from(v);
                }
                i += 1;
            }
            other => {
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
