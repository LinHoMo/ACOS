//! ACOS verification pipeline — P1-2 Semantic Verification.
//!
//! Three-layer verification for run correctness:
//! 1. **Structural** — artifact exists, non-empty, required sections present
//! 2. **Semantic** — numeric claims match ground-truth dataset statistics
//! 3. **Evidence** — evidence entries exist and link to claims
//!
//! Scope: deterministic checks only. No LLM Judge, no open-ended quality scoring.

#![warn(missing_docs)]

use acos_core::error::AcosError;
use acos_core::id::RunId;
use acos_core::traits::EventStore;
use serde::Deserialize;

// ── Ground Truth types ──────────────────────────────────────────────────────

/// Ground truth for a single CSV file.
#[derive(Debug, Clone, Deserialize)]
pub struct FileTruth {
    /// Display name (e.g., "Q1 Sales").
    #[serde(default)]
    pub display_name: String,
    /// Raw row count (before cleaning).
    #[serde(default)]
    pub raw_row_count: u64,
    /// Column count.
    #[serde(default)]
    pub column_count: u64,
    /// Column names.
    #[serde(default)]
    pub columns: Vec<String>,
    /// Sum of revenue column (after standard cleaning).
    #[serde(default)]
    pub total_revenue: Option<f64>,
    /// Sum of units column.
    #[serde(default)]
    pub total_units: Option<u64>,
    /// Unique row count (after dedup).
    #[serde(default)]
    pub unique_row_count: Option<u64>,
    /// Has duplicate rows.
    #[serde(default)]
    pub has_duplicates: bool,
    /// Has missing values (NA, NULL, empty).
    #[serde(default)]
    pub has_missing_values: bool,
    /// Has negative values.
    #[serde(default)]
    pub has_negative_values: bool,
    /// Has extreme outliers.
    #[serde(default)]
    pub has_outliers: bool,
    /// Has column-name drift.
    #[serde(default)]
    pub has_column_drift: bool,
    /// Has currency formatting in numbers.
    #[serde(default)]
    pub has_currency_format: bool,
    /// Category list.
    #[serde(default)]
    pub categories: Vec<String>,
    /// Number of issues found.
    #[serde(default)]
    pub issue_count: u64,
    /// Issue type tags.
    #[serde(default)]
    pub issues: Vec<String>,
}

impl Default for FileTruth {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            raw_row_count: 0,
            column_count: 0,
            columns: Vec::new(),
            total_revenue: None,
            total_units: None,
            unique_row_count: None,
            has_duplicates: false,
            has_missing_values: false,
            has_negative_values: false,
            has_outliers: false,
            has_column_drift: false,
            has_currency_format: false,
            categories: Vec::new(),
            issue_count: 0,
            issues: Vec::new(),
        }
    }
}

/// Aggregate statistics across all files.
#[derive(Debug, Clone, Deserialize)]
pub struct AggregateTruth {
    /// Total files processed.
    #[serde(default)]
    pub total_files: u64,
    /// Total raw rows across all files.
    #[serde(default)]
    pub total_raw_rows: u64,
    /// Number of files with detected issues.
    #[serde(default)]
    pub files_with_issues: u64,
    /// Number of clean files.
    #[serde(default)]
    pub clean_files: u64,
    /// Grand total revenue (sum of all files).
    #[serde(default)]
    pub grand_total_revenue: Option<f64>,
    /// Required sections in the report.
    #[serde(default)]
    pub required_sections: Vec<String>,
    /// Required data points that must appear.
    #[serde(default)]
    pub required_data_points: Vec<String>,
}

impl Default for AggregateTruth {
    fn default() -> Self {
        Self {
            total_files: 0,
            total_raw_rows: 0,
            files_with_issues: 0,
            clean_files: 0,
            grand_total_revenue: None,
            required_sections: Vec::new(),
            required_data_points: Vec::new(),
        }
    }
}

