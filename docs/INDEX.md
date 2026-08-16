# ACOS 文档索引 / Documentation Index

这是权威开发文档树（canonical development documentation tree）。

**单一事实源原则 / Single source of truth**：架构真理的权威来源是 `docs/`。每个规范（spec）必须有对应的代码或测试**锚点（anchor）**，否则视为"未落地设计"。

## 文档分类 / Documentation taxonomy

| 类别 / Category | 读者 / Audience | 用途 / Purpose | 位置 / Location |
|---|---|---|---|
| **Specs（规范）** | 实现者、SDK 用户 | 定义"是什么"与契约 | `specs/` |
| **Guides（指南）** | 开发者、运维 | 教"怎么做" | `guides/` |
| **ADRs（架构决策记录）** | 维护者、未来自己 | 记录"为什么这样决定" | `adrs/` |
| **Internal（内部）** | 核心维护者 | 设计备忘、权衡、未决问题 | `internal/` |

## 交叉引用与状态约定 / Cross-reference & status conventions

- 每个 **spec** 顶部标注：状态（`draft` / `stable` / `deprecated`）、代码锚点、模式（schema）、上次验证日期。
- 每个 **guide** 标注：适用版本、前置阅读。
- 每个 **ADR** 标注：状态（`proposed` / `accepted` / `superseded`）、决策日期。

## 从这里开始 / Start here

1. [项目概述 / Project Overview](internal/project_overview.md)
2. [架构 / Architecture](internal/architecture.md)
3. [技术栈 / Tech Stack](internal/tech_stack.md)
4. [路线图 / Roadmap](guides/roadmap.md)
5. [全链路指导性建议 / Holistic Guidance](GUIDANCE.md)

---

## 规范 / Specs

契约与"是什么"的定义。

- [认知原语规范 v0.1 / Cognitive Primitive Specification v0.1](specs/cognitive_primitive_spec.md)
- [认知任务规范 v0.1 / Cognitive Task Specification v0.1](specs/task_spec.md)
- [认知中间表示（CIR）v0.1 / Cognitive Intermediate Representation (CIR) v0.1](specs/cir_spec.md)
- [ACOS Mini MVP 规范 / ACOS Mini MVP Specification](specs/mvp_spec.md)
- [运行时模型 / Runtime Model](specs/runtime_model.md)
- [执行模型 / Execution Model](specs/execution_model.md)
- [状态与事件模型 / State and Event Model](specs/state_and_event_model.md)
- [验证架构 / Verification Architecture](specs/verification.md)
- [经验系统 / Experience System](specs/experience_system.md)
- [插件系统 / Plugin System](specs/plugin_system.md)
- [Web UI / Web 用户界面](specs/web_ui.md)
- [安全模型 / Security Model](specs/security.md)

## 指南 / Guides

"怎么做"的操作手册。

- [开发指南 / Development Guide](guides/development_guide.md)
- [贡献指南 / Contribution Guide](guides/contribution.md)
- [测试策略 / Testing Strategy](guides/testing.md)
- [部署 / Deployment](guides/deployment.md)
- [运维与可观测性 / Operations and Observability](guides/operations.md)
- [API 与兼容性指南 / API and Compatibility Guidelines](guides/api_guidelines.md)
- [路线图 / Roadmap](guides/roadmap.md)
- [Windows 平台指南 / Windows Platform Guide](guides/platform_windows.md)
- [Linux 平台指南 / Linux Platform Guide](guides/platform_linux.md)
- [macOS 平台指南 / macOS Platform Guide](guides/platform_macos.md)

## 架构决策记录 / ADRs

- [ADR 索引 / ADR Index](adrs/adr_index.md)
- [ADR-0001: 用户空间认知运行时 / User-space Cognitive Runtime](adrs/adr-0001-user-space-runtime.md)
- [ADR-0002: 编译器/运行时分离 / Compiler-Runtime Split](adrs/adr-0002-compiler-runtime-split.md)
- [ADR-0003: Rust 核心运行时 / Rust as Core Runtime](adrs/adr-0003-rust-core.md)
- [ADR-0004: SQLite 作为 MVP 状态存储 / SQLite as MVP State Store](adrs/adr-0004-sqlite-state-store.md)
- [ADR-0005: Protobuf 线上格式与 JSON Manifest / Protobuf Wire & JSON Manifests](adrs/adr-0005-proto-wire-json-manifests.md)
- [ADR-0006: 原生进程优先的插件运行时 / Native-first Plugin Runtime](adrs/adr-0006-native-plugin-runtime.md)
- [ADR-0007: 经验反馈回路在 MVP 中剥离 / Experience Loop Deferred in MVP](adrs/adr-0007-experience-loop-deferred.md)

## 内部设计备忘 / Internal

设计权衡、对比、实现细节。

- [架构 / Architecture](internal/architecture.md)
- [编译器设计 / Compiler Design](internal/compiler_design.md)
- [设计原则 / Design Principles](internal/design_principles.md)
- [技术栈 / Tech Stack](internal/tech_stack.md)
- [仓库结构 / Repository Structure](internal/repository_structure.md)
- [项目概述 / Project Overview](internal/project_overview.md)
- [术语表 / Glossary](internal/glossary.md)
- [相关系统对比 / Comparison with Related Systems](internal/comparison.md)
- [项目优化评估报告 / Project Optimization Report](internal/PROJECT_OPTIMIZATION_REPORT.md)
