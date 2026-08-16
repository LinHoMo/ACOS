# ACOS Mini MVP 规范 / ACOS Mini MVP Specification

- **状态 / Status**: stable
- **代码锚点 / Code anchor**: `tests/e2e/`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 目标 / Objective

证明 ACOS 可以将结构化的自然语言任务转换为已验证的执行图，持久地执行它，验证结果，并保留经验记录。

## 必需组件 / Required components

- Rust 运行时（Rust runtime）
- 任务规范解析器（Task Specification parser）
- 最小 CIR（minimal CIR）
- 能力解析器（capability resolver）
- 规则优先的规划器，带模型辅助回退（rule-first planner with model-assisted fallback）
- SQLite 状态/事件存储（SQLite state/event store）
- 验证流水线（verification pipeline）
- CLI
- 五个内置原语（five built-in primitives）
- 补偿机制（compensation mechanism for all declared effects）
- 插件热加载（plugin hot loading/unloading）
- Web MVP（执行图可视化 + 任务面板 + 事件日志）

## 必需演示 / Required demo

一个多输入数据/报告任务，带有条件处理和最终工件。

示例：

> 分析五个 CSV 文件。对每个文件，检测格式错误的列，必要时清洗它们，计算关键统计量，聚合结果，生成 markdown 报告，并在外部发送报告之前请求审批。

## 验收标准 / Acceptance criteria

- 编译器生成可检查的图（compiler generates an inspectable graph）
- 图在执行前被验证（graph is validated before execution）
- 运行可以在模拟失败后恢复（run can resume after simulated failure）
- 所有副作用都被声明（all side effects are declared）
- 每个声明的副作用都有补偿操作（every declared effect has a compensation）
- 插件可以在运行时热加载/卸载（plugins can be hot-loaded/unloaded at runtime）
- 最终报告有证据引用（final report has evidence references）
- 经验记录被发出（experience record is emitted）
- 基准测试结果可复现（benchmark results are reproducible）
- 任何模型可见的输入都能从事件日志重建（model-visible means logged invariant holds）
