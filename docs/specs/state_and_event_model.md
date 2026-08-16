# 状态与事件模型 / State and Event Model

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-state/src/store.rs`（pending）
- **模式 / Schema**: `schemas/events/events.proto`
- **上次验证 / Last verified**: —

## 原则 / Principle

持久状态应可从追加导向的事件历史（append-oriented event history）加上确定性/物化投影（deterministic/materialized projections）重建。

## 核心不变量：模型可见即已记录 / Core invariant: model-visible means logged

任何到达模型请求的输入都必须能从事件日志重建。这是 ACOS 可复现性和可审计性的基础保证。

### 含义

- 影响模型决策的所有信息必须通过事件日志传递
- CIR 的组装必须可追溯到具体事件
- 验证时能证明"模型在何时看到了什么"
- 新增强模型可见的输入 = 新增一个事件类型

### 理由

如果不遵守此不变量：
- 无法复现过去的运行（模型看到的上下文丢失）
- 无法审计模型决策（无法追溯决策依据）
- 验证无法完整覆盖（存在未记录的输入）

### 例外

以下信息不需要通过事件日志：
- 纯计算中间结果（不进入模型上下文）
- 运行时内部调度状态（与模型决策无关）
- 缓存命中信息（可由缓存策略重建）

## 事件信封 / Event envelope

```json
{
  "event_id": "uuid",
  "event_type": "TaskStarted",
  "task_id": "uuid",
  "run_id": "uuid",
  "timestamp": "2026-01-01T00:00:00Z",
  "schema_version": 1,
  "producer": "acos-runtime",
  "payload": {}
}
```

## 状态投影 / State projections

至少：

- 任务状态（task state）
- 执行图状态（execution graph state）
- 工件索引（artifact index）
- 证据索引（evidence index）
- 资源/预算状态（resource/budget state）

## 并发 / Concurrency

初始系统应在物化状态周围使用乐观版本检查（optimistic version checks）。当变更冲突重要时，命令必须指定其观察到的版本。

## 重放 / Replay

相同的事件流应能重建任务执行状态。如果精确的逐字节重放不可能，提供者的非确定性（provider nondeterminism）必须记录在输出/元数据中。

## 保留 / Retention

并非每个内部思考轨迹（internal thought trace）都需要持久存储。默认保留任务输入、输出、决策、工件、验证结果和紧凑经验记录。敏感轨迹的保留由策略控制。
