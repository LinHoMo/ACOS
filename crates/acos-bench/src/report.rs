//! Case outcomes and the human-readable report table.

use std::fmt;

/// Per-case result status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStatus {
    /// Contract satisfied.
    Pass,
    /// Contract violated.
    Fail,
    /// Not exercised (e.g. model-only case without a key).
    Skip,
}

impl fmt::Display for CaseStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::Skip => "SKIP",
        })
    }
}

/// The outcome of running a single fixture.
#[derive(Debug, Clone)]
pub struct CaseResult {
    /// Fixture id.
    pub id: String,
    /// Suite (top-level fixture dir name).
    pub suite: String,
    /// Overall status.
    pub status: CaseStatus,
    /// None when the compile step was skipped (mode = run, not exercised here).
    pub compile: Option<bool>,
    /// Whether execution succeeded.
    pub execution: Option<bool>,
    /// Observed recovery label, if any (`retry` | `rule` | `model`).
    pub recovery: Option<String>,
    /// Richer recovery telemetry, e.g. `retry(x3)` or `replan:rule`.
    /// `None` when no recovery was observed.
    pub recovery_detail: Option<String>,
    /// Whether verification passed.
    pub verification: Option<bool>,
    /// Human-readable note (empty on clean pass).
    pub note: String,
}

/// Aggregated benchmark report.
#[derive(Debug, Default)]
pub struct BenchReport {
    /// Per-case results.
    pub cases: Vec<CaseResult>,
}

impl BenchReport {
    /// Total number of cases.
    pub fn total(&self) -> usize {
        self.cases.len()
    }

    /// Number of passing cases.
    pub fn passed(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.status == CaseStatus::Pass)
            .count()
    }

    /// Number of failing cases.
    pub fn failed(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.status == CaseStatus::Fail)
            .count()
    }

    /// Number of skipped cases.
    pub fn skipped(&self) -> usize {
        self.cases
            .iter()
            .filter(|c| c.status == CaseStatus::Skip)
            .count()
    }

    /// Prints the report table to stdout.
    pub fn print(&self) {
        println!("ACOS Benchmark v0.1");
        println!(
            "{:<28} {:<6} {:<8} {:<8} {:<8} {:<16} {}",
            "Case", "Result", "Compile", "Execute", "Recover", "Detail", "Note"
        );
        println!("{}", "-".repeat(76));
        for case in &self.cases {
            let compile = case
                .compile
                .map(|b| if b { "PASS" } else { "FAIL" })
                .unwrap_or("-");
            let execution = case
                .execution
                .map(|b| if b { "PASS" } else { "FAIL" })
                .unwrap_or("-");
            let recovery = case.recovery.as_deref().unwrap_or("-");
            let detail = case.recovery_detail.as_deref().unwrap_or("-");
            println!(
                "{:<28} {:<6} {:<8} {:<8} {:<8} {:<16} {}",
                case.id, case.status, compile, execution, recovery, detail, case.note
            );
        }
        println!("{}", "-".repeat(76));
        println!(
            "{} cases / {} passed / {} failed / {} skipped",
            self.total(),
            self.passed(),
            self.failed(),
            self.skipped()
        );
    }
}
