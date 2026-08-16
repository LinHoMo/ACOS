# ADR-0001: ACOS 是用户空间认知运行时，而非主机操作系统

- 状态（Status）：已接受（Accepted）
- 日期（Date）：2026-08-16
- 决策者（Deciders）：ACOS 架构委员会

## 背景 / Context

ACOS 早期版本（v1.0）使用"人工认知操作系统（Artificial Cognitive Operating System）"的命名和定位，暗示它是一个类似 Linux 的操作系统内核。这导致了几个问题：

1. **范围膨胀**：试图定义内存管理、进程调度、设备驱动等操作系统级概念，但这些已由主机操作系统解决。
2. **工程误导**：开发者可能期望 ACOS 包含内核级代码，而实际上它是一个用户态运行时。
3. **与现有系统的关系不清**：ACOS 与 Windows/Linux/macOS 的关系没有明确定义。

## 决策 / Decision

ACOS 被重新定位为**用户空间认知运行时（user-space cognitive runtime）**，运行在主机操作系统之上，类似于 JVM 之于 Java 应用、ROS 之于机器人应用。

具体含义：

- ACOS 不替代主机操作系统内核。
- ACOS 不管理硬件资源（CPU、内存、GPU 的底层调度由主机 OS 和容器运行时负责）。
- ACOS 管理的是**认知资源**：原语调度、能力解析、执行状态、证据、验证和经验优化。
- ACOS 的安装和运行方式与普通用户级应用相同（Windows Service / systemd / launchd）。

## 理由 / Rationale

1. **聚焦核心价值**：ACOS 的差异化在于"认知程序的编译与执行"，而不是"重新发明操作系统"。
2. **工程可行性**：用户空间运行时可以用 Rust 实现，跨平台分发，不需要内核级开发。
3. **安全性**：用户空间运行时的安全边界更清晰，不需要 root/administrator 权限。
4. **类比清晰**：JVM（Java 虚拟机）和 ROS（机器人操作系统）都是成功的用户空间运行时先例。虽然 ROS 名字里有"Operating System"，但它实际上运行在 Linux 之上。

## 后果 / Consequences

### 正面（Positive）

- 架构边界清晰：ACOS 负责什么、不负责什么有明确划分。
- 开发复杂度降低：不需要处理内核态/用户态切换、驱动开发等问题。
- 跨平台更容易：用户空间代码可以在 Windows/Linux/macOS 上统一实现。
- 安全模型更简单：基于主机 OS 的进程隔离和权限系统。

### 负面（Negative）

- 命名"ACOS（Artificial Cognitive Operating System）"与实际定位有张力。保留"OS"是为了品牌连续性，但文档中始终强调"cognitive runtime"的实际定位。
- 性能上限受限于主机 OS 的进程间通信和调度机制。但认知任务的延迟在秒级，这不是瓶颈。

## 参考 / References

- [项目概述 / Project Overview](project_overview.md)
- [架构 / Architecture](architecture.md)
- [设计原则 / Design Principles](design_principles.md)
