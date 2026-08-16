# 运维与可观测性 / Operations and Observability

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 运维
- **前置阅读 / Prspecs**: [状态与事件模型](../specs/state_and_event_model.md)

## 日志 / Logs

结构化日志应包括任务/运行/节点 ID、提供者身份和事件关联 ID（event correlation IDs）。

## 指标 / Metrics

最小运行时指标：

- tasks_started（任务已启动）
- tasks_succeeded（任务已成功）
- tasks_failed（任务已失败）
- compile_failures（编译失败）
- primitive_failures（原语失败）
- compensation_executions（补偿执行次数）
- compensation_failures（补偿失败次数——需人工干预）
- retries（重试）
- verification_failures（验证失败）
- plugin_loads（插件加载次数）
- plugin_unloads（插件卸载次数）
- active_runs（活跃运行）
- token/cost usage when available（可用时的 token/成本使用）
- latency（延迟）

## 追踪 / Tracing

运行时应暴露跨编译器、调度器、原语、验证和持久化层的执行跨度（execution spans）。

## 健康 / Health

每个提供者应支持健康检查，包含：

- readiness（就绪）
- compatibility（兼容性）
- dependency status（依赖状态）
- permission status（权限状态）
- version（版本）
