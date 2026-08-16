# ADR 索引 / ADR Index

架构决策记录（Architecture Decision Records）记录影响接口、执行语义、安全边界、持久化或兼容性保证的重大决策。

**规则 / Rule**：任何更改公共模式、引入不可逆约束、或影响跨层兼容性的决策，必须创建或更新 ADR。

## 状态约定 / Status conventions

- `proposed` — 已提议，待接受
- `accepted` — 已接受
- `superseded` — 已被后续 ADR 取代（需注明取代者）

## 索引 / Index

| ID | 标题 / Title | 状态 / Status | 日期 / Date |
|---|---|---|---|
| ADR-0001 | [用户空间认知运行时 / User-space Cognitive Runtime](adr-0001-user-space-runtime.md) | accepted | 2026-08-16 |
| ADR-0002 | [编译器/运行时分离 / Compiler-Runtime Split](adr-0002-compiler-runtime-split.md) | accepted | 2026-08-16 |
| ADR-0003 | [Rust 核心运行时 / Rust as Core Runtime](adr-0003-rust-core.md) | accepted | 2026-08-16 |
| ADR-0004 | [SQLite 作为 MVP 状态存储 / SQLite as MVP State Store](adr-0004-sqlite-state-store.md) | accepted | 2026-08-16 |
| ADR-0005 | [Protobuf 线上格式与 JSON Manifest / Protobuf Wire & JSON Manifests](adr-0005-proto-wire-json-manifests.md) | accepted | 2026-08-16 |
| ADR-0006 | [原生进程优先的插件运行时 / Native-first Plugin Runtime](adr-0006-native-plugin-runtime.md) | accepted | 2026-08-16 |
| ADR-0007 | [经验反馈回路在 MVP 中剥离 / Experience Loop Deferred in MVP](adr-0007-experience-loop-deferred.md) | accepted | 2026-08-16 |
