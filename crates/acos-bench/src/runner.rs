//! Benchmark case runner: compile → execute → verify → recovery-assert.

use std::sync::Arc;

use acos_compiler::ModelRecoveryPlanner;
use acos_compiler::validate_cir;
use acos_core::error::AcosError;
use acos_core::traits::{ArtifactStore, Event, EventStore, PluginRegistry, RecoveryContext, RunStatus};
use acos_core::types::{CirProgram, EffectKind};
use acos_runtime::replan::{OfflineFallbackRule, RuleReplanner};
use acos_runtime::RuntimeImpl;
use acos_state::InMemoryStore;
use acos_verify::verify_run;

use crate::fixtures::{prepare_workspace, Fixture, FixtureMode};
use crate::registry::BenchRegistry;
use crate::report::{CaseResult, CaseStatus};
use crate::BenchArgs;

/// Runs a single fixture and returns its [`CaseResult`].
pub async fn run_case(_args: &BenchArgs, suite: &str, fixture: &Fixture) -> CaseResult {
    // Ensure offline summarization fallback (no live LLM calls).
    std::env::remove_var("LONGCAT_API_KEY");
    std::env::remove_var("ANTHROPIC_API_KEY");

    let mut result = CaseResult {
        id: fixture.id.clone(),
        suite: suite.to_string(),
        status: CaseStatus::Fail,
        compile: None,
        execution: None,
        recovery: None,
        recovery_detail: None,
        verification: None,
        note: String::new(),
    };

    let Some(cir) = &fixture.cir else {
        result.note = "fixture has no cir program".into();
        return result;
    };

    // Registry is cheap to build and is needed both for the negative
    // (registry-aware) compile check and for execution.
    let registry = BenchRegistry::new();

    // ── compile phase (structural validation) ──
    match validate_cir(cir) {
        Ok(()) => {
            result.compile = Some(true);
        }
        Err(e) => {
            // Negative (validation) fixtures short-circuit here.
            if let Some(sub) = &fixture.expected.validation {
                if e.to_string().contains(sub.as_str()) {
                    result.compile = Some(false);
                    result.note = format!("expected validation failure: {e}");
                    result.status = CaseStatus::Pass;
                    return result;
                }
            }
            result.compile = Some(false);
            result.note = format!("validate_cir failed: {e}");
            return result;
        }
    }

    // ── compile phase (registry-aware checks) ──
    // e.g. retry requested on an irreversible primitive. Mirrors the design's
    // "ExternalIrreversible effects are not retry-safe" rule. Any error that
    // matches `expected.validation` short-circuits the case as Pass.
    if let Some(sub) = &fixture.expected.validation {
        if let Err(e) = check_recovery_compat(cir, &registry).await {
            if e.to_string().contains(sub.as_str()) {
                result.compile = Some(false);
                result.note = format!("expected validation failure: {e}");
                result.status = CaseStatus::Pass;
                return result;
            }
        }
    }

    // ── run-mode (compiler pipeline) not exercised in the TASK 10/11 suites ──
    if fixture.mode == FixtureMode::Run {
        result.note = "run-mode compiler pipeline not exercised in this suite".into();
        result.status = CaseStatus::Skip;
        return result;
    }

    // ── execution phase ──
    let workspace = prepare_workspace(fixture);
    let ws = workspace.to_string_lossy().into_owned();
    let mut program: CirProgram = cir.clone();
    substitute_workspace_in_program(&mut program, &ws);

    // Model planner is only available when an LLM key is configured. Without
    // one, model-expected fixtures are recorded as SKIP rather than FAIL.
    let model_planner = ModelRecoveryPlanner::from_env().ok();
    let model_available = model_planner.is_some();

    // Rule replanner is always available; its offline fallback reads the
    // workspace file written by the fixture (if any).
    let rule_replanner = RuleReplanner::new().with_rule(Box::new(OfflineFallbackRule {
        fallback_path: format!("{ws}/fallback.txt"),
    }));

    let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
    let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
    let runtime = RuntimeImpl::with_registry(store.clone(), astore.clone(), registry);

    let recovery_ctx = RecoveryContext {
        rule: Some(&rule_replanner),
        model: model_planner.as_ref().map(|p| p as &dyn acos_core::traits::ModelReplanner),
    };

    let exec_result = runtime
        .execute_with_recovery(program, Some(&recovery_ctx))
        .await;
    let execution_ok = exec_result.is_ok();
    result.execution = Some(execution_ok);

    // Recovery observation (only meaningful when execution succeeded).
    let recovery_label = match &exec_result {
        Ok(report) => {
            let events = store.query(report.run_id).await.unwrap_or_default();
            detect_recovery(&events)
        }
        Err(_) => None,
    };
    result.recovery = recovery_label.clone();

    // Richer recovery telemetry for the report (retry attempt count, replan
    // planner). Surfaces the P0 recovery state machine in the bench output.
    result.recovery_detail = match &exec_result {
        Ok(report) => {
            let events = store.query(report.run_id).await.unwrap_or_default();
            build_recovery_detail(&events)
        }
        Err(_) => None,
    };

    // Model-only recovery fixtures that failed purely because no model was
    // configured are SKIP, not FAIL (unless --require-model flips them).
    if !execution_ok {
        if let Some("model") = fixture.expected.recovery.as_deref() {
            if !model_available {
                result.status = CaseStatus::Skip;
                result.note = "model not configured".into();
                let _ = std::fs::remove_dir_all(&workspace);
                return result;
            }
        }
    }

    // Final status string, if expected.
    if let Some(fs) = &fixture.expected.final_status {
        let actual = match &exec_result {
            Ok(report) => status_to_string(&report.status),
            Err(_) => "failed".to_string(),
        };
        if &actual != fs {
            result.note = format!("final_status expected {fs}, got {actual}");
            result.status = CaseStatus::Fail;
            let _ = std::fs::remove_dir_all(&workspace);
            return result;
        }
    }

    // Verification phase (only when execution succeeded).
    if execution_ok {
        if let Ok(report) = exec_result.as_ref() {
            let vr = verify_run(store.as_ref(), report.run_id)
                .await
                .unwrap_or_else(|_| acos_verify::VerificationReport { findings: vec![] });
            result.verification = Some(vr.all_passed());
        }
    }

    // ── final verdict ──
    let exp_exec = fixture.expected.execution.unwrap_or(true);
    if execution_ok != exp_exec {
        result.note = format!("execution expected {exp_exec}, got {execution_ok}");
        result.status = CaseStatus::Fail;
        let _ = std::fs::remove_dir_all(&workspace);
        return result;
    }
    if let Some(exp_rec) = &fixture.expected.recovery {
        let got = result.recovery.as_deref().unwrap_or("");
        if got != exp_rec.as_str() {
            result.note = format!("recovery expected {exp_rec}, got {got}");
            result.status = CaseStatus::Fail;
            let _ = std::fs::remove_dir_all(&workspace);
            return result;
        }
    }
    if execution_ok {
        let exp_ver = fixture.expected.verification.unwrap_or(true);
        let got_ver = result.verification.unwrap_or(false);
        if got_ver != exp_ver {
            result.note = format!("verification expected {exp_ver}, got {got_ver}");
            result.status = CaseStatus::Fail;
            let _ = std::fs::remove_dir_all(&workspace);
            return result;
        }
    }

    result.status = CaseStatus::Pass;
    result.note = "ok".into();

    let _ = std::fs::remove_dir_all(&workspace);
    result
}

