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
- 阶段（Stage）：**ACOS Mini MVP 已完成，进入 P1 实证对比阶段**
- 架构事实的权威来源（Canonical source of architectural truth）：`docs/`
- 已完成：
  - ✅ M0 脚手架（仓库、workspace、CI、schema）
  - ✅ 9 个核心 Rust crate 实现
  - ✅ 认知编译器（规则优先 + Claude 模型辅助）
  - ✅ 运行时执行引擎（图执行、工件、证据、验证）
  - ✅ 三层确定性验证器（Structural/Semantic/Evidence）
  - ✅ 全部测试通过
  - ✅ **Web 端**（`http://localhost:8080`，可直观看到 agent 规划与执行）
  - ✅ **Claude API 集成**（龙猫代理，动态规划）
  - ✅ P1-0 Flagship Task + P1-1 Golden CIR + P1-2 Semantic Verification

## 快速开始 / Quick start

> **前置条件 / Prerequisite**：安装 Rust 工具链。推荐通过 [rustup](https://rustup.rs/) 安装：
> ```bash
> curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
> ```
> 安装后验证：`cargo --version` 与 `rustc --version`。

### 1. 构建整个 workspace

```bash
cargo build --workspace
```

### 2. 运行测试

```bash
cargo test --workspace
```

### 3. 启动 Web 端（推荐）

```bash
# 配置（可选，也可通过 .env 文件）
export LONGCAT_API_KEY=your_api_key        # 龙猫/Anthropic API key
export ACOS_PORT=8080                       # 可选，默认 8080

# 启动服务器
cargo run -p acos-server

# 浏览器打开 http://localhost:8080
```

Web 界面支持：
- 编辑任务 YAML（含示例模板）
- 选择规划器：**Claude 模型辅助**（需 API key）或**规则优先**（无需 key）
- 实时查看规划 → 编译 → 执行 → 验证全流程
- 查看产出的工件（如 `report.md`）

### 4. 命令行使用

```bash
# Claude 规划并执行
cargo run -p acos-cli -- run task.yaml

# 仅查看 Claude 生成的执行图
cargo run -p acos-cli -- compile task.yaml

# 使用规则规划器（无需 API key）
cargo run -p acos-cli -- run task.yaml --rules
```

### 6. 基准测试 / Benchmark

`fixture-as-contract` 回归套件（P0：控制语义 + 失败恢复）。每个 fixture 是一份行为契约
（CIR 程序 + 期望结果），由 `crates/acos-bench` 运行并聚合报告。

```bash
# 运行指定套件
cargo run -p acos-cli -- bench --suite condition
cargo run -p acos-cli -- bench --suite loop
cargo run -p acos-cli -- bench --suite retry
cargo run -p acos-cli -- bench --suite recovery
cargo run -p acos-cli -- bench --suite negative

# 全量
cargo run -p acos-cli -- bench

# CI 严格模式：未配置 LLM key 的 model 恢复用例视为失败
cargo run -p acos-cli -- bench --require-model
```

套件说明：

- `condition`：条件分支出（conditional + else_children）
- `loop`：`for_each` / `while` 循环映射（loop_map）
- `retry`：暂态失败的自动重试（retry）
- `recovery`：`rule` 重规划（`OfflineFallbackRule` 本地回退）+ `model` 重规划（需 LLM key）
- `negative`：编译期拒绝契约（缺 `max_iterations`、重试次数 0、对不可逆原语重试）

恢复观测（rule/model/retry）来自运行期事件流，报告表 `Recover` 列显示命中的恢复标签。

### 5. 配置文件（.env）

复制 `.env.example` 为 `.env` 并填入：

```bash
cp .env.example .env
# 编辑 .env 填入 LONGCAT_API_KEY 等
```

服务器启动时会自动加载 `.env`。详见 `.env.example`。

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

ACOS Mini 已证明：**结构化任务规范可以被编译为可执行认知程序，并由 Runtime 可靠执行和验证**。开放式自然语言目标到复杂 Cognitive Program 的自动编译能力仍处于 P1 实证验证阶段。

当前 5 个 MVP 原语：

- `search`（搜索）
- `read_file`（读文件）
- `write_file`（写文件）
- `execute_python`（执行 Python）
- `summarize`（总结，已集成 Claude）

P0 基准测试覆盖条件密集型任务（condition-heavy task）、循环、重试、失败恢复与负面案例。P1 正在进行 ACOS 与传统 Agent/Workflow 方法的实证对比评估。

## P1 实证阶段 / P1 Empirical Phase

当前重点：**验证 Cognitive Compiler 是否比传统方法具有真实优势**。

```
                    ACOS
                      │
        ┌─────────────┴─────────────┐
        │                           │
    Architecture                Runtime
        ✅                         ✅
        │                           │
        └────────────┬──────────────┘
                     │
               P1 Evidence
                     │
        ┌────────────┼────────────┐
        │            │            │
      Golden       Oracle       Benchmark
       CIR          ✅            ✅
        │
        ▼
   Core Question
        │
        ▼
   "Compiler 到底值不值？"
```

路线图：
- ~~P0 控制语义 + 失败恢复~~ ✅
- ~~P1-0 Flagship Task~~ ✅
- ~~P1-1 Golden CIR / Runtime Validation~~ ✅
- ~~P1-2 Semantic Verification~~ ✅
- P1-3 Direct Tool-Loop Baseline
- P1-4 Fixed Workflow Baseline
- P1-5 ModelCompiler Comparative Evaluation

详见 [项目状态 / Project Status](PROJECT_STATUS.md)。

## 文档导航 / Documentation map

从这里开始：

1. [项目概述 / Project Overview](docs/internal/project_overview.md)
2. [架构 / Architecture](docs/internal/architecture.md)
3. [技术栈 / Tech Stack](docs/internal/tech_stack.md)
4. [运行时模型 / Runtime Model](docs/specs/runtime_model.md)
5. [认知原语规范 / Cognitive Primitive Specification](docs/specs/cognitive_primitive_spec.md)
6. [任务规范 / Task Specification](docs/specs/task_spec.md)
7. [CIR 规范 / CIR Specification](docs/specs/cir_spec.md)
8. [编译器设计 / Compiler Design](docs/internal/compiler_design.md)
9. [执行模型 / Execution Model](docs/specs/execution_model.md)
10. [验证 / Verification](docs/specs/verification.md)
11. [插件系统 / Plugin System](docs/specs/plugin_system.md)
12. [路线图 / Roadmap](docs/guides/roadmap.md)
13. [全链路指导性建议 / Holistic Guidance](docs/GUIDANCE.md)
14. [项目状态 / Project Status](PROJECT_STATUS.md)

## 仓库结构 / Repository structure

```text
ACOS/
├── .env.example              # 配置模板
├── Cargo.toml                # Workspace 根
├── crates/
│   ├── acos-core/            # 类型、trait、错误、schema
│   ├── acos-compiler/        # 认知编译器（规则 + 模型辅助）
│   ├── acos-runtime/         # 运行时执行引擎
│   ├── acos-state/           # 状态存储（内存/SQLite）
│   ├── acos-plugin/          # 内置原语 + 注册表
│   ├── acos-verify/          # 验证流水线
│   ├── acos-llm/             # LLM 提供者（龙猫/Anthropic）
│   ├── acos-cli/             # 命令行入口
│   ├── acos-bench/           # fixture-as-contract 基准回归套件
│   └── acos-server/          # Web 服务器
├── schemas/                  # Protobuf / JSON Schema
├── static/                   # Web 前端
├── docs/                     # 文档（specs/guides/adrs/internal）
└── tests/                    # 测试夹具
```

## 设计原则 / Design principles

完整设计原则见 [设计原则 / Design Principles](docs/internal/design_principles.md)。

### 三支柱 / Three Pillars

- **Stable Core + Everything Extensible**：稳定的核心运行时 + 一切可替换能力皆以标准化插件接入
- **Cognitive Program as First-Class Citizen**：认知程序是一等公民，Agent 只是程序的运行时执行实体
- **Reliable by Default**：状态可追踪、执行可恢复、副作用可管理、过程可回放、失败可重规划、证据可追溯

## 许可证 / License

待定（TBD）。公开发布前决定。
