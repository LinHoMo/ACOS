# 路线图 / Roadmap

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 全部
- **前置阅读 / Prspecs**: [项目概述](../internal/project_overview.md)

## Phase 0 — 契约基础 / Contract foundation

- 原语规范 v0.1（Primitive Specification v0.1）
- 任务规范 v0.1（Task Specification v0.1）
- 模式工具（schema tooling）
- 五个 MVP 原语（five MVP primitives）
- 能力接缝三角色模型（Capability Seam three-role model）
- "模型可见即已记录"不变量（model-visible means logged invariant）

## Phase 1 — ACOS Mini

- 本地运行时（local runtime）
- 编译器原型（compiler prototype）
- CIR 验证（CIR validation）
- 执行图（execution graph）
- SQLite/事件日志（SQLite/event log）
- 一个基准测试套件（one benchmark suite）
- 补偿机制（compensation mechanism for effects）
- 基础插件注册表 + 热加载（basic plugin registry + hot loading）
- Web MVP（执行图可视化 + 任务面板 + 事件日志）

## Phase 2 — 可靠性 / Reliability

- 持久执行（durable execution）
- 重试/检查点（retries/checkpoints）
- 验证流水线（verification pipeline）
- 增强插件注册表（签名验证、依赖管理）（enhanced plugin registry）
- 提供者故障转移（provider failover）
- Profile/Bundle 分层组合（Profile/Bundle composition model）

## Phase 3 — 经验优化 / Experience optimization

- 经验存储（experience store）
- 能力排名（capability ranking）
- 历史图模板（historical graph templates）
- 成本/延迟估算（cost/latency estimation）

## Phase 4 — 生态系统 / Ecosystem

- 签名插件（signed plugins）
- SDK 稳定化（SDK stabilization）
- 跨平台安装器（cross-platform installers）
- 仓库格式（repository format）
- 兼容性工具（compatibility tooling）

## 推迟 / Deferred

- WASM 插件沙盒作为默认（WASM plugin sandbox as a default）
- 分布式多主机运行时（distributed multi-host runtime）
- 自主自修改（autonomous self-modification）
- 大型市场（large marketplace）
