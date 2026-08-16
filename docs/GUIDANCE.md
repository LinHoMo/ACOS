# ACOS 全链路指导性建议 / Holistic Guidance for ACOS

- **适用版本 / Since**: v0.1
- **读者 / Audience**：项目维护者（独立开发者）
- **性质 / Nature**：策略性建议文档，非规范性 spec。完整版本与执行细节见计划文件 `mutable-swimming-cosmos.md`。

> **核心判断 / Core thesis**：ACOS 当前是"33 个设计文档、零代码"状态。最大风险是**设计-实现断层**。应以**可运行的 ACOS Mini** 为北极星，用代码反向验证与固化设计；文档与代码同仓库、同评审、同演进。设计不为自身服务——每一条规范都应当能被一段可执行代码或一个测试所验证。

---

## 0. 现状诊断 / Diagnosis

### 做得好 / Strengths
- 架构论点清晰：认知程序是一等公民，Agent 只是运行时实体。
- 六层划分（接口/编译器/运行时/原语/状态/主机集成）职责边界清楚。
- MVP 有明确验收标准；技术栈已选定；非目标明确。

### 关键风险 / Key Risks（按严重度排序）

| # | 风险 | 严重度 |
|---|---|---|
| R1 | 设计-实现断层（33 文档、0 代码，缺验证闭环） | 高 |
| R2 | 关键线上格式未固化（CIR、本体、WASM ABI） | 高 |
| R3 | 层间接口契约缺失（compiler↔runtime↔state↔primitive） | 高 |
| R4 | 脚手架与工具链空白 | 中 |
| R5 | v0.1 基线缺少"完成定义" | 中 |
| R6 | 文档结构扁平（33 文件平铺，已重组为 specs/guides/adrs/internal） | 中 |
| R7 | ADR 数量不足（已补写 ADR-0002~0007） | 中 |
| R8 | 经验反馈回路欠设计 | 中 |
| R9 | 测试策略完备但无测试设施 | 中 |

---

## 1. 系统架构设计 / Architecture

### 1.1 三条不变量 / Three invariants（架构"宪法"）

1. **Cognitive Program 是一等公民**：所有可执行语义必须能表示为程序；Agent 只是程序的运行实例。
2. **机制与策略分离**：运行时拥有进程/持久化/调度/效果/通信/检查点；模型选择、规划策略、优化权重保持在核心之外。
3. **一切执行结果可验证**：状态可追踪、执行可恢复、副作用可管理、过程可回放、失败可重规划、证据可追溯。

### 1.2 模块与接口 / Modules & Interfaces

| Crate | 职责 | 向上暴露 |
|---|---|---|
| `acos-core` | 类型、trait、错误、schema 原语 | 公共 trait |
| `acos-compiler` | 任务分析→CIR 生成→图验证→优化 | `compile(task) -> CirProgram` |
| `acos-runtime` | 调度、持久执行、重试、检查点、效果执行 | `execute(program) -> RunHandle` |
| `acos-state` | 事件日志、物化状态、工件、经验 | `EventStore`、`WorldState` |
| `acos-verify` | 验证流水线、证据收集 | `verify(...) -> VerificationReport` |
| `acos-plugin` | 插件注册表、热加载、健康检查 | `PluginRegistry`、`Provider` |
| `acos-cli` | 命令行入口 | `acos run / compile / verify / ...` |

**接口先行（interface-first）**：先在 `acos-core` 定义 trait 与错误类型，其他 crate 依赖 trait 并行开发。错误类型 `AcosError` 应覆盖 `architecture.md` 列出的全部 7 个失败域。

### 1.3 可执行要点 / Executable notes
- 序列化双轨：外部 manifest 用 JSON（+ JSON Schema），运行时 RPC 用 Protobuf。`schemas/` 按 `task/`、`primitive/`、`cir/`、`events/` 存放 `.proto` 与 `.jsonschema`。
- 经验反馈回路剥离：MVP 中 `ExperienceStore` 仅 append-only 记录，不接入编译回路；用 feature flag `experience-feedback` 隔离，Phase 3 启用。

---

## 2. 落地实现工程化 / Implementation

### 2.1 并行三轨道 / Three parallel tracks

- **轨道 A（契约先行）**：schema、trait、错误、Protobuf/JSON Schema → `acos-core` + `schemas/`
- **轨道 B（运行时骨架）**：`acos-runtime` + `acos-state` 最小实现（内存版）→ 能跑通执行图
- **轨道 C（工具链）**：脚手架、CI、基准测试 harness → 让 A/B 有运行与验证环境

三条轨道在第 3–4 周汇合于"ACOS Mini 端到端 demo"。

### 2.2 独立开发者工作流 / Solo workflow
- **Conventional Commits**：`feat:` / `fix:` / `spec:` / `refactor:` / `test:`（见 `guides/contribution.md`）。
- **单分支主干（trunk-based）**：`solo` 分支直接集成；重大实验用短期 feature 分支（< 1 天存活）。
- **每次提交必须可编译**：本地 `cargo check` 为前提，CI 为门卫。
- **测试驱动**：以 `mvp_spec.md` 的"必需演示"（5 CSV 分析 + 条件处理 + 审批）作为第一个端到端测试。

