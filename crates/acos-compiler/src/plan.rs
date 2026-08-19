//! P1-5B v0.2: Plan IR layer 鈥?Structured Program Synthesis.
//!
//! Design (docs/specs/2026-08-19-modelcompiler-v0.2-structured-program-synthesis-design.md):
//!
//! - The LLM produces a **Plan IR** (intent-level: goal, steps, data flow,
//!   control flow). It never writes CIR nodes, bindings, node ids, or
//!   environment plumbing.
//! - [`validate_plan`] checks the Plan against the frozen schema and binding
//!   rules. A valid Plan is the *contract* for compilation.
//! - [`compile_plan`] is a **total function**: `PlanIR.valid() => compile_plan(plan)
//!   => CIR.valid()`. It deterministically lowers the Plan into a [`CirProgram`]
//!   whose bindings, node ids, scopes, and types are compiler-generated, so
//!   undefined-binding errors cannot exist at runtime by construction.
//!
//! Two sources of truth are deliberately avoided: all `${ref}` resolution is
//! compiler-managed. The Plan may only reference:
//! - prior steps' declared outputs (topological order, `steps` array order),
//! - the special `inputs` source (task input files, compiler-read via
//!   generated `read_file` nodes),
//! - the `item` binding inside a `foreach` body (compiler-managed item var).

use serde::{Deserialize, Serialize};

use acos_core::id::{ProgramId, TaskId};
use acos_core::types::{
    CirNode, CirNodeKind, CirProgram, ConditionSpec, ControlSpec, EffectDecl, FieldSpec, LoopKind,
    LoopSpec, OutputSpec, RetryPolicy, RetryStrategy, TaskSpec,
};

use crate::contract::extract_refs;
use crate::validate_cir_semantic;

/// Name of the compiler-managed iteration variable inside a `foreach` body.
pub const ITEM_VAR: &str = "item";

/// Special source name for the task's declared input files.
pub const INPUTS_SOURCE: &str = "inputs";

/// Binding name the compiler gives the task input list when `over: "inputs"`.
pub const INPUTS_BINDING: &str = "input_files";

// 鈹€鈹€ Plan IR 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// Intent-level program plan (P1-5B v0.2).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanIR {
    /// Restatement of the task goal in the planner's own words.
    pub goal: String,
    /// Ordered top-level steps (execution order = array order).
    pub steps: Vec<PlanStep>,
    /// Declared data dependencies, cross-checked against step bindings.
    #[serde(default)]
    pub data_flow: Vec<DataFlowDecl>,
    /// Declared control-flow intents, cross-checked against step kinds.
    #[serde(default)]
    pub control_flow: Vec<ControlDecl>,
}

/// The kind of a plan step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// A single primitive invocation (capability + code).
    Primitive,
    /// Iterate over a list binding; body runs per element.
    Foreach,
    /// Conditional execution of the body.
    Conditional,
    /// Primitive invocation wrapped in a retry policy.
    Retry,
}

/// A single plan step (intent-level, no CIR structures).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlanStep {
    /// Globally unique, identifier-shaped name (becomes the CIR node id).
    pub name: String,
    /// Step kind.
    pub kind: StepKind,
    /// One-sentence description of what the step does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Capability id for primitive/retry steps (e.g. `execute_python`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability: Option<String>,
    /// Python code for primitive/retry steps. May reference `${binding}`
    /// values produced by prior steps (or `${item}` inside a foreach body).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    /// Foreach: source of the list to iterate. Either a prior step's name
    /// (whose output must be a List) or the special name `inputs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<String>,
    /// Conditional: boolean expression (may reference `${binding}`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
    /// Input bindings: parameter name -> (source step, declared output).
    #[serde(default)]
    pub input_bindings: Vec<BindingRef>,
    /// Declared output (name + type + optional field schema).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<StepOutput>,
    /// Body steps for foreach/conditional steps.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub body: Vec<PlanStep>,
    /// Retry policy for retry steps.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetrySpec>,
    /// Output file path for `write_file` steps (output paths are Plan-owned;
    /// input paths are never written by the Plan).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub write_path: Option<String>,
}

/// A single input binding: `param` is bound to the output `binding` declared
/// by step `source`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BindingRef {
    /// Parameter name of the consuming primitive.
    pub param: String,
    /// Source step name (or `inputs`).
    pub source: String,
    /// Declared output name of the source step.
    pub binding: String,
}

/// Declared output of a plan step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StepOutput {
    /// Binding name visible to later steps / `data_flow`.
    pub name: String,
    /// Declared type name (e.g. `CsvAnalysisResult`, `List<CsvAnalysisResult>`).
    pub type_name: String,
    /// Field-level schema for record types (R4). May be empty.
    #[serde(default)]
    pub fields: Vec<FieldSpec>,
}

/// Redundant data-flow declaration (cross-checked against step bindings).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DataFlowDecl {
    /// Source step name.
    pub from_step: String,
    /// Consumer step name.
    pub to_step: String,
    /// Binding that flows between them.
    pub binding: String,
}

/// Redundant control-flow declaration (cross-checked against step kinds).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub struct ControlDecl {
    /// Step name declaring control flow.
    pub step: String,
    /// Declared control kind.
    pub kind: String,
    /// Foreach: iteration source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub over: Option<String>,
    /// Conditional: condition expression.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<String>,
}

/// Retry policy declaration for a retry step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetrySpec {
    /// Total attempts including the first (>= 2 meaningful).
    pub max_attempts: u32,
}

