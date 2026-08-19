## P1-5B Formal Evaluation v0.3 (Capability Contract & Typed Execution)

- Groups: A = historical frozen control (v0.2 traces @ 7a3b36a, NOT re-sampled); B = + csv.inspect_schema (observe); C = + csv.aggregate (runtime schema enforcement)
- Runs: B x 5, C x 5, P1-FLAGSHIP-001, LongCat-2.0, main @ a111486
- Spec: docs/specs/2026-08-19-p1-5b-v0.3-capability-contract-typed-execution-design.md (FROZEN)

### Layer matrix (A vs B vs C)

| Group | Compile | Contract | Execute | Adequacy |
|---|---:|---:|---:|---:|
| A (v0.2 frozen control) | 80% | see v0.2 report | 0/5 | 0% |
| B (Observe) | 100% | 100% | 0% | 0% |
| C (Enforce) | 20% | 20% | 0% | 0% |

### Per-run detail (B)

| run | compile | contract | execute | adequacy | repairs | inspect | aggregate | error |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| run-001.trace | pass | pass | fail | fail | 1 | 1 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |
| run-002.trace | pass | pass | fail | fail | 2 | 0 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |
| run-003.trace | pass | pass | fail | fail | 1 | 0 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |
| run-004.trace | pass | pass | fail | fail | 3 | 1 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |
| run-005.trace | pass | pass | fail | fail | 3 | 0 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |

### Per-run detail (C)

| run | compile | contract | execute | adequacy | repairs | inspect | aggregate | error |
|---|---:|---:|---:|---:|---:|---:|---:|---|
| run-001.trace | pass | pass | fail | fail | 2 | 1 | 0 | primitive failure: python exited Some(1): Traceback (most recent call last): |
| run-002.trace | fail | fail | fail | fail | 3 | 0 | 0 |  |
| run-003.trace | fail | fail | fail | fail | 3 | 0 | 0 |  |
| run-004.trace | fail | fail | fail | fail | 3 | 0 | 0 |  |
| run-005.trace | fail | fail | fail | fail | 3 | 0 | 0 |  |

### Capability metrics (B / C)

- inspect usage rate (inspect_requested / total_plans): B 40% (2 plans) / C 20% (1 plans)
- inspect result consumed (later step binds the inspect output): B 1 / C 0
- schema utilization rate (consumed / inspect plans): B 50% / C 0%
- schema hallucination rate (invalid_field_references / all_field_references, 0/0 -> N/A): B N/A / C N/A  [v1 proxy = runtime 'unknown column' rejections (B 0 / C 0); no csv.aggregate steps in B or C this round -> no field references -> 0/0 -> N/A]
- code defect rate (defective execute_python steps / total execute_python steps): B 14% (5/35) / C 14% (1/7)  [v1 proxy = 1 defective step per failing run; classes NameError/KeyError/SyntaxError/Other]
- repair rate (avg repairs per run): A (v0.2) 0.60 / B 2.00 / C 2.80

### H-C (Capability Contract Hypothesis) verdict

- C Compile >= 80%: False (1/5) ; C Adequacy >= 60%: False (0/5)
- **H-C NOT SUPPORTED** - C passing supports the Capability Contract intermediate proposition; it does NOT by itself establish Proposition B (end-to-end discovery of executable, adequate Cognitive Programs), which requires validation on more tasks.
- B reaching Compile >= 80% AND Adequacy >= 60% (False) would support the claim that the model can autonomously use capability contracts (not observed).


### Root cause analysis (from traces)

- **C Compile collapse (4/5) - code-as-map serialization error.** In every failed C run the model
  emitted `"code": {"path": "${item}"}` (object) in the Plan IR instead of the required JSON
  string form `"code": "{\"path\": \"${item}\"}"`; CIR validation rejected it with
  `CIR schema mismatch: invalid type: map, expected a string`. All 3 repair attempts per run
  re-emitted the same shape (repair prompts show the error + excerpt; the model does not learn
  the string-serialization rule). The C-mode CSV teaching (params described as JSON objects,
  runtime auto-parse) invites the object form at plan level, which the schema forbids.
- **B ignore-the-capability:** only 2/5 B plans contain csv.inspect_schema (1 consumed via
  data-flow binding); 3/5 reproduced the v0.2 plan verbatim (plain execute_python only).
- **Dominant execution defect unchanged:** all 6 executable runs (B 5/5, C 1/1) failed with
  `NameError: name 'env' is not defined` in execute_python code - the exact v0.2 defect.
  Capability contracts targeted schema hallucination, not this class; code defect rate is
  identical across groups (14%).
- **csv.aggregate never adopted (0 steps in B and C).** No run attempted the enforcement
  capability, so runtime schema enforcement was never exercised; schema hallucination
  rate = 0/0 = N/A as designed.
- **Repair tax rising without benefit:** repair rate A 0.60 -> B 2.00 -> C 2.80, yet no repair
  attempt fixed the underlying misconception; repairs were burned on unfixable-under-current-
  prompt errors, inflating cost (C-005 compile alone: 24.9 min).

### Findings

1. Capability introduction without serialization-level teaching (C) breaks compile (20% vs 80%
   frozen v0.2); observe-mode introduction (B) preserves compile but is ignored 60% of the time.
2. The persistent `env[]` misconception (100% of executed failures) is orthogonal to schema
   hallucination; it requires its own treatment (prompt, runtime, or removal of env).
3. H-C (Capability Contract Hypothesis): **NOT SUPPORTED** - C Compile 20% < 80% threshold and
   C Adequacy 0% < 60% threshold. The intermediate proposition (capability contracts
   meaningfully raise reliability) is not supported by this round; autonomous capability
   contract use by the model is also not observed (B Adequacy 0%).
4. Proposition B remains out of scope (requires multi-task validation); this round neither
   supports nor refutes it.

### Next steps (recommendations for v0.4)

1. Explicit serialization contract in the prompt: "code is a JSON STRING in the plan; at runtime
   the platform parses it into a params object" + worked example (the smoke-verified
   inspect -> inputBindings -> aggregate pattern).
2. Address the env[] defect class directly (runtime removal of env, or stronger teaching);
   without it Adequacy cannot move regardless of capability contracts.
3. Re-run C x 5 after prompt fix; re-measure Compile/Adequacy and schema hallucination
   (expected to become measurable once csv.aggregate is adopted).
4. Keep A as frozen v0.2 control; do not re-sample.

### Artifacts

- Traces: formal-eval-v0.3-results-b/run-00X.trace.json, formal-eval-v0.3-results-c/run-00X.trace.json
- Harness: formal-eval-v0.3.ps1 (spec-aligned metrics, ASCII-safe output, -AggregateOnly mode)
- Spec: docs/specs/2026-08-19-p1-5b-v0.3-capability-contract-typed-execution-design.md (FROZEN)
- Pipeline smoke (all passed): plan-smoke-v0.3.json via p1-5b-plan-smoke