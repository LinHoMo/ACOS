# 更新日志 / Changelog

本文件记录 ACOS 项目文档的重要变更。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [语义化版本](https://semver.org/lang/zh-CN/)。

## [Unreleased]

### Added / 新增

- `docs/comparison.md`：相关系统对比分析（LangChain、AutoGen、Temporal、ROS、MCP、Kubernetes）
- `docs/adr-0001-user-space-runtime.md`：首个完整 ADR 示例文档
- `CHANGELOG.md`：本文件

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