/// Parsed ground truth configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct GroundTruth {
    /// Per-file statistics.
    pub files: std::collections::HashMap<String, FileTruth>,
    /// Aggregate statistics.
    pub aggregate: AggregateTruth,
    /// Numeric comparison tolerance.
    #[serde(default)]
    pub numeric_tolerance: f64,
}

impl Default for GroundTruth {
    fn default() -> Self {
        Self {
            files: std::collections::HashMap::new(),
            aggregate: AggregateTruth {
                total_files: 0,
                total_raw_rows: 0,
                files_with_issues: 0,
                clean_files: 0,
                grand_total_revenue: None,
                required_sections: Vec::new(),
                required_data_points: Vec::new(),
            },
            numeric_tolerance: 0.01,
        }
    }
}

impl GroundTruth {
    /// Loads ground truth from a YAML file.
    pub fn from_yaml(path: &str) -> Result<Self, AcosError> {
        let content = std::fs::read_to_string(path).map_err(|e| AcosError::ValidationFailure {
            message: format!("cannot read ground truth file {path}: {e}"),
        })?;
        serde_yaml::from_str(&content).map_err(|e| AcosError::ValidationFailure {
            message: format!("cannot parse ground truth YAML: {e}"),
        })
    }
}

// ── Verification types ──────────────────────────────────────────────────────

/// A verification finding from any layer.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationFinding {
    /// Whether this finding passed.
    pub passed: bool,
    /// Layer that produced this finding.
    pub layer: VerificationLayer,
    /// Human-readable message.
    pub message: String,
}

/// Verification layer that produced a finding.
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationLayer {
    /// Structural checks (artifact exists, sections present).
    Structural,
    /// Semantic checks (numeric consistency vs ground truth).
    Semantic,
    /// Evidence checks (evidence entries exist and linked).
    Evidence,
}

/// A full verification report for a run.
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationReport {
    /// Findings from all layers.
    pub findings: Vec<VerificationFinding>,
}

impl VerificationReport {
    /// Returns `true` if all findings passed.
    pub fn all_passed(&self) -> bool {
        !self.findings.is_empty() && self.findings.iter().all(|f| f.passed)
    }

    /// Returns findings from a specific layer.
    pub fn layer_findings(&self, layer: VerificationLayer) -> Vec<&VerificationFinding> {
        self.findings.iter().filter(|f| f.layer == layer).collect()
    }

    /// Returns `true` if a specific layer fully passed.
    pub fn layer_passed(&self, layer: VerificationLayer) -> bool {
        let layer_finds: Vec<_> = self.findings.iter().filter(|f| f.layer == layer).collect();
        !layer_finds.is_empty() && layer_finds.iter().all(|f| f.passed)
    }
}

// ── Layer 1: Structural Checker ─────────────────────────────────────────────

/// Checks that the artifact exists, is non-empty, and has required sections.
pub fn check_structural(
    artifact_content: Option<&[u8]>,
    ground_truth: &GroundTruth,
) -> Vec<VerificationFinding> {
    let mut findings = vec![];

    // Check 1: artifact exists
    let content = match artifact_content {
        Some(c) => c,
        None => {
            findings.push(VerificationFinding {
                passed: false,
                layer: VerificationLayer::Structural,
                message: "artifact: missing — no output artifact produced".into(),
            });
            return findings;
        }
    };

    // Check 2: artifact non-empty
    if content.is_empty() {
        findings.push(VerificationFinding {
            passed: false,
            layer: VerificationLayer::Structural,
            message: "artifact: empty — output artifact has zero bytes".into(),
        });
        return findings;
    }

    // Check 3: required sections present
    let text = String::from_utf8_lossy(content);
    for section in &ground_truth.aggregate.required_sections {
        // Accept section as header (# section), as markdown bold (**section**),
        // or as a word boundary match for flexibility.
        let section_lower = section.to_lowercase();
        let text_lower = text.to_lowercase();
        let found = text_lower.contains(&section_lower);
        findings.push(VerificationFinding {
            passed: found,
            layer: VerificationLayer::Structural,
            message: if found {
                format!("section '{section}': present")
            } else {
                format!("section '{section}': MISSING")
            },
        });
    }

    findings
}

