//! ACOS cognitive runtime.
//!
//! Executes CIR programs by walking the graph, resolving primitives via a
//! registry, storing events/artifacts, and producing a verifiable run report.

#![warn(missing_docs)]

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use serde_json::Value;
use tokio::sync::Mutex;

use acos_core::error::AcosError;
use acos_core::expr;
use acos_core::id::RunId;
use acos_core::traits::{
    ArtifactStore, Event, EventStore, PluginRegistry, Primitive, RecoveryContext, RunStatus,
};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, EffectKind, FailureContext, LoopKind, RecoveryProposal,
    TypedValue, ValueType,
};
use acos_plugin::BuiltinRegistry;

// ── Run report ──────────────────────────────────────────────────────────────

/// A piece of evidence collected during a run.
#[derive(Debug, Clone, PartialEq)]
pub struct Evidence {
    /// Human-readable description.
    pub description: String,
    /// Whether this evidence was persisted to the event log.
    pub logged: bool,
}

impl Evidence {
    /// Returns `true` if this evidence was logged.
    pub fn is_logged(&self) -> bool {
        self.logged
    }
}

/// The result of executing a program.
#[derive(Debug, Clone, PartialEq)]
pub struct RunReport {
    /// The run id.
    pub run_id: RunId,
    /// Final status.
    pub status: RunStatus,
    /// Names of produced artifacts.
    pub artifacts: Vec<String>,
    /// Collected evidence.
    pub evidence: Vec<Evidence>,
}

impl RunReport {
    /// Returns the names of produced artifacts.
    pub fn artifacts(&self) -> &[String] {
        &self.artifacts
    }

    /// Returns the collected evidence.
    pub fn evidence(&self) -> &[Evidence] {
        &self.evidence
    }
}

// ── Runtime ──────────────────────────────────────────────────────────────────

/// The concrete ACOS runtime.
#[derive(Debug, Clone)]
pub struct RuntimeImpl {
    registry: Arc<dyn PluginRegistry>,
    event_store: Arc<dyn EventStore>,
    artifact_store: Arc<dyn ArtifactStore>,
}

impl RuntimeImpl {
    /// Creates a runtime with the default built-in registry and the given store.
    ///
    /// The store is used for both events and artifacts.
    pub fn new(store: Arc<dyn EventStore + Send + Sync>, store2: Arc<dyn ArtifactStore + Send + Sync>) -> Self {
        Self::with_registry(store, store2, BuiltinRegistry::new())
    }

    /// Creates a runtime with a custom registry.
    pub fn with_registry(
        event_store: Arc<dyn EventStore + Send + Sync>,
        artifact_store: Arc<dyn ArtifactStore + Send + Sync>,
        registry: impl PluginRegistry + 'static,
    ) -> Self {
        Self {
            registry: Arc::new(registry),
            event_store,
            artifact_store,
        }
    }

    /// Maximum recovery (replan) attempts per run before failing.
    pub const MAX_RECOVERY_ATTEMPTS: u32 = 3;

    /// Executes a program and returns a report.
    pub async fn execute(&self, program: CirProgram) -> Result<RunReport, AcosError> {
        self.execute_with_recovery(program, None).await
    }

    /// Executes a program with optional failure recovery.
    ///
    /// On failure: the rule replanner is tried first, then the model
    /// replanner. A proposal is only committed after [`Self::validate_proposal`]
    /// passes; the failing node is replaced with the subgraph root (which keeps
    /// the failing node's id) and the whole program is re-run from `entry`. The
    /// env carries over across attempts so produced bindings survive.
    pub async fn execute_with_recovery(
        &self,
        program: CirProgram,
        recovery: Option<&RecoveryContext<'_>>,
    ) -> Result<RunReport, AcosError> {
        self.execute_with_env(program, recovery, HashMap::new()).await
    }

