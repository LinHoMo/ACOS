# ADR-0006: 原生进程优先、WASM 延后的插件运行时 / Native-first Plugin Runtime, WASM Deferred

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 的认知原语（Cognitive Primitive）需要由插件/提供者（provider）实现并提供。插件运行时的候选方案包括：原生进程（native process）、WASM 沙盒、容器、远程服务。

## 决策 / Decision

MVP 阶段使用**原生进程**作为插件运行时的默认实现，**WASM 推迟到插件契约被证明稳定之后**（Phase 4）。

具体含义：

- 每个原语提供者作为独立进程运行，通过 protobuf IPC（gRPC over local transport）与 ACOS 运行时通信。
- 插件清单（manifest）声明 capability、input/output schema、effects、supported platforms。
- 插件注册表支持热加载/卸载。
- WASM 作为未来的沙盒/插件目标，待原生进程验证契约稳定性后再引入。

## 理由 / Rationale

1. **易于调试**：原生进程可直接使用现有 AI 库（Python ML 生态、Node 工具链），开发体验更友好。
2. **契约先行**：先用原生进程验证 Primitive trait 与 CIR 契约，再引入 WASM 的额外复杂性。
3. **实用安全边界**：MVP 使用主机 OS 进程隔离 / 容器（当可用时），而非一开始构建 WASM 沙盒。
4. **与路线图一致**：`roadmap.md` 明确"WASM 插件沙盒作为默认"在 Deferred 列表。

## 后果 / Consequences

### 正面 / Positive

- 插件开发者可用任意语言实现，降低贡献门槛。
- 契约稳定后再引入 WASM，避免在接口频繁变动时维护两套沙盒。

### 负面 / Negative

- 原生进程的隔离性弱于 WASM 沙盒（通过主机 OS 进程隔离 + 策略缓解）。
- 未来需要从原生进程迁移到 WASM 的路径（通过稳定的 Primitive trait 降低迁移成本）。

## 参考 / References

- [插件系统 / Plugin System](specs/plugin_system.md)
- [认知原语规范 / Cognitive Primitive Specification](specs/cognitive_primitive_spec.md)
- [安全模型 / Security Model](specs/security.md)
- [路线图 / Roadmap](guides/roadmap.md)
