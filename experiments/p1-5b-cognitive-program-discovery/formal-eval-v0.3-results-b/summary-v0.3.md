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

