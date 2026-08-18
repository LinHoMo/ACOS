# 设计原则 / Design Principles

## 三支柱 / Three Pillars

1. **Stable Core + Everything Extensible**：稳定的核心运行时 + 一切可替换能力（LLM Provider、Memory、Search、Browser、Code Runtime、Planner、Verifier、Knowledge Store、Environment Adapter）皆以标准化插件接入
2. **Cognitive Program as First-Class Citizen**：认知程序是一等公民，Agent 只是某个 Cognitive Program 在运行时形成的动态执行实体
3. **Reliable by Default**：状态可追踪、执行可恢复、副作用可管理、结果可验证、过程可回放、失败可重新规划、证据可追溯

## 机制原则 / Mechanism Principles

## 1. 编译器优先于编排 / Compiler before orchestration

差异化在于认知程序的综合（synthesis of cognitive programs），而不是通用工作流执行（generic workflow execution）。

## 2. 显式契约 / Explicit contracts

任何期望被组合的东西都必须暴露机器可读的契约（machine-readable contracts）。

## 3. 尽可能确定性 / Deterministic where possible

在模型判断（model judgment）之前，使用模式（schemas）、测试（tests）、哈希（hashes）、规则（rules）和形式化检查（formal checks）。

## 4. LLM 是提议者，而非不容置疑的权威 / LLM as proposer, not unquestioned authority

模型可以提议任务分解、映射和审查。运行时强制执行契约和策略。

## 5. 持久状态在临时 Agent 之外 / Durable state outside ephemeral agents

工作器可能死亡；任务状态必须存活。

## 6. 用户拥有工作区 / User-owned workspace

已安装的能力应可检查（inspectable）、可导出（exportable）、可移除（removable）。

## 7. 本地优先的 MVP / Local-first MVP

在测量证明需要之前，避免不必要的分布式基础设施。

## 8. 模型可见即已记录 / Model-visible means logged

任何到达模型请求的输入都必须能从事件日志重建。这是可复现性和可审计性的基础保证——如果不遵守此不变量，就无法复现过去的运行、无法审计模型决策、无法完整验证。

## 9. 完成是外部验证状态 / Completion is an external verification state

Agent 的自我声明只能表示 `self_reported_success`，不能直接决定任务完成。`task_success` 必须由 Runtime 执行状态与独立 Verification 结果共同决定。

**动机**：P1-R1 实验（`SUCCESS-004`）中，Direct Tool-Loop Baseline 的 self-reported success 为 100%（5/5），但 verified success 为 0%（0/5）。模型每次都"认为"自己完成了任务，但实际上从未产出符合规范的输出。这种 **Completion Illusion** 是 LLM Agent 的系统性缺陷。

**状态模型**：

```text
RUNNING
  ↓
EXECUTION_COMPLETED      ← maps to self_reported_success
  ↓
VERIFICATION             ← 独立验证器判定
  ├── PASS → SUCCEEDED   ← maps to verified_success / task_success
  └── FAIL → FAILED / RECOVERABLE
```

**四个术语的严格对应**：

| 术语 | 产生方 | 含义 | 不应作为 |
|------|--------|------|----------|
| `self_reported_success` | Agent 内部 | "我认为我做完了" | 任务完成的充分条件 |
| `execution_completed` | Runtime | 程序执行完毕，无 panic | 输出正确的证据 |
| `verified_success` | Verifier | 输出通过独立验证 | —— |
| `task_success` | 系统对外 | = `verified_success`，唯一可信的完成状态 | —— |

**设计约束**：
- Runtime 必须区分 `execution_completed`（程序跑完）与 `verification_passed`（输出正确）
- Verifier 必须独立于 Agent 的执行逻辑（不能复用 Agent 自己的判断）
- 实验报告必须同时记录 self-reported 与 verified 两个指标
- 任何外部系统（UI、API、下游任务）只能依赖 `verified_success` / `task_success`