// 鈹€鈹€ Validation 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// Validates a Plan IR against the frozen schema.
///
/// A valid plan is the contract for [`compile_plan`]: every reference is
/// closed, every type is consistent, and every step is compilable. The error
/// strings are human-readable so they can be fed back to the LLM in a repair
/// round (they are also printed verbatim in the trace).
pub fn validate_plan(plan: &PlanIR) -> Result<(), String> {
    if plan.goal.trim().is_empty() {
        return Err("plan.goal must not be empty".into());
    }
    if plan.steps.is_empty() {
        return Err("plan.steps must contain at least one step".into());
    }

    let mut declared: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();

    // Names must be globally unique and identifier-shaped (they become node ids).
    let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
    for step in &plan.steps {
        validate_step(step, &mut seen_names, &mut declared, None)?;
    }

    validate_data_flow(plan, &declared)?;
    validate_control_flow(plan)?;
    Ok(())
}

/// Validates one step (recursively for bodies).
///
/// `scope` = declared outputs visible inside this step's body (ancestor
/// foreach aggregates / prior siblings), plus the `item` binding when inside
/// a foreach body.
#[allow(clippy::too_many_arguments)]
fn validate_step(
    step: &PlanStep,
    seen_names: &mut std::collections::HashSet<String>,
    declared: &mut std::collections::HashMap<String, StepOutput>,
    scope: Option<&std::collections::HashMap<String, StepOutput>>,
) -> Result<(), String> {
    let path = if scope.is_some() { "step in a foreach body" } else { "top-level step" };
    if !is_identifier(&step.name) {
        return Err(format!(
            "{path} '{0}': name must be a valid identifier (letters, digits, underscore)",
            step.name
        ));
    }
    if !seen_names.insert(step.name.clone()) {
        return Err(format!(
            "{path} name '{0}' is duplicated (names must be globally unique)",
            step.name
        ));
    }

    // Binding visibility: everything declared before this step (top-level
    // order) plus ancestor scope bindings plus `item` inside a foreach body.
    let mut visible: std::collections::HashMap<String, StepOutput> = declared.clone();
    if let Some(s) = scope {
        for (k, v) in s {
            visible.insert(k.clone(), v.clone());
        }
    }

    match step.kind {
        StepKind::Primitive | StepKind::Retry => {
            let cap = step.capability.as_deref().ok_or_else(|| {
                format!("step '{}': primitive/retry steps require a capability", step.name)
            })?;
            if !super::ALLOWED_CAPABILITIES.contains(&cap) {
                return Err(format!(
                    "step '{}': capability '{cap}' is not in the allowed set {}",
                    step.name,
                    super::ALLOWED_CAPABILITIES.join(", ")
                ));
            }
            if cap == "write_file" {
                let p = step.write_path.as_deref().unwrap_or("");
                if p.trim().is_empty() {
                    return Err(format!(
                        "step '{}': write_file requires a non-empty writePath",
                        step.name
                    ));
                }
            } else if step.write_path.is_some() {
                return Err(format!(
                    "step '{}': writePath is only valid on write_file steps",
                    step.name
                ));
            }
            if step.kind == StepKind::Retry {
                let r = step.retry.as_ref().ok_or_else(|| {
                    format!("step '{}': retry step requires a retry policy", step.name)
                })?;
                if r.max_attempts < 2 {
                    return Err(format!("step '{}': retry.maxAttempts must be >= 2", step.name));
                }
            } else if step.retry.is_some() {
                return Err(format!("step '{}': retry policy is only valid on retry steps", step.name));
            }
            if cap == "execute_python" {
                let code = step.code.as_deref().unwrap_or("");
                if code.trim().is_empty() {
                    return Err(format!("step '{}': execute_python requires non-empty code", step.name));
                }
                check_code_refs(step, &visible)?;
            }
            if !step.body.is_empty() {
                return Err(format!("step '{}': primitive/retry steps cannot have a body", step.name));
            }
        }
        StepKind::Foreach => {
            let over = step.over.as_deref().ok_or_else(|| {
                format!("step '{}': foreach step requires an 'over' source", step.name)
            })?;
            let ty = resolve_list_source(over, &visible, step)?;
            if step.body.is_empty() {
                return Err(format!("step '{}': foreach step requires a non-empty body", step.name));
            }
            // item binding shadows nothing in the outer scope.
            if visible.contains_key(ITEM_VAR) {
                return Err(format!(
                    "step '{}': foreach item binding '{}' would shadow an outer binding",
                    step.name, ITEM_VAR
                ));
            }
            // The loop body's scope: outer visible bindings + `item`.
            let mut body_scope: std::collections::HashMap<String, StepOutput> = visible.clone();
            body_scope.insert(
                ITEM_VAR.into(),
                StepOutput { name: ITEM_VAR.into(), type_name: element_type(&ty).to_string(), fields: vec![] },
            );
            let mut body_declared: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
            for body_step in &step.body {
                validate_step(body_step, seen_names, &mut body_declared, Some(&body_scope))?;
            }
            // Loop aggregate output must be List<last body output>.
            if let Some(out) = &step.output {
                let last = step.body.iter().rev().find_map(|b| b.output.as_ref());
                match last {
                    Some(last_out) => {
                        let expected = format!("List<{}>", last_out.type_name);
                        if out.type_name != expected {
                            return Err(format!(
                                "step '{}': foreach aggregate output type '{}' must be '{}' (last body step output)",
                                step.name, out.type_name, expected
                            ));
                        }
                    }
                    None => {
                        return Err(format!(
                            "step '{}': foreach declares output '{}' but no body step produces a value",
                            step.name, out.name
                        ));
                    }
                }
                check_output(out, step)?;
            }
        }
        StepKind::Conditional => {
            let cond = step.condition.as_deref().ok_or_else(|| {
                format!("step '{}': conditional step requires a condition", step.name)
            })?;
            if cond.trim().is_empty() {
                return Err(format!("step '{}': condition must not be empty", step.name));
            }
            for raw in extract_refs(&serde_json::Value::String(cond.to_string())) {
                check_ref(&step.name, &raw, &visible, &step.name)?;
            }
            if step.body.is_empty() {
                return Err(format!("step '{}': conditional step requires a non-empty body", step.name));
            }
            let mut body_declared: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
            for body_step in &step.body {
                validate_step(body_step, seen_names, &mut body_declared, Some(&visible))?;
            }
            if step.output.is_some() {
                return Err(format!(
                    "step '{}': conditional steps cannot declare outputs (branch outputs do not escape)",
                    step.name
                ));
            }
        }
    }

    // Input bindings resolve against the visible set; type must be compatible.
    for b in &step.input_bindings {
        check_ref(&step.name, &b.binding, &visible, b.source.as_str())?;
        // The compiler binds param -> producer type automatically (R3 by construction).
        let producer = visible.get(&b.binding).ok_or_else(|| {
            format!("step '{}': binding '{}' from source '{}' is not visible", step.name, b.binding, b.source)
        })?;
        if producer.type_name.is_empty() {
            return Err(format!("step '{}': source '{}' output '{}' has empty type_name", step.name, b.source, b.binding));
        }
        if producer.name != b.binding {
            return Err(format!(
                "step '{}': binding '{}' does not match source '{}' declared output '{}'",
                step.name, b.binding, b.source, producer.name
            ));
        }
    }

    // The step's own output must not collide with visible bindings.
    if let Some(out) = &step.output {
        check_output(out, step)?;
        if visible.contains_key(&out.name) {
            return Err(format!(
                "step '{}': output '{}' would shadow an existing binding",
                step.name, out.name
            ));
        }
        declared.insert(out.name.clone(), out.clone());
    }
    Ok(())
}

