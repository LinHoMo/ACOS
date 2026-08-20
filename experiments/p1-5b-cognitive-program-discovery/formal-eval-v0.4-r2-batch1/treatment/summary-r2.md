## P1-5B Formal Evaluation v0.4-R2 (Structured Execution Binding 鈥?H-E controlled)

- Design: single-variable Control vs Treatment (spec docs/specs/2026-08-20-p1-5b-v0.4-r2-structured-execution-binding-controlled-design.md)
- Runs: Control x 1 / Treatment x 0 (target 10 each); task = P1-FLAGSHIP-001
- Fixed conditions: model=deepseek-v4-flash provider=openai temperature=0.0 max_tokens=32768 timeout=600s commit=3e9ef41356af26339814751b74546f59bdced3b2 clock_sanity=unverified
- Session start (UTC): 2026-08-20T04:37:43.7759953Z

### Failure-class matrix

| class | Control | Treatment |
|---|---:|---:|
| execution_program | 1 |  |
| external | 9 | 10 |

### Layer matrix

| arm | Compile | Contract | Execute | Adequacy |
|---|---:|---:|---:|---:|
| Control | 1/1 | 1/1 | 0/1 | 0/1 |
| Treatment | 0/0 | 0/0 | 0/0 | 0/0 |

### Key metrics

- serialization_failure_rate: Control 0% (0) / Treatment N/A (0)
- env_failure_rate: Control 0% (0/1) / Treatment N/A (0/0)
- env_persistence_rate: Control 100% (1/1) / Treatment N/A (0/0)
- binding_adoption_rate (run-level): Control 0% (0/1) / Treatment N/A (0/0)
- binding_adoption_rate (step-level): Control 0% / Treatment N/A
- empty_response_rate: Control 0/1 / Treatment 0/0
- repair_rate: Control 3.00 / Treatment N/A
- latency wall_ms: Control avg 226,971 / med 226971; Treatment avg N/A / med N/A
- token cost (total_tokens/run): Control avg 32,847 / med 32847; Treatment avg N/A / med N/A

### Per-run detail (Control)

| run | class | compile | contract | execute | adequacy | repairs | py_steps | env_refs | inputs_refs | tokens | wall_ms |
|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|
| run-001.trace | execution_program | pass | pass | fail | fail | 3 | 8 | 8 | 0 | 32847 | 226971 |

### Per-run detail (Treatment)

| run | class | compile | contract | execute | adequacy | repairs | py_steps | env_refs | inputs_refs | tokens | wall_ms |
|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|

### Verdict gates (spec section 5)

- G1 binding_adoption: Treatment >= 0.8 AND Control <= 0.2 -> FAIL (N/A / 0%)
- G2 env: Treatment env_failure <= 0.2 AND env_persistence <= 0.2 AND Control env_failure >= 0.5 -> FAIL
- G3 execute: Treatment >= Control - 1 -> PASS (0 vs 0)
- G4 adequacy: Treatment >= Control -> PASS (0 vs 0)
- G5 empty response: Treatment < 5/10 -> PASS (0)

**H-E VERDICT: H-E INCONCLUSIVE**

- H-E SUPPORTED requires G1-G5 all PASS. H-E is an intermediate proposition; Proposition B is out of scope.

### Metadata completeness

- All runs carry complete metadata (commit/provider/model/temperature/max_tokens/timeout/clock/key).

