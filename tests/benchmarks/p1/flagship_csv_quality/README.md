# P1-FLAGSHIP-001 — Heterogeneous CSV Quality Analysis & Recovery

The canonical P1 benchmark task. Exercises the full ACOS pipeline and
serves as the first data point for the ACOS Reliability Benchmark v0.1.

## Task Summary

Given a directory of 4 quarterly sales CSV files with deliberate data-quality
issues (schema drift, type errors, missing values, duplicates, outliers,
negative values, extreme outliers), ACOS must:

1. Iterate over all files
2. Detect and classify data-quality issues per file
3. Repair recoverable issues (normalize schemas, coerce types, handle missing values, deduplicate, flag outliers)
4. Revalidate repaired inputs
5. Compute quarterly statistics
6. Merge and review
7. Generate a Markdown report + evidence log

The task injects three failure classes to force the recovery pipeline:

| Failure class | Type | Expected recovery |
|---|---|---|
| Transient network/timeout | retry | `control.retry` succeeds |
| Deterministic capability failure | rule replan | `RuleReplanner` substitutes alternative |
| Unrecoverable local failure | model replan | `ModelRecoveryPlanner` generates new subgraph |

## Dataset

```
datasets/
├── sales_q1.csv  — clean baseline (happy path)
├── sales_q2.csv  — column-name drift + currency formatting + N/A values
├── sales_q3.csv  — duplicate rows + missing values (NULL, N/A, empty)
└── sales_q4.csv  — negative values + extreme outliers (10x normal)
```

## Disturbance Modes

The flagship task has 5 variants. All share the same dataset but inject
different failure patterns:

| Variant | ID | Disturbance |
|---|---|---|
| Normal | `001` | No injected failures (happy path) |
| Malformed Schema | `002` | Extra column + reordered columns in q2 |
| Transient Failure | `003` | First read of q3 times out (retry succeeds) |
| Rule Recovery | `004` | `execute_python` fails deterministically (rule replan substitutes `read_file`) |
| Model Recovery | `005` | Novel failure requiring LLM-generated recovery subgraph |

## Success Metrics (v0.1)

### Primary (comparative)

| Metric | Definition |
|---|---|
| Task Success Rate | % of runs producing a valid report + evidence |
| Compile Success Rate | % of runs where compiler produces valid CIR |
| Recovery Success Rate | % of injected failures that were recovered |
| Verification Accuracy | % of repaired files correctly validated |
| Total Cost | USD (LLM tokens + compute) |
| Total Latency | Wall-clock seconds end-to-end |
| LLM Calls | Total LLM API calls consumed |

### ACOS-Specific (diagnostic)

| Metric | Definition |
|---|---|
| Primitive Count | Number of primitives in compiled program |
| Replan Count | Total replan invocations (rule + model) |
| Retry Count | Total retry attempts |
| Program Depth | Longest path in CIR |
| Program Nodes | Total nodes in CIR |

## Baseline Comparison Protocol

Three systems run the same task suite (all 5 variants):

1. **ACOS** — full pipeline (compile → execute → recover → verify)
2. **Baseline Agent** — single LLM agent with tool use, no compilation layer
3. **Fixed Workflow** — hand-written sequential script with hardcoded error handling

Each system runs each variant 3 times (n=15 per system, n=45 total).
Results are compared on all 7 primary metrics.

## Expected Behavior

A correct ACOS execution MUST exhibit:

- **Iteration**: loops over all 4 files (not hardcoded per-file)
- **Condition**: branches on data-quality check results
- **Retry**: recovers from transient failures automatically
- **Recovery**: recovers from deterministic failures via rule replan
- **Verification**: validates repaired data before analysis
- **Evidence**: logs every decision with timestamp and node reference

## Output Contract

See `expected/schema.yaml` for the mandatory report structure and evidence
requirements. The report MUST contain all 4 required sections; the evidence
log MUST contain all 3 required entry types.

## Version History

- `0.1` (2026-08-18): Initial flagship specification. ACOS-only phase.