/// Output name/type sanity (shared by primitive/foreach).
fn check_output(out: &StepOutput, step: &PlanStep) -> Result<(), String> {
    if out.name.trim().is_empty() || out.type_name.trim().is_empty() {
        return Err(format!("step '{}': output must have non-empty name and typeName", step.name));
    }
    if !is_identifier(&out.name) {
        return Err(format!("step '{}': output name '{}' must be a valid identifier", step.name, out.name));
    }
    if out.name == ITEM_VAR {
        return Err(format!("step '{}': output name cannot be '{ITEM_VAR}' (reserved for loop iteration)", step.name));
    }
    for f in &out.fields {
        if f.name.trim().is_empty() || f.type_name.trim().is_empty() {
            return Err(format!("step '{}': output '{}' has an incomplete field schema", step.name, out.name));
        }
    }
    Ok(())
}

/// Cross-checks `data_flow` declarations against actual step bindings.
fn validate_data_flow(plan: &PlanIR, declared: &std::collections::HashMap<String, StepOutput>) -> Result<(), String> {
    let all_steps = collect_steps(plan);
    let by_name: std::collections::HashMap<&str, &PlanStep> =
        all_steps.iter().map(|s| (s.name.as_str(), *s)).collect();
    for df in &plan.data_flow {
        if !by_name.contains_key(df.from_step.as_str()) {
            return Err(format!("data_flow: from_step '{}' is not a declared step", df.from_step));
        }
        if !by_name.contains_key(df.to_step.as_str()) {
            return Err(format!("data_flow: to_step '{}' is not a declared step", df.to_step));
        }
        if !declared.contains_key(&df.binding) {
            return Err(format!("data_flow: binding '{}' is not declared by any step", df.binding));
        }
        let to = by_name[df.to_step.as_str()];
        let uses = to
            .input_bindings
            .iter()
            .any(|b| b.binding == df.binding)
            || (to.kind == StepKind::Foreach && to.over.as_deref() == Some(df.from_step.as_str()));
        if !uses {
            return Err(format!(
                "data_flow: step '{}' does not consume binding '{}' from '{}'",
                df.to_step, df.binding, df.from_step
            ));
        }
    }
    Ok(())
}

/// Cross-checks `control_flow` declarations against step kinds.
fn validate_control_flow(plan: &PlanIR) -> Result<(), String> {
    let all_steps = collect_steps(plan);
    let by_name: std::collections::HashMap<&str, &PlanStep> =
        all_steps.iter().map(|s| (s.name.as_str(), *s)).collect();
    for cf in &plan.control_flow {
        let step = by_name.get(cf.step.as_str()).ok_or_else(|| {
            format!("control_flow: step '{}' is not a declared step", cf.step)
        })?;
        match (cf.kind.as_str(), &step.kind) {
            ("foreach", StepKind::Foreach) => {
                if let Some(over) = &cf.over {
                    if step.over.as_deref() != Some(over.as_str()) {
                        return Err(format!(
                            "control_flow: step '{}' declared over '{}' but step says '{}'",
                            cf.step,
                            over,
                            step.over.as_deref().unwrap_or("<none>")
                        ));
                    }
                }
            }
            ("conditional", StepKind::Conditional) => {
                if let Some(cond) = &cf.condition {
                    if step.condition.as_deref() != Some(cond.as_str()) {
                        return Err(format!(
                            "control_flow: step '{}' declared condition differs from step's own",
                            cf.step
                        ));
                    }
                }
            }
            ("retry", StepKind::Retry) => {}
            (kind, _) => {
                return Err(format!(
                    "control_flow: step '{}' has kind '{}' but declared control kind '{}'",
                    cf.step,
                    kind_of(&step.kind),
                    kind
                ));
            }
        }
    }
    Ok(())
}

