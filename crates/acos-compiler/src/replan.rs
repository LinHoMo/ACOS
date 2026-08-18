//! Model-driven recovery planning: asks Claude (via LongCat) to produce a
//! `RecoverySubgraph` patch for a runtime failure.

use async_trait::async_trait;

use acos_core::error::AcosError;
use acos_core::traits::ModelReplanner;
use acos_core::types::{CirProgram, FailureContext, RecoveryProposal};

use crate::extract_json_object;

/// System prompt teaching the model the RecoverySubgraph JSON format.
const RECOVERY_SYSTEM_PROMPT: &str = r#"You are the ACOS Recovery Planner. A cognitive program failed at runtime and you must produce a **RecoverySubgraph**: a minimal patch that replaces ONE failing node.

# Available primitives (capabilities)
- `search` ({ "query": "..." } -> DocumentList, network read)
- `read_file` ({ "path": "..." } -> Document, fs read)
- `write_file` ({ "path": "...", "content": "..." } -> ArtifactRef, fs write)
- `execute_python` ({ "code": "..." } -> ExecutionResult, process spawn)
- `summarize` ({ "document": "..." } or { "documents": [...] } -> Summary)

# Rules
1. Respond with ONLY valid JSON (no markdown, no commentary):
   { "replaceNode": "<failing node id>", "reason": "<why>", "subgraph": [ <CIR nodes> ] }
2. The subgraph ROOT node MUST reuse replaceNode as its nodeId — do not introduce a new id for the root.
3. Only reference node outputs that already exist in the provided program.
4. Only use the capabilities listed above.
5. Do not declare new top-level effects; reuse the program's existing effect kinds.
6. Keep the patch minimal: prefer replacing a primitive with another primitive over adding containers.
7. If the failing node is `execute_python` with a missing-module error, you may emit a sequence root whose children are: an install step (execute_python with pip-install code, new nodeId) then the retry of the original node (new nodeId).
"#;

/// Model-assisted recovery planner backed by LongCat.
#[derive(Debug, Clone)]
pub struct ModelRecoveryPlanner {
    llm: acos_llm::LongCatClient,
}

impl ModelRecoveryPlanner {
    /// Creates a planner from environment configuration.
    pub fn from_env() -> Result<Self, AcosError> {
        Ok(Self {
            llm: acos_llm::LongCatClient::from_env()?,
        })
    }

    /// Parses a RecoverySubgraph JSON response into a proposal.
    ///
    /// Tolerates markdown fences and surrounding commentary.
    pub fn parse_proposal(&self, raw: &str) -> Result<RecoveryProposal, AcosError> {
        let json_str = extract_json_object(raw);
        let value: serde_json::Value = serde_json::from_str(&json_str).map_err(|e| {
            AcosError::CompilerFailure {
                message: format!(
                    "model returned invalid RecoverySubgraph JSON: {e}\n--- raw ---\n{raw}"
                ),
            }
        })?;
        serde_json::from_value(value).map_err(|e| AcosError::CompilerFailure {
            message: format!("RecoverySubgraph does not match schema: {e}\n--- raw ---\n{raw}"),
        })
    }

    /// Builds the user prompt with failure context and the current program.
    #[allow(dead_code)]
    fn build_user_prompt(&self, failure: &FailureContext, program: &CirProgram) -> String {
        let program_json = serde_json::to_string_pretty(program)
            .unwrap_or_else(|_| format!("{program:?}"));
        let failure_json = serde_json::to_string_pretty(failure)
            .unwrap_or_else(|_| format!("{failure:?}"));
        format!(
            "The runtime failed. Produce a RecoverySubgraph.\n\n## Failure\n```json\n{failure_json}\n```\n\n## Current program\n```json\n{program_json}\n```"
        )
    }
}

#[async_trait]
impl ModelReplanner for ModelRecoveryPlanner {
    async fn propose(
        &self,
        failure: &FailureContext,
        program: &CirProgram,
    ) -> Result<Option<RecoveryProposal>, AcosError> {
        let prompt = self.build_user_prompt(failure, program);
        let raw = self.llm.complete(RECOVERY_SYSTEM_PROMPT, &prompt).await?;
        Ok(Some(self.parse_proposal(&raw)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::RunId;
    use acos_core::types::{CirNode, CirNodeKind, FailureClass, OutputSpec};

    fn failure() -> FailureContext {
        FailureContext {
            run_id: RunId::new(),
            node_id: "B".into(),
            error_class: FailureClass::Unknown,
            error_message: "boom".into(),
            attempts: 0,
            recent_events: vec![],
        }
    }

    #[test]
    fn parses_recovery_subgraph_json() {
        let raw = r#"{
            "replaceNode": "B",
            "reason": "use a local read instead",
            "subgraph": [
                { "kind": "primitive_invocation", "nodeId": "B", "capability": "read_file", "output": { "name": "results", "typeName": "String", "fields": [] }, "children": [], "inputs": { "path": "fallback.txt" } }
            ]
        }"#;
        let planner = ModelRecoveryPlanner {
            llm: acos_llm::LongCatClient::dummy(),
        };
        let proposal = planner.parse_proposal(raw).unwrap();
        assert_eq!(proposal.replace_node, "B");
        assert_eq!(proposal.subgraph.len(), 1);
        assert_eq!(proposal.subgraph[0].node_id, "B");
    }

    #[test]
    fn parses_proposal_wrapped_in_markdown() {
        let raw = "```json\n{\"replaceNode\":\"B\",\"reason\":\"r\",\"subgraph\":[]}\n```";
        let planner = ModelRecoveryPlanner {
            llm: acos_llm::LongCatClient::dummy(),
        };
        let proposal = planner.parse_proposal(raw).unwrap();
        assert_eq!(proposal.replace_node, "B");
    }

    #[test]
    fn rejects_invalid_proposal_json() {
        let planner = ModelRecoveryPlanner {
            llm: acos_llm::LongCatClient::dummy(),
        };
        assert!(planner.parse_proposal("not json").is_err());
    }

    #[test]
    fn builds_prompt_containing_failure_and_program() {
        let planner = ModelRecoveryPlanner {
            llm: acos_llm::LongCatClient::dummy(),
        };
        let program = CirProgram {
            id: acos_core::id::ProgramId::new(),
            task_id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            entry: vec!["B".into()],
            nodes: vec![CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id: "B".into(),
                capability: Some("search".into()),
                output: Some(OutputSpec {
                    name: "results".into(),
                    type_name: "String".into(),
                    fields: vec![],
                }),
                input_types: std::collections::HashMap::new(),
                children: vec![],
                else_children: vec![],
                inputs: Default::default(),
                control: None,
            }],
            effects: vec![],
        };
        let prompt = planner.build_user_prompt(&failure(), &program);
        assert!(prompt.contains("RecoverySubgraph"));
        assert!(prompt.contains("B"));
    }
}
