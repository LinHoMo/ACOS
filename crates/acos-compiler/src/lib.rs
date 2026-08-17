//! ACOS cognitive compiler.
//!
//! Two planners:
//! - [`RuleCompiler`]: fast, deterministic, hardcoded `read → summarize → write`
//!   pipeline for file-to-report tasks.
//! - [`ModelCompiler`]: asks Claude (via LongCat) to generate a CIR execution
//!   graph as JSON, then parses it into a [`CirProgram`]. This is the
//!   "cognitive" path — the compiler delegates planning to a model.

#![warn(missing_docs)]

use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;

use acos_core::error::AcosError;
use acos_core::id::{ProgramId, TaskId};
use acos_core::traits::{CompileResult, Compiler, Diagnostic, DiagnosticLevel};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, EffectDecl, EffectKind, TaskSpec,
};

/// System prompt that teaches Claude the CIR JSON format and the available
/// primitives. Kept as a constant so it is easy to version and audit.
const PLANNER_SYSTEM_PROMPT: &str = r#"You are the ACOS Cognitive Planner. Your job is to compile a structured task into a **Cognitive Intermediate Representation (CIR)** — a JSON execution graph the ACOS runtime can execute.

# Available primitives (capabilities)

| capability | input | output | effects |
|---|---|---|---|
| `search` | `{ "query": "..." }` | `DocumentList` | network read |
| `read_file` | `{ "path": "..." }` | `Document` | fs read |
| `write_file` | `{ "path": "...", "content": "..." }` | `ArtifactRef` | fs write |
| `execute_python` | `{ "code": "..." }` | `ExecutionResult` | process spawn |
| `summarize` | `{ "document": "..." }` or `{ "documents": [...] }` | `Summary` | (none) |

# CIR JSON schema

You MUST respond with **only valid JSON** (no markdown, no commentary) matching this exact shape:

```json
{
  "id": "<uuid>",
  "taskId": "<uuid>",
  "entry": ["root"],
  "nodes": [
    { "kind": "sequence", "nodeId": "root", "capability": null, "output": null, "children": ["step_0"], "inputs": {} },
    { "kind": "primitive_invocation", "nodeId": "step_0", "capability": "read_file", "output": "raw_0", "children": [], "inputs": { "path": "/tmp/data/a.txt" } },
    { "kind": "primitive_invocation", "nodeId": "step_1", "capability": "write_file", "output": "report_ref", "children": [], "inputs": { "path": "report.md", "content": "${raw_0}" } }
  ],
  "effects": [
    { "kind": "fs_read", "description": "read input files", "reversible": true },
    { "kind": "fs_write", "description": "write report artifact", "reversible": true }
  ]
}
```

# Rules

1. `nodes` is an array. Each node has: `kind`, `node_id`, `capability` (null for containers), `output` (null unless it binds a value), `children`, `inputs`.
2. `kind` must be exactly one of: `sequence`, `parallel`, `conditional`, `primitive_invocation`.
3. A top-level `sequence` container whose `children` list is the execution order is required; put its id in `entry`.
4. Use `primitiveInvocation` for every primitive call. Set `capability` to the primitive name.
5. To pass data between nodes, bind an `output` name on the producer and reference it as `"${outputName}"` in the consumer's `inputs`. Do NOT invent other reference syntax.
6. `inputs` is an object mapping parameter name -> literal string or `"${reference}"`.
7. Declare every side effect your graph uses in the `effects` array. `kind` must be one of: `fs_read`, `fs_write`, `network_read`, `network_write`, `process_spawn`, `secret_read`, `external_irreversible`. Set `reversible: false` only for `externalIrreversible`.
8. Choose primitives appropriate to the task. Prefer `read_file` + `summarize` + `write_file` for document/report tasks; use `execute_python` only when the task needs real computation.
9. Do not add nodes that are not implied by the task goal.

Think step by step, then output ONLY the JSON.

"#;

/// Model-assisted compiler: asks Claude to generate the CIR.
#[derive(Debug, Clone)]
pub struct ModelCompiler {
    llm: acos_llm::LongCatClient,
}

impl ModelCompiler {
    /// Creates a model compiler backed by the given LLM client.
    pub fn new(llm: acos_llm::LongCatClient) -> Self {
        Self { llm }
    }

    /// Creates a model compiler from environment configuration.
    pub fn from_env() -> Result<Self, AcosError> {
        Ok(Self::new(acos_llm::LongCatClient::from_env()?))
    }

    /// Builds the user prompt describing the task to plan.
    fn build_user_prompt(&self, task: &TaskSpec) -> String {
        let task_json =
            serde_json::to_string_pretty(task).unwrap_or_else(|_| format!("{task:?}"));
        format!(
            "Compile the following ACOS task into a CIR execution graph.\n\n```json\n{task_json}\n```"
        )
    }