fn kind_of(k: &StepKind) -> &'static str {
    match k {
        StepKind::Primitive => "primitive",
        StepKind::Foreach => "foreach",
        StepKind::Conditional => "conditional",
        StepKind::Retry => "retry",
    }
}

// 鈹€鈹€ Compilation (total function) 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

/// Compilation failure of a *valid* plan. Reaching this is a compiler bug
/// (the total-function contract guarantees it cannot happen for valid plans);
/// it is never an LLM error.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanCompileError {
    /// Human-readable explanation.
    pub message: String,
}

impl std::fmt::Display for PlanCompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "plan compiler internal error: {}", self.message)
    }
}

impl std::error::Error for PlanCompileError {}

/// Deterministically lowers a **valid** Plan IR into a CIR program.
///
/// Total-function contract: if `validate_plan(plan)` returns `Ok`, this
/// function never returns an error (asserted, then re-verified by running the
/// full CIR validation pipeline on the output).
pub fn compile_plan(
    plan: &PlanIR,
    task: &TaskSpec,
    task_id: TaskId,
    program_id: ProgramId,
) -> Result<CirProgram, PlanCompileError> {
    let mut nodes: Vec<CirNode> = Vec::new();
    let mut children: Vec<String> = Vec::new();

    // Compiler-managed task input injection (one per program): any foreach
    // over `inputs` reads the binding produced here. Emitted as the first
    // root child so the loop's LoopSpec.input reference is always in scope.
    if plan_uses_inputs(plan) {
        compile_input_injector(&mut nodes, task)?;
        children.push("task_inputs".into());
    }

    let mut declared: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
    let empty: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
    for step in &plan.steps {
        let id = compile_step(step, &mut nodes, &mut declared, &empty, None, task)?;
        children.push(id);
    }

    let root = CirNode {
        kind: CirNodeKind::Sequence,
        node_id: "plan_root".into(),
        capability: None,
        output: None,
        children,
        else_children: vec![],
        inputs: Default::default(),
        input_types: Default::default(),
        control: None,
    };
    nodes.push(root);

    let program = CirProgram {
        id: program_id,
        task_id,
        entry: vec!["plan_root".into()],
        nodes,
        effects: Vec::<EffectDecl>::new(),
    };

    // Total-function sentinel: a valid plan must compile to a valid CIR.
    validate_cir_semantic(&program)
        .map_err(|e| PlanCompileError { message: e.to_string() })?;
    crate::contract::validate_data_contract(&program)
        .map_err(|e| PlanCompileError { message: e.to_string() })?;

    Ok(program)
}

/// Compiles one step; returns its CIR node id.
///
/// `ancestors` = foreach chain context (outermost first) for body steps;
/// `scope` carries declared outputs of prior siblings.
#[allow(clippy::too_many_arguments)]
fn compile_step(
    step: &PlanStep,
    nodes: &mut Vec<CirNode>,
    siblings: &mut std::collections::HashMap<String, StepOutput>,
    outer: &std::collections::HashMap<String, StepOutput>,
    item_var_ctx: Option<&str>,
    task: &TaskSpec,
) -> Result<String, PlanCompileError> {
    match step.kind {
        StepKind::Primitive | StepKind::Retry => {
            compile_primitive(step, nodes, siblings, outer, item_var_ctx)
        }
        StepKind::Foreach => compile_foreach(step, nodes, siblings, outer, task),
        StepKind::Conditional => compile_conditional(step, nodes, siblings, outer, task),
    }
}

/// Compiles a primitive/retry step into a single `PrimitiveInvocation` node.
fn compile_primitive(
    step: &PlanStep,
    nodes: &mut Vec<CirNode>,
    siblings: &mut std::collections::HashMap<String, StepOutput>,
    outer: &std::collections::HashMap<String, StepOutput>,
    item_var_ctx: Option<&str>,
) -> Result<String, PlanCompileError> {
    let mut inputs: std::collections::HashMap<String, serde_json::Value> = std::collections::HashMap::new();
    let mut input_types: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    if let Some(code) = &step.code {
        inputs.insert("code".into(), serde_json::Value::String(code.clone()));
    }
    if step.capability.as_deref() == Some("write_file") {
        inputs.insert(
            "path".into(),
            serde_json::Value::String(step.write_path.clone().unwrap_or_default()),
        );
    }
    for b in &step.input_bindings {
        // Inside a foreach body, references to the foreach step's own output
        // mean the current element (item var).
        let binding = if item_var_ctx.is_some() && b.source == item_var_ctx.unwrap_or("") {
            ITEM_VAR.to_string()
        } else {
            b.binding.clone()
        };
        inputs.insert(b.param.clone(), serde_json::Value::String(format!("${{{binding}}}")));
        let ty = resolve_binding_type(&b.binding, siblings, outer).ok_or_else(|| {
            PlanCompileError { message: format!("binding '{}' of '{}' not in scope", b.binding, step.name) }
        })?;
        input_types.insert(b.param.clone(), ty);
    }

    let output = step.output.as_ref().map(|o| OutputSpec {
        name: o.name.clone(),
        type_name: o.type_name.clone(),
        fields: o.fields.clone(),
    });

    let retry = if step.kind == StepKind::Retry {
        let r = step.retry.as_ref().ok_or_else(|| PlanCompileError {
            message: format!("retry step '{}' missing policy", step.name),
        })?;
        Some(RetryPolicy {
            max_attempts: r.max_attempts,
            backoff_ms: 100,
            strategy: RetryStrategy::Fixed,
            retry_on: vec![],
        })
    } else {
        None
    };

    let node = CirNode {
        kind: CirNodeKind::PrimitiveInvocation,
        node_id: step.name.clone(),
        capability: step.capability.clone(),
        output,
        input_types,
        children: vec![],
        else_children: vec![],
        inputs,
        control: retry.map(|r| ControlSpec { condition: None, loop_spec: None, retry: Some(r) }),
    };
    nodes.push(node);
    if let Some(out) = &step.output {
        siblings.insert(out.name.clone(), out.clone());
    }
    Ok(step.name.clone())
}

