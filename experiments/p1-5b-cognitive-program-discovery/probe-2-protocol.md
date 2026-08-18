# P1-5B Probe-2 Protocol

## Hypothesis

Probe-1 showed that the model ignores TaskSpec.inputs and generates trivial
programs. Probe-2 tests whether **Semantic Grounding Prompt + Structured
Compile Context** fixes this without giving away the answer.

## What Changed

1. **System Prompt**: Added 7 Semantic Grounding Rules (compiler contract)
2. **User Prompt**: Expanded TaskSpec into structured Compile Context with
   explicit Input Bindings section

## What We Measure

### Primary: Input Binding Accuracy
- Do generated CIR paths match declared TaskSpec.inputs paths?
- Probe-1 result: 0/3 (all hallucinated /tmp/...)
- Probe-2 target: > 0 (at least some runs use correct paths)

### Secondary: Complexity Adequacy
- Does the CIR reflect the multi-step nature of the goal?
- Probe-1 result: 1-3 nodes, no loops, no conditions
- Probe-2 target: more nodes, at least one loop or validation chain

### Tertiary: Execution & Verification
- Does the program run? Does it pass ground truth?
- Probe-1 result: 0/3 execution (failed on missing files)
- Probe-2 target: execution success if paths are correct

## Decision Tree

```
             Probe-2 result
                  │
      ┌───────────┴───────────┐
      │                       │
  Binding OK              Binding still fails
  Complexity ↑            No improvement
      │                       │
      ▼                       ▼
  Prompt/Context            Try B (few-shot)
  was the issue             │
      │                     ├─→ improves → B works
      ▼                     └─→ no improve → C (model limit)
  Formal P1-5B
```

## Runs

3 runs (same as Probe-1), LongCat-2.0, same flagship task.
