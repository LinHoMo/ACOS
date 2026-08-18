# SUCCESS-003: P1-3 Direct Tool-Loop Baseline v0.2

**Date**: 2025-08-18
**Status**: PASS — baseline frozen, ready for first real experiment
**Scope**: Simplest LLM+Tools+Loop agent as empirical baseline for ACOS comparison

## Goal

Build a **Direct Tool-Loop Agent** — the simplest possible LLM agent:
- System prompt + tool definitions
- LLM uses **native tool calling** (Anthropic `tool_use`)
- Agent executes tool, feeds result back to LLM
- Loop until LLM outputs final report (no tool call)

**No planner. No compiler. No CIR. No replanner. No memory. No ACOS.**

This serves as the empirical baseline: same tools, same task, same verifier as ACOS.

## Experimental Discipline

Per P1 methodology, the following are frozen:

| Parameter | Value |
|-----------|-------|
| Model | LongCat-2.0 (via `LONGCAT_API_KEY`) |
| Max turns | 20 |
| Max tokens | 4096 |
| Tools | `read_file`, `write_file`, `execute_python` |
| Tool call format | **Native Anthropic tool_use** (not custom XML) |
| Verifier | `acos-verify` three-layer (Structural/Semantic/Evidence) |
| System prompt | Domain-agnostic (no task-specific hints) |

## v0.2 Changes (from v0.1)

### 1. Native Tool Calling (was custom XML parsing)

**Problem**: v0.1 used `<tool_call>JSON</tool_call>` custom format, which added an artificial failure point and made the comparison unfair ("Direct Tool Loop + custom DSL" vs "ACOS").

**Fix**: Now uses Anthropic native `tool_use` content blocks via `LongCatClient::chat_with_tools()`.

```rust
// acos-llm now supports:
let response = llm.chat_with_tools(system, &conversation, Some(&tools)).await?;
// response.tool_calls -> Vec<LlmToolCall> with .id, .name, .input
```

### 2. Cross-Platform Python Detection (was Windows-only)

**Problem**: v0.1 used `where` command (Windows-only), breaking Linux/macOS.

**Fix**: Platform-conditional compilation:

```rust
#[cfg(windows)]
fn find_python() -> Option<&'static str> { /* uses "where" */ }

#[cfg(not(windows))]
fn find_python() -> Option<&'static str> { /* uses "which" */ }
```

### 3. Domain-Agnostic System Prompt (was task-specific)

**Problem**: v0.1 prompt said "You are a data analysis assistant" — giving Baseline advance knowledge of task type.

**Fix**: Now only defines capabilities, not solution approach:

```
You are a general-purpose task agent.

You may use the following tools when needed.
Use tools only when necessary to complete the task accurately.
Return the final result when the task is complete.
```

### 4. Metrics: Self-Reported vs Verified Success (was conflated)

**Problem**: v0.1 had `reported_success = true` whenever LLM stopped calling tools — conflating "LLM said done" with "task actually succeeded".

**Fix**: Three distinct metrics:

| Metric | Meaning |
|--------|---------|
| `self_reported_success` | LLM stopped calling tools |
| `verified_success` | Verifier passed (set externally) |
| `task_success` | Alias for verified_success |

### 5. Honest Token/Cost Display (was misleading $0.0000)

**Problem**: v0.1 showed `Estimated cost: $0.0000` even when cost was unknown.

**Fix**: Now shows `N/A` when unknown:

```rust
self.estimated_cost_usd.map(|c| format!("${:.4}", c)).unwrap_or("N/A".into())
```

### 6. Evidence Adapter Concept (documented)

Baseline produces its own native trace, then adapts to common verification model:

```
Baseline native trace
        ↓
Evidence Adapter
        ↓
Common Verification Model (acos-verify)
```

This is NOT "Baseline pretending to be ACOS Runtime" — it's a clean adapter pattern.

## Implementation

### Files

