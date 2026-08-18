//! ACOS cognitive compiler.
//!
//! Two planners:
//! - [`RuleCompiler`]: fast, deterministic, hardcoded `read → summarize → write`
//!   pipeline for file-to-report tasks.
//! - [`ModelCompiler`]: asks Claude (via LongCat) to generate a CIR execution
//!   graph as JSON, then parses it into a [`CirProgram`]. This is the
//!   "cognitive" path — the compiler delegates planning to a model.
//!
//! P1-5A: ModelCompiler Frontend Robustness — error classification, targeted
//! repair prompts, bounded retry, deterministic failure semantics.

#![warn(missing_docs)]

use std::collections::HashMap;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;

use acos_core::error::AcosError;
use acos_core::id::{ProgramId, TaskId};
use acos_core::traits::{CompileResult, Compiler, Diagnostic, DiagnosticLevel};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, EffectDecl, EffectKind, LoopKind, OutputSpec, TaskSpec,
};

pub mod replan;

mod contract;

pub use replan::ModelRecoveryPlanner;

use crate::contract::validate_data_contract;

// ── P1-5A: Compiler Error Classification ─────────────────────────────────────

/// Specific error types produced during ModelCompiler frontend processing.
///
/// Each variant maps to a stage in the compile pipeline where failure can
/// occur, enabling targeted repair prompts.
#[derive(Debug, Clone, PartialEq)]
pub enum CompilerError {
    /// JSON syntax is malformed (trailing comma, unquoted key, etc.).
    JsonSyntaxError {
        /// Error message from serde_json.
        message: String,
        /// Excerpt of the raw input for repair context.
        raw_excerpt: String,
    },
    /// JSON parses but does not match the CIR schema shape.
    JsonShapeError {
        /// Error message from serde_json::from_value.
        message: String,
    },
    /// A required field is missing at the CIR root or node level.
    MissingRequiredField {
        /// Dot-path to the missing field (e.g. `"entry"`, `"nodes[].node_id"`).
        field: String,
    },
    /// A node references a capability not in the allowed set.
    UnknownCapability {
        /// Node that declared the unknown capability.
        node_id: String,
        /// The unrecognized capability string.
        capability: String,
    },
    /// A node references an output, child, or entry that does not exist.
    InvalidReference {
        /// Node that contains the bad reference.
        node_id: String,
        /// The referenced name that could not be resolved.
        referenced: String,
    },
    /// Control semantics (condition/loop/retry) violate invariants.
    InvalidControlSemantics {
        /// Node that contains the violation.
        node_id: String,
        /// Human-readable explanation.
        message: String,
    },
    /// Effect declaration is invalid (unknown kind or inconsistent).
    InvalidEffect {
        /// Human-readable explanation.
        message: String,
    },
    /// The program contains no executable node (no primitives at all).
    ///
    /// Added in P1-5B Probe-2 analysis: zero-node / zero-primitive graphs
    /// trivially pass structural checks but cannot satisfy any task output.
    NoOpProgram,
    /// Nodes that are not reachable from any entry node.
    ///
    /// Added in P1-5B Probe-2c analysis (run-001): orphan nodes silently
    /// reduce what the program actually does (e.g. a `write_file` that never
    /// runs). Reject so the repair loop re-links them into the graph.
    UnreachableNodes {
        /// Node ids that are unreachable from `program.entry`.
        node_ids: Vec<String>,
    },
    /// A `${ref}` in inputs/control does not resolve to any producer binding.
    UnresolvedBinding {
        /// Node that contains the bad reference.
        node_id: String,
        /// The unresolved binding name (dotted path prefix).
        binding: String,
    },
    /// Data contract rule violation (type/field/ordering/completeness).
    DataContractViolation {
        /// Node that violates the contract.
        node_id: String,
        /// Human-readable explanation.
        message: String,
    },
    /// Repair was attempted but did not produce a valid CIR within the attempt limit.
    RepairExhausted {
        /// Number of repair attempts made (excluding the initial attempt).
        attempts: u32,
        /// The last error that prevented success.
        last_error: Box<CompilerError>,
    },
}

impl std::fmt::Display for CompilerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CompilerError::JsonSyntaxError { message, .. } => {
                write!(f, "JSON syntax error: {message}")
            }
            CompilerError::JsonShapeError { message } => {
                write!(f, "CIR schema mismatch: {message}")
            }
            CompilerError::MissingRequiredField { field } => {
                write!(f, "missing required field: {field}")
            }
            CompilerError::UnknownCapability { node_id, capability } => {
                write!(f, "node '{node_id}' uses unknown capability '{capability}'")
            }
            CompilerError::InvalidReference { node_id, referenced } => {
                write!(f, "node '{node_id}' references unknown id '{referenced}'")
            }
            CompilerError::InvalidControlSemantics { node_id, message } => {
                write!(f, "node '{node_id}' has invalid control semantics: {message}")
            }
            CompilerError::InvalidEffect { message } => {
                write!(f, "invalid effect declaration: {message}")
            }
            CompilerError::NoOpProgram => {
                write!(f, "no-op program: the CIR contains no executable node")
            }
            CompilerError::UnreachableNodes { node_ids } => {
                write!(
                    f,
                    "unreachable nodes: {} are not reachable from any entry node",
                    node_ids.join(", ")
                )
            }
            CompilerError::UnresolvedBinding { node_id, binding } => {
                write!(f, "node '{node_id}' references unresolved binding '${{{binding}}}'")
            }
            CompilerError::DataContractViolation { node_id, message } => {
                write!(f, "node '{node_id}' violates data contract: {message}")
            }
            CompilerError::RepairExhausted { attempts, last_error } => {
                write!(f, "repair exhausted after {attempts} attempts; last error: {last_error}")
            }
        }
    }
}

impl std::error::Error for CompilerError {}

/// Converts a CompilerError into the public AcosError type.
impl From<CompilerError> for AcosError {
    fn from(e: CompilerError) -> Self {
        AcosError::CompilerFailure {
            message: e.to_string(),
        }
    }
}

