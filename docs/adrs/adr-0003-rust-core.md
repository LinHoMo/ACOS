# ADR-0003: Rust 作为核心运行时语言 / Rust as Core Runtime

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 核心运行时需要：内存安全、高性能、跨平台分发能力、成熟的 FFI 与工具链。候选语言包括 Rust、Go、C++、Zig。

## 决策 / Decision

核心运行时（`acos-core`、`acos-runtime`、`acos-state`、`acos-compiler`、`acos-verify`、`acos-plugin`、`acos-cli`）使用 **Rust** 实现。

具体含义：

- 核心 crate 全部位于 `crates/` 目录，以 Cargo workspace 组织。
- SDK 使用其他语言（TypeScript、Python），通过 gRPC/FFI 与核心运行时通信。
- 插件提供者（primitive providers）在 MVP 阶段以原生进程运行，可用任意语言实现，通过 protobuf IPC 与运行时交互。

## 理由 / Rationale

1. **内存安全**：ACOS 处理不可信任务与外部输入，Rust 的所有权模型在不牺牲性能的前提下消除整类内存漏洞。
2. **跨平台分发**：Rust 的交叉编译与静态链接支持 Windows/Linux/macOS 的单一工具链分发。
3. **性能**：认知运行时涉及大量图遍历、状态投影、事件序列化，Rust 的零成本抽象适合此场景。
4. **工具链成熟**：Cargo、clippy、rustfmt、docs.rs 提供开箱即用的工程化体验。
5. **与架构不变量一致**：Rust 的强类型与错误处理机制天然支持"一切执行结果可验证"。

## 后果 / Consequences

### 正面 / Positive

- 单一核心语言降低维护负担，对独立开发者友好。
- 生态成熟：tokio（异步）、serde（序列化）、tonic（gRPC）、rusqlite 等关键库可直接使用。

### 负面 / Negative

- Rust 学习曲线对贡献者有一定门槛（通过良好的文档与模板缓解，见 `guides/contribution.md`）。
- 编译时间较长（通过 workspace 缓存与增量编译缓解）。

## 参考 / References

- [技术栈 / Tech Stack](internal/tech_stack.md)
- [仓库结构 / Repository Structure](internal/repository_structure.md)
