# 相关系统对比 / Comparison with Related Systems

## 目的 / Purpose

本文档将 ACOS 与现有相关系统进行对比，明确 ACOS 的差异化定位和技术选择依据。

## 对比维度 / Comparison dimensions

| 维度 | 说明 |
|---|---|
| 编排模型 | 静态工作流 vs 动态编译 |
| 能力抽象 | 工具函数 vs 类型化原语 |
| 状态管理 | 无状态 vs 事件溯源持久状态 |
| 验证机制 | 无 vs 多层验证 |
| 经验复用 | 无 vs 结构化经验记录 |
| 插件模型 | 包管理 vs 能力注册表 |

## 系统对比 / System comparison

### 1. LangChain / LangGraph

**定位**：LLM 应用开发框架，提供链式调用和图编排。

| 维度 | LangChain/LangGraph | ACOS |
|---|---|---|
| 编排模型 | 开发者手写 Chain/Graph | 编译器从任务规范动态生成 |
| 能力抽象 | Tool（函数调用） | Cognitive Primitive（类型化+效果+元数据） |
| 状态管理 | 有限的检查点 | 事件溯源 + 物化状态 + 可重放 |
| 验证 | 无内置机制 | 五层验证（证据/契约/执行/策略/语义审查） |
| 经验复用 | 无 | Experience Record 驱动优化 |

**关键差异**：LangGraph 要求开发者预先定义图结构；ACOS 的图是编译产物。LangChain 的 Tool 是无类型的函数；ACOS 的 Primitive 有强类型契约和效果声明。

### 2. AutoGen / CrewAI

**定位**：多 Agent 协作框架，通过角色定义和对话驱动任务。

| 维度 | AutoGen/CrewAI | ACOS |
|---|---|---|
| Agent 模型 | 固定角色（Researcher/Coder/Reviewer） | 临时执行实例，从认知程序生成 |
| 协作方式 | Agent 间对话 | 数据依赖+控制依赖的执行图 |
| 可预测性 | 低（对话路径不确定） | 高（图结构可检查、可验证） |
| 状态 | Agent 内部上下文 | 全局持久状态 + 事件日志 |

**关键差异**：多 Agent 框架的核心是"让多个 LLM 聊天"；ACOS 的核心是"编译出一个确定性的执行图，LLM 只在提议层使用"。

### 3. Temporal / Airflow / Dagster

**定位**：通用工作流引擎，专注于持久执行和调度。

| 维度 | Temporal/Airflow | ACOS |
|---|---|---|
| 工作流来源 | 开发者手写代码 | 编译器从自然语言+结构化约束生成 |
| 活动抽象 | Activity/Operator（任意代码） | Cognitive Primitive（类型化+效果+能力描述） |
| 能力发现 | 无 | 能力注册表 + 三级匹配（Exact/Ontology/LLM） |
| 验证 | 无（工作流正确性由开发者保证） | 编译时静态验证 + 运行时多层验证 |
| 优化 | 无 | 基于经验的成本/延迟/可靠性优化 |

**关键差异**：Temporal 解决"如何可靠地执行一个已定义的工作流"；ACOS 解决"如何从目标生成一个工作流，然后可靠地执行它"。ACOS 的价值在生成层，执行层可以借鉴 Temporal 的持久执行机制。

### 4. ROS (Robot Operating System)

**定位**：机器人中间件，提供节点通信、参数服务器、包管理。

| 维度 | ROS | ACOS |
|---|---|---|
| 节点模型 | Node（发布/订阅主题） | Cognitive Primitive（类型化调用+效果） |
| 通信 | DDS（发布/订阅） | gRPC（请求/响应）+ 事件总线 |
| 任务编排 | launch 文件（静态） | Cognitive Compiler（动态） |
| 包管理 | catkin/colcon + ROS Index | ACOS Plugin Registry + 能力匹配 |