/// Compiles a foreach step into a `LoopMap` node with a compiled body.
fn compile_foreach(
    step: &PlanStep,
    nodes: &mut Vec<CirNode>,
    siblings: &mut std::collections::HashMap<String, StepOutput>,
    outer: &std::collections::HashMap<String, StepOutput>,
    task: &TaskSpec,
) -> Result<String, PlanCompileError> {
    let over = step.over.as_deref().ok_or_else(|| PlanCompileError {
        message: format!("foreach step '{}' missing over", step.name),
    })?;

    let mut child_ids: Vec<String> = Vec::new();
    // Body scope: outer bindings + this foreach's item var.
    let mut body_outer: std::collections::HashMap<String, StepOutput> = outer.clone();
    for (k, v) in siblings.iter() {
        body_outer.insert(k.clone(), v.clone());
    }
    let list_type = if over == INPUTS_SOURCE {
        "List<String>".to_string()
    } else {
        let ty = resolve_binding_type(over, siblings, outer).ok_or_else(|| PlanCompileError {
            message: format!("foreach '{}': source '{over}' has no declared output", step.name),
        })?;
        if !is_list_type(&ty) {
            return Err(PlanCompileError {
                message: format!("foreach '{}': source '{over}' output type '{ty}' is not a List", step.name),
            });
        }
        ty
    };
    body_outer.insert(
        ITEM_VAR.into(),
        StepOutput { name: ITEM_VAR.into(), type_name: element_type(&list_type).to_string(), fields: vec![] },
    );
    let mut body_siblings: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
    for body_step in &step.body {
        let id = compile_step(body_step, nodes, &mut body_siblings, &body_outer, Some(&step.name), task)?;
        child_ids.push(id);
    }

    let loop_input = if over == INPUTS_SOURCE {
        // The injector was emitted by `compile_plan` (plan_uses_inputs guard);
        // if it is missing the compiler is broken, not the plan.
        if !nodes.iter().any(|n| n.node_id == "task_inputs") {
            return Err(PlanCompileError {
                message: "internal: input injector missing for foreach over 'inputs'".into(),
            });
        }
        format!("${{{INPUTS_BINDING}}}")
    } else {
        let out = resolve_binding(over, siblings, outer).ok_or_else(|| PlanCompileError {
            message: format!("foreach '{}': source '{over}' has no declared output", step.name),
        })?;
        format!("${{{}}}", out.name)
    };

    let output = step.output.as_ref().map(|o| OutputSpec {
        name: o.name.clone(),
        type_name: o.type_name.clone(),
        fields: o.fields.clone(),
    });

    let node = CirNode {
        kind: CirNodeKind::LoopMap,
        node_id: step.name.clone(),
        capability: None,
        output,
        input_types: Default::default(),
        children: child_ids,
        else_children: vec![],
        inputs: Default::default(),
        control: Some(ControlSpec {
            condition: None,
            loop_spec: Some(LoopSpec {
                kind: LoopKind::ForEach,
                condition: None,
                max_iterations: None,
                input: Some(loop_input),
                item_var: Some(ITEM_VAR.into()),
            }),
            retry: None,
        }),
    };
    nodes.push(node);
    if let Some(out) = &step.output {
        siblings.insert(out.name.clone(), out.clone());
    }
    Ok(step.name.clone())
}

/// Emits the compiler-managed task input injection for `over: "inputs"`.
///
/// The Plan never contains file paths (P1-5B-A finding: path hallucination).
/// The compiler materializes the task's declared input paths as a
/// `List<String>` binding via a compiler-generated `execute_python` node. The
/// code is a deterministic template 鈥?it is substrate code, not model code.
fn compile_input_injector(nodes: &mut Vec<CirNode>, task: &TaskSpec) -> Result<String, PlanCompileError> {
    const NODE_ID: &str = "task_inputs";
    if nodes.iter().any(|n| n.node_id == NODE_ID) {
        return Ok(INPUTS_BINDING.into());
    }
let paths: Vec<String> = task.inputs.iter().map(|i| i.path.clone()).collect();
    let paths_json = serde_json::to_string(&paths).map_err(|e| PlanCompileError {
        message: format!("cannot serialize task input paths: {e}"),
    })?;
    let code = format!("import json\nprint(json.dumps({paths_json}))\n");
    let node = CirNode {
        kind: CirNodeKind::PrimitiveInvocation,
        node_id: NODE_ID.into(),
        capability: Some("execute_python".into()),
        output: Some(OutputSpec {
            name: INPUTS_BINDING.into(),
            type_name: "List<String>".into(),
            fields: vec![],
        }),
        input_types: Default::default(),
        children: vec![],
        else_children: vec![],
        inputs: std::collections::HashMap::from([(
            "code".into(),
            serde_json::Value::String(code),
        )]),
        control: None,
    };
    nodes.push(node);
    Ok(INPUTS_BINDING.into())
}

