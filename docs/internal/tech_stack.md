# 技术栈 / Technology Stack

## 基线目标 / Baseline target

首个实现应优化为小型、可移植、可检查的运行时，而不是最大的分布式规模。

| 层次 / Layer | 默认 / Default | 理由 / Rationale |
|---|---|---|
| 核心运行时 / Core runtime | Rust | 内存安全、性能、跨平台分发、强大的 FFI/工具链。 |
| API/SDK | TypeScript + Python | 开发者人体工学和 AI 生态覆盖。 |
| IPC | gRPC over local transport for MVP | 类型化契约、成熟工具、语言中立。 |
| 序列化 / Serialization | JSON for external manifests; Protobuf for runtime RPC | 人类可读的配置加上高效的内部线上格式。 |
| 模式 / Schema | JSON Schema for manifests; Protobuf/IDL for RPC | 成熟的验证和兼容性工具。 |
| 状态存储 / State store | SQLite for MVP | 单文件、可移植、事务性。 |
| 事件日志 / Event log | SQLite tables initially; abstract interface for future EventStore/Kafka-like backends | 保持 MVP 简单，同时保留持久事件模型。 |
| 工件 / Artifacts | Host filesystem with content-addressed metadata | 易于本地检查和可复现性。 |
| 搜索/知识 / Search/knowledge | SQLite FTS + optional vector index in MVP | 避免过早的基础设施。 |
| 插件运行时 / Plugin runtime | Native process providers first; WASM later | 更易于调试和访问现有 AI 库。 |
| 进程隔离 / Process isolation | Host OS process sandbox / container when available | 实用的安全边界。 |
| LLM 提供者 / LLM providers | Provider adapters; no hard dependency on one vendor | 防止模型锁定。 |
| Web UI | React + TypeScript | 成熟的组件生态、执行图可视化支持。 |
| Web UI 通信 | REST API + WebSocket | 请求/响应 + 实时事件流。 |
| 构建 / Build | Cargo + npm/pnpm + uv/pip | 将系统运行时与开发者 SDK 分离。 |
| CI | GitHub Actions initially | 跨平台矩阵和发布自动化。 |

## 技术选择原则 / Technology selection principles

1. 除非 ACOS 真正需要新基础设施，否则使用成熟技术。
2. 首个版本优先使用本地优先组件（local-first components）。
3. 保持协议定义语言中立（language-neutral）。
4. 将 WASM 视为未来的沙盒/插件目标，而非先决条件。
5. 保持运行时独立于特定 LLM 供应商。
