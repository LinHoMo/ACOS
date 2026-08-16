# 执行模型 / Execution Model

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-runtime/src/executor.rs`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 图执行 / Graph execution

执行是持久的且有检查点的（durable and checkpointed）。

支持的控制结构（Supported control structures）：

- sequence（序列）
- parallel（并行）
- conditional（条件）
- loop/map（循环/映射）
- retry（重试）
- timeout（超时）
- compensation（补偿）
- human approval（人工审批）

## 幂等性 / Idempotency

每个有副作用的操作必须在可行时暴露幂等性策略（idempotency strategy）。默认禁止在没有补偿的情况下重试非幂等操作。

## 检查点 / Checkpoints

检查点状态包括：

- 已完成的操作（completed operations）
- 输入/输出引用（input/output references）
- 提供者/版本身份（provider/version identities）
- 世界状态版本（world-state version）
- 待处理的审批（pending approvals）
- **已声明但未执行的补偿操作（declared but unexecuted compensations）**——任务失败时按 LIFO 顺序执行

## 失败策略 / Failure policies

支持的策略类别（Supported policy classes）：

- retry with backoff（带退避的重试）
- provider failover（提供者故障转移）
- replan from checkpoint（从检查点重新规划）
- compensate（补偿）
- pause for approval（暂停等待审批）
- fail task（任务失败）

## 补偿机制 / Compensation Mechanism

每个声明效果的原语必须提供补偿操作（compensation）。补偿是效果的逆操作，用于在任务失败或验证未通过时回滚副作用。

### 补偿规则

1. **声明即承诺**：原语声明效果时，必须同时声明其补偿操作
2. **补偿不强制成功**：补偿可能失败（如已发送的邮件无法撤回），此时记录补偿失败并标记需人工干预
3. **补偿顺序**：多个效果的补偿按声明顺序的逆序执行（LIFO）
4. **幂等补偿**：补偿操作本身必须是幂等的——多次执行不产生额外副作用

### 补偿示例

| 效果 / Effect | 补偿 / Compensation |
|---|---|
| `fs.write(path, content)` | `fs.restore(path, previous_content)` 或 `fs.delete(path)` |
| `network.send(email)` | 不可逆——标记为 `external.irreversible`，需审批 |
| `process.spawn(cmd)` | `process.terminate(pid)` |
| `model.inference(prompt)` | 不可逆——但无外部副作用，无需补偿 |

### 补偿与检查点

检查点记录已完成的效果及其补偿。任务失败时，运行时：
1. 定位到最后一个成功的检查点
2. 逆序执行该检查点之后所有已完成效果的补偿
3. 从检查点恢复或触发重新规划

## 外部副作用 / External side effects

示例：发送邮件、发布代码、删除数据、更改系统设置。这些应表示为显式的有效果操作（explicit effectful operations），并可能需要审批。不可逆的外部效果（如发送邮件）必须在执行前获得显式审批。