/// Compiles a conditional step into a `Conditional` node.
fn compile_conditional(
    step: &PlanStep,
    nodes: &mut Vec<CirNode>,
    siblings: &mut std::collections::HashMap<String, StepOutput>,
    outer: &std::collections::HashMap<String, StepOutput>,
    task: &TaskSpec,
) -> Result<String, PlanCompileError> {
    let cond = step.condition.clone().ok_or_else(|| PlanCompileError {
        message: format!("conditional step '{}' missing condition", step.name),
    })?;
    let mut child_ids: Vec<String> = Vec::new();
    // Branch body scope: outer bindings + prior siblings (validator: body
    // steps see the enclosing visible set). Branch outputs do not escape.
    let mut body_outer: std::collections::HashMap<String, StepOutput> = outer.clone();
    for (k, v) in siblings.iter() {
        body_outer.insert(k.clone(), v.clone());
    }
    let mut body_siblings: std::collections::HashMap<String, StepOutput> = std::collections::HashMap::new();
    for body_step in &step.body {
        let id = compile_step(body_step, nodes, &mut body_siblings, &body_outer, None, task)?;
        child_ids.push(id);
    }
    let node = CirNode {
        kind: CirNodeKind::Conditional,
        node_id: step.name.clone(),
        capability: None,
        output: None,
        input_types: Default::default(),
        children: child_ids,
        else_children: vec![],
        inputs: Default::default(),
        control: Some(ControlSpec {
            condition: Some(ConditionSpec { expression: cond }),
            loop_spec: None,
            retry: None,
        }),
    };
    nodes.push(node);
    Ok(step.name.clone())
}

// 鈹€鈹€ Shared helpers 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn is_list_type(t: &str) -> bool {
    t.trim_start().starts_with("List<") || t == "List"
}

/// Element type of a `List<T>` type name.
fn element_type(t: &str) -> &str {
    let t = t.trim();
    if let Some(inner) = t.strip_prefix("List<") {
        inner.strip_suffix('>').unwrap_or(inner)
    } else {
        "Any"
    }
}

/// Resolves a foreach `over` source to its list type.
fn resolve_list_source(
    over: &str,
    visible: &std::collections::HashMap<String, StepOutput>,
    step: &PlanStep,
) -> Result<String, String> {
    if over == INPUTS_SOURCE {
        return Ok("List<String>".into());
    }
    let out = visible.get(over).ok_or_else(|| {
        format!("step '{}': foreach over '{over}' is not a visible step output", step.name)
    })?;
    if !is_list_type(&out.type_name) {
        return Err(format!(
            "step '{}': foreach over '{over}' has output type '{}' which is not a List",
            step.name, out.type_name
        ));
    }
    Ok(out.type_name.clone())
}

/// Checks `${...}` references in step code against the visible bindings.
fn check_code_refs(step: &PlanStep, visible: &std::collections::HashMap<String, StepOutput>) -> Result<(), String> {
    let code = step.code.clone().unwrap_or_default();
    for raw in extract_refs(&serde_json::Value::String(code)) {
        check_ref(&step.name, &raw, visible, &step.name)?;
    }
    Ok(())
}

/// Resolves a single binding reference against the visible set.
fn check_ref(
    step_name: &str,
    raw: &str,
    visible: &std::collections::HashMap<String, StepOutput>,
    source: &str,
) -> Result<(), String> {
    let mut parts = raw.split('.');
    let name = parts.next().unwrap_or("");
    let spec = visible.get(name).ok_or_else(|| {
        format!(
            "step '{step_name}': reference '${raw}' (source '{source}') does not resolve: no binding '{name}' is visible here"
        )
    })?;
    for field in parts {
        let f = spec.fields.iter().find(|f| f.name == field).ok_or_else(|| {
            format!(
                "step '{step_name}': reference '${raw}' (source '{source}') does not resolve: binding '{name}' has no field '{field}'"
            )
        })?;
        if f.type_name == "List" || f.type_name == "Record" {
            return Err(format!(
                "step '{step_name}': field '{field}' of '{name}' requires indexing (not supported)"
            ));
        }
    }
    Ok(())
}

/// Looks up a binding's declared output (sibling scope first, then outer).
fn resolve_binding<'a>(
    name: &str,
    siblings: &'a std::collections::HashMap<String, StepOutput>,
    outer: &'a std::collections::HashMap<String, StepOutput>,
) -> Option<&'a StepOutput> {
    siblings.get(name).or_else(|| outer.get(name))
}

/// Resolves a binding's declared type name.
fn resolve_binding_type(
    name: &str,
    siblings: &std::collections::HashMap<String, StepOutput>,
    outer: &std::collections::HashMap<String, StepOutput>,
) -> Option<String> {
    resolve_binding(name, siblings, outer).map(|o| o.type_name.clone())
}

/// Whether any (nested) step iterates over the task inputs.
fn plan_uses_inputs(plan: &PlanIR) -> bool {
    fn walk(steps: &[PlanStep]) -> bool {
        steps.iter().any(|s| {
            (s.kind == StepKind::Foreach && s.over.as_deref() == Some(INPUTS_SOURCE))
                || walk(&s.body)
        })
    }
    walk(&plan.steps)
}

/// Collects every step (recursively) in array order.
fn collect_steps(plan: &PlanIR) -> Vec<&PlanStep> {
    fn walk<'a>(steps: &'a [PlanStep], out: &mut Vec<&'a PlanStep>) {
        for s in steps {
            out.push(s);
            walk(&s.body, out);
        }
    }
    let mut out = Vec::new();
    walk(&plan.steps, &mut out);
    out
}

// 鈹€鈹€ Tests 鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

#[cfg(test)]
mod tests {
    use super::*;
    use acos_core::id::ProgramId;