### 2.3 第 1 周动作 / Week 1 actions
1. 初始化 Git 仓库（若尚未初始化）、添加 `.gitignore`。
2. 创建 workspace `Cargo.toml` + `acos-core/Cargo.toml`，定义 trait 骨架。
3. 编写 `schemas/cir/cir.proto`、`schemas/task/task.proto`（仅 MVP 必需字段）。
4. 搭建 CI：`cargo build --workspace`、`cargo test`、`cargo clippy -- -D warnings`、`cargo fmt --check`。
5. 实现第一个端到端测试骨架（编译一个硬编码任务 → 执行 → 断言产出工件）。

**acos-core 初始模块结构**：
```
crates/acos-core/src/
├── lib.rs          # 公共导出
├── types.rs        # Cir, Task, Program, Evidence 等核心类型
├── traits.rs       # Primitive, Compiler, Runtime, EventStore, ...
├── error.rs        # AcosError 枚举（覆盖 7 失败域）
├── schema.rs       # schema 加载/校验辅助
└── id.rs           # RunId, ProgramId, ArtifactId 等新类型
```

---

## 3. 文档体系 / Documentation

已按以下结构重组 `docs/`（本次同步完成）：

```
docs/
├── INDEX.md               # 总入口（已重写）
├── GUIDANCE.md            # 本文件
├── specs/                 # 规范（契约）— 12 个文件，含 frontmatter
├── guides/                # 指南（怎么做）— 10 个文件，含 frontmatter
├── adrs/                  # 架构决策记录 — 7 个 ADR（+6）
└── internal/              # 内部设计备忘 — 9 个文件
```

**单一事实源原则**：架构真理权威来源是 `docs/`；每个 spec 必须有对应的代码或测试**锚点**，否则视为"未落地设计"。

**交叉引用约定**：spec 标注状态/代码锚点/schema/上次验证；guide 标注版本/读者/前置阅读；ADR 标注状态/日期。

**ADR 模板与 Spec frontmatter 模板**见计划文件 §3.3。本次已补写 ADR：
- ADR-0002 编译器/运行时分离
- ADR-0003 Rust 核心运行时
- ADR-0004 SQLite 作为 MVP 状态存储
- ADR-0005 Protobuf 线上格式与 JSON Manifest
- ADR-0006 原生进程优先的插件运行时
- ADR-0007 经验反馈回路在 MVP 中剥离

---

## 4. 项目管理与演进 / Project Management

### 4.1 里程碑 / Milestones（4–8 周）

| 里程碑 | 时间盒 | 完成定义 |
|---|---|---|
| **M0 脚手架就绪** | 第 1 周 | 仓库可构建、CI 通过、第一个 e2e 测试骨架存在 |
| **M1 契约固化** | 第 2–3 周 | CIR/Task/Primitive/Events schema 稳定；`acos-core` trait 完成 |
| **M2 ACOS Mini 端到端** | 第 4–6 周 | 5-CSV 分析 demo 跑通全部验收标准 |
| **M3 可复现基准** | 第 7–8 周 | 基准测试套件 7 类全部可运行、结果可复现、CI 回归防护 |

### 4.2 周节奏 / Weekly cadence
- **周一**：本周目标 + 更新看板（30 min）
- **每日**：1–2 个聚焦编码块（各 90 min），提交可编译的增量
- **周五**：本周演示（哪怕只是本地跑通）+ 写"周记录"（做了什么 / 卡在哪 / 下周做什么）

### 4.3 自 Review 清单 / Self-review checklist
- [ ] `cargo check` 通过
- [ ] `cargo clippy -- -D warnings` 无新警告
- [ ] `cargo fmt --check` 格式一致
- [ ] 新增/变更的公共 API 有文档注释
- [ ] 架构变更已写/更新 ADR
- [ ] 规范变更已更新对应 spec 状态与锚点

---

## 5. 验证方式 / Verification

- **设计验证**：每个 spec 有代码/测试锚点（frontmatter 中的"代码锚点"字段）。
- **实现验证**：`cargo test` 全绿；ACOS Mini demo 在 Windows/Linux/macOS 三平台可复现。
- **文档验证**：`docs/INDEX.md` 交叉引用无死链；ADR 状态与代码一致。
- **里程碑验证**：M0–M3 完成定义逐项勾选；周记录可追溯。

---

## 6. 落地状态 / Implementation status（本次同步）

- [x] 重组 `docs/` 为 specs/guides/adrs/internal
- [x] 重写 `docs/INDEX.md`
- [x] 重写 `docs/adrs/adr_index.md`
- [x] 补写 ADR-0002~0007
- [x] 给 12 个 spec 文件加 frontmatter
- [x] 给 10 个 guide 文件加 frontmatter
- [x] 新增 `docs/GUIDANCE.md`（本文件）
- [ ] 初始化仓库与 M0 脚手架（待执行）
- [ ] `acos-core` trait 骨架与 schema（待执行）
- [ ] 第一个端到端测试骨架（待执行）
- [ ] ACOS Mini 端到端 demo（M2）
- [ ] 基准测试套件（M3）
