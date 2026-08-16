# ADR-0004: SQLite 作为 MVP 状态与事件存储 / SQLite as MVP State and Event Store

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 需要持久化状态存储，承载事件日志（event log）、物化世界状态（materialized world state）、工件元数据（artifact metadata）、证据（evidence）和经验（experience）。候选方案包括 SQLite、PostgreSQL、嵌入式 KV（RocksDB/Sled）、以及未来可能的 EventStore/Kafka。

## 决策 / Decision

MVP 阶段使用 **SQLite** 作为状态与事件存储的默认实现，并通过 `EventStore` / `WorldState` 等 trait 抽象，保留未来替换为更强后端的可能性。

具体含义：

- 事件日志以追加导向（append-only）的 SQLite 表实现，作为可恢复执行的事实来源。
- 物化状态是事件历史的确定性投影，可重建。
- 搜索/知识使用 SQLite FTS，可选向量索引。
- 所有状态访问通过 `acos-state` crate 的 trait 暴露，不直接依赖 SQLite API。

## 理由 / Rationale

1. **单文件、可移植、事务性**：SQLite 无需独立服务进程，适合本地优先（local-first）的 MVP 目标。
2. **避免过早基础设施**：MVP 阶段引入分布式事件系统会显著增加复杂度与运维负担。
3. **可替换性**：通过 trait 抽象，Phase 2+ 可替换为 PostgreSQL/EventStore 而无需改动上层逻辑。
4. **与架构不变量一致**：SQLite 的 ACID 事务支持"状态可追踪、执行可恢复"。

## 后果 / Consequences

### 正面 / Positive

- 零配置、单文件部署，独立开发者开箱即用。
- 成熟的 Rust 绑定（`rusqlite` / `sqlx`）。

### 负面 / Negative

- SQLite 在极高并发写入或分布式场景下存在上限（通过 trait 抽象 + Phase 2 替换缓解）。
- 事件日志增长需要归档策略（Phase 2 考虑）。

## 参考 / References

- [技术栈 / Tech Stack](internal/tech_stack.md)
- [状态与事件模型 / State and Event Model](specs/state_and_event_model.md)