    fn step(name: &str, kind: StepKind) -> PlanStep {
        PlanStep {
            name: name.into(),
            kind,
            description: None,
            capability: None,
            code: None,
            over: None,
            condition: None,
            input_bindings: vec![],
            output: None,
            body: vec![],
            retry: None,
            write_path: None,
        }
    }

    fn out(name: &str, ty: &str) -> StepOutput {
        StepOutput { name: name.into(), type_name: ty.into(), fields: vec![] }
    }

    fn task() -> TaskSpec {
        TaskSpec {
            api_version: "acos.io/v1".into(),
            id: acos_core::id::TaskId(uuid::Uuid::new_v4()),
            goal: "analyze csv files".into(),
            inputs: vec![
                acos_core::types::TaskInput {
                    input_type: "File".into(),
                    path: "data/a.csv".into(),
                    format: Some("csv".into()),
                },
                acos_core::types::TaskInput {
                    input_type: "File".into(),
                    path: "data/b.csv".into(),
                    format: Some("csv".into()),
                },
            ],
            outputs: vec![acos_core::types::TaskOutput {
                output_type: "Report".into(),
                format: Some("markdown".into()),
            }],
            constraints: None,
            optimization: None,
            approval: None,
        }
    }

    fn compile_ok(plan: &PlanIR) -> CirProgram {
        validate_plan(plan).unwrap_or_else(|e| panic!("plan invalid: {e}"));
        compile_plan(plan, &task(), TaskId(uuid::Uuid::new_v4()), ProgramId::new()).unwrap()
    }

    fn validate_data_contract_via_program(program: &CirProgram) -> Result<(), String> {
        crate::contract::validate_data_contract(program).map_err(|e| e.to_string())
    }