    /// Parses Claude's JSON response into a [`CirProgram`].
    ///
    /// Tolerates responses wrapped in markdown fences or surrounded by
    /// commentary by extracting the first JSON object.
    fn parse_cir(&self, raw: &str, task_id: TaskId) -> Result<CirProgram, AcosError> {
        let json_str = extract_json_object(raw);
        let mut value: Value =
            serde_json::from_str(&json_str).map_err(|e| AcosError::CompilerFailure {
                message: format!("model returned invalid CIR JSON: {e}\n--- raw ---\n{raw}"),
            })?;

        // Inject a stable program id (the model should not be relied on to
            // produce valid UUIDs).
        if let Value::Object(ref mut map) = value {
            map.insert(
                "id".into(),
                Value::String(uuid::Uuid::new_v4().to_string()),
            );
            map.insert(
                "taskId".into(),
                Value::String(task_id.0.to_string()),
            );
        }

        let program: CirProgram =
            serde_json::from_value(value).map_err(|e| AcosError::CompilerFailure {
                message: format!("CIR JSON does not match schema: {e}
--- raw ---
{raw}"),
            })?;

        validate_cir(&program)?;
        Ok(program)
    }
}

#[async_trait]
impl Compiler for ModelCompiler {
    async fn compile(&self, task: TaskSpec) -> Result<CompileResult, AcosError> {
        let task_id = task.id;
        let prompt = self.build_user_prompt(&task);

        let raw = self
            .llm
            .complete(PLANNER_SYSTEM_PROMPT, &prompt)
            .await?;

        let mut program = self.parse_cir(&raw, task_id)?;

        // Ensure the program references the source task.
        program.task_id = task_id;

        let diagnostics = vec![Diagnostic {
            level: DiagnosticLevel::Note,
            message: format!(
                "model planner generated program {} ({} nodes) via {}",
                program.id.0,
                program.nodes.len(),
                self.llm.model()
            ),
        }];

        Ok(CompileResult {
            program,
            diagnostics,
        })
    }
}

/// The original rule-first compiler (kept as a fast deterministic fallback).
#[derive(Debug, Clone, Default)]
pub struct RuleCompiler;

impl RuleCompiler {
    /// Creates a new rule-first compiler.
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Compiler for RuleCompiler {
    async fn compile(&self, task: TaskSpec) -> Result<CompileResult, AcosError> {
        let program_id = ProgramId::new();
        let task_id = task.id;
        let mut nodes: Vec<CirNode> = vec![];
        let mut diagnostics: Vec<Diagnostic> = vec![];

        let file_inputs: Vec<_> = task
            .inputs
            .iter()
            .filter(|i| i.input_type.eq_ignore_ascii_case("file"))
            .collect();

        if file_inputs.is_empty() {
            return Err(AcosError::CompilerFailure {
                message:
                    "no file inputs found; rule-first planner requires at least one file input"
                        .into(),
            });
        }

        // Phase 1: parallel reads.
        let read_parallel_id = "reads";
        let mut read_children: Vec<String> = vec![];
        for (i, input) in file_inputs.iter().enumerate() {
            let node_id = format!("read_{i}");
            read_children.push(node_id.clone());
            nodes.push(CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id,
                capability: Some("read_file".to_string()),
                output: Some(format!("raw_{i}")),
                children: vec![],
                inputs: vec![("path".to_string(), serde_json::Value::String(input.path.clone()))]
                    .into_iter()
                    .collect(),
            });
        }

        nodes.push(CirNode {
            kind: CirNodeKind::Parallel,
            node_id: read_parallel_id.to_string(),
            capability: None,
            output: None,
            children: read_children.clone(),
            inputs: HashMap::new(),
        });

        // Phase 2: summarize all raw documents into a report text.
        let summarize_id = "summarize";
        let raw_refs: Vec<String> = (0..file_inputs.len())
            .map(|i| format!("${{raw_{i}}}"))
            .collect();
        nodes.push(CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: summarize_id.to_string(),
            capability: Some("summarize".to_string()),
            output: Some("report_text".to_string()),
            children: vec![],
            inputs: vec![(
                "documents".to_string(),
                serde_json::Value::String(serde_json::to_string(&raw_refs).unwrap()),
            )]
            .into_iter()
            .collect(),
        });

        // Phase 3: write the report artifact.
        let write_id = "write_report";
        nodes.push(CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: write_id.to_string(),
            capability: Some("write_file".to_string()),
            output: Some("report_ref".to_string()),
            children: vec![],
            inputs: vec![
                ("path".to_string(), serde_json::Value::String("report.md".to_string())),
                ("content".to_string(), serde_json::Value::String("${report_text}".to_string())),
            ]
            .into_iter()
            .collect(),
        });

