# ADR-0005: Protobuf 用于运行时 RPC，JSON 用于外部 Manifest / Protobuf for Runtime RPC, JSON for External Manifests

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 需要两类序列化：（1）运行时内部 RPC（编译器↔运行时↔状态↔原语），要求高效、类型化、语言中立；（2）用户/SDK 可见的 manifest（任务规范、原语清单、CIR 导出），要求人类可读、易于调试。

## 决策 / Decision

采用**双轨序列化**：

- **运行时 RPC** 使用 **Protobuf**（通过 gRPC/tonic）。模式定义在 `schemas/*.proto`。
- **外部 manifest** 使用 **JSON**，并辅以 **JSON Schema** 验证。

具体含义：

- `schemas/` 目录按 `task/`、`primitive/`、`cir/`、`events/` 分别存放 `.proto` 与 `.jsonschema`。
- Protobuf 是运行时内部契约的权威来源；JSON manifest 可视为 Protobuf 的人类可读投影。
- 代码生成（Rust + TypeScript + Python）从 proto 与 jsonschema 派生。

## 理由 / Rationale

1. **高效内部通信**：Protobuf 二进制紧凑、解析快、向前/向后兼容性好，适合高频运行时 RPC。
2. **人类可读配置**：JSON 让任务规范、原语清单对用户和 SDK 开发者友好，易于手工编写与调试。
3. **语言中立**：Protobuf 与 JSON 均有成熟的跨语言工具链，支撑 Rust 核心 + TS/Python SDK 的多语言架构。
4. **兼容性治理**：.proto 与 .jsonschema 文件纳入版本控制，变更需 ADR + 迁移说明。

## 后果 / Consequences

### 正面 / Positive

- 内部效率与外部可读性兼得。
- schema 文件成为"单一事实源"，驱动代码生成与兼容性检查。

### 负面 / Negative

- 需要维护 proto ↔ JSON 的双轨生成脚本（`scripts/gen_schema.*`）。
- schema 演进需遵循兼容性规则（见 `guides/api_guidelines.md`）。

## 参考 / References

- [技术栈 / Tech Stack](internal/tech_stack.md)
- [API 与兼容性指南 / API and Compatibility Guidelines](guides/api_guidelines.md)
- [任务规范 / Task Specification](specs/task_spec.md)
- [CIR 规范 / CIR Specification](specs/cir_spec.md)