| File | Action |
|------|--------|
| `crates/acos-llm/src/lib.rs` | Extended with `chat_with_tools`, `ToolDefinition`, `LlmToolCall`, `ChatResponse` |
| `crates/acos-baseline/src/agent.rs` | Rewritten to use native tool calling |
| `crates/acos-baseline/src/tools.rs` | Cross-platform Python detection |
| `crates/acos-baseline/src/metrics.rs` | Added `input_tokens`, `output_tokens`, `self_reported_success`, `verified_success`, `task_success` |
| `crates/acos-baseline/src/evidence.rs` | Unchanged (already had adapter pattern) |
| `crates/acos-cli/src/main.rs` | Unchanged (uses `metrics.summary()`) |

### Architecture

```
┌─────────────────────────────────────────────────────────┐
|  ToolLoopAgent                                          |
|                                                         |
|  ┌─────────────┐     ┌──────────────────┐              |
|  | System Prompt│────→|                  |              |
|  | (domain-     |     |  LLM Call        |              |
|  |  agnostic)   |     |  (native tools)  |              |
|  └─────────────┘     |                  |              |
|                       |  LongCatClient   |              |
|  ┌─────────────┐     |  .chat_with_     |              |
|  | Conversation│────→|  tools()         |              |
|  | History     |     └────────┬─────────┘              |
|  └─────────────┘              │                         |
|                               ▼                         |
|                    ┌────────────────────┐               |
|                    | Response:          |               |
|                    | - text             |               |
|                    | - tool_calls[]     |               |
|                    └────────┬───────────┘               |
|                             │                           |
|              ┌──────────────┼──────────────┐            |
|              ▼                             ▼            |
|     ┌──────────────┐          ┌──────────────┐         |
|     | Execute Tool |          | Final Report |         |
|     | (read/write/ |          | (no tool     |         |
|     |  python)     |          |  calls)      |         |
|     └──────┬───────┘          └──────────────┘         |
|            │                                            |
|            ▼                                            |
|     ┌──────────────┐                                    |
|     | Tool Result  │──→ back to Conversation           |
|     | (native      │                                    |
|     |  format)     │                                    |
|     └──────────────┘                                    |
└─────────────────────────────────────────────────────────┘
```

## Test Results

### Unit Tests

```
acos-llm:     3 passed (chat_message, tool_result, tool_calls)
acos-baseline: 5 passed (config, tools, read_file, write_file, python)
acos-verify:   7 passed
acos-bench:    6 passed (condition, loop, negative, recovery x2, retry)
acos-compiler: 12 passed
acos-core:     15 passed
acos-runtime:  12 passed
acos-plugin:   2 passed
e2e_mini:      3 passed

Total: 65 tests, 0 failures
```

## Acceptance Criteria

| Criterion | Status |
|-----------|--------|
| Native tool calling (not custom XML) | ✓ PASS |
| Cross-platform Python detection | ✓ PASS |
| Domain-agnostic system prompt | ✓ PASS |
| Self-reported vs verified success separated | ✓ PASS |
| Honest cost display (N/A when unknown) | ✓ PASS |
| Evidence adapter documented | ✓ PASS |
| All workspace tests pass | ✓ PASS |
| Bench regression unaffected | ✓ PASS |

## Frozen Configuration

The following are **frozen** for P1 experiments:

```rust
AgentConfig {
    max_turns: 20,
    max_tokens: 4096,
}

Tools: ["read_file", "write_file", "execute_python"]
Tool format: Native Anthropic tool_use
Model: LongCat-2.0 (configurable via ACOS_LLM_MODEL)
```

## Next Steps

1. **Run first real experiment**: ACOS × 5 vs Baseline × 5 on flagship task
2. **Record**: verified_success, duration, LLM calls, tool calls, tokens, cost, verification score
3. **Analyze**: mean, median, stddev, min, max, success rate
4. **Then decide**: P1-4 Fixed Workflow worth doing, or fix Compiler first

---

**Conclusion**: P1-3 Direct Tool-Loop Baseline v0.2 is frozen and ready for experiments. The baseline is now a clean control group — no ACOS advantages, no artificial disadvantages.
