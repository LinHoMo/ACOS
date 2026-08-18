# P1-5B Probe-2 Analysis Template

This file will be populated after running Probe-2.

## Per-Run Analysis

### Run 1
- **Compile**: OK/FAIL
- **Input Binding Accuracy**: X/4 paths matched
- **Behavioral Requirements**: X/7 PASS
- **Execution**: OK/FAIL
- **Verification**: PASS/FAIL

### Run 2
- ...

### Run 3
- ...

## Aggregate

| Metric | Probe-1 | Probe-2 | Delta |
|--------|---------|---------|-------|
| Compile Success | 3/3 | ? | |
| First Pass Success | 0/3 | ? | |
| Input Binding Accuracy | 0% | ? | |
| Avg Nodes | 2-3 | ? | |
| Avg Loops | 0 | ? | |
| Execution Success | 0/3 | ? | |
| Verification Pass | 0/3 | ? | |
| Behavioral Reqs (avg) | 0/7 | ? | |

## Decision

Based on Probe-2 results:
- If binding accuracy > 0 AND complexity improved → Prompt/Context was the issue → Formal P1-5B
- If no improvement → Try B (few-shot)
- If B fails → C (model capability limit)
