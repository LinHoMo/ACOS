//! ACOS cognitive compiler.
//!
//! Implements a **rule-first planner**: given a task spec, it analyzes the
//! inputs and outputs and generates a CIR execution graph without requiring a
//! model. Model-assisted planning is a future extension (Phase 3+).

#![warn(missing_docs)]

use async_trait::async_trait;

use acos_core::error::AcosError;
use acos_core::id::{ProgramId, TaskId};
use acos_core::traits::{CompileResult, Compiler, Diagnostic, DiagnosticLevel};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, EffectDecl, EffectKind, TaskSpec,
};

/// The rule-first compiler.
#[derive(Debug, Clone, Default)]
pub struct RuleCompiler;

impl RuleCompiler {
    /// Creates a new rule-first compiler.
    pub fn new() -> Self {
        Self
    }

    /// Generates a node id.
    fn node_id(&self, prefix: &str, index: usize) -> String {
        format!("{prefix}_{index}")
    }
}

#[async_trait]
impl Compiler for RuleCompiler {
    async fn compile(&self, task: TaskSpec) -> Result<CompileResult, AcosError> {
        let program_id = ProgramId::new();
        let task_id = task.id;
        let mut nodes: Vec<CirNode> = vec![];
        let mut diagnostics: Vec<Diagnostic> = vec![];

        // Rule: if we have file inputs and a report-like output, build a
        // read → summarize → write pipeline.
        let file_inputs: Vec<_> = task
            .inputs
            .iter()
            .filter(|i| i.input_type.eq_ignore_ascii_case("file"))
            .collect();

        if file_inputs.is_empty() {
            return Err(AcosError::CompilerFailure {
                message: "no file inputs found; rule-first planner requires at least one file input".into(),
            });
        }

        // Phase 1: parallel reads.
        let read_parallel_id = "reads";
        let mut read_children: Vec<String> = vec![];
        for (i, input) in file_inputs.iter().enumerate() {
            let node_id = self.node_id("read", i);
            read_children.push(node_id.clone());
            nodes.push(CirNode {
                kind: CirNodeKind::PrimitiveInvocation,
                node_id,
                capability: Some("read_file".to_string()),
                output: Some(format!("raw_{i}")),
                children: vec![],
                inputs: vec![("path".to_string(), input.path.clone())]
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
            inputs: Default::default(),
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
            inputs: vec![("documents".to_string(), serde_json::to_string(&raw_refs).unwrap())]
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
                ("path".to_string(), "report.md".to_string()),
                ("content".to_string(), "${report_text}".to_string()),
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
            inputs: Default::default(),
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
    let result = RuleCompiler::new().compile(task).await?;
    Ok(result.program)
}

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::types::{TaskInput, TaskSpec};

    fn sample_task() -> TaskSpec {
        TaskSpec {
            api_version: "acos.io/v1".into(),
            id: acos_core::id::TaskId::new(),
            goal: "analyze csv files".into(),
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
    async fn rule_compiler_generates_read_summarize_write() {
        let task = sample_task();
        let result = RuleCompiler::new().compile(task).await.expect("compile");
        let prog = result.program;

        let kinds: Vec<_> = prog.nodes.iter().map(|n| &n.kind).collect();
        assert!(kinds.contains(&&CirNodeKind::Sequence));
        assert!(kinds.contains(&&CirNodeKind::Parallel));
        assert!(kinds.contains(&&CirNodeKind::PrimitiveInvocation));

        // 2 reads + 1 summarize + 1 write + 1 parallel + 1 sequence = 6 nodes.
        assert_eq!(prog.nodes.len(), 6);
        assert_eq!(prog.entry, vec!["root"]);
    }
}
