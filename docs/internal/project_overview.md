# 项目概述 / Project Overview

## 使命 / Mission

构建一个具备认知编译层的插件化认知运行时（pluginized cognitive runtime with a compilation layer），使人类目标能够被编译为可执行、可检查、可验证的认知程序（cognitive programs），并对程序进行可靠执行、验证与演化。

ACOS **不是**一个 Multi-Agent Framework。Agent 只是某个 Cognitive Program 在运行时形成的动态执行实体。ACOS 的核心抽象是 **Cognitive Program（认知程序）**。

## 三支柱 / Three Pillars

| 支柱 / Pillar | 说明 / Description |
|---|---|
| **Pluginized Cognitive Runtime** | 稳定的核心运行时 + 一切可替换能力（LLM Provider、Memory、Search、Browser、Code Runtime、Planner、Verifier、Knowledge Store、Environment Adapter）皆以标准化插件接入 |
| **Cognitive Compilation** | 将人类目标编译为可执行认知程序（Task Specification → Compiler → CIR → Cognitive Program） |
| **Reliable Cognitive Execution** | 状态可追踪、执行可恢复、副作用可管理、结果可验证、过程可回放、失败可重新规划、证据可追溯 |

## 问题陈述 / Problem statement

当前的 Agent 系统通常将 LLM 绑定到提示词（prompts）、工具（tools）和手写工作流（hand-authored workflows）上。这使得系统难以泛化、难以检查，且重新配置成本高昂。

现有系统的核心抽象是 **Agent**——一个由 LLM 驱动的、通过 Tool Call 与环境交互的执行实体。这种范式的问题在于：

1. **不可预测**：LLM 每次决策路径可能不同，执行图不是预先确定的
2. **不可编译**：目标不能编译为可检查的程序，只能在运行时"试错"
3. **不可验证**：结果没有形式化的验证保证，依赖 LLM 自我检查
4. **不可复用**：执行经验不能系统性地改善未来的规划

ACOS 将该问题视为**编译与运行时问题**（compilation and runtime problem）：

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

## 核心抽象 / Core abstractions

### 认知程序 / Cognitive Program  ← **一等公民 / First-Class Citizen**
目标、控制流、数据流、效果（effects）和验证要求的可执行表示。**ACOS 的核心抽象**。

### 认知编译器 / Cognitive Compiler  ← **差异化核心 / Differentiation Core**
将任务规范翻译为可执行认知程序的子系统。任务分析、能力解析、规划、CIR 生成、图验证、优化。

### 认知运行时 / Cognitive Runtime  ← **可靠执行引擎 / Reliable Execution Engine**
调度原语、持久化状态、处理失败并发出事件的执行引擎。保障：状态可追踪、执行可恢复、副作用可管理、过程可回放。

### 认知原语 / Cognitive Primitive
原子的、有类型的、可独立测试的认知操作（atomic, typed, independently testable cognitive operation）。以插件形式接入。

### Agent / Agent  ← **运行时概念 / Runtime Concept**
某个 Cognitive Program 在运行时形成的动态执行实体。**不是** ACOS 的一等公民。工作器进程可以动态创建和销毁，持久状态不得仅存在于 Agent 进程中。

### 世界模型 / World Model
任务状态、知识、证据和工件（artifacts）的当前与历史表示。

## 主要用例 / Primary use cases

- 编码与软件维护（coding and software maintenance）
- 研究与证据综合（research and evidence synthesis）
- 数据分析与报告（data analysis and reporting）
- 带验证的文档生成（document generation with verification）
- 本地环境中的受控自动化（controlled automation across local environments）
- 可恢复执行的长期项目（long-running projects with resumable execution）

## 成功标准 / Success criteria

ACOS 应在动态编译重要的任务上，相对于强基线展示可衡量的优势：

- 在变化的约束下更高的任务完成率（higher task completion rate under changing constraints）；
- 比手写工作流更低的编写工作量（lower authoring effort than hand-built workflows）；
- 可复现的执行追踪（reproducible execution traces）；
- 失败后的可恢复性（recoverability after failure）；
- 重要输出附带显式证据（explicit evidence attached to important outputs）；
- 无需重写编译器或运行时即可替换插件（plugin replacement without rewriting the compiler or runtime）。

## 架构边界 / Architecture boundary

ACOS 是用户空间运行时（user-space runtime）。硬件资源隔离仍由主机操作系统和容器/运行时层负责。ACOS 负责认知调度（cognitive scheduling）、能力解析（capability resolution）、执行状态（execution state）、证据（evidence）、验证（verification）和经验优化（experience optimization）。