**关键差异**：ROS 的节点是长期运行的进程，通过主题通信；ACOS 的原语是按需调度的操作，通过执行图组织。ROS 没有"编译器"概念——launch 文件是手写的。

### 5. MCP (Model Context Protocol)

**定位**：标准化 LLM 与外部工具/数据源的连接协议。

| 维度 | MCP | ACOS |
|---|---|---|
| 抽象层级 | 工具/资源/提示词的传输协议 | 完整的编译+运行时+验证+经验系统 |
| 能力描述 | Tool 定义（名称+参数+描述） | Primitive Manifest（能力+类型+效果+元数据+提供者） |
| 编排 | 无（由 LLM 客户端决定调用顺序） | Cognitive Compiler 生成执行图 |
| 状态 | 无 | 事件溯源持久状态 |

**关键差异**：MCP 是一个连接协议，解决"LLM 怎么调用工具"；ACOS 是一个运行时，解决"怎么把目标变成工具调用序列并可靠执行"。ACOS 可以将 MCP 服务器作为 Primitive Provider 接入。

### 6. Kubernetes

**定位**：容器编排平台，管理分布式应用的部署和伸缩。

| 维度 | Kubernetes | ACOS |
|---|---|---|
| 调度对象 | Pod/Container（计算资源） | Cognitive Primitive（认知操作） |
| 声明方式 | YAML 清单（期望状态） | Task Specification（目标+约束） |
| 控制器 | 控制循环（调和期望状态与实际状态） | Cognitive Compiler（生成执行图）+ Runtime（执行） |
| 状态 | etcd（分布式键值存储） | SQLite + 事件日志（本地优先） |

**关键差异**：K8s 编排的是"已经写好的容器"；ACOS 编排的是"需要被编译出来的认知程序"。K8s 的声明式是"我要 3 个副本"，ACOS 的声明式是"我要完成这个目标"。

## ACOS 的独特定位 / ACOS unique positioning

ACOS 不是上述任何一个系统的替代品，而是填补了一个空白：

> **从高层目标到可执行、可验证、可优化的认知程序的编译层。**

- 比 LangChain 更有结构（编译时验证 vs 运行时试错）
- 比 AutoGen 更可预测（确定性图 vs 对话式协作）
- 比 Temporal 更抽象（目标驱动 vs 工作流驱动）
- 比 ROS 更动态（编译生成 vs 静态 launch）
- 比 MCP 更完整（运行时+验证+经验 vs 仅连接协议）

## 可以借鉴的设计 / Designs to borrow

| 来源 | 借鉴点 |
|---|---|
| Temporal | 持久执行、重试、检查点、补偿事务 |
| ROS | 节点生命周期、能力发现、包管理 |
| Kubernetes | 声明式 API、控制器模式、健康检查 |
| LLVM | 中间表示设计、优化 Pass、目标后端 |
| Git | 事件溯源、内容寻址、分支与合并 |
| Nix | 声明式包管理、可复现构建、沙盒 |
| DeepSeek Harness (DSH) | 一切皆是插件架构、Profile/Bundle/Patch 分层组合、热模块替换 |
| Cordis（DSH 底层框架） | 可逆效果（reversible effects）、响应式协效果（reactive coeffects）、能力接缝三角色模型、"模型可见即已记录"不变量 |

## 不应做的 / What not to do

- 不要重新实现 Temporal 的分布式执行引擎——MVP 用 SQLite + 单进程即可。
- 不要重新实现 Kubernetes 的调度器——认知调度比容器调度简单得多。
- 不要与 MCP 竞争——将 MCP 作为 Provider 接入，而不是替代它。
- 不要追求 ROS 级别的实时通信——认知任务的延迟在秒级，不是毫秒级。
- 不要重新实现 Cordis 的运行时——借鉴其设计概念，但 ACOS 有自己的运行时和编译层。
- 不要试图替代 DSH——ACOS 的核心差异在编译层、验证层和经验系统，不是插件架构本身。