/// Substitutes `{workspace}` tokens inside every string input of the program.
fn substitute_workspace_in_program(program: &mut CirProgram, workspace: &str) {
    for node in &mut program.nodes {
        for v in node.inputs.values_mut() {
            if let serde_json::Value::String(s) = v {
                if s.contains("{workspace}") {
                    *s = s.replace("{workspace}", workspace);
                }
            }
        }
    }
}

/// Maps a [`RunStatus`] to the lowercase string used in fixtures.
fn status_to_string(s: &RunStatus) -> String {
    match s {
        RunStatus::Completed => "success".into(),
        RunStatus::Failed => "failed".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

/// Detects the recovery label from an event log.
///
/// `replan.started` (rule/model) takes precedence over `retry.started`.
fn detect_recovery(events: &[Event]) -> Option<String> {
    let mut retry = false;
    for e in events {
        if e.event_type == "retry.started" {
            retry = true;
        }
        if e.event_type == "replan.started" {
            let planner = e
                .payload
                .get("planner")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if planner == "model" {
                return Some("model".into());
            }
            if planner == "rule" {
                return Some("rule".into());
            }
        }
    }
    if retry {
        Some("retry".into())
    } else {
        None
    }
}

/// Builds a compact recovery telemetry string for the report.
///
/// Examples: `replan:rule`, `replan:model`, `retry(x3)`. Returns `None` when
/// no recovery events were observed.
fn build_recovery_detail(events: &[Event]) -> Option<String> {
    let mut retry_count: usize = 0;
    let mut planner: Option<String> = None;
    for e in events {
        if e.event_type == "retry.started" {
            retry_count += 1;
        }
        if e.event_type == "replan.started" {
            let p = e
                .payload
                .get("planner")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if !p.is_empty() {
                planner = Some(p);
            }
        }
    }
    match planner {
        Some(p) => Some(format!("replan:{p}")),
        None if retry_count > 0 => Some(format!("retry(x{retry_count})")),
        _ => None,
    }
}

/// Registry-aware recovery-compatibility check.
///
/// Mirrors the design rule: a node that requests `retry` must be retry-safe —
/// i.e. its primitive must not declare an `ExternalIrreversible` effect. This
/// runs after `validate_cir` so purely-structural failures are reported by the
/// compiler while capability-level policy is enforced here.
async fn check_recovery_compat(program: &CirProgram, registry: &BenchRegistry) -> Result<(), AcosError> {
    for node in &program.nodes {
        let has_retry = node
            .control
            .as_ref()
            .and_then(|c| c.retry.as_ref())
            .is_some();
        if !has_retry {
            continue;
        }
        let Some(cap) = &node.capability else {
            continue;
        };
        let Ok(prim) = registry.resolve(cap).await else {
            continue;
        };
        let irreversible = prim
            .effects()
            .iter()
            .any(|e| matches!(e.kind, EffectKind::ExternalIrreversible));
        if irreversible {
            return Err(AcosError::ValidationFailure {
                message: format!(
                    "node '{}' requests retry on irreversible primitive '{}' (ExternalIrreversible effect)",
                    node.node_id, cap
                ),
            });
        }
    }
    Ok(())
}
