# 更新日志 / Changelog

本文件记录 ACOS 项目文档的重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added / 新增

- `docs/comparison.md`：相关系统对比分析（LangChain、AutoGen、Temporal、ROS、MCP、Kubernetes）
- `docs/adr-0001-user-space-runtime.md`：首个完整 ADR 示例文档
- `CHANGELOG.md`：本文件

### P0 — 控制语义 + 失败恢复 + 基准（2026-08-17）

- **控制语义类型**：`CirNode.control`（`condition` / `loop_spec` / `retry`）与 `else_children`（`crates/acos-core`）
- **编译期校验**：`acos_compiler::validate_cir` 校验条件标识符、循环 `max_iterations >= 1`、重试 `max_attempts >= 1`、不可逆原语禁止重试
- **失败分类**：`FailureClass`（timeout / rate_limit / transient / invalid_input / permission_denied / syntax_error / unknown）
- **运行时控制执行**：conditional / loop_map（while / until / for_each）/ retry 策略（`crates/acos-runtime`）
- **恢复状态机**：`execute_with_recovery` + 事务式提交门（`MAX_RECOVERY_ATTEMPTS = 3`）
- **RuleReplanner**：`OfflineFallbackRule` 将暂态失败节点替换为本地 `read_file` 回退
- **ModelRecoveryPlanner**：LLM 生成 `RecoverySubgraph` 补丁（需 `LONGCAT_API_KEY`）
- **基准套件**：`crates/acos-bench`（condition / loop / retry / recovery / negative 五套 fixture）+ CLI `acos bench [--suite S] [--case C] [--require-model]`
- **CIR proto 同步**：`schemas/cir/cir.proto` 补齐 `control` / `else_children` 与 `ControlSpec` / `LoopSpec` / `RetryPolicy` 等消息

### Changed / 变更

- 全部 32 个 Markdown 文档完成双语化（中文为主体，英文术语标注）
  - 根目录：`README.md`、`PROJECT_STATUS.md`
  - `docs/`：全部 30 个文档
- 双语化规范：标题采用"中文 / English"格式，正文中文为主，关键术语首次出现时标注英文，代码块/YAML/JSON 保持原样

### Documentation / 文档

- 统一了术语翻译：Cognitive Primitive → 认知原语、Cognitive Runtime → 认知运行时、Cognitive IR → 认知中间表示、Effect → 效果、Evidence → 证据、Artifact → 工件、Experience Record → 经验记录

## [0.1.0] - 2026-08-16

### Added / 新增

- 初始文档集：32 个 Markdown 文档，覆盖架构、规范、运行时、编译器、平台、开发、测试、路线图等
- 核心规范：Cognitive Primitive Specification v0.1、Cognitive Task Specification v0.1、CIR v0.1（实验性）
- ADR 索引：10 个初始架构决策记录索引
