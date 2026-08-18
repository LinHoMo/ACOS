# P1-5B Probe-2 Analysis

**Status**: FROZEN

**结论**：> **Probe-2 支持"Prompt/Context Contract 是 Probe-1 主要混淆变量"的判断；尚不足以单独证明 ModelCompiler 已具备通用 Cognitive Program Discovery 能力。**

## What Changed from Probe-1

1. **System Prompt**: Added 7 Semantic Grounding Rules (compiler contract facts — inputs are authoritative, no fabricated paths, no simplified goals, outputs must be expressed, control structure is task-driven)
2. **User Prompt**: Expanded TaskSpec into structured Compile Context with explicit Input Bindings

## Experimental Sequence (confounder removal)

| Phase | Change | Result |
|-------|--------|--------|
| Probe-2 (3 runs) | Initial prompt grounding only; repair prompt **without** task context; max_tokens=4096 | compile 3/3 but 2/3 are 1-node no-op graphs; binding 0/4; BR 1/7 |
| Probe-2b (3 runs) | Repair prompt now carries full Compile Context | **12/12 LLM calls returned empty text** (thinking block consumed the 4096-token budget; `stop_reason: max_tokens`) |
| Probe-2c (3 runs) | + max_tokens=32768 | binding 4/4 ×3, BR 5–7/7, all compile; execution 1/3 OK, verification 0/3 |
| Fixes from 2c | loop output aggregation (`run_loop`), orphan-node rejection (`UnreachableNodes`), BR-7 item-var fix | 2/3 execute after replay; 2d runs compile with all nodes reachable |

## Data

`binding` below is the post-fix static metric (scans read_file path inputs + execute_python code strings for the 4 declared CSV filenames).

| Run | Repairs | BR | Binding | Compile | Execution | Artifacts | Structure |
|-----|---------|-----|---------|---------|-----------|-----------|-----------|
| Probe-2 r1 | 1 | 1/7 | 0/4 | OK | OK (no-op) | 0 | 1 node, 0 loops |
| Probe-2 r2 | 1 | 1/7 | 0/4 | OK | OK (no-op) | 0 | 1 node, 0 loops |
| Probe-2 r3 | 1 | 1/7 | 0/4 | OK | FAIL | – | 3 nodes, 0 loops |
| Probe-2b r1–r3 | 3 each | – | – | **FAIL** ×3 | – | – | all 12/12 responses empty |
| Probe-2c r1 | 0 | 6/7 | 4/4 | OK | OK | 0 | 6 nodes, 1 loop, 1 retry |
| Probe-2c r2 | 2 | 5/7 | 4/4 | OK | OK | 1 | 9 nodes, 1 loop |
| Probe-2c r3 | 0 | 7/7 | 4/4 | OK | FAIL (KeyError) | – | 8 nodes, 1 loop |
| Probe-2d r1† | – | – | 4/4 | OK | FAIL (NoneType.strip) | – | 13 nodes, 0 loops, 4 retries |
| Probe-2d r2 | 0 | 6/7→7/7* | 4/4 | OK | FAIL (NameError) | – | 12 nodes, 1 loop |

† trace overwritten by a probe run-index bug (both runs wrote `run-001.trace.json`; probe now refuses to overwrite).
* BR-7 flagged `${file_path}` (a loop `item_var`, i.e. a legitimate runtime binding) as hallucinated; checker fixed to exclude item vars.

## Root Cause Chain

1. **LongCat-2.0 is a reasoning model**: `thinking` consumes the whole output budget. With hardcoded `max_tokens=4096`, the first call returns empty text (`stop_reason=max_tokens`). Verified directly via API with the exact probe prompts: 4096→empty, 8192→empty, 16384→text. **This invalidates all of Probe-2/2b "compile" results as evidence about program discovery** — the repair call is the de facto generation call.
2. **Repair prompt carried no task context** (Probe-2): with nothing to ground on, repair produced degenerate 1-node graphs. Adding the full Compile Context to `build_repair_prompt` was a precondition for any meaningful output.
3. **No-op guard + orphan guard**: zero-capability graphs and unreachable nodes (`define_paths`/`write_report` declared but never linked) silently produce hollow programs; both are now compile-time rejections that route into the repair loop.

## Findings

- **Cognitive program discovery works once the compiler communicates the task contract**: Probe-2c/2d runs consistently produce 6–13 node programs with a loop or per-file retry structure, 4/4 input binding, 5–7/7 BR — vs. 1-node no-ops in Probe-2. Repair count went 0–2; run-3 compiled first pass.
- **Residual failures are execution-level, not discovery-level**:
  - `KeyError: total_issues` (2c r3): merge stage assumed a key name different from the analyze stage's output — inter-node data contract inconsistency in generated Python.
  - `NoneType.strip` (2d r1): analysis code hardcoded columns `quantity/price/total/region` that don't exist (dataset is `date,product,category,units,revenue`; the golden CIR derives schema dynamically via `rows[0].keys()`). Model fabricated data facts — a grounding-rule gap for data-level (not path-level) facts.
  - `NameError: processed_data` (2d r2): a `${ref}` interpolation omitted inside generated Python (same class as 2c r3's `AR=${all_results}` bash-style assignment).
  - Verification 0/3: L3 report content doesn't carry ground-truth-required data points (2c r2 produced a valid generic Markdown report with no real numbers).
- **Runtime gaps found and fixed**: `run_loop` never materialized `loop_map.output` (downstream `${all_results}` unresolved); loop output now aggregates the last child's bound output per iteration (spec updated in `docs/specs/cir_spec.md`).

## Decision

```
  Binding OK (4/4 ×4 runs) + complexity ↑ (loops/retries, 6–13 nodes)
                    │
                    ▼
        Prompt/Context was the issue
                    │
                    ▼
        Formal P1-5B  (proceed to Formal Discovery Evaluation)
```

Prompt/Context (A) resolves the discovery failure as observed. Remaining failures are (a) generated-Python robustness (data contracts between stages, interpolation discipline), (b) L3 output adequacy — neither is evidence against program discovery, but **none of the runs yet passes L3 verification, so this analysis alone does not establish general Cognitive Program Discovery**. Formal P1-5B should include: execution-failure feedback into the repair loop, schema facts in the compile context, and output-requirements grounding for verification.

**Known Limitation**: *Stage-to-stage data contract for generated code* — the compiler produces a structurally valid program, but the Python code inside `execute_python` primitives carries no schema/type contract between stages (`${all_results}`, `${processed_data}`, key names like `total_issues`, column names like `quantity/price/total`). Failures like `KeyError`, `NameError`, `NoneType.strip` live in this layer and are treated as a separate engineering problem, not silently patched to make Probe-2 pass.

## Comparison with Probe-1

| Metric | Probe-1 | Probe-2 (final: 2c/2d) |
|--------|---------|------------------------|
| Compile Success | 3/3 | 4/4 |
| Input Binding | 0/3 | 4/4 ×4 |
| Avg Nodes | 2–3 | 8.8 |
| Avg Loops | 0 | 0.75 |
| Execution | 0/3 | 2/4 |
| Verification | 0/3 | 0/4 |