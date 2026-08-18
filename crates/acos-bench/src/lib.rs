//! ACOS benchmark harness — fixtures-as-contracts.
//!
//! Each YAML fixture under `fixtures/` is a behavioral contract: it declares a
//! CIR program (or a compiler goal) together with the expected outcome
//! (compile / execute / verify / recover). The harness runs every selected
//! fixture and aggregates a [`BenchReport`]. Replanner work (Rule/Model) must
//! pass these fixtures to be considered conformant.
//!
//! See `docs/superpowers/specs/2026-08-17-control-semantics-recovery-design.md` §5.

pub mod fixtures;
pub mod registry;
pub mod report;
pub mod runner;

use std::path::PathBuf;

use report::BenchReport;

/// Top-level CLI-facing arguments for a benchmark run.
#[derive(Debug, Clone)]
pub struct BenchArgs {
    /// Directory containing the fixture suites (top-level sub-dirs = suites).
    pub fixtures_dir: PathBuf,
    /// Restrict to one suite (top-level fixture dir name).
    pub suite: Option<String>,
    /// Restrict to one case by id.
    pub case: Option<String>,
    /// Turn `Skip` into `Fail` (used by CI to require model-backed recovery).
    pub require_model: bool,
}

/// Runs all selected fixtures and returns the aggregated report.
pub async fn run(args: BenchArgs) -> BenchReport {
    let fixtures = fixtures::load_fixtures(&args.fixtures_dir);
    let mut report = BenchReport::default();
    for (suite, fixture) in fixtures {
        if let Some(s) = &args.suite {
            if suite != *s {
                continue;
            }
        }
        if let Some(c) = &args.case {
            if fixture.id != *c {
                continue;
            }
        }
        let mut case = runner::run_case(&args, &suite, &fixture).await;
        if args.require_model && case.status == report::CaseStatus::Skip {
            case.status = report::CaseStatus::Fail;
            if case.note.is_empty() {
                case.note = "require_model: skipped case treated as failure".into();
            }
        }
        report.cases.push(case);
    }
    report
}