        // Top-level sequence tying the phases together.
        let root_id = "root";
        nodes.push(CirNode {
            kind: CirNodeKind::Sequence,
            node_id: root_id.to_string(),
            capability: None,
            output: None,
            children: vec![
                read_parallel_id.to_string(),
                summarize_id.to_string(),
                write_id.to_string(),
            ],
            inputs: HashMap::new(),
        });

        let effects = vec![
            EffectDecl {
                kind: EffectKind::FsRead,
                description: "read input files".into(),
                reversible: true,
            },
            EffectDecl {
                kind: EffectKind::FsWrite,
                description: "write report artifact".into(),
                reversible: true,
            },
        ];

        let program = CirProgram {
            id: program_id,
            task_id,
            entry: vec![root_id.to_string()],
            nodes,
            effects,
        };

        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Note,
            message: format!(
                "rule-first planner generated program {} for task {:?}",
                program_id.0, task_id.0
            ),
        });

        Ok(CompileResult {
            program,
            diagnostics,
        })
    }
}

/// Convenience: compile a task spec into a program, returning just the program.
pub async fn compile_task(task: TaskSpec) -> Result<CirProgram, AcosError> {
    let result = ModelCompiler::from_env()?.compile(task).await?;
    Ok(result.program)
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Extracts the first balanced JSON object from text that may contain
/// markdown fences or surrounding commentary.
fn extract_json_object(text: &str) -> String {
    // Strip common markdown fences if the whole thing is fenced.
    let trimmed = text.trim();
    let candidate = if trimmed.starts_with("```") {
        let without_open = trimmed
            .trim_start_matches('`')
            .trim_start();
        let body = without_open
            .strip_prefix("json")
            .unwrap_or(without_open);
        let end = body.find("```").unwrap_or(body.len());
        body[..end].trim()
    } else {
        trimmed
    };

    // Find the first '{' and the last '}' after it.
    let Some(start) = candidate.find('{') else {
        return candidate.to_string();
    };
    let Some(end) = candidate.rfind('}') else {
        return candidate.to_string();
    };
    candidate[start..=end].to_string()
}

/// Validates structural invariants of a CIR program.
fn validate_cir(program: &CirProgram) -> Result<(), AcosError> {
    if program.entry.is_empty() {
        return Err(AcosError::ValidationFailure {
            message: "CIR program must have at least one entry node".into(),
        });
    }
    let ids: std::collections::HashSet<&str> = program
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    for entry in &program.entry {
        if !ids.contains(entry.as_str()) {
            return Err(AcosError::ValidationFailure {
                message: format!("CIR entry '{entry}' does not match any node"),
            });
        }
    }
    for node in &program.nodes {
        for child in &node.children {
            if !ids.contains(child.as_str()) {
                return Err(AcosError::ValidationFailure {
                    message: format!(
                        "node '{}' references unknown child '{}'",
                        node.node_id, child
                    ),
                });
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::types::TaskInput;

    fn sample_task() -> TaskSpec {
        TaskSpec {
            api_version: "acos.io/v1".into(),
            id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            goal: "analyze csv files".into(),
            inputs: vec![
                TaskInput {
                    input_type: "File".into(),
                    path: "a.csv".into(),
                    format: None,
                },
                TaskInput {
                    input_type: "File".into(),
                    path: "b.csv".into(),
                    format: None,
                },
            ],
            outputs: vec![],
            constraints: None,
            optimization: None,
            approval: None,
        }
    }

    #[tokio::test]
    async fn rule_compiler_generates_read_summarize_write() {
        let task = sample_task();
        let result = RuleCompiler::new().compile(task).await.expect("compile");
        let prog = result.program;

        let kinds: Vec<_> = prog.nodes.iter().map(|n| &n.kind).collect();
        assert!(kinds.contains(&&CirNodeKind::Sequence));
        assert!(kinds.contains(&&CirNodeKind::Parallel));
        assert!(kinds.contains(&&CirNodeKind::PrimitiveInvocation));

        assert_eq!(prog.nodes.len(), 6);
        assert_eq!(prog.entry, vec!["root"]);
    }

    #[test]
    fn extract_json_strips_markdown_fences() {
        let wrapped = "```json\n{\"a\": 1}\n```";
        assert_eq!(extract_json_object(wrapped), "{\"a\": 1}");
    }

    #[test]
    fn extract_json_handles_commentary() {
        let text = "Here is the plan:\n{\"x\": 42}\nDone.";
        assert_eq!(extract_json_object(text), "{\"x\": 42}");
    }
}
