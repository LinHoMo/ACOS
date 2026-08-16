# 运行时模型 / Runtime Model

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-runtime/src/lib.rs`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 进程模型 / Process model

提交的认知程序成为运行时执行实例（runtime execution instance）：

```text
Submitted（已提交）
  ↓
Compiled（已编译）
  ↓
Validated（已验证）
  ↓
Scheduled（已调度）
  ↓
Running（运行中）
  ↓
Succeeded | Failed | Cancelled | Paused（成功 | 失败 | 已取消 | 已暂停）
  ↓
Finalized（已终结）
  ↓
Experience emitted（经验已发出）
```

## 工作器 / Workers

工作器（worker）是能够运行一个或多个原语的实现进程/服务（implementation process/service）。多个提供者可以实现同一个能力。

## 调度器职责 / Scheduler responsibilities

- 依赖就绪（dependency readiness）
- 并发限制（concurrency limits）
- 预算执行（budget enforcement）
- 优先级处理（priority handling）
- 重试（retries）
- 检查点边界（checkpoint boundaries）
- 取消（cancellation）

## 运行时隔离 / Runtime isolation

运行时不应假设每个原语都是可信的。有风险的效果必须跨越显式边界（explicit boundary）。

## 临时 Agent / Ephemeral agents

Agent 主要是围绕认知程序的运行范围身份（run-scoped identity）。工作器进程可以动态创建和销毁。持久状态不得仅存在于 Agent 进程中。
