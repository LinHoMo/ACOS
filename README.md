# ACOS

人工认知编排系统（Artificial Cognitive Orchestration System）/ 认知运行时（Cognitive Runtime）

> **ACOS is a pluginized cognitive runtime with a compilation layer: goals are compiled into cognitive programs, and cognitive programs are reliably executed, verified, and evolved.**
>
> **ACOS 是一个具备认知编译层的插件化认知运行时：将目标编译为认知程序，并对程序进行可靠执行、验证与演化。**

## 核心定义 / Core Definition

```text
ACOS = Pluginized Cognitive Runtime
     + Cognitive Compilation
     + Reliable Cognitive Execution
```

**三句传播 / Three guiding principles：**

> **一切可替换能力皆可插件化；一切复杂任务皆可程序化；一切执行结果都必须可验证。**

## 状态 / Status

- 版本（Version）：0.1 架构基线（architecture baseline）
- 阶段（Stage）：M0 脚手架已完成，进入 ACOS Mini 实现
- 主要验证目标（Primary validation target）：ACOS Mini
- 架构事实的权威来源（Canonical source of architectural truth）：`docs/`
- M0 已完成：仓库初始化、`acos-core` 接口骨架、Protobuf schema、CI 配置、首个 e2e 测试骨架

## 快速开始 / Quick start

> **前置条件 / Prerequisite**：安装 Rust 工具链。推荐通过 [rustup](https://rustup.rs/) 安装：
> ```bash
> curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
> ```
> 安装后验证：`cargo --version` 与 `rustc --version`。

```bash
# 构建整个 workspace（含 acos-core）
cargo build --workspace

# 运行 acos-core 测试（含类型 roundtrip 测试）
cargo test -p acos-core
```

完成 M1 后，`cargo build --workspace` 将覆盖全部 7 个核心 crate。开发顺序与并行策略见 [GUIDANCE.md](docs/GUIDANCE.md)。

## ACOS 是什么 / What ACOS is

ACOS **不是** Windows、Linux 或 macOS 的替代品。它运行在用户空间（user space），为目标驱动的认知计算（goal-driven cognitive computation）提供运行时。

ACOS **不是**一个 Multi-Agent Framework。它的核心抽象是 **Cognitive Program（认知程序）**，Agent 只是某个 Cognitive Program 在运行时形成的动态执行实体。

概念栈（Conceptual stack）：

```text
Hardware（硬件）
  ↓
Windows / Linux / macOS
  ↓
ACOS Runtime（ACOS 运行时）
  ↓
Cognitive Compiler（认知编译器）
  ↓
Cognitive Program（认知程序）  ← 一等公民 / First-class citizen
  ↓
Cognitive Primitives（认知原语 / 插件化能力）
  ↓
External World（外部世界）
```

## 核心理念 / Core idea

```text
Human Intent（人类意图）
  ↓
Task Specification（任务规范）
  ↓
Cognitive Compiler（认知编译器）
  ↓
Cognitive IR (CIR)（认知中间表示）
  ↓
Cognitive Program（认知程序）
  ↓
Runtime（运行时）
  ↓
Verification + Evidence（验证 + 证据）
  ↓
Experience Record（经验记录）
  ↓
Future Compilation（反馈优化未来编译）
```

## MVP / 最小可行产品

ACOS Mini 应当证明：自然语言目标加上结构化约束，可以通过一个小型原语集（primitive set）被编译为可靠的可执行图。

- `search`（搜索）
- `read_file`（读文件）
- `write_file`（写文件）
- `execute_python`（执行 Python）
- `summarize`（总结）

MVP 基准测试必须至少包含一个条件密集型任务（condition-heavy task），而不仅仅是简单的线性 CSV 任务。

## 文档导航 / Documentation map

从这里开始：

1. [项目概述 / Project Overview](docs/project_overview.md)
2. [架构 / Architecture](docs/architecture.md)
3. [技术栈 / Tech Stack](docs/tech_stack.md)
4. [运行时模型 / Runtime Model](docs/runtime_model.md)
5. [认知原语规范 / Cognitive Primitive Specification](docs/cognitive_primitive_spec.md)
6. [任务规范 / Task Specification](docs/task_spec.md)
7. [CIR 规范 / CIR Specification](docs/cir_spec.md)
8. [编译器设计 / Compiler Design](docs/compiler_design.md)
9. [执行模型 / Execution Model](docs/execution_model.md)
10. [状态与事件模型 / State and Event Model](docs/state_and_event_model.md)
11. [验证 / Verification](docs/verification.md)
12. [经验系统 / Experience System](docs/experience_system.md)
13. [插件系统 / Plugin System](docs/plugin_system.md)
14. [Web UI / Web 用户界面](docs/web_ui.md)
15. [安全 / Security](docs/security.md)
16. [部署 / Deployment](docs/deployment.md)
17. [平台指南 / Platform Guides](docs/platform_windows.md)、[Linux](docs/platform_linux.md)、[macOS](docs/platform_macos.md)
18. [开发指南 / Development Guide](docs/development_guide.md)
19. [测试策略 / Testing Strategy](docs/testing.md)
20. [路线图 / Roadmap](docs/roadmap.md)
21. [ADR 索引 / ADR Index](docs/adr_index.md)

## 设计原则 / Design principles

完整设计原则见 [设计原则 / Design Principles](docs/design_principles.md)。

### 三支柱 / Three Pillars

- **Stable Core + Everything Extensible**：稳定的核心运行时 + 一切可替换能力皆以标准化插件接入
- **Cognitive Program as First-Class Citizen**：认知程序是一等公民，Agent 只是程序的运行时执行实体
- **Reliable by Default**：状态可追踪、执行可恢复、副作用可管理、结果可验证、过程可回放、失败可重新规划、证据可追溯

## 首个版本的非目标 / Non-goals for the first release

- 替代主机操作系统内核（host OS kernel）。
- 构建通用的分布式云调度器（distributed cloud scheduler）。
- 解决 AGI（通用人工智能）。
- 完整的自修改代码（self-modifying code）。
- 在本地插件契约稳定之前建立插件市场（plugin marketplace）。

## 许可证 / License

待定（TBD）。公开发布前决定。
