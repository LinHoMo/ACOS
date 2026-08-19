## P1-5B Formal Evaluation v0.2 (Structured Program Synthesis)

- Runs: 5 x ModelCompiler (Plan IR), P1-FLAGSHIP-001, LongCat-2.0
- Spec: docs/specs/2026-08-19-modelcompiler-v0.2-structured-program-synthesis-design.md (FROZEN)

### Layer matrix (v0.2 vs frozen v0.1)

| System | Compile | Contract | Execute | Adequacy |
|---|---:|---:|---:|---:|
| ModelCompiler v0.2 (Plan IR) | 80% | 80% | 0% | 0% |
| ModelCompiler v0.1 (direct CIR, frozen) | see formal-eval-v0.1-results | | | |

### Per-run detail (Experiment A)

| run | compile | contract | execute | adequacy | repairs | control intent | adopted | recall | completeness | coverage |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| run-001.trace | fail | fail | fail | fail | 1 | 2 | 0 | 0% | 33% | 67% |
| run-002.trace | pass | pass | fail | fail | 1 | 1 | 1 | 100% | 33% | 33% |
| run-003.trace | pass | pass | fail | fail | 1 | 1 | 1 | 100% | 33% | 33% |
| run-004.trace | pass | pass | fail | fail | 0 | 1 | 1 | 100% | 33% | 33% |
| run-005.trace | pass | pass | fail | fail | 0 | 1 | 1 | 100% | 33% | 33% |

### Plan metrics (Experiment A)

- Control Intent Recall: 67% (adopted 4 / 6 declared control intents)
- Plan completeness (avg): 33% (6 behavioral requirements)
- Control coverage (avg): 40% (required foreach/conditional/retry = 3)

### Two-stage comparison (Experiment B, vs frozen v0.1)

- v0.1 Compile 1/5 (20%), v0.2 Compile 4/5 (80%)
- Repair Tax: v0.2 average repairs per run = 0.60 (first-pass success 2/5)

### Contract integration (Experiment C)

- Compile-time contract failures surfaced (final_error): 1
- Repair attempts (contract violations caught by repair loop): 3
- Plan binding closure: 20/20 (100%)

## Proposition B verdict

- **NOT SUPPORTED**