/// Allowed capability names for CIR primitive invocations.
///
/// This list covers both core primitives (defined in acos-plugin) and
/// bench-only stubs (defined in acos-bench). In production this would be
/// derived from the plugin registry; for P1-5A it is a static allowlist.
const ALLOWED_CAPABILITIES: &[&str] = &[
    // Core primitives
    "search",
    "read_file",
    "write_file",
    "execute_python",
    "summarize",
    // Bench stubs
    "flaky_search",
    "list_source",
    "irreversible",
    "unstable_search",
];

/// Allowed effect kinds (must match EffectKind serde variants).
const ALLOWED_EFFECT_KINDS: &[&str] = &[
    "fs_read",
    "fs_write",
    "network_read",
    "network_write",
    "process_spawn",
    "secret_read",
    "external_irreversible",
];

// ── System Prompt ────────────────────────────────────────────────────────────

/// System prompt that teaches the model the CIR JSON format, the available
/// primitives, and the **semantic grounding contract** — what the model may
/// not invent or simplify away.
///
/// P1-5B-A: Semantic Grounding Prompt. The rules here are compiler contract,
/// not workflow hints. They tell the model what facts are authoritative and
/// what it must preserve — without dictating specific control structures.
const PLANNER_SYSTEM_PROMPT: &str = r#"You are the ACOS Cognitive Planner. Your job is to compile a structured task into a **Cognitive Intermediate Representation (CIR)** — a JSON execution graph the ACOS runtime can execute.

# Semantic Grounding Rules (COMPILER CONTRACT — MUST OBEY)

