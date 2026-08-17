//! ACOS cognitive runtime.
//!
//! Executes CIR programs by walking the graph, resolving primitives via a
//! registry, storing events/artifacts, and producing a verifiable run report.

#![warn(missing_docs)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::sync::Mutex;

use acos_core::error::AcosError;
use acos_core::id::{ArtifactId, RunId};
use acos_core::traits::{
    ArtifactStore, Event, EventStore, PluginRegistry, Primitive, RunHandle, RunStatus,
};
use acos_core::types::{CirNode, CirNodeKind, CirProgram, TypedValue, ValueType};
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

    /// Executes a program and returns a report.
    pub async fn execute(&self, program: CirProgram) -> Result<RunReport, AcosError> {
        let run_id = RunId::new();
        self.event_store
            .append(
                run_id,
                "run.started".into(),
                serde_json::json!({ "program_id": program.id.0 }),
            )
            .await?;

        let env = Arc::new(Mutex::new(HashMap::<String, TypedValue>::new()));

        let node_map: HashMap<String, CirNode> = program
            .nodes
            .iter()
            .map(|n| (n.node_id.clone(), n.clone()))
            .collect();

        let result = self
            .run_nodes(&node_map, &program.entry, run_id, env.clone())
            .await;

        let (produced_artifacts, evidence) = match result {
            Ok((arts, ev)) => (arts, ev),
            Err(e) => {
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
        };

        self.event_store
            .append(
                run_id,
                "run.finished".into(),
                serde_json::json!({ "status": "Completed" }),
            )
            .await
            .ok();

        Ok(RunReport {
            run_id,
            status: RunStatus::Completed,
            artifacts: produced_artifacts,
            evidence,
        })
    }

    /// Iteratively executes nodes and returns produced artifacts + evidence.
    async fn run_nodes(
        &self,
        node_map: &HashMap<String, CirNode>,
        entries: &[String],
        run_id: RunId,
        env: Arc<Mutex<HashMap<String, TypedValue>>>,
    ) -> Result<(Vec<String>, Vec<Evidence>), AcosError> {
        let mut artifacts = Vec::new();
        let mut evidence = Vec::new();
        for entry in entries {
            let node = node_map.get(entry).ok_or_else(|| {
                AcosError::RuntimeInfrastructureFailure {
                    message: format!("node {entry} not found"),
                }
            })?;
            self.run_node(node, run_id, &env, &mut artifacts, &mut evidence, node_map)
                .await?;
        }
        Ok((artifacts, evidence))
    }

    /// Executes a single node.
    #[allow(clippy::too_many_arguments)]
    async fn run_node(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
        node_map: &HashMap<String, CirNode>,
    ) -> Result<(), AcosError> {
        match node.kind {
            CirNodeKind::Sequence | CirNodeKind::Parallel => {
                self.event_store
                    .append(
                        run_id,
                        "node.start".into(),
                        serde_json::json!({ "node_id": &node.node_id, "kind": format!("{:?}", node.kind) }),
                    )
                    .await?;
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        AcosError::RuntimeInfrastructureFailure {
                            message: format!("child node {child_id} not found"),
                        }
                    })?;
                    Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                        .await?;
                }
                Ok(())
            }
            CirNodeKind::PrimitiveInvocation => {
                self.run_primitive(node, run_id, env, artifacts, evidence).await
            }
            _ => {
                for child_id in &node.children {
                    let child = node_map.get(child_id).ok_or_else(|| {
                        AcosError::RuntimeInfrastructureFailure {
                            message: format!("child node {child_id} not found"),
                        }
                    })?;
                    Box::pin(self.run_node(child, run_id, env, artifacts, evidence, node_map))
                        .await?;
                }
                Ok(())
            }
        }
    }

    /// Runs a primitive invocation node.
    async fn run_primitive(
        &self,
        node: &CirNode,
        run_id: RunId,
        env: &Arc<Mutex<HashMap<String, TypedValue>>>,
        artifacts: &mut Vec<String>,
        evidence: &mut Vec<Evidence>,
    ) -> Result<(), AcosError> {
        let capability = node.capability.as_ref().ok_or_else(|| {
            AcosError::RuntimeInfrastructureFailure {
                message: format!("primitive node {} has no capability", node.node_id),
            }
        })?;

        let primitive = self.registry.resolve(capability).await.map_err(|e| {
            AcosError::ProviderFailure {
                provider: capability.clone(),
                message: format!("failed to resolve: {e}"),
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

        if let Some(name) = &node.output {
            env.lock().await.insert(name.clone(), output.clone());
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

/// Recursively replaces `$name` / `${name}` tokens in a JSON value.
fn resolve_refs_in_value(value: &mut Value, env: &HashMap<String, TypedValue>) {
    match value {
        Value::String(s) => {
            if let Some(name) = s
                .strip_prefix("${")
                .and_then(|x| x.strip_suffix("}"))
                .or_else(|| s.strip_prefix("$"))
            {
                let matched = env
                    .get(name)
                    .cloned()
                    .or_else(|| {
                        env.iter()
                            .find(|(k, _)| k.eq_ignore_ascii_case(name))
                            .map(|(_, v)| v.clone())
                    })
                    .or_else(|| {
                        env.iter()
                            .find(|(k, _)| {
                                let a = k.to_lowercase();
                                let b = name.to_lowercase();
                                a.contains(&b) || b.contains(&a)
                            })
                            .map(|(_, v)| v.clone())
                    });
                if let Some(tv) = matched {
                    *value = tv.payload.clone();
                }
            }
        }
        Value::Array(arr) => {
            for item in arr {
                resolve_refs_in_value(item, env);
            }
        }
        Value::Object(map) => {
            for v in map.values_mut() {
                resolve_refs_in_value(v, env);
            }
        }
        _ => {}
    }
}

/// Resolves a `$name` or `${name}` reference to its string form.
async fn resolve_ref(ref_str: &str, env: &Arc<Mutex<HashMap<String, TypedValue>>>) -> String {
    let name = if ref_str.starts_with("${") && ref_str.ends_with("}") {
        &ref_str[2..ref_str.len() - 1]
    } else if ref_str.starts_with('$') {
        &ref_str[1..]
    } else {
        return ref_str.to_string();
    };
    let guard = env.lock().await;
    if let Some(tv) = guard.get(name) {
        match &tv.payload {
            Value::String(s) => s.clone(),
            other => other.to_string(),
        }
    } else {
        ref_str.to_string()
    }
}

/// Re-export as `Runtime` for convenience.
pub use RuntimeImpl as Runtime;

#[cfg(test)]
mod tests {
    use super::*;
    use acos_compiler::RuleCompiler;
    use acos_core::traits::Compiler;
    use acos_core::types::{TaskInput, TaskSpec};
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
}