    /// Executes a program with a pre-seeded environment (for Golden CIR testing
    /// and programmatic invocation where inputs are already materialized).
    pub async fn execute_with_env(
        &self,
        program: CirProgram,
        recovery: Option<&RecoveryContext<'_>>,
        seed_env: HashMap<String, TypedValue>,
    ) -> Result<RunReport, AcosError> {
        let run_id = RunId::new();
        self.event_store
            .append(
                run_id,
                "run.started".into(),
                serde_json::json!({ "program_id": program.id.0 }),
            )
            .await?;

        let env = Arc::new(Mutex::new(seed_env));
        let mut program = program;
        let mut attempts = 0u32;

        loop {
            let node_map: HashMap<String, CirNode> = program
                .nodes
                .iter()
                .map(|n| (n.node_id.clone(), n.clone()))
                .collect();

            match self
                .run_nodes(&node_map, &program.entry, run_id, env.clone())
                .await
            {
                Ok((produced, evidence)) => {
                    self.event_store
                        .append(
                            run_id,
                            "run.finished".into(),
                            serde_json::json!({ "status": "Completed" }),
                        )
                        .await
                        .ok();
                    return Ok(RunReport {
                        run_id,
                        status: RunStatus::Completed,
                        artifacts: produced,
                        evidence,
                    });
                }
                Err((node_id, e)) => {
                    let mut recovered = false;
                    if attempts < Self::MAX_RECOVERY_ATTEMPTS {
                        if let Some(ctx) = recovery {
                            let class = e.classify();
                            let recent_events: Vec<Event> = self
                                .event_store
                                .query(run_id)
                                .await
                                .unwrap_or_default()
                                .into_iter()
                                .rev()
                                .take(5)
                                .collect();
                            let failure = FailureContext {
                                run_id,
                                node_id: node_id.clone(),
                                error_class: class,
                                error_message: e.to_string(),
                                attempts,
                                recent_events,
                            };
                            if let Some(rule) = ctx.rule {
                                if let Some(proposal) = rule.propose(&failure, &program) {
                                    recovered = self
                                        .try_commit(&run_id, &mut program, &proposal, "rule")
                                        .await;
                                }
                            }
                            if !recovered {
                                if let Some(model) = ctx.model {
                                    if let Ok(Some(proposal)) =
                                        model.propose(&failure, &program).await
                                    {
                                        recovered = self
                                            .try_commit(&run_id, &mut program, &proposal, "model")
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                    if !recovered {
                        self.event_store
                            .append(
                                run_id,
                                "run.finished".into(),
                                serde_json::json!({ "status": "Failed" }),
                            )
                            .await
                            .ok();
                        return Err(e);
                    }
                    attempts += 1;
                }
            }
        }
    }

    /// Validates a recovery proposal and, if valid, commits it into `program`
    /// (transactional patch). Emits `replan.started` / `replan.completed` /
    /// `replan.rejected`.
    async fn try_commit(
        &self,
        run_id: &RunId,
        program: &mut CirProgram,
        proposal: &RecoveryProposal,
        planner: &str,
    ) -> bool {
        self.event_store
            .append(
                *run_id,
                "replan.started".into(),
                serde_json::json!({
                    "planner": planner,
                    "node_id": proposal.replace_node,
                    "reason": proposal.reason,
                }),
            )
            .await
            .ok();
        match self.validate_proposal(program, proposal).await {
            Ok(()) => {
                program.nodes.retain(|n| n.node_id != proposal.replace_node);
                program.nodes.extend(proposal.subgraph.clone());
                self.event_store
                    .append(
                        *run_id,
                        "replan.completed".into(),
                        serde_json::json!({
                            "planner": planner,
                            "node_id": proposal.replace_node,
                            "subgraph_nodes": proposal
                                .subgraph
                                .iter()
                                .map(|n| &n.node_id)
                                .collect::<Vec<_>>(),
                        }),
                    )
                    .await
                    .ok();
                true
            }
            Err(ve) => {
                self.event_store
                    .append(
                        *run_id,
                        "replan.rejected".into(),
                        serde_json::json!({
                            "planner": planner,
                            "node_id": proposal.replace_node,
                            "error": ve.to_string(),
                        }),
                    )
                    .await
                    .ok();
                false
            }
        }
    }

    /// Transactional commit gate for a recovery proposal.
    ///
    /// 1. Subgraph root must reuse `replace_node`'s id.
    /// 2. Node ids unique within program ∪ subgraph.
    /// 3. Every child reference resolves within subgraph ∪ program.
    /// 4. Every capability resolves via the registry.
    /// 5. Every effect kind declared by the subgraph primitives is already
    ///    declared in `program.effects`.
    pub async fn validate_proposal(
        &self,
        program: &CirProgram,
        proposal: &RecoveryProposal,
    ) -> Result<(), AcosError> {
        let root = proposal.subgraph.first().ok_or_else(|| AcosError::ValidationFailure {
            message: "recovery proposal has empty subgraph".into(),
        })?;
        if root.node_id != proposal.replace_node {
            return Err(AcosError::ValidationFailure {
                message: format!(
                    "recovery subgraph root '{}' must reuse replace_node id '{}'",
                    root.node_id, proposal.replace_node
                ),
            });
        }
        let mut known: HashSet<&str> =
            program.nodes.iter().map(|n| n.node_id.as_str()).collect();
        for node in &proposal.subgraph {
            if node.node_id != proposal.replace_node && !known.insert(node.node_id.as_str()) {
                return Err(AcosError::ValidationFailure {
                    message: format!(
                        "recovery subgraph introduces duplicate node id '{}'",
                        node.node_id
                    ),
                });
            }
        }
        for node in &proposal.subgraph {
            for child in &node.children {
                let in_subgraph = proposal.subgraph.iter().any(|s| &s.node_id == child);
                if !in_subgraph && !known.contains(child.as_str()) {
                    return Err(AcosError::ValidationFailure {
                        message: format!("recovery subgraph references unknown child '{child}'"),
                    });
                }
            }
        }
        for node in &proposal.subgraph {
            if let Some(capability) = &node.capability {
                let primitive = self
                    .registry
                    .resolve(capability)
                    .await
                    .map_err(|_| AcosError::ValidationFailure {
                        message: format!(
                            "recovery subgraph capability '{capability}' is unavailable"
                        ),
                    })?;
                for effect in primitive.effects() {
                    if !program.effects.iter().any(|d| d.kind == effect.kind) {
                        return Err(AcosError::ValidationFailure {
                            message: format!(
                                "recovery subgraph effect {:?} not declared in program.effects",
                                effect.kind
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// Iteratively executes nodes and returns produced artifacts + evidence.
    async fn run_nodes(
        &self,
        node_map: &HashMap<String, CirNode>,
        entries: &[String],
        run_id: RunId,
        env: Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> Result<(Vec<String>, Vec<Evidence>), (String, AcosError)> {
        let mut artifacts = Vec::new();
        let mut evidence = Vec::new();
        for entry in entries {
            let node = node_map.get(entry).ok_or_else(|| {
                (
                    entry.clone(),
                    AcosError::RuntimeInfrastructureFailure {
                        message: format!("node {entry} not found"),
                    },
                )
            })?;
            self.run_node(node, run_id, &env, &mut artifacts, &mut evidence, node_map)
                .await?;
        }
        Ok((artifacts, evidence))
    }

    /// Executes a single node (with its `control.retry` policy applied).
    #[allow(clippy::too_many_arguments)]
    async fn run_node(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        // Resolve the primitive once so retries reuse the same instance
        // (preserving internal failure-count state across attempts).
        let resolved: Option<Arc<dyn Primitive>> =
            if node.kind == CirNodeKind::PrimitiveInvocation {
                let cap = node.capability.clone().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::RuntimeInfrastructureFailure {
                            message: format!("primitive node {} has no capability", node.node_id),
                        },
                    )
                })?;
                Some(Arc::from(
                    self.registry
                        .resolve(&cap)
                        .await
                        .map_err(|e| (node.node_id.clone(), e))?,
                ))
            } else {
                None
            };

        let retry = node.control.as_ref().and_then(|c| c.retry.clone());
        if let Some(policy) = retry {
            let mut attempt = 0u32;
            loop {
                attempt += 1;
                match self
                    .run_node_inner(node, run_id, env, artifacts, evidence, node_map, &resolved)
                    .await
                {
                    Ok(()) => return Ok(()),
                    Err((failing_id, e)) => {
                        let class = e.classify();
                        let retryable =
                            policy.retry_on.is_empty() || policy.retry_on.contains(&class);
                        let safe = self.retry_safe(node, resolved.as_ref()).await;
                        if attempt >= policy.max_attempts || !retryable || !safe {
                            if attempt > 1 {
                                self.event_store
                                    .append(
                                        run_id,
                                        "retry.exhausted".into(),
                                        serde_json::json!({
                                            "node_id": &node.node_id,
                                            "attempts": attempt,
                                        }),
                                    )
                                    .await
                                    .ok();
                            }
                            return Err((failing_id, e));
                        }
                        self.event_store
                            .append(
                                run_id,
                                "retry.started".into(),
                                serde_json::json!({
                                    "node_id": &node.node_id,
                                    "attempt": attempt,
                                    "class": format!("{class:?}"),
                                }),
                            )
                            .await
                            .ok();
                        tokio::time::sleep(Duration::from_millis(policy.backoff_ms)).await;
                    }
                }
            }
        }
        self.run_node_inner(node, run_id, env, artifacts, evidence, node_map, &resolved)
            .await
    }

    /// Retry-safety gate: only pure-read effects or explicitly idempotent
    /// primitives may be auto-retried (conservative MVP rule).
    async fn retry_safe(&self, node: &CirNode, resolved: Option<&Arc<dyn Primitive>>) -> bool {
        if node.kind != CirNodeKind::PrimitiveInvocation {
            return true;
        }
        let Some(prim) = resolved else {
            return false;
        };
        prim.idempotent()
            || prim
                .effects()
                .iter()
                .all(|e| matches!(e.kind, EffectKind::FsRead | EffectKind::NetworkRead))
    }

    /// Executes a single node by kind (no retry policy applied).
    #[allow(clippy::too_many_arguments)]
    async fn run_node_inner(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
        resolved: &Option<Arc<dyn Primitive>>,
    ) -> Result<(), (String, AcosError)> {
        match node.kind {
            CirNodeKind::Sequence | CirNodeKind::Parallel => {
                self.event_store
                    .append(
                        run_id,
                        "node.start".into(),
                        serde_json::json!({
                            "node_id": &node.node_id,
                            "kind": format!("{:?}", node.kind),
                        }),
                    )
                    .await
                    .map_err(|e| (node.node_id.clone(), e))?;
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        (
                            node.node_id.clone(),
                            AcosError::RuntimeInfrastructureFailure {
                                message: format!("child node {child_id} not found"),
                            },
                        )
                    })?;
                    Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                        .await?;
                }
                Ok(())
            }
            CirNodeKind::PrimitiveInvocation => {
                let prim = match resolved {
                    Some(p) => p.clone(),
                    None => Arc::from(
                        self.registry
                            .resolve(node.capability.as_deref().unwrap_or(""))
                            .await
                            .map_err(|e| (node.node_id.clone(), e))?,
                    ),
                };
                self.run_primitive(node, run_id, env, artifacts, evidence, &*prim)
                    .await
                    .map_err(|e| (node.node_id.clone(), e))
            }
            CirNodeKind::Conditional => {
                self.run_conditional(node, run_id, env, artifacts, evidence, node_map)
                    .await
            }
            CirNodeKind::LoopMap => {
                self.run_loop(node, run_id, env, artifacts, evidence, node_map)
                    .await
            }
            _ => {
                // Checkpoint / Verification / ArtifactRef / Retry (deprecated):
                // passthrough children, unchanged.
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        (
                            node.node_id.clone(),
                            AcosError::RuntimeInfrastructureFailure {
                                message: format!("child node {child_id} not found"),
                            },
                        )
                    })?;
                    Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                        .await?;
                }
                Ok(())
            }
        }
    }

    /// Executes a conditional node: evaluate `control.condition`, then run the
    /// `then` branch (`children`) or the `else` branch (`else_children`).
    #[allow(clippy::too_many_arguments)]
    async fn run_conditional(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        let cond = node
            .control
            .as_ref()
            .and_then(|c| c.condition.as_ref())
            .ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::ValidationFailure {
                        message: format!(
                            "conditional node '{}' has no control.condition",
                            node.node_id
                        ),
                    },
                )
            })?;
        let expr = expr::parse(&cond.expression).map_err(|e| {
            (
                node.node_id.clone(),
                AcosError::ValidationFailure {
                    message: format!("conditional node '{}': {e}", node.node_id),
                },
            )
        })?;
        let guard = env.lock().await;
        let result = expr::evaluate(&expr, &guard)
            .map_err(|e| (node.node_id.clone(), e))?;
        drop(guard);

        let branch = if result { &node.children } else { &node.else_children };
        self.event_store
            .append(
                run_id,
                "node.start".into(),
                serde_json::json!({
                    "node_id": &node.node_id,
                    "kind": "conditional",
                    "branch": if result { "then" } else { "else" },
                }),
            )
            .await
            .map_err(|e| (node.node_id.clone(), e))?;

        for child_id in branch {
            let child = node_map.get(child_id).ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::RuntimeInfrastructureFailure {
                        message: format!("child node {child_id} not found"),
                    },
                )
            })?;
            Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map)).await?;
        }
        Ok(())
    }

    /// Executes a loop node (while/until/for_each) per the CIR spec.
    #[allow(clippy::too_many_arguments)]
    async fn run_loop(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), (String, AcosError)> {
        let spec = node
            .control
            .as_ref()
            .and_then(|c| c.loop_spec.as_ref())
            .ok_or_else(|| {
                (
                    node.node_id.clone(),
                    AcosError::ValidationFailure {
                        message: format!("loop node '{}' has no control.loop_spec", node.node_id),
                    },
                )
            })?;

        let mut iteration: u32 = 0;
        match spec.kind {
            LoopKind::While | LoopKind::Until => {
                let cond = spec.condition.clone().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: format!(
                                "loop node '{}' {:?} requires a condition",
                                node.node_id, spec.kind
                            ),
                        },
                    )
                })?;
                let mut collected: Vec<serde_json::Value> = Vec::new();
                loop {
                    if let Some(max) = spec.max_iterations {
                        if iteration >= max {
                            return Err((
                                node.node_id.clone(),
                                AcosError::RuntimeInfrastructureFailure {
                                    message: format!(
                                        "loop node '{}' reached max_iterations ({max}) without \
                                         satisfying exit condition",
                                        node.node_id
                                    ),
                                },
                            ));
                        }
                    }
                    iteration += 1;
                    let expr = expr::parse(&cond).map_err(|e| {
                        (
                            node.node_id.clone(),
                            AcosError::ValidationFailure {
                                message: format!("loop node '{}': {e}", node.node_id),
                            },
                        )
                    })?;
                    let guard = env.lock().await;
                    let result = expr::evaluate(&expr, &guard)
                        .map_err(|e| (node.node_id.clone(), e))?;
                    drop(guard);

                    if spec.kind == LoopKind::Until && result {
                        break;
                    }
                    if spec.kind == LoopKind::While && !result {
                        break;
                    }

                    self.emit_iteration(run_id, &node.node_id, iteration).await?;
                    for child_id in &node.children {
                        let child = node_map.get(child_id).ok_or_else(|| {
                            (
                                node.node_id.clone(),
                                AcosError::RuntimeInfrastructureFailure {
                                    message: format!("child node {child_id} not found"),
                                },
                            )
                        })?;
                        Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                            .await?;
                    }
                    self.capture_loop_output(node, node_map, env, &mut collected).await;
                    self.complete_iteration(run_id, &node.node_id, iteration).await?;
                }
                self.bind_loop_output(node, env, collected).await;
                Ok(())
            }
            LoopKind::ForEach => {
                let input = spec.input.clone().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: format!(
                                "loop node '{}' for_each requires input",
                                node.node_id
                            ),
                        },
                    )
                })?;
                let item_var = spec.item_var.clone().ok_or_else(|| {
                    (
                        node.node_id.clone(),
                        AcosError::ValidationFailure {
                            message: format!(
                                "loop node '{}' for_each requires itemVar",
                                node.node_id
                            ),
                        },
                    )
                })?;
                let items = self
                    .resolve_ref_value(&input, env)
                    .await
                    .map(|tv| tv.payload.as_array().cloned().unwrap_or_default())
                    .unwrap_or_default();
                let max = spec.max_iterations.map(|m| m as usize);
                let mut collected: Vec<serde_json::Value> = Vec::new();
                for (i, item) in items.iter().enumerate() {
                    if let Some(max) = max {
                        if i >= max {
                            break;
                        }
                    }
                    {
                        let mut guard = env.lock().await;
                        guard.insert(
                            item_var.clone(),
                            TypedValue {
                                value_type: ValueType::Scalar,
                                payload: item.clone(),
                            },
                        );
                    }
                    self.emit_iteration(run_id, &node.node_id, (i + 1) as u32).await?;
                    for child_id in &node.children {
                        let child = node_map.get(child_id).ok_or_else(|| {
                            (
                                node.node_id.clone(),
                                AcosError::RuntimeInfrastructureFailure {
                                    message: format!("child node {child_id} not found"),
                                },
                            )
                        })?;
                        Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                            .await?;
                    }
                    self.capture_loop_output(node, node_map, env, &mut collected).await;
                    self.complete_iteration(run_id, &node.node_id, (i + 1) as u32)
                        .await?;
                }
                self.bind_loop_output(node, env, collected).await;
                Ok(())
            }
        }
    }

    /// Captures the last child's declared output value into `collected`.
    ///
    /// Semantics: a `loop_map` node's `output` is an array of the per-iteration
    /// values produced by its last child. This is how a loop aggregates
    /// per-item results for downstream nodes (e.g. `"${all_results}"`).
    async fn capture_loop_output(
        &self,
        node: &CirNode,
        node_map: &HashMap<String, CirNode>,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        collected: &mut Vec<serde_json::Value>,
    ) {
        let Some(child_id) = node.children.last() else { return };
        let Some(child) = node_map.get(child_id) else { return };
        let Some(out_name) = child.output.as_ref().map(|o| o.name.clone()) else { return };
        let guard = env.lock().await;
        if let Some(tv) = guard.get(&out_name) {
            collected.push(tv.payload.clone());
        }
    }

    /// Binds the loop node's declared `output` to the collected per-iteration
    /// values (empty array if no iterations completed).
    async fn bind_loop_output(
        &self,
        node: &CirNode,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        collected: Vec<serde_json::Value>,
    ) {
        let Some(out_name) = node.output.as_ref().map(|o| o.name.clone()) else { return };
        let mut guard = env.lock().await;
        guard.insert(
            out_name,
            TypedValue {
                value_type: ValueType::List,
                payload: serde_json::Value::Array(collected),
            },
        );
    }

    /// Emits an `iteration.started` event.
    async fn emit_iteration(
        &self,
        run_id: RunId,
        node_id: &str,
        index: u32,
    ) -> Result<(), (String, AcosError)> {
        self.event_store
            .append(
                run_id,
                "iteration.started".into(),
                serde_json::json!({ "node_id": node_id, "index": index }),
            )
            .await
            .map_err(|e| (node_id.to_string(), e))?;
        Ok(())
    }

    /// Emits an `iteration.completed` event.
    async fn complete_iteration(
        &self,
        run_id: RunId,
        node_id: &str,
        index: u32,
    ) -> Result<(), (String, AcosError)> {
        self.event_store
            .append(
                run_id,
                "iteration.completed".into(),
                serde_json::json!({ "node_id": node_id, "index": index }),
            )
            .await
            .map_err(|e| (node_id.to_string(), e))?;
        Ok(())
    }

    /// Runs a primitive invocation node.
    async fn run_primitive(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        primitive: &dyn Primitive,
    ) -> Result<(), AcosError> {
        let capability = node.capability.as_ref().ok_or_else(|| {
            AcosError::RuntimeInfrastructureFailure {
                message: format!("primitive node {} has no capability", node.node_id),
            }
        })?;

        let input_value = self.resolve_inputs(&node.inputs, env).await;

        self.event_store
            .append(
                run_id,
                "primitive.start".into(),
                serde_json::json!({ "node_id": &node.node_id, "capability": capability }),
            )
            .await?;

        let output = primitive.invoke(input_value).await?;

        if let Some(name) = node.output.as_ref().map(|o| o.name.clone()) {
            env.lock().await.insert(name, output.clone());
        }

        // Record artifact for write_file.
        if capability == "write_file" {
            if let Some(path_val) = node.inputs.get("path") {
                let path_str = path_val.as_str().unwrap_or("");
                let content_val = node.inputs.get("content").cloned().unwrap_or_default();
                let content_str = match &content_val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                let resolved_content = resolve_ref(&content_str, env).await;
                let bytes = resolved_content.into_bytes();
                // Write to host filesystem so the artifact is usable.
                let _ = tokio::fs::write(path_str, &bytes).await;
                match self
                    .artifact_store
                    .put(run_id, path_str.to_string(), bytes)
                    .await
                {
                    Ok(id) => {
                        artifacts.push(path_str.to_string());
                        evidence.push(Evidence {
                            description: format!("produced artifact {} ({id:?})", path_str),
                            logged: true,
                        });
                    }
                    Err(e) => return Err(e),
                }
            }
        } else {
            evidence.push(Evidence {
                description: format!("invoked {capability} at node {}", node.node_id),
                logged: true,
            });
        }

        self.event_store
            .append(
                run_id,
                "primitive.end".into(),
                serde_json::json!({ "node_id": &node.node_id, "capability": capability, "ok": true }),
            )
            .await?;

        Ok(())
    }

    /// Resolves a `${name}` / `$name` reference into its `TypedValue` from the
    /// environment (exact lookup; no fuzzy matching).
    async fn resolve_ref_value(
        &self,
        ref_str: &str,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> Option<TypedValue> {
        let name = if ref_str.starts_with("${") && ref_str.ends_with('}') {
            &ref_str[2..ref_str.len() - 1]
        } else if ref_str.starts_with('$') {
            &ref_str[1..]
        } else {
            return None;
        };
        env.lock().await.get(name).cloned()
    }

    /// Resolves a node's input bindings into a single TypedValue.
    async fn resolve_inputs(
        &self,
        inputs: &HashMap<String, serde_json::Value>,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> TypedValue {
        if inputs.is_empty() {
            return TypedValue {
                value_type: ValueType::Scalar,
                payload: Value::Null,
            };
        }

        let mut map = serde_json::Map::new();
        for (k, v) in inputs {
            let resolved = self.resolve_value(v, env).await;
            map.insert(k.clone(), resolved.payload);
        }
        TypedValue {
            value_type: ValueType::Record,
            payload: Value::Object(map),
        }
    }

    /// Resolves a single input value with reference substitution.
    async fn resolve_value(
        &self,
        raw: &serde_json::Value,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> TypedValue {
        let resolved = resolve_ref(raw.as_str().unwrap_or(&raw.to_string()), env).await;
        let payload = serde_json::from_str::<Value>(&resolved)
            .unwrap_or_else(|_| Value::String(resolved));
        let value_type = match &payload {
            Value::Array(_) => ValueType::List,
            Value::Object(_) => ValueType::Record,
            _ => ValueType::Scalar,
        };
        TypedValue { value_type, payload }
    }
}

/// Resolves a dotted `${a.b.c}` reference into its nested `TypedValue` from
/// the environment: the first segment names an env binding, the remaining
/// segments walk the payload's JSON object fields. Static paths only — no
/// `[index]` support. Returns `None` when any segment is missing or the
/// intermediate value is not an object.
async fn resolve_dotted(
    ref_str: &str,
    env: &Arc<Mutex<HashMap<String, TypedValue>>>,
) -> Option<TypedValue> {
    let name = ref_str.strip_prefix("${").and_then(|s| s.strip_suffix('}'))?;
    let mut parts = name.split('.');
    let head = parts.next()?;
    let guard = env.lock().await;
    let mut cur = guard.get(head)?.clone();
    drop(guard);
    for seg in parts {
        match &cur.payload {
            serde_json::Value::Object(map) => {
                let v = map.get(seg)?;
                cur = TypedValue {
                    value_type: if v.is_array() { ValueType::List } else if v.is_object() { ValueType::Record } else { ValueType::Scalar },
                    payload: v.clone(),
                };
            }
            _ => return None,
        }
    }
    Some(cur)
}

/// Resolves a single `$name` / `${name}` token: exact env lookup first, then
/// dotted path walk. Returns the token unchanged when unresolvable.
async fn resolve_ref_token(token: &str, env: &Arc<Mutex<HashMap<String, TypedValue>>>) -> String {
    let tv = {
        let guard = env.lock().await;
        let name = token
            .strip_prefix("${")
            .and_then(|s| s.strip_suffix('}'))
            .or_else(|| token.strip_prefix('$'));
        match name {
            Some(name) => guard.get(name).cloned(),
            None => return token.to_string(),
        }
    };
    let tv = match tv {
        Some(tv) => Some(tv),
        None => resolve_dotted(token, env).await,
    };
    match tv {
        Some(tv) => match &tv.payload {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        },
        None => token.to_string(),
    }
}

/// Resolves a `$name` or `${name}` reference to its string form.
///
/// If the entire string is a single reference, returns the resolved value.
/// Otherwise, performs template interpolation: every `${name}` substring is
/// replaced with the resolved value (supports embedded references inside
/// larger strings like Python source code).
async fn resolve_ref(ref_str: &str, env: &Arc<Mutex<HashMap<String, TypedValue>>>) -> String {
    // Fast path: the whole string is exactly one reference.
    if (ref_str.starts_with("${") && ref_str.ends_with('}'))
        || (ref_str.starts_with('$') && !ref_str[1..].contains(' '))
    {
        return resolve_ref_token(ref_str, env).await;
    }

    // Slow path: template interpolation — replace all `${name}` occurrences.
    let mut result = String::with_capacity(ref_str.len());
    let mut rest = ref_str;
    while let Some(start) = rest.find("${") {
        result.push_str(&rest[..start]);
        if let Some(end) = rest[start..].find('}') {
            let token = &rest[start..start + end + 1];
            result.push_str(&resolve_ref_token(token, env).await);
            rest = &rest[start + end + 1..];
        } else {
            result.push_str(&rest[start..]);
            rest = "";
        }
    }
    result.push_str(rest);
    result
}

/// Re-export as `Runtime` for convenience.
pub use RuntimeImpl as Runtime;

pub mod replan;

pub use replan::{OfflineFallbackRule, RecoveryRule, RuleReplanner};

#[cfg(test)]
mod tests {
    use super::*;
    use acos_compiler::RuleCompiler;
    use async_trait::async_trait;
    use acos_core::id::PrimitiveId;
    use acos_core::traits::{
        CapabilityDesc, Compiler, PluginRegistry, Primitive, PrimitiveManifest, RecoveryContext,
        Replanner,
    };
    use acos_core::types::{
        CirNode, CirNodeKind, ConditionSpec, ControlSpec, EffectDecl, EffectKind, FailureClass,
        FailureContext, LoopKind, LoopSpec, OutputSpec, RecoveryProposal, RetryPolicy,
        RetryStrategy, TaskInput, TaskSpec,
    };
    use acos_state::InMemoryStore;

    fn csv_task() -> TaskSpec {
        TaskSpec {
            api_version: "acos.io/v1".into(),
            id: acos_core::id::TaskId::new(),
            goal: "analyze csv files and report".into(),
            inputs: vec![
                TaskInput { input_type: "File".into(), path: "a.csv".into(), format: None },
                TaskInput { input_type: "File".into(), path: "b.csv".into(), format: None },
            ],
            outputs: vec![],
            constraints: None,
            optimization: None,
            approval: None,
        }
    }

    #[tokio::test]
    async fn runtime_executes_pipeline_and_produces_report_artifact() {
        std::env::remove_var("LONGCAT_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        let dir = std::env::temp_dir().join(format!("acos-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.csv"), "name,value\nalpha,1\nbeta,2\n").unwrap();
        std::fs::write(dir.join("b.csv"), "name,value\ngamma,3\ndelta,4\n").unwrap();

        let mut task = csv_task();
        task.inputs[0].path = dir.join("a.csv").to_string_lossy().to_string();
        task.inputs[1].path = dir.join("b.csv").to_string_lossy().to_string();

        let program = RuleCompiler::new().compile(task).await.unwrap().program;
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let report = runtime.execute(program).await.expect("execute");

        assert_eq!(report.status, RunStatus::Completed);
        assert!(
            report.artifacts().contains(&"report.md".to_string()),
            "expected report.md in {:?}",
            report.artifacts()
        );
        assert!(report.evidence().iter().all(|e| e.is_logged()));

        assert!(report.artifacts().contains(&"report.md".to_string()), "artifact persisted");

        std::fs::remove_dir_all(&dir).ok();
    }

    // ── Control-semantics tests ───────────────────────────────────────────────

    fn primitive_node(id: &str, capability: &str, output: Option<&str>) -> CirNode {
        CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: id.into(),
            capability: Some(capability.into()),
            output: output.map(|name| OutputSpec {
                name: name.into(),
                type_name: "String".into(),
                fields: vec![],
            }),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            input_types: HashMap::new(),
            control: None,
        }
    }

    fn program_from(nodes: Vec<CirNode>) -> CirProgram {
        CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["root".into()],
            nodes,
            effects: vec![],
        }
    }

    async fn events_for(program: &CirProgram) -> Vec<String> {
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let report = runtime.execute(program.clone()).await.unwrap();
        store
            .query(report.run_id)
            .await
            .unwrap()
            .into_iter()
            .map(|e| e.event_type)
            .collect()
    }

    #[tokio::test]
    async fn conditional_selects_then_branch() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["search".into(), "check".into(), "then_summarize".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            primitive_node("search", "search", Some("results")),
            CirNode {
                kind: CirNodeKind::Conditional,
                node_id: "check".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["then_summarize".into()],
                else_children: vec!["else_write".into()],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: Some(ConditionSpec {
                        expression: "exists(results)".into(),
                    }),
                    loop_spec: None,
                    retry: None,
                }),
            },
            primitive_node("then_summarize", "summarize", Some("summary")),
            primitive_node("else_write", "write_file", Some("ref")),
        ];
        let events = events_for(&program_from(nodes)).await;
        let then = events.iter().filter(|t| *t == "primitive.end").count();
        assert!(then >= 2, "then branch should run summarize (+search)");
    }

    #[tokio::test]
    async fn conditional_selects_else_branch_on_false_condition() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["check".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::Conditional,
                node_id: "check".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["then_summarize".into()],
                else_children: vec!["else_write".into()],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: Some(ConditionSpec {
                        expression: "1 == 2".into(),
                    }),
                    loop_spec: None,
                    retry: None,
                }),
            },
            primitive_node("then_summarize", "summarize", Some("summary")),
            primitive_node("else_write", "write_file", Some("ref")),
        ];
        let dir = std::env::temp_dir().join(format!("acos-else-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut nodes = nodes;
        let write = nodes.iter_mut().find(|n| n.node_id == "else_write").unwrap();
        write.inputs.insert(
            "path".into(),
            serde_json::Value::String(dir.join("out.txt").to_string_lossy().to_string()),
        );
        write.inputs.insert("content".into(), serde_json::Value::String("fallback".into()));
        let program = program_from(nodes);
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let report = runtime.execute(program).await.unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        assert!(
            report.artifacts.contains(&dir.join("out.txt").to_string_lossy().to_string()),
            "else branch write_file must produce the artifact"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn for_each_loop_over_empty_list_is_ok() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["search".into(), "loop".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            primitive_node("search", "search", Some("items")),
            CirNode {
                kind: CirNodeKind::LoopMap,
                node_id: "loop".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["body".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: None,
                    loop_spec: Some(LoopSpec {
                        kind: LoopKind::ForEach,
                        condition: None,
                        max_iterations: None,
                        input: Some("${items}".into()),
                        item_var: Some("item".into()),
                    }),
                    retry: None,
                }),
            },
            primitive_node("body", "summarize", Some("summary")),
        ];
        let events = events_for(&program_from(nodes)).await;
        assert_eq!(events.iter().filter(|t| *t == "iteration.started").count(), 0);
    }

    #[tokio::test]
    async fn for_each_loop_binds_aggregated_output() {
        let mut body = primitive_node("body", "echo_item", Some("file_result"));
        body.inputs.insert("item".into(), serde_json::Value::String("${item}".into()));
        let loop_node = CirNode {
            kind: CirNodeKind::LoopMap,
            node_id: "loop".into(),
            capability: None,
            output: Some(OutputSpec {
                name: "all_results".into(),
                type_name: "List<String>".into(),
                fields: vec![],
            }),
            children: vec!["body".into()],
            else_children: vec![],
            inputs: HashMap::new(),
            input_types: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: Some(LoopSpec {
                    kind: LoopKind::ForEach,
                    condition: None,
                    max_iterations: None,
                    input: Some("${items}".into()),
                    item_var: Some("item".into()),
                }),
                retry: None,
            }),
        };
        let mut consumer = primitive_node("consumer", "verify_output", Some("consumed"));
        consumer.inputs.insert("item".into(), serde_json::Value::String("${all_results}".into()));
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["init".into(), "loop".into(), "consumer".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            {
                let mut init = primitive_node("init", "echo_list", Some("items"));
                init.inputs.insert(
                    "items".into(),
                    serde_json::json!(["a", "b", "c"]),
                );
                init
            },
            loop_node,
            body,
            consumer,
        ];

        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, EchoRegistry { inner: acos_plugin::BuiltinRegistry::new() });
        let report = runtime.execute(program_from(nodes)).await.unwrap();
        assert_eq!(
            report.status,
            RunStatus::Completed,
            "loop output aggregation must feed downstream consumers"
        );
    }

    #[tokio::test]
    async fn while_loop_hits_limit_and_fails() {
        let nodes = vec![
            CirNode {
                kind: CirNodeKind::LoopMap,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["body".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: Some(ControlSpec {
                    condition: None,
                    loop_spec: Some(LoopSpec {
                        kind: LoopKind::While,
                        condition: Some("1 == 1".into()),
                        max_iterations: Some(2),
                        input: None,
                        item_var: None,
                    }),
                    retry: None,
                }),
            },
            primitive_node("body", "search", Some("r")),
        ];
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::new(store.clone(), astore);
        let err = runtime.execute(program_from(nodes)).await.unwrap_err();
        assert!(err.to_string().contains("max_iterations"));
    }

    #[tokio::test]
    async fn retry_recovers_transient_failure_then_succeeds() {
        let nodes = vec![CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "root".into(),
            capability: Some("flaky_search".into()),
            output: Some(OutputSpec {
                name: "r".into(),
                type_name: "String".into(),
                fields: vec![],
            }),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            input_types: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: None,
                retry: Some(RetryPolicy {
                    max_attempts: 3,
                    backoff_ms: 1,
                    strategy: RetryStrategy::Fixed,
                    retry_on: vec![FailureClass::Timeout],
                }),
            }),
        }];
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let report = runtime.execute(program_from(nodes)).await.unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        let events = store.query(report.run_id).await.unwrap();
        assert!(events.iter().any(|e| e.event_type == "retry.started"));
    }

    // ── Flaky test primitive + registry (inline; bench gets a shared one) ─────

    #[derive(Debug)]
    struct FlakySearchPrimitive {
        remaining: std::sync::atomic::AtomicUsize,
    }

    impl FlakySearchPrimitive {
        fn new(failures: usize) -> Self {
            Self {
                remaining: std::sync::atomic::AtomicUsize::new(failures),
            }
        }
    }

    #[async_trait]
    impl Primitive for FlakySearchPrimitive {
        fn capability(&self) -> CapabilityDesc {
            CapabilityDesc {
                id: "flaky_search".into(),
                name: "Flaky Search".into(),
                input_type: "SearchQuery".into(),
                output_type: "DocumentList".into(),
            }
        }

        fn effects(&self) -> Vec<EffectDecl> {
            vec![EffectDecl {
                kind: EffectKind::NetworkRead,
                description: "network read".into(),
                reversible: true,
            }]
        }

        async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
            if self
                .remaining
                .load(std::sync::atomic::Ordering::SeqCst)
                > 0
            {
                self.remaining
                    .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                return Err(AcosError::PrimitiveFailure {
                    message: "simulated timeout".into(),
                    primitive_id: Some("flaky_search".into()),
                    class: FailureClass::Timeout,
                });
            }
            Ok(TypedValue {
                value_type: ValueType::List,
                payload: serde_json::json!([]),
            })
        }

        fn has_compensation(&self, _effect: &EffectDecl) -> bool {
            false
        }

        async fn compensate(
            &self,
            _effect: &EffectDecl,
            _input: TypedValue,
        ) -> Result<(), AcosError> {
            Ok(())
        }
    }

    #[derive(Debug)]
    struct FlakyRegistry {
        inner: acos_plugin::BuiltinRegistry,
    }

    // ── Loop output aggregation test primitives ──────────────────────────────

    #[derive(Debug)]
    struct EchoItemPrimitive;

    #[async_trait]
    impl Primitive for EchoItemPrimitive {
        fn capability(&self) -> CapabilityDesc {
            CapabilityDesc {
                id: "echo_item".into(),
                name: "Echo Item".into(),
                input_type: "EchoRequest".into(),
                output_type: "EchoResult".into(),
            }
        }
        fn effects(&self) -> Vec<EffectDecl> {
            vec![]
        }
        async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
            let item = input.payload.get("item").cloned().unwrap_or(serde_json::Value::Null);
            Ok(TypedValue { value_type: ValueType::Scalar, payload: item })
        }
        fn has_compensation(&self, _e: &EffectDecl) -> bool { false }
        async fn compensate(&self, _e: &EffectDecl, _i: TypedValue) -> Result<(), AcosError> { Ok(()) }
    }

    #[derive(Debug)]
    struct EchoListPrimitive;

    #[async_trait]
    impl Primitive for EchoListPrimitive {
        fn capability(&self) -> CapabilityDesc {
            CapabilityDesc {
                id: "echo_list".into(),
                name: "Echo List".into(),
                input_type: "EchoRequest".into(),
                output_type: "EchoResult".into(),
            }
        }
        fn effects(&self) -> Vec<EffectDecl> {
            vec![]
        }
        async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
            let items = input.payload.get("items").cloned().unwrap_or(serde_json::Value::Null);
            Ok(TypedValue {
                value_type: if items.is_array() { ValueType::List } else { ValueType::Scalar },
                payload: items,
            })
        }
        fn has_compensation(&self, _e: &EffectDecl) -> bool { false }
        async fn compensate(&self, _e: &EffectDecl, _i: TypedValue) -> Result<(), AcosError> { Ok(()) }
    }

    #[derive(Debug)]
    struct VerifyOutputPrimitive;

    #[async_trait]
    impl Primitive for VerifyOutputPrimitive {
        fn capability(&self) -> CapabilityDesc {
            CapabilityDesc {
                id: "verify_output".into(),
                name: "Verify Output".into(),
                input_type: "EchoRequest".into(),
                output_type: "EchoResult".into(),
            }
        }
        fn effects(&self) -> Vec<EffectDecl> {
            vec![]
        }
        async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
            let item = input.payload.get("item").cloned().unwrap_or_default();
            if item == serde_json::json!(["a", "b", "c"]) {
                Ok(TypedValue { value_type: ValueType::List, payload: item })
            } else {
                Err(AcosError::PrimitiveFailure {
                    message: format!("expected aggregated [a,b,c], got: {item}"),
                    primitive_id: Some("verify_output".into()),
                    class: FailureClass::Unknown,
                })
            }
        }
        fn has_compensation(&self, _e: &EffectDecl) -> bool { false }
        async fn compensate(&self, _e: &EffectDecl, _i: TypedValue) -> Result<(), AcosError> { Ok(()) }
    }

    #[derive(Debug)]
    struct EchoRegistry {
        inner: acos_plugin::BuiltinRegistry,
    }

    #[async_trait]
    impl PluginRegistry for EchoRegistry {
        fn list(&self) -> Vec<CapabilityDesc> {
            self.inner.list()
        }
        async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError> {
            match capability_id {
                "echo_item" => Ok(Box::new(EchoItemPrimitive)),
                "echo_list" => Ok(Box::new(EchoListPrimitive)),
                "verify_output" => Ok(Box::new(VerifyOutputPrimitive)),
                _ => self.inner.resolve(capability_id).await,
            }
        }
        async fn load(&self, m: PrimitiveManifest) -> Result<PrimitiveId, AcosError> {
            self.inner.load(m).await
        }
        async fn unload(&self, id: PrimitiveId) -> Result<(), AcosError> {
            self.inner.unload(id).await
        }
    }

    impl FlakyRegistry {
        fn new() -> Self {
            Self {
                inner: acos_plugin::BuiltinRegistry::new(),
            }
        }
    }

    #[async_trait]
    impl PluginRegistry for FlakyRegistry {
        fn list(&self) -> Vec<CapabilityDesc> {
            self.inner.list()
        }

        async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError> {
            if capability_id == "flaky_search" {
                Ok(Box::new(FlakySearchPrimitive::new(1)))
            } else {
                self.inner.resolve(capability_id).await
            }
        }

        async fn load(&self, m: PrimitiveManifest) -> Result<PrimitiveId, AcosError> {
            self.inner.load(m).await
        }

        async fn unload(&self, id: PrimitiveId) -> Result<(), AcosError> {
            self.inner.unload(id).await
        }
    }

    // ── Recovery (execute_with_recovery) tests ────────────────────────────────

    #[derive(Debug)]
    struct FixedPathRule(String);

    impl Replanner for FixedPathRule {
        fn propose(
            &self,
            failure: &FailureContext,
            program: &CirProgram,
        ) -> Option<RecoveryProposal> {
            let failing = program.nodes.iter().find(|n| n.node_id == failure.node_id)?;
            let mut root = failing.clone();
            root.kind = CirNodeKind::PrimitiveInvocation;
            root.capability = Some("read_file".into());
            root.children = vec![];
            root.control = None;
            root.inputs = vec![(
                "path".into(),
                serde_json::Value::String(self.0.clone()),
            )]
            .into_iter()
            .collect();
            Some(RecoveryProposal {
                replace_node: failure.node_id.clone(),
                subgraph: vec![root],
                reason: "fallback to local read".into(),
            })
        }
    }

    #[tokio::test]
    async fn recovery_replaces_failing_node_and_completes() {
        let dir = std::env::temp_dir().join(format!("acos-recover-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("fallback.txt"), "cached").unwrap();
        let fallback_path = dir.join("fallback.txt").to_string_lossy().to_string();

        let nodes = vec![
            CirNode {
                kind: CirNodeKind::Sequence,
                node_id: "root".into(),
                capability: None,
                output: None,
                input_types: HashMap::new(),
                children: vec!["search".into(), "write".into()],
                else_children: vec![],
                inputs: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "search".into(),
                capability: Some("flaky_search".into()),
                output: Some(OutputSpec {
                    name: "results".into(),
                    type_name: "String".into(),
                    fields: vec![],
                }),
                children: vec![],
                else_children: vec![],
                inputs: HashMap::new(),
                input_types: HashMap::new(),
                control: None,
            },
            CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "write".into(),
                capability: Some("write_file".into()),
                output: Some(OutputSpec {
                    name: "ref".into(),
                    type_name: "String".into(),
                    fields: vec![],
                }),
                children: vec![],
                else_children: vec![],
                inputs: vec![
                    (
                        "path".into(),
                        serde_json::Value::String(
                            dir.join("out.txt").to_string_lossy().to_string(),
                        ),
                    ),
                    ("content".into(), serde_json::Value::String("${results}".into())),
                ]
                .into_iter()
                .collect(),
                input_types: HashMap::new(),
                control: None,
            },
        ];
        let mut program = program_from(nodes);
        program.effects = vec![
            EffectDecl {
                kind: EffectKind::NetworkRead,
                description: "search".into(),
                reversible: true,
            },
            EffectDecl {
                kind: EffectKind::FsRead,
                description: "read".into(),
                reversible: true,
            },
            EffectDecl {
                kind: EffectKind::FsWrite,
                description: "write".into(),
                reversible: true,
            },
        ];

        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let rule = FixedPathRule(fallback_path);
        let ctx = RecoveryContext {
            rule: Some(&rule),
            model: None,
        };
        let report = runtime
            .execute_with_recovery(program, Some(&ctx))
            .await
            .unwrap();
        assert_eq!(report.status, RunStatus::Completed);
        let events = store.query(report.run_id).await.unwrap();
        assert!(events.iter().any(|e| e.event_type == "replan.started"));
        assert!(events.iter().any(|e| e.event_type == "replan.completed"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn proposal_must_reuse_replace_node_id() {
        let program = program_from(vec![primitive_node("a", "search", None)]);
        let bad = RecoveryProposal {
            replace_node: "a".into(),
            subgraph: vec![primitive_node("b", "search", None)],
            reason: "bad root id".into(),
        };
        let store: Arc<dyn EventStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let astore: Arc<dyn ArtifactStore + Send + Sync> = Arc::new(InMemoryStore::new());
        let runtime = RuntimeImpl::with_registry(store.clone(), astore, FlakyRegistry::new());
        let err = runtime.validate_proposal(&program, &bad).await.unwrap_err();
        assert!(err.to_string().contains("reuse replace_node"));
    }

    #[tokio::test]
    async fn dotted_path_resolves_nested_field() {
        let env: Arc<Mutex<HashMap<String, TypedValue>>> = Arc::new(Mutex::new(HashMap::new()));
        env.lock().await.insert("vr".into(), TypedValue {
            value_type: ValueType::Record,
            payload: serde_json::json!({"total_issues": 3, "issues": ["a", "b"]}),
        });
        let out = resolve_ref("${vr.total_issues}", &env).await;
        assert_eq!(out, "3");
        let out2 = resolve_ref("prefix ${vr.total_issues} suffix", &env).await;
        assert_eq!(out2, "prefix 3 suffix");
    }

    #[tokio::test]
    async fn dotted_path_missing_field_keeps_ref_unchanged() {
        let env: Arc<Mutex<HashMap<String, TypedValue>>> = Arc::new(Mutex::new(HashMap::new()));
        env.lock().await.insert("vr".into(), TypedValue {
            value_type: ValueType::Record,
            payload: serde_json::json!({"total_issues": 3}),
        });
        let out = resolve_ref("${vr.nonexistent}", &env).await;
        assert_eq!(out, "${vr.nonexistent}");
    }
}