1. **TaskSpec.inputs are authoritative.** Use the exact paths declared in the task specification. Never invent, replace, normalize, or guess input paths. If the task declares 4 CSV files, your CIR must reference all 4 declared paths.
2. **Every external resource referenced by the generated CIR must originate from a declared input, an environment binding, or a capability-produced output.** Do not reference files, URLs, or services not present in the task context.
3. **Preserve the semantics of the task goal and explicit constraints.** Do not simplify away required operations. If the task requires detection, repair, validation, and aggregation, your CIR must contain nodes for each.
4. **Every required output declared by TaskSpec must be represented by the generated program.** If the task declares a Markdown report, your CIR must produce it.
5. **Generated nodes must have explicit data/control dependencies.** Do not emit a generic read→summarize→write template when the task requires iteration, validation, branching, recovery, or aggregation.
6. **Choose control structures based on task semantics.** Use loops when the task requires per-item processing; use conditionals when the task requires branching; use retry when the task requires recovery. Do not use a control structure merely because an example uses it.
7. **The generated CIR must be executable using only the declared capabilities.** Every `primitive_invocation` must reference a capability from the Available Primitives table.

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
    { "kind": "primitive_invocation", "nodeId": "step_0", "capability": "read_file", "output": { "name": "raw_0", "typeName": "String", "fields": [] }, "children": [], "inputs": { "path": "/absolute/path/to/file" } },
    { "kind": "primitive_invocation", "nodeId": "step_1", "capability": "write_file", "output": { "name": "report_ref", "typeName": "String", "fields": [] }, "children": [], "inputs": { "path": "report.md", "content": "${raw_0}" } }
  ],
  "effects": [
    { "kind": "fs_read", "description": "read input files", "reversible": true },
    { "kind": "fs_write", "description": "write report artifact", "reversible": true }
  ]
}
```

# Structural Rules

1. `nodes` is an array. Each node has: `kind`, `node_id`, `capability` (null for containers), `output` (null unless it binds a value; when bound: `{ "name": "<binding>", "typeName": "<type>", "fields": [...] }`), `children`, `inputs`.
2. `kind` must be exactly one of: `sequence`, `parallel`, `conditional`, `loop_map`, `primitive_invocation`.
3. A top-level `sequence` container whose `children` list is the execution order is required; put its id in `entry`.
4. Use `primitive_invocation` for every primitive call. Set `capability` to the primitive name.
5. To pass data between nodes, bind an `output` name on the producer and reference it as `"${outputName}"` in the consumer's `inputs`. Do NOT invent other reference syntax.
6. `inputs` is an object mapping parameter name -> literal string or `"${reference}"`.
7. Declare every side effect your graph uses in the `effects` array. `kind` must be one of: `fs_read`, `fs_write`, `network_read`, `network_write`, `process_spawn`, `secret_read`, `external_irreversible`. Set `reversible: false` only for `external_irreversible`.
8. Do not add nodes that are not implied by the task goal.
9. Control semantics: conditions, loops, and retries are expressed via the
    node-level `control` object, NEVER via extra `inputs` keys and NEVER via a
    node kind of `retry`:
    - conditional: `{ "kind": "conditional", "control": { "condition": { "expression": "exists(doc)" } }, "children": [then...], "elseChildren": [else...] }`
    - loop_map: `{ "kind": "loop_map", "control": { "loopSpec": { "kind": "while|until|for_each", "condition": "...", "maxIterations": 5, "input": "${files}", "itemVar": "item" } }, "children": [body...] }`
      while/until MUST set max_iterations; for_each uses input + itemVar.
    - retry: attach `"control": { "retry": { "maxAttempts": 3, "backoffMs": 200, "strategy": "fixed", "retryOn": ["timeout", "rate_limit", "transient_network_error"] } }` to the executable node.
    Expression language (acos-expr): exists(name), not_exists(name), field paths
    like `test.exit_code`, comparisons == != > < >= <=, && || !, string literals
    in single quotes, numbers, true/false. Only reference `output` names that
    other nodes in the graph declare.

Think step by step, then output ONLY the JSON.

"#;

// ── ModelCompiler ────────────────────────────────────────────────────────────

/// Model-assisted compiler: asks Claude to generate the CIR.
#[derive(Debug, Clone)]
pub struct ModelCompiler {
    llm: acos_llm::LongCatClient,
}

/// Full trace of a single compile operation (used by P1-5B Discovery Probe).
///
/// Captures every LLM request/response, intermediate errors, and timing so
/// the compiler's behavior can be audited independently of the program's
/// runtime result.
#[derive(Debug, Clone, Serialize)]
pub struct CompileTrace {
    /// The user prompt sent on the initial attempt.
    pub initial_prompt: String,
    /// Raw text returned by the LLM on the initial attempt.
    pub initial_response: String,
    /// Error from parsing/validating the initial response, if any.
    pub initial_error: Option<String>,
    /// Repair attempts (empty on first-pass success).
    pub repair_attempts: Vec<RepairTraceEntry>,
    /// The final error if compilation failed entirely.
    pub final_error: Option<String>,
    /// Wall-clock timings (milliseconds).
    pub timing: CompileTiming,
}

/// A single repair attempt's captured data.
#[derive(Debug, Clone, Serialize)]
pub struct RepairTraceEntry {
    /// 1-based attempt number.
    pub attempt: u32,
    /// Prompt sent to the LLM (system + repair context).
    pub prompt: String,
    /// Raw text returned by the LLM.
    pub response: String,
    /// Validation error if the response was rejected, null on success.
    pub validation_error: Option<String>,
}

/// Timing breakdown for a compile operation.
#[derive(Debug, Clone, Serialize, Default)]
pub struct CompileTiming {
    /// Latency of the initial LLM call (ms).
    pub initial_llm_ms: u64,
    /// Cumulative latency of all repair LLM calls (ms).
    pub repair_llm_ms: u64,
    /// Total wall-clock for the entire compile (ms).
    pub total_ms: u64,
}

/// Result of a traced compile: the diagnostics-bearing result plus a full
/// audit trace of every LLM exchange.
#[derive(Debug, Clone)]
pub struct TracedCompile {
    /// The compile result (program + diagnostics).
    pub result: Result<CompileResult, AcosError>,
    /// Full trace of LLM exchanges and errors.
    pub trace: CompileTrace,
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

    /// Builds a **structured Compile Context** prompt from the TaskSpec.
    ///
    /// P1-5B-A: Instead of dumping raw TaskSpec JSON, we expand it into an
    /// explicit, labeled structure that makes input bindings, outputs,
    /// constraints, and capabilities unambiguous to the model. This prevents
    /// the "hallucinated /tmp/... paths" failure mode seen in Probe-1.
    fn build_user_prompt(&self, task: &TaskSpec) -> String {
        // ── Goal ──────────────────────────────────────────────────────────
        let mut out = String::new();
        out.push_str("Compile the following ACOS task into a CIR execution graph.\n\n");
        out.push_str("# Task Goal\n\n");
        out.push_str(&task.goal);
        out.push_str("\n\n");

        // ── Input Bindings (authoritative) ────────────────────────────────
        if !task.inputs.is_empty() {
            out.push_str("# Input Bindings (AUTHORITATIVE — use these exact paths)\n\n");
            out.push_str("The following input files are declared by the task and MUST be used:\n\n");
            for (i, input) in task.inputs.iter().enumerate() {
                out.push_str(&format!(
                    "{}. type=\"{}\", path=\"{}\"",
                    i + 1,
                    input.input_type,
                    input.path
                ));
                if let Some(ref fmt) = input.format {
                    out.push_str(&format!(", format=\"{fmt}\""));
                }
                out.push('\n');
            }
            out.push_str("\n");
        }

        // ── Required Outputs ─────────────────────────────────────────────
        if !task.outputs.is_empty() {
            out.push_str("# Required Outputs\n\n");
            out.push_str("The CIR MUST produce:\n\n");
            for output in &task.outputs {
                out.push_str(&format!("- type=\"{}\"", output.output_type));
                if let Some(ref fmt) = output.format {
                    out.push_str(&format!(", format=\"{fmt}\""));
                }
                out.push('\n');
            }
            out.push('\n');
        }

        // ── Constraints ──────────────────────────────────────────────────
        if let Some(ref c) = task.constraints {
            out.push_str("# Constraints\n\n");
            if let Some(t) = c.timeout_seconds {
                out.push_str(&format!("- timeoutSeconds: {t}\n"));
            }
            if let Some(cost) = c.max_cost {
                out.push_str(&format!("- maxCost: {cost}\n"));
            }
            if let Some(net) = c.allowed_network {
                out.push_str(&format!("- allowedNetwork: {net}\n"));
            }
            out.push('\n');
        }

        // ── Optimization ─────────────────────────────────────────────────
        if let Some(ref opt) = task.optimization {
            out.push_str(&format!(
                "# Optimization: primary=\"{}\"",
                opt.primary
            ));
            if let Some(ref sec) = opt.secondary {
                out.push_str(&format!(", secondary=\"{sec}\""));
            }
            out.push_str("\n\n");
        }

        // ── Approval ─────────────────────────────────────────────────────
        if let Some(ref appr) = task.approval {
            out.push_str(&format!(
                "# Approval: externalSideEffects=\"{}\"\n\n",
                appr.external_side_effects
            ));
        }

        // ── Full TaskSpec (reference) ────────────────────────────────────
        out.push_str("# Full TaskSpec (reference JSON)\n\n```json\n");
        let task_json =
            serde_json::to_string_pretty(task).unwrap_or_else(|_| format!("{task:?}"));
        out.push_str(&task_json);
        out.push_str("\n```\n");

        out
    }

    /// Builds a repair prompt from a previous failed attempt.
    ///
    /// P1-5B Probe-2 finding: first-pass responses are empty (EOF) in 100% of
    /// runs, making the repair call the de facto generation call. The old
    /// repair prompt carried only the error — no task facts — so the model
    /// could not bind inputs or preserve goal semantics. The Compile Context
    /// (facts, not answers) is now included in every repair call.
    fn build_repair_prompt(
        &self,
        task: &TaskSpec,
        raw_output: &str,
        error: &CompilerError,
    ) -> String {
        let excerpt = if raw_output.len() > 500 {
            &raw_output[..500]
        } else {
            raw_output
        };
        format!(
            "Your previous output failed CIR validation. Recompile the task below.\n\n\
             {context}\n\n\
             Previous error:\n\
             Error type: {}\n\
             Details: {}\n\n\
             Original output excerpt:\n```\n{}\n```\n\n\
             Please analyze the error and return a corrected CIR JSON.\n\
             Respond with ONLY the corrected JSON, no commentary.",
            std::any::type_name_of_val(error)
                .split("::")
                .last()
                .unwrap_or("Unknown"),
            error,
            excerpt,
            context = self.build_user_prompt(task),
        )
    }

    /// Parses Claude's JSON response into a CIR program with specific error types.
    ///
    /// Returns `CompilerError` variants instead of opaque `AcosError` so the
    /// caller can build targeted repair prompts.
    fn parse_cir(&self, raw: &str, task_id: TaskId) -> Result<CirProgram, CompilerError> {
        let json_str = extract_json_object(raw);

        // Stage 1: JSON syntax
        let mut value: Value = serde_json::from_str(&json_str).map_err(|e| {
            CompilerError::JsonSyntaxError {
                message: e.to_string(),
                raw_excerpt: raw.chars().take(200).collect(),
            }
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

        // Stage 2: Schema shape
        let program: CirProgram = serde_json::from_value(value).map_err(|e| {
            let msg = e.to_string();
            // Distinguish missing-field errors from general shape errors
            if msg.contains("missing field") {
                let field = msg
                    .split("missing field `")
                    .nth(1)
                    .and_then(|s| s.split('`').next())
                    .unwrap_or("unknown")
                    .to_string();
                CompilerError::MissingRequiredField { field }
            } else {
                CompilerError::JsonShapeError { message: msg }
            }
        })?;

        // Stage 3: Semantic validation
        validate_cir_semantic(&program)?;

        Ok(program)
    }

    /// Compiles with bounded repair retry, capturing a full trace.
    ///
    /// This is the P1-5B Discovery Probe entry point. It performs the same
    /// compilation logic as [`compile_with_repair`] but records every LLM
    /// exchange for later analysis.
    pub async fn compile_traced(
        &self,
        task: &TaskSpec,
        max_repair_attempts: u32,
    ) -> TracedCompile {
        let task_id = task.id;
        let user_prompt = self.build_user_prompt(task);
        let mut trace = CompileTrace {
            initial_prompt: user_prompt.clone(),
            initial_response: String::new(),
            initial_error: None,
            repair_attempts: Vec::new(),
            final_error: None,
            timing: CompileTiming::default(),
        };
        let total_start = std::time::Instant::now();
        let mut diagnostics: Vec<Diagnostic> = vec![];

        // Initial attempt
        let init_start = std::time::Instant::now();
        let raw = match self.llm.complete(PLANNER_SYSTEM_PROMPT, &user_prompt).await {
            Ok(r) => r,
            Err(e) => {
                trace.timing.total_ms = total_start.elapsed().as_millis() as u64;
                trace.final_error = Some(e.to_string());
                return TracedCompile {
                    result: Err(e),
                    trace,
                }
            }
        };
        trace.timing.initial_llm_ms = init_start.elapsed().as_millis() as u64;
        trace.initial_response = raw.clone();

        diagnostics.push(Diagnostic {
            level: DiagnosticLevel::Note,
            message: "compile.started: initial LLM call succeeded".into(),
        });

        match self.parse_cir(&raw, task_id) {
            Ok(program) => {
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Note,
                    message: format!(
                        "compile.succeeded: model planner generated program {} ({} nodes) via {}",
                        program.id.0,
                        program.nodes.len(),
                        self.llm.model()
                    ),
                });
                trace.timing.total_ms = total_start.elapsed().as_millis() as u64;
                return TracedCompile {
                    result: Ok(CompileResult { program, diagnostics }),
                    trace,
                };
            }
            Err(first_error) => {
                trace.initial_error = Some(first_error.to_string());
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Warning,
                    message: format!("compile.parse_failed: {first_error}"),
                });

                let mut last_error = first_error;
                let mut raw = raw;

                // Repair loop
                for attempt in 1..=max_repair_attempts {
                    let repair_prompt = self.build_repair_prompt(task, &raw, &last_error);

                    diagnostics.push(Diagnostic {
                        level: DiagnosticLevel::Note,
                        message: format!(
                            "compile.repair.started: attempt {attempt}/{max_repair_attempts}"
                        ),
                    });

                    let repair_start = std::time::Instant::now();
                    match self.llm.complete(PLANNER_SYSTEM_PROMPT, &repair_prompt).await {
                        Ok(retry_raw) => {
                            trace.timing.repair_llm_ms += repair_start.elapsed().as_millis() as u64;
                            match self.parse_cir(&retry_raw, task_id) {
                                Ok(program) => {
                                    trace.repair_attempts.push(RepairTraceEntry {
                                        attempt,
                                        prompt: repair_prompt,
                                        response: retry_raw,
                                        validation_error: None,
                                    });
                                    diagnostics.push(Diagnostic {
                                        level: DiagnosticLevel::Note,
                                        message: format!(
                                            "compile.repair.succeeded on attempt {}: program {} ({} nodes)",
                                            attempt,
                                            program.id.0,
                                            program.nodes.len(),
                                        ),
                                    });
                                    trace.timing.total_ms = total_start.elapsed().as_millis() as u64;
                                    let mut program = program;
                                    program.task_id = task_id;
                                    return TracedCompile {
                                        result: Ok(CompileResult { program, diagnostics }),
                                        trace,
                                    };
                                }
                                Err(e) => {
                                    trace.repair_attempts.push(RepairTraceEntry {
                                        attempt,
                                        prompt: repair_prompt,
                                        response: retry_raw.clone(),
                                        validation_error: Some(e.to_string()),
                                    });
                                    diagnostics.push(Diagnostic {
                                        level: DiagnosticLevel::Warning,
                                        message: format!(
                                            "compile.repair.validation_failed (attempt {attempt}): {e}"
                                        ),
                                    });
                                    last_error = e;
                                    raw = retry_raw;
                                }
                            }
                        }
                        Err(llm_err) => {
                            trace.timing.repair_llm_ms += repair_start.elapsed().as_millis() as u64;
                            trace.repair_attempts.push(RepairTraceEntry {
                                attempt,
                                prompt: repair_prompt,
                                response: String::new(),
                                validation_error: Some(llm_err.to_string()),
                            });
                            diagnostics.push(Diagnostic {
                                level: DiagnosticLevel::Error,
                                message: format!(
                                    "compile.repair.llm_error (attempt {attempt}): {llm_err}"
                                ),
                            });
                            trace.timing.total_ms = total_start.elapsed().as_millis() as u64;
                            trace.final_error = Some(llm_err.to_string());
                            return TracedCompile {
                                result: Err(llm_err),
                                trace,
                            };
                        }
                    }
                }

                // Exhausted
                let final_error = CompilerError::RepairExhausted {
                    attempts: max_repair_attempts,
                    last_error: Box::new(last_error),
                };
                diagnostics.push(Diagnostic {
                    level: DiagnosticLevel::Error,
                    message: format!("compile.failed: {final_error}"),
                });
                trace.timing.total_ms = total_start.elapsed().as_millis() as u64;
                trace.final_error = Some(final_error.to_string());

                TracedCompile {
                    result: Err(AcosError::CompilerFailure {
                        message: final_error.to_string(),
                    }),
                    trace,
                }
            }
        }
    }

    /// Compiles with bounded repair retry.
    ///
    /// On the first failure, builds a repair prompt from the specific error
    /// and retries up to `max_repair_attempts` times. All intermediate errors
    /// are recorded in diagnostics.
    async fn compile_with_repair(
        &self,
        task: &TaskSpec,
        max_repair_attempts: u32,
    ) -> Result<CompileResult, AcosError> {
        // Delegate to the traced variant, discard the trace.
        let traced = self.compile_traced(task, max_repair_attempts).await;
        traced.result
    }
}