// ── Layer 2: Semantic Checker ───────────────────────────────────────────────

/// Extracts numeric value near a keyword in text.
/// Looks for patterns like "key: 123.45" or "key = 123.45" or "key 123.45".
///
/// Scans ONLY the text after the keyword (not the keyword itself) to avoid
/// matching digits that are part of the keyword (e.g., "q1" contains "1").
fn extract_numeric_near_keyword(text: &str, keyword: &str) -> Option<f64> {
    let text_lower = text.to_lowercase();
    let keyword_lower = keyword.to_lowercase();

    // Find keyword, then scan for the next number in the text AFTER the keyword
    if let Some(pos) = text_lower.find(&keyword_lower) {
        let after_keyword = pos + keyword.len();
        if after_keyword >= text.len() {
            return None;
        }
        let window = &text[after_keyword..text.len().min(after_keyword + 80)];
        // Scan chars, building numbers digit-by-digit, skipping commas (thousands sep)
        let mut i = 0;
        let chars: Vec<char> = window.chars().collect();
        while i < chars.len() {
            if chars[i].is_ascii_digit() {
                let start = i;
                let mut has_dot = false;
                while i < chars.len() && (chars[i].is_ascii_digit() || chars[i] == ',' || (chars[i] == '.' && !has_dot)) {
                    if chars[i] == '.' { has_dot = true; }
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().filter(|c| c.is_ascii_digit() || **c == '.').collect();
                if let Ok(n) = num_str.parse::<f64>() {
                    if n > 0.0 {
                        return Some(n);
                    }
                }
            } else {
                i += 1;
            }
        }
    }
    None
}

/// Checks that numeric claims in the report match ground truth.
pub fn check_semantic(
    artifact_content: Option<&[u8]>,
    ground_truth: &GroundTruth,
) -> Vec<VerificationFinding> {
    let mut findings = vec![];

    let content = match artifact_content {
        Some(c) => c,
        None => {
            findings.push(VerificationFinding {
                passed: false,
                layer: VerificationLayer::Semantic,
                message: "semantic: no artifact to check".into(),
            });
            return findings;
        }
    };

    let text = String::from_utf8_lossy(content);
    let tolerance = ground_truth.numeric_tolerance;

    // Check per-file revenue claims
    for (file_name, file_truth) in &ground_truth.files {
        if let Some(expected_revenue) = file_truth.total_revenue {
            let q_name = file_name
                .trim_end_matches(".csv")
                .split('_')
                .nth(1)
                .unwrap_or(file_name);

            // Try multiple keyword patterns
            let keywords = [
                &format!("{} revenue", q_name),
                &format!("{}", q_name),
                &format!("revenue {}", q_name),
            ];

            let mut found_match = false;
            for kw in &keywords {
                if let Some(claimed) = extract_numeric_near_keyword(&text, kw) {
                    let diff = (claimed - expected_revenue).abs();
                    if diff <= tolerance * expected_revenue.max(1.0) {
                        found_match = true;
                        findings.push(VerificationFinding {
                            passed: true,
                            layer: VerificationLayer::Semantic,
                            message: format!(
                                "semantic: {q_name} revenue = {claimed} (expected {expected_revenue})"
                            ),
                        });
                        break;
                    }
                }
            }

            if !found_match {
                findings.push(VerificationFinding {
                    passed: false,
                    layer: VerificationLayer::Semantic,
                    message: format!(
                        "semantic: {q_name} revenue claim MISSING or WRONG (expected {expected_revenue})"
                    ),
                });
            }
        }
    }

    // Check aggregate claims
    if let Some(expected_total) = ground_truth.aggregate.grand_total_revenue {
        let keywords = ["grand total", "total revenue", "overall revenue", "sum"];
        let mut found = false;
        for kw in &keywords {
            if let Some(claimed) = extract_numeric_near_keyword(&text, kw) {
                let diff = (claimed - expected_total).abs();
                if diff <= tolerance * expected_total.max(1.0) {
                    found = true;
                    findings.push(VerificationFinding {
                        passed: true,
                        layer: VerificationLayer::Semantic,
                        message: format!(
                            "semantic: grand total revenue = {claimed} (expected {expected_total})"
                        ),
                    });
                    break;
                }
            }
        }
        if !found {
            findings.push(VerificationFinding {
                passed: false,
                layer: VerificationLayer::Semantic,
                message: format!(
                    "semantic: grand total revenue claim MISSING or WRONG (expected {expected_total})"
                ),
            });
        }
    }

    // Check files-with-issues count
    let expected_issues = ground_truth.aggregate.files_with_issues;
    let keywords = ["files with issues", "files with problems", "files needing repair"];
    let mut found = false;
    for kw in &keywords {
        if let Some(claimed) = extract_numeric_near_keyword(&text, kw) {
            if (claimed as u64) == expected_issues {
                found = true;
                findings.push(VerificationFinding {
                    passed: true,
                    layer: VerificationLayer::Semantic,
                    message: format!(
                        "semantic: files with issues = {claimed} (expected {expected_issues})"
                    ),
                });
                break;
            }
        }
    }
    if !found {
        findings.push(VerificationFinding {
            passed: false,
            layer: VerificationLayer::Semantic,
            message: format!(
                "semantic: files-with-issues count MISSING or WRONG (expected {expected_issues})"
            ),
        });
    }

    findings
}

// ── Layer 3: Evidence Checker ───────────────────────────────────────────────

/// Checks that evidence entries exist in the event log.
pub fn check_evidence(
    events: &[acos_core::traits::Event],
    _ground_truth: &GroundTruth,
) -> Vec<VerificationFinding> {
    let mut findings = vec![];

    // Check 1: run started
    let has_start = events.iter().any(|e| e.event_type == "run.started");
    findings.push(VerificationFinding {
        passed: has_start,
        layer: VerificationLayer::Evidence,
        message: if has_start {
            "evidence: run.started event present".into()
        } else {
            "evidence: MISSING run.started event".into()
        },
    });

    // Check 2: run finished
    let has_finish = events.iter().any(|e| e.event_type == "run.finished");
    findings.push(VerificationFinding {
        passed: has_finish,
        layer: VerificationLayer::Evidence,
        message: if has_finish {
            "evidence: run.finished event present".into()
        } else {
            "evidence: MISSING run.finished event".into()
        },
    });

    // Check 3: primitives executed (artifact.stored removed — Runtime does not emit it;
    // artifact existence is a Structural concern, not Evidence)
    let primitive_count = events.iter().filter(|e| e.event_type == "primitive.end").count();
    findings.push(VerificationFinding {
        passed: primitive_count > 0,
        layer: VerificationLayer::Evidence,
        message: format!("evidence: {primitive_count} primitives executed"),
    });

    findings
}

// ── Combined verification ───────────────────────────────────────────────────

/// Full three-layer verification of a run.
///
/// Checks structural (artifact), semantic (numeric claims), and evidence (events).
///
/// # Arguments
/// - `event_store`: The event store to replay events from.
/// - `artifact_content`: The content of the output artifact (if any).
/// - `run_id`: The run to verify.
/// - `ground_truth`: The ground-truth configuration.
pub async fn verify_run_full(
    event_store: &dyn EventStore,
    artifact_content: Option<Vec<u8>>,
    run_id: RunId,
    ground_truth: &GroundTruth,
) -> Result<VerificationReport, AcosError> {
    let events = event_store.replay(run_id).await?;

    let mut findings = vec![];

    findings.extend(check_structural(artifact_content.as_deref(), ground_truth));
    findings.extend(check_semantic(artifact_content.as_deref(), ground_truth));
    findings.extend(check_evidence(&events, ground_truth));

    Ok(VerificationReport { findings })
}

/// Legacy MVP verification — kept for backward compatibility.
///
/// For the MVP, this checks:
/// 1. The run reached a `run.finished` event.
/// 2. At least one artifact-producing primitive succeeded.
pub async fn verify_run(
    event_store: &dyn EventStore,
    run_id: RunId,
) -> Result<VerificationReport, AcosError> {
    let events = event_store.replay(run_id).await?;
    let gt = GroundTruth::default();

    let mut findings = vec![];
    findings.extend(check_evidence(&events, &gt));

    Ok(VerificationReport { findings })
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_state::InMemoryStore;

    #[test]
    fn structural_empty_artifact_fails() {
        let gt = GroundTruth::default();
        let findings = check_structural(None, &gt);
        assert_eq!(findings.len(), 1);
        assert!(!findings[0].passed);
        assert!(findings[0].message.contains("missing"));
    }

    #[test]
    fn structural_whitespace_artifact_fails() {
        let gt = GroundTruth {
            aggregate: AggregateTruth {
                required_sections: vec!["summary".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let findings = check_structural(Some(b""), &gt);
        assert!(findings.iter().any(|f| !f.passed && f.message.contains("empty")));
    }

    #[test]
    fn structural_required_section_missing() {
        let gt = GroundTruth {
            aggregate: AggregateTruth {
                required_sections: vec!["summary".into(), "details".into()],
                ..Default::default()
            },
            ..Default::default()
        };
        let findings = check_structural(Some(b"# Summary\nThis is a report."), &gt);
        let summary_findings: Vec<_> = findings.iter().filter(|f| f.message.contains("summary")).collect();
        let details_findings: Vec<_> = findings.iter().filter(|f| f.message.contains("details")).collect();
        assert!(summary_findings.iter().all(|f| f.passed));
        assert!(details_findings.iter().any(|f| !f.passed));
    }

    #[test]
    fn semantic_revenue_match() {
        let gt = GroundTruth {
            files: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "sales_q1.csv".into(),
                    FileTruth {
                        total_revenue: Some(33850.0),
                        ..Default::default()
                    },
                );
                m
            },
            ..Default::default()
        };
        let text = "q1 revenue: 33850.00";
        let findings = check_semantic(Some(text.as_bytes()), &gt);
        let revenue_findings: Vec<_> = findings.iter().filter(|f| f.message.contains("revenue")).collect();
        assert!(revenue_findings.iter().all(|f| f.passed));
    }

    #[test]
    fn semantic_revenue_mismatch() {
        let gt = GroundTruth {
            files: {
                let mut m = std::collections::HashMap::new();
                m.insert(
                    "sales_q1.csv".into(),
                    FileTruth {
                        total_revenue: Some(33850.0),
                        ..Default::default()
                    },
                );
                m
            },
            ..Default::default()
        };
        let text = "q1 revenue: 99999.00";
        let findings = check_semantic(Some(text.as_bytes()), &gt);
        assert!(findings.iter().any(|f| !f.passed));
    }

    #[test]
    fn evidence_full_run_passes() {
        let gt = GroundTruth::default();
        let events = vec![
            acos_core::traits::Event {
                seq: 1,
                run_id: RunId::new(),
                event_type: "run.started".into(),
                payload: serde_json::json!({}),
            },
            acos_core::traits::Event {
                seq: 2,
                run_id: RunId::new(),
                event_type: "primitive.end".into(),
                payload: serde_json::json!({"ok": true}),
            },
            acos_core::traits::Event {
                seq: 3,
                run_id: RunId::new(),
                event_type: "run.finished".into(),
                payload: serde_json::json!({}),
            },
        ];
        let findings = check_evidence(&events, &gt);
        assert!(findings.iter().all(|f| f.passed));
    }

    #[tokio::test]
    async fn verify_run_passes_for_completed_run() {
        let store = InMemoryStore::new();
        let run_id = RunId::new();
        store
            .append(run_id, "run.started".into(), serde_json::json!({}))
            .await
            .unwrap();
        store
            .append(
                run_id,
                "primitive.end".into(),
                serde_json::json!({"ok": true}),
            )
            .await
            .unwrap();
        store
            .append(run_id, "run.finished".into(), serde_json::json!({}))
            .await
            .unwrap();

        let report = verify_run(&store, run_id).await.unwrap();
        assert!(report.all_passed());
    }
}