    #[test]
    fn valid_plan_with_inputs_loop_compiles() {
        let plan = PlanIR {
            goal: "analyze csv files".into(),
            steps: vec![
                PlanStep {
                    name: "analyze_each".into(),
                    kind: StepKind::Foreach,
                    description: Some("analyze one file".into()),
                    capability: None,
                    code: None,
                    over: Some(INPUTS_SOURCE.into()),
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out("per_file", "List<CsvAnalysisResult>")),
                    body: vec![PlanStep {
                        name: "process_file".into(),
                        kind: StepKind::Primitive,
                        description: None,
                        capability: Some("execute_python".into()),
                        code: Some("r = analyze(\"${item}\")".into()),
                        over: None,
                        condition: None,
                        input_bindings: vec![],
                        output: Some(out("analysis", "CsvAnalysisResult")),
                        body: vec![],
                        retry: None,
                        write_path: None,
                    }],
                    retry: None,
                    write_path: None,
                },
            ],
            data_flow: vec![],
            control_flow: vec![ControlDecl {
                step: "analyze_each".into(),
                kind: "foreach".into(),
                over: Some(INPUTS_SOURCE.into()),
                condition: None,
            }],
        };
        let program = compile_ok(&plan);
        assert_eq!(program.entry, vec!["plan_root".to_string()]);
        assert_eq!(program.nodes.len(), 4); // root + input injector + loop + body
        let loop_node = program.nodes.iter().find(|n| n.node_id == "analyze_each").unwrap();
        assert_eq!(loop_node.kind, CirNodeKind::LoopMap);
        let spec = loop_node.control.as_ref().unwrap().loop_spec.as_ref().unwrap();
        assert_eq!(spec.input.as_deref(), Some("${input_files}"));
        assert_eq!(spec.item_var.as_deref(), Some(ITEM_VAR));
        let body = program.nodes.iter().find(|n| n.node_id == "process_file").unwrap();
        assert_eq!(body.inputs.get("code").unwrap(), "r = analyze(\"${item}\")");
        // code templates are interpolated at runtime; the contract already
        // resolved `${item}` against the loop's item var.
        assert!(validate_data_contract_via_program(&program).is_ok());
    }

    #[test]
    fn valid_plan_rejects_undefined_binding_at_plan_level() {
        let mut plan = PlanIR {
            goal: "x".into(),
            steps: vec![PlanStep {
                name: "use_missing".into(),
                kind: StepKind::Primitive,
                description: None,
                capability: Some("execute_python".into()),
                code: Some("x = ${missing_binding}".into()),
                over: None,
                condition: None,
                input_bindings: vec![],
                output: Some(out("r", "String")),
                body: vec![],
                retry: None,
                write_path: None,
            }],
            data_flow: vec![],
            control_flow: vec![],
        };
        let err = validate_plan(&mut plan).unwrap_err();
        assert!(err.contains("missing_binding"), "unexpected error: {err}");
    }

    #[test]
    fn valid_plan_foreach_aggregate_type_must_match_body() {
        let plan = PlanIR {
            goal: "x".into(),
            steps: vec![PlanStep {
                name: "loop".into(),
                kind: StepKind::Foreach,
                description: None,
                capability: None,
                code: None,
                over: Some(INPUTS_SOURCE.into()),
                condition: None,
                input_bindings: vec![],
                output: Some(out("results", "List<OtherType>")),
                body: vec![PlanStep {
                    name: "inner".into(),
                    kind: StepKind::Primitive,
                    description: None,
                    capability: Some("execute_python".into()),
                    code: Some("pass".into()),
                    over: None,
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out("r", "CsvAnalysisResult")),
                    body: vec![],
                    retry: None,
                    write_path: None,
                }],
                retry: None,
                write_path: None,
            }],
            data_flow: vec![],
            control_flow: vec![],
        };
        let err = validate_plan(&plan).unwrap_err();
        assert!(err.contains("List<CsvAnalysisResult>"), "unexpected error: {err}");
    }

    #[test]
    fn valid_plan_body_can_consume_foreach_item() {
        let plan = PlanIR {
            goal: "x".into(),
            steps: vec![PlanStep {
                name: "loop".into(),
                kind: StepKind::Foreach,
                description: None,
                capability: None,
                code: None,
                over: Some(INPUTS_SOURCE.into()),
                condition: None,
                input_bindings: vec![],
                output: None,
                body: vec![PlanStep {
                    name: "inner".into(),
                    kind: StepKind::Primitive,
                    description: None,
                    capability: Some("execute_python".into()),
                    code: Some("f(\"${item}\")".into()),
                    over: None,
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out("r", "String")),
                    body: vec![],
                    retry: None,
                    write_path: None,
                }],
                retry: None,
                write_path: None,
            }],
            data_flow: vec![],
            control_flow: vec![],
        };
        let p = compile_ok(&plan);
        let body = p.nodes.iter().find(|n| n.node_id == "inner").unwrap();
        assert_eq!(body.inputs.get("code").unwrap(), "f(\"${item}\")");
        assert!(validate_plan(&plan).is_ok());
    }

    #[test]
    fn invalid_duplicate_names_rejected() {
        let plan = PlanIR {
            goal: "x".into(),
            steps: vec![step("dup", StepKind::Primitive), step("dup", StepKind::Primitive)],
            data_flow: vec![],
            control_flow: vec![],
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn invalid_item_binding_shadow_rejected() {
        let plan = PlanIR {
            goal: "x".into(),
            steps: vec![
                PlanStep {
                    name: "producer".into(),
                    kind: StepKind::Primitive,
                    description: None,
                    capability: Some("execute_python".into()),
                    code: Some("x = 1".into()),
                    over: None,
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out(ITEM_VAR, "String")),
                    body: vec![],
                    retry: None,
                    write_path: None,
                },
            ],
            data_flow: vec![],
            control_flow: vec![],
        };
        let err = validate_plan(&plan).unwrap_err();
        assert!(err.contains("reserved"), "unexpected error: {err}");
    }

    #[test]
    fn retry_step_requires_policy() {
        let mut plan = PlanIR {
            goal: "x".into(),
            steps: vec![step("r", StepKind::Retry)],
            data_flow: vec![],
            control_flow: vec![],
        };
        assert!(validate_plan(&plan).is_err());
        plan.steps[0].capability = Some("execute_python".into());
        plan.steps[0].code = Some("pass".into());
        plan.steps[0].retry = Some(RetrySpec { max_attempts: 3 });
        assert!(validate_plan(&plan).is_ok());
        let p = compile_ok(&plan);
        let node = p.nodes.iter().find(|n| n.node_id == "r").unwrap();
        assert_eq!(node.control.as_ref().unwrap().retry.as_ref().unwrap().max_attempts, 3);
    }

    #[test]
    fn conditional_requires_condition_and_no_output() {
        let plan = PlanIR {
            goal: "x".into(),
            steps: vec![PlanStep {
                name: "cond".into(),
                kind: StepKind::Conditional,
                description: None,
                capability: None,
                code: None,
                over: None,
                condition: Some("${has_data}.length > 0".into()),
                input_bindings: vec![],
                output: None,
                body: vec![PlanStep {
                    name: "inner".into(),
                    kind: StepKind::Primitive,
                    description: None,
                    capability: Some("execute_python".into()),
                    code: Some("pass".into()),
                    over: None,
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out("r", "String")),
                    body: vec![],
                    retry: None,
                    write_path: None,
                }],
                retry: None,
                write_path: None,
            }],
            data_flow: vec![],
            control_flow: vec![],
        };
        let err = validate_plan(&plan).unwrap_err();
        assert!(err.contains("has_data"), "unexpected error: {err}");
    }

    #[test]
    fn total_function_valid_plan_always_compiles() {
        let plans = vec![
            PlanIR {
                goal: "a".into(),
                steps: vec![PlanStep {
                    name: "s1".into(),
                    kind: StepKind::Primitive,
                    description: None,
                    capability: Some("execute_python".into()),
                    code: Some("x = 1".into()),
                    over: None,
                    condition: None,
                    input_bindings: vec![],
                    output: Some(out("r1", "Number")),
                    body: vec![],
                    retry: None,
                    write_path: None,
                }],
                data_flow: vec![],
                control_flow: vec![],
            },
            PlanIR {
                goal: "b".into(),
                steps: vec![
                    PlanStep {
                        name: "s1".into(),
                        kind: StepKind::Primitive,
                        description: None,
                        capability: Some("execute_python".into()),
                        code: Some("x = 1".into()),
                        over: None,
                        condition: None,
                        input_bindings: vec![],
                        output: Some(out("r1", "Number")),
                        body: vec![],
                        retry: None,
                        write_path: None,
                    },
                    PlanStep {
                        name: "s2".into(),
                        kind: StepKind::Primitive,
                        description: None,
                        capability: Some("execute_python".into()),
                        code: Some("y = ${r1}".into()),
                        over: None,
                        condition: None,
                        input_bindings: vec![BindingRef { param: "r1".into(), source: "s1".into(), binding: "r1".into() }],
                        output: None,
                        body: vec![],
                        retry: None,
                        write_path: None,
                    },
                ],
                data_flow: vec![DataFlowDecl { from_step: "s1".into(), to_step: "s2".into(), binding: "r1".into() }],
                control_flow: vec![],
            },
        ];
        for plan in &plans {
            compile_ok(plan);
        }
    }
}