#[async_trait]
impl Compiler for ModelCompiler {
    async fn compile(&self, task: TaskSpec) -> Result<CompileResult, AcosError> {
        // Use repair-aware compilation with a bounded retry limit
        self.compile_with_repair(&task, MAX_REPAIR_ATTEMPTS).await
    }
}

/// Maximum number of repair attempts before giving up.
const MAX_REPAIR_ATTEMPTS: u32 = 3;

// ── RuleCompiler ─────────────────────────────────────────────────────────────

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
                output: Some(OutputSpec {
                    name: format!("raw_{i}"),
                    type_name: "String".into(),
                    fields: vec![],
                }),
                input_types: HashMap::new(),
                children: vec![],
                else_children: vec![],
                inputs: vec![("path".to_string(), serde_json::Value::String(input.path.clone()))]
                    .into_iter()
                    .collect(),
                control: None,
            });
        }

        nodes.push(CirNode {
            kind: CirNodeKind::Parallel,
            node_id: read_parallel_id.to_string(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: read_children.clone(),
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
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
            output: Some(OutputSpec {
                name: "report_text".to_string(),
                type_name: "String".into(),
                fields: vec![],
            }),
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: vec![(
                "documents".to_string(),
                serde_json::Value::String(serde_json::to_string(&raw_refs).unwrap()),
            )]
            .into_iter()
            .collect(),
            control: None,
        });

        // Phase 3: write the report artifact.
        let write_id = "write_report";
        nodes.push(CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: write_id.to_string(),
            capability: Some("write_file".to_string()),
            output: Some(OutputSpec {
                name: "report_ref".to_string(),
                type_name: "String".into(),
                fields: vec![],
            }),
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: vec![
                ("path".to_string(), serde_json::Value::String("report.md".to_string())),
                ("content".to_string(), serde_json::Value::String("${report_text}".to_string())),
            ]
            .into_iter()
            .collect(),
            control: None,
        });

        // Top-level sequence tying the phases together.
        let root_id = "root";
        nodes.push(CirNode {
            kind: CirNodeKind::Sequence,
            node_id: root_id.to_string(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: vec![
                read_parallel_id.to_string(),
                summarize_id.to_string(),
                write_id.to_string(),
            ],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
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

// ── Semantic Validation (P1-5A: returns specific CompilerError) ───────────────

/// Validates structural and semantic invariants of a CIR program.
///
/// This is the public entry point used by `acos-bench` and other external
/// callers. It wraps [`validate_cir_semantic`] and converts specific errors
/// into `AcosError` for backward compatibility.
pub fn validate_cir(program: &CirProgram) -> Result<(), AcosError> {
    validate_cir_semantic(program).map_err(AcosError::from)
}

/// Internal validation that returns specific `CompilerError` variants for
/// targeted repair prompts.
fn validate_cir_semantic(program: &CirProgram) -> Result<(), CompilerError> {
    // Structural: entry must exist
    if program.entry.is_empty() {
        return Err(CompilerError::MissingRequiredField {
            field: "entry".into(),
        });
    }

    let ids: std::collections::HashSet<&str> = program
        .nodes
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();

    // Entry references must resolve
    for entry in &program.entry {
        if !ids.contains(entry.as_str()) {
            return Err(CompilerError::InvalidReference {
                node_id: "<entry>".into(),
                referenced: entry.clone(),
            });
        }
    }

    // Children references must resolve
    for node in &program.nodes {
        for child in &node.children {
            if !ids.contains(child.as_str()) {
                return Err(CompilerError::InvalidReference {
                    node_id: node.node_id.clone(),
                    referenced: child.clone(),
                });
            }
        }
    }

    // else_children validation (must be before else_children reference check)
    for node in &program.nodes {
        if !matches!(node.kind, CirNodeKind::Conditional) && !node.else_children.is_empty() {
            return Err(CompilerError::InvalidControlSemantics {
                node_id: node.node_id.clone(),
                message: "else_children is only valid on conditional nodes".into(),
            });
        }
        for child in &node.else_children {
            if !ids.contains(child.as_str()) {
                return Err(CompilerError::InvalidReference {
                    node_id: node.node_id.clone(),
                    referenced: child.clone(),
                });
            }
        }
    }

    // Capability and control-semantic validation
    validate_capabilities(program)?;
    validate_control_semantics_detailed(program)?;
    validate_effects(program)?;

    // Reachability: every node must be reachable from an entry node.
    // Orphan nodes (P1-5B Probe-2c run-001) silently do nothing: the model
    // declared `define_paths`/`write_report` but never linked them into the
    // graph, so execution "succeeded" with no artifact produced.
    let mut reachable: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut stack: Vec<&str> = program.entry.iter().map(String::as_str).collect();
    let mut by_id: std::collections::HashMap<&str, &CirNode> = program
        .nodes
        .iter()
        .map(|n| (n.node_id.as_str(), n))
        .collect();
    while let Some(id) = stack.pop() {
        if !reachable.insert(id) {
            continue;
        }
        if let Some(node) = by_id.get(id) {
            stack.extend(node.children.iter().map(String::as_str));
            stack.extend(node.else_children.iter().map(String::as_str));
        }
    }
    let orphans: Vec<String> = program
        .nodes
        .iter()
        .filter(|n| !reachable.contains(n.node_id.as_str()))
        .map(|n| n.node_id.clone())
        .collect();
    if !orphans.is_empty() {
        return Err(CompilerError::UnreachableNodes { node_ids: orphans });
    }

    // Stage data contract (R1–R5).
    validate_data_contract(program)?;

    // No-op guard: a program with zero primitives cannot produce any task
    // output. Reject it so "compile success" means the program does something.
    if !program.nodes.iter().any(|n| n.capability.is_some()) {
        return Err(CompilerError::NoOpProgram);
    }

    Ok(())
}

/// Validates that all capabilities are in the allowed set.
fn validate_capabilities(program: &CirProgram) -> Result<(), CompilerError> {
    for node in &program.nodes {
        if let Some(ref cap) = node.capability {
            if !ALLOWED_CAPABILITIES.contains(&cap.as_str()) {
                return Err(CompilerError::UnknownCapability {
                    node_id: node.node_id.clone(),
                    capability: cap.clone(),
                });
            }
        }
    }
    Ok(())
}

/// Validates effect declarations.
fn validate_effects(program: &CirProgram) -> Result<(), CompilerError> {
    for effect in &program.effects {
        let kind_str = serde_json::to_value(&effect.kind)
            .ok()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        if !ALLOWED_EFFECT_KINDS.contains(&kind_str.as_str()) {
            return Err(CompilerError::InvalidEffect {
                message: format!("unknown effect kind: {kind_str}"),
            });
        }
        // external_irreversible must be irreversible
        if effect.kind == EffectKind::ExternalIrreversible && effect.reversible {
            return Err(CompilerError::InvalidEffect {
                message: "external_irreversible effect must have reversible: false".into(),
            });
        }
    }
    Ok(())
}

/// Validates control semantics with specific error reporting.
fn validate_control_semantics_detailed(program: &CirProgram) -> Result<(), CompilerError> {
    let outputs: std::collections::HashSet<&str> = program
        .nodes
        .iter()
        .filter_map(|n| n.output.as_ref().map(|o| o.name.as_str()))
        .collect();

    for node in &program.nodes {
        match node.kind {
            CirNodeKind::Conditional => {
                let cond = node
                    .control
                    .as_ref()
                    .and_then(|c| c.condition.as_ref())
                    .ok_or_else(|| CompilerError::InvalidControlSemantics {
                        node_id: node.node_id.clone(),
                        message: "conditional node must have control.condition".into(),
                    })?;
                let expr = acos_core::expr::parse(&cond.expression).map_err(|e| {
                    CompilerError::InvalidControlSemantics {
                        node_id: node.node_id.clone(),
                        message: format!("invalid condition expression: {e}"),
                    }
                })?;
                for id in acos_core::expr::collect_identifiers(&expr) {
                    if !outputs.contains(id.as_str()) {
                        return Err(CompilerError::InvalidReference {
                            node_id: node.node_id.clone(),
                            referenced: id,
                        });
                    }
                }
            }
            CirNodeKind::LoopMap => {
                let spec = node
                    .control
                    .as_ref()
                    .and_then(|c| c.loop_spec.as_ref())
                    .ok_or_else(|| CompilerError::InvalidControlSemantics {
                        node_id: node.node_id.clone(),
                        message: "loop node must have control.loop_spec".into(),
                    })?;
                match spec.kind {
                    LoopKind::While | LoopKind::Until => {
                        if spec.condition.is_none() {
                            return Err(CompilerError::InvalidControlSemantics {
                                node_id: node.node_id.clone(),
                                message: format!(
                                    "{:?} loop must set control.loop_spec.condition",
                                    spec.kind
                                ),
                            });
                        }
                        if spec.max_iterations.is_none() {
                            return Err(CompilerError::InvalidControlSemantics {
                                node_id: node.node_id.clone(),
                                message: format!(
                                    "{:?} loop must set max_iterations (termination guarantee)",
                                    spec.kind
                                ),
                            });
                        }
                    }
                    LoopKind::ForEach => {
                        if spec.input.is_none() || spec.item_var.is_none() {
                            return Err(CompilerError::InvalidControlSemantics {
                                node_id: node.node_id.clone(),
                                message: "for_each loop must set input and item_var".into(),
                            });
                        }
                    }
                }
                if spec.max_iterations == Some(0) {
                    return Err(CompilerError::InvalidControlSemantics {
                        node_id: node.node_id.clone(),
                        message: "max_iterations must be >= 1".into(),
                    });
                }
            }
            _ => {}
        }

        // Retry validation
        if let Some(retry) = node.control.as_ref().and_then(|c| c.retry.as_ref()) {
            if retry.max_attempts == 0 {
                return Err(CompilerError::InvalidControlSemantics {
                    node_id: node.node_id.clone(),
                    message: "retry.max_attempts must be >= 1".into(),
                });
            }
        }
    }
    Ok(())
}

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Extracts the first balanced JSON object from text that may contain
/// markdown fences or surrounding commentary.
pub(crate) fn extract_json_object(text: &str) -> String {
    // Strip common markdown fences if the whole thing is fenced.
    let trimmed = text.trim();
    let candidate = if trimmed.starts_with("```") {
        let without_open = trimmed.trim_start_matches('`').trim_start();
        let body = without_open.strip_prefix("json").unwrap_or(without_open);
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

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::{ProgramId, TaskId};
    use acos_core::types::TaskInput;
    use acos_core::types::{
        CirNode, CirNodeKind, ConditionSpec, ControlSpec, LoopKind, LoopSpec, RetryPolicy,
        RetryStrategy,
    };

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

    // ── P1-5A Robustness Suite ────────────────────────────────────────────────

    /// Helper: creates a ModelCompiler with a dummy LLM client for parse-only tests.
    fn make_test_compiler() -> ModelCompiler {
        ModelCompiler::new(acos_llm::LongCatClient::dummy())
    }

    /// Helper: creates a minimal valid CIR JSON string.
    fn minimal_valid_cir() -> String {
        r#"{
            "id": "test-id",
            "taskId": "task-id",
            "entry": ["root"],
            "nodes": [
                {"kind": "sequence", "nodeId": "root", "capability": null, "output": null, "children": ["step_0"], "inputs": {}},
                {"kind": "primitive_invocation", "nodeId": "step_0", "capability": "read_file", "output": {"name": "raw_0", "typeName": "String", "fields": []}, "children": [], "inputs": {"path": "/tmp/test.txt"}}
            ],
            "effects": [
                {"kind": "fs_read", "description": "read file", "reversible": true}
            ]
        }"#
        .to_string()
    }

    #[test]
    fn robustness_case_1_valid_cir_passes() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let result = compiler.parse_cir(&minimal_valid_cir(), task_id);
        assert!(result.is_ok(), "valid CIR should parse successfully");
        let program = result.unwrap();
        assert_eq!(program.entry, vec!["root"]);
        assert_eq!(program.nodes.len(), 2);
    }

    #[test]
    fn robustness_case_2_markdown_fence_stripped() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let fenced = format!("```json\n{}```", minimal_valid_cir());
        let result = compiler.parse_cir(&fenced, task_id);
        assert!(result.is_ok(), "markdown-fenced CIR should be extracted and parsed");
    }

    #[test]
    fn robustness_case_3_commentary_stripped() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let with_context = format!(
            "Here is my analysis plan:\n\n{}\n\nThis should work.",
            minimal_valid_cir()
        );
        let result = compiler.parse_cir(&with_context, task_id);
        assert!(result.is_ok(), "CIR with surrounding commentary should be extracted");
    }

    #[test]
    fn robustness_case_4_malformed_json_syntax_error() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let malformed = r#"{"nodes": [}"#;
        let result = compiler.parse_cir(malformed, task_id);
        assert!(matches!(result, Err(CompilerError::JsonSyntaxError { .. })));
    }

    #[test]
    fn robustness_case_5_empty_json_missing_fields() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let empty = r#"{}"#;
        let result = compiler.parse_cir(empty, task_id);
        assert!(
            matches!(result, Err(CompilerError::MissingRequiredField { .. })),
            "empty JSON should fail with MissingRequiredField, got: {:?}",
            result
        );
    }

    #[test]
    fn robustness_case_6_missing_entry_field() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let no_entry = r#"{
            "nodes": [
                {"kind": "primitive_invocation", "nodeId": "a", "capability": "read_file", "output": null, "children": [], "inputs": {}}
            ]
        }"#;
        let result = compiler.parse_cir(no_entry, task_id);
        assert!(
            matches!(result, Err(CompilerError::MissingRequiredField { ref field }) if field == "entry"),
            "missing entry should fail with MissingRequiredField, got: {:?}",
            result
        );
    }

    #[test]
    fn robustness_case_7_unknown_capability() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let unknown_cap = r#"{
            "entry": ["root"],
            "nodes": [
                {"kind": "sequence", "nodeId": "root", "capability": null, "output": null, "children": ["step_0"], "inputs": {}},
                {"kind": "primitive_invocation", "nodeId": "step_0", "capability": "teleport", "output": {"name": "x", "typeName": "String", "fields": []}, "children": [], "inputs": {}}
            ],
            "effects": []
        }"#;
        let result = compiler.parse_cir(unknown_cap, task_id);
        assert!(
            matches!(result, Err(CompilerError::UnknownCapability { ref node_id, ref capability }) if node_id == "step_0" && capability == "teleport"),
            "unknown capability should fail with UnknownCapability, got: {:?}",
            result
        );
    }

    #[test]
    fn robustness_case_8_invalid_node_reference() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let bad_ref = r#"{
            "entry": ["root"],
            "nodes": [
                {"kind": "sequence", "nodeId": "root", "capability": null, "output": null, "children": ["nonexistent"], "inputs": {}}
            ],
            "effects": []
        }"#;
        let result = compiler.parse_cir(bad_ref, task_id);
        assert!(
            matches!(result, Err(CompilerError::InvalidReference { ref node_id, ref referenced }) if node_id == "root" && referenced == "nonexistent"),
            "invalid child reference should fail with InvalidReference, got: {:?}",
            result
        );
    }

    #[test]
    fn robustness_case_9_while_without_max_iterations() {
        let compiler = make_test_compiler();
        let task_id = TaskId(uuid::Uuid::new_v4());
        let bad_loop = r#"{
            "entry": ["root"],
            "nodes": [
                {"kind": "sequence", "nodeId": "root", "capability": null, "output": null, "children": ["loop_0"], "inputs": {}},
                {"kind": "loop_map", "nodeId": "loop_0", "capability": null, "output": null, "children": [], "inputs": {}, "control": {"loopSpec": {"kind": "while", "condition": "true"}}}
            ],
            "effects": []
        }"#;
        let result = compiler.parse_cir(bad_loop, task_id);
        assert!(
            matches!(result, Err(CompilerError::InvalidControlSemantics { ref node_id, .. }) if node_id == "loop_0"),
            "while loop without max_iterations should fail with InvalidControlSemantics, got: {:?}",
            result
        );
    }

    // ── Additional semantic validation tests ─────────────────────────────────

    #[test]
    fn validate_rejects_loop_without_max_iterations() {
        let loop_node = CirNode {
            kind: CirNodeKind::LoopMap,
            node_id: "loop".into(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: Some(LoopSpec {
                    kind: LoopKind::While,
                    condition: Some("1 == 1".into()),
                    max_iterations: None,
                    input: None,
                    item_var: None,
                }),
                retry: None,
            }),
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["loop".into()],
            nodes: vec![loop_node],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(matches!(err, CompilerError::InvalidControlSemantics { .. }));
    }

    #[test]
    fn validate_rejects_retry_zero_attempts() {
        let node = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "p".into(),
            capability: Some("read_file".into()),
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: None,
                loop_spec: None,
                retry: Some(RetryPolicy {
                    max_attempts: 0,
                    backoff_ms: 1,
                    strategy: RetryStrategy::Fixed,
                    retry_on: vec![],
                }),
            }),
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["p".into()],
            nodes: vec![node],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(matches!(err, CompilerError::InvalidControlSemantics { .. }));
    }

    #[test]
    fn validate_rejects_else_children_on_non_conditional() {
        let node = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "p".into(),
            capability: Some("read_file".into()),
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec!["x".into()],
            inputs: HashMap::new(),
            control: None,
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["p".into()],
            nodes: vec![node],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(matches!(err, CompilerError::InvalidControlSemantics { .. }));
    }

    #[test]
    fn validate_accepts_valid_conditional() {
        let search = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "search".into(),
            capability: Some("search".into()),
            output: Some(OutputSpec {
                name: "out_search".into(),
                type_name: "String".into(),
                fields: vec![],
            }),
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        let then_node = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "then".into(),
            capability: Some("read_file".into()),
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        let cond = CirNode {
            kind: CirNodeKind::Conditional,
            node_id: "check".into(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: vec!["then".into()],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: Some(ConditionSpec {
                    expression: "exists(out_search)".into(),
                }),
                loop_spec: None,
                retry: None,
            }),
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["search".into(), "check".into()],
            nodes: vec![search, then_node, cond],
            effects: vec![],
        };
        assert!(validate_cir_semantic(&program).is_ok());
    }

    #[test]
    fn validate_rejects_condition_with_undeclared_identifier() {
        let search = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "search".into(),
            capability: Some("search".into()),
            output: Some(OutputSpec {
                name: "out_search".into(),
                type_name: "String".into(),
                fields: vec![],
            }),
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        let cond = CirNode {
            kind: CirNodeKind::Conditional,
            node_id: "check".into(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: Some(ControlSpec {
                condition: Some(ConditionSpec {
                    expression: "exists(nonexistent_var)".into(),
                }),
                loop_spec: None,
                retry: None,
            }),
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["search".into(), "check".into()],
            nodes: vec![search, cond],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(matches!(err, CompilerError::InvalidReference { .. }));
    }

    #[test]
    fn validate_rejects_no_op_program() {
        let root = CirNode {
            kind: CirNodeKind::Sequence,
            node_id: "root".into(),
            capability: None,
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["root".into()],
            nodes: vec![root],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(
            matches!(err, CompilerError::NoOpProgram),
            "zero-primitive program should be rejected, got: {:?}",
            err
        );
    }

    #[test]
    fn validate_rejects_unreachable_nodes() {
        let mut run = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "run".into(),
            capability: Some("execute_python".into()),
            output: Some(OutputSpec {
                name: "result".into(),
                type_name: "String".into(),
                fields: vec![],
            }),
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        run.inputs.insert(
            "code".into(),
            serde_json::Value::String("print('x')".into()),
        );
        let mut write = CirNode {
            kind: CirNodeKind::PrimitiveInvocation,
            node_id: "write_report".into(),
            capability: Some("write_file".into()),
            output: None,
            input_types: HashMap::new(),
            children: vec![],
            else_children: vec![],
            inputs: HashMap::new(),
            control: None,
        };
        write.inputs.insert(
            "path".into(),
            serde_json::Value::String("report.md".into()),
        );
        let program = CirProgram {
            id: ProgramId::new(),
            task_id: TaskId(uuid::Uuid::new_v4()),
            entry: vec!["run".into()],
            nodes: vec![run, write],
            effects: vec![],
        };
        let err = validate_cir_semantic(&program).unwrap_err();
        assert!(
            matches!(
                err,
                CompilerError::UnreachableNodes { ref node_ids } if node_ids == &vec!["write_report".to_string()]
            ),
            "orphan node should be rejected, got: {:?}",
            err
        );
    }

    #[test]
    fn repair_prompt_includes_compile_context() {
        let compiler = make_test_compiler();
        let task = sample_task();
        let err = CompilerError::JsonSyntaxError {
            message: "boom".into(),
            raw_excerpt: "{}".into(),
        };
        let prompt = compiler.build_repair_prompt(&task, "", &err);
        assert!(
            prompt.contains("a.csv") && prompt.contains("b.csv"),
            "repair prompt must carry declared input paths (task facts)"
        );
        assert!(prompt.contains("Input Bindings"));
    }

    #[test]
    fn compiler_error_display_formats_correctly() {        let err = CompilerError::UnknownCapability {
            node_id: "n1".into(),
            capability: "fly".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("n1"));
        assert!(msg.contains("fly"));
    }

    #[test]
    fn compiler_error_converts_to_acos_error() {
        let err = CompilerError::MissingRequiredField {
            field: "entry".into(),
        };
        let acos_err: AcosError = err.into();
        assert!(matches!(acos_err, AcosError::CompilerFailure { .. }));
    }
}
