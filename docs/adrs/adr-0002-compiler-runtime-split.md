# ADR-0002: 编译器/运行时分离 / Compiler-Runtime Split

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 的核心流程是"意图 → 任务规范 → 编译器 → 认知程序 → 运行时 → 原语"。早期设计未明确编译器与运行时是否应为独立边界，存在将编译逻辑与执行调度耦合的风险。

## 决策 / Decision

将编译器（Compiler）与运行时（Runtime）拆分为独立层次与独立 crate（`acos-compiler`、`acos-runtime`），通过 CIR（认知中间表示）作为两者之间的稳定契约。

具体含义：

- 编译器负责：任务分析、能力解析、规划、CIR 生成、图验证、优化。产物是 `CIR Program`。
- 运行时负责：调度、持久执行、重试、检查点、效果强制执行、进程/工作器管理、事件发出。输入是 `CIR Program`。
- CIR 是两者之间的唯一耦合面；编译器无需知道执行是本地还是分布式的，运行时无需知道程序是规则生成还是模型生成的。

## 理由 / Rationale

1. **关注点分离**：编译策略（规则优先 / 模型辅助）可以独立于执行语义演进。
2. **可替换性**：同一编译产物可在不同运行时实现上执行（本地进程、容器、未来分布式）。
3. **可测试性**：编译器可针对 CIR 产物做单元测试，无需启动运行时；运行时可用夹具 CIR 做集成测试。
4. **与架构不变量一致**：符合"机制与策略分离"——运行时是机制，编译策略是策略。

## 后果 / Consequences

### 正面 / Positive

- 编译器和运行时可由不同人以不同节奏开发。
- CIR 成为可独立版本化的契约层，驱动 schema 优先的开发方式。

### 负面 / Negative

- 需要维护 CIR 的模式演进与兼容性（通过 `schemas/cir.proto` + 版本号）。
- 两个 crate 之间的调试需要跨边界追踪（通过 `RunId` / `ProgramId` 关联）。

## 参考 / References

- [架构 / Architecture](internal/architecture.md)
- [编译器设计 / Compiler Design](internal/compiler_design.md)
- [CIR 规范 v0.1](specs/cir_spec.md)
- [运行时模型 / Runtime Model](specs/runtime_model.md)
