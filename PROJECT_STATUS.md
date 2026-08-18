# ACOS 项目状态 / Project Status

## 当前阶段 / Current stage

架构已收敛为用户空间认知运行时（user-space cognitive runtime），采用编译器/运行时分离（compiler/runtime split）。

**ACOS Mini MVP 已完成并验证**：
- 9 个核心 Rust crate 实现
- 认知编译器（规则优先 + Claude 模型辅助）
- 运行时执行引擎
- 验证流水线（三层确定性验证）
- Web 端（`acos-server`，端口 8080）
- 全部测试通过
- 端到端验证：目标 → 编译 → 执行 → 工件 → 验证

## P0 里程碑（控制语义 + 失败恢复 + 基准）✅

**目标**：让 CIR 支持条件分支、循环映射、暂态重试，并在运行时失败时有可验证的恢复路径；以 fixture 为契约的回归套件守护这些行为。

- [x] **控制语义类型**（`CirNode.control`：`condition` / `loop_spec` / `retry`，`else_children`）
- [x] **编译期校验**（`acos_compiler::validate_cir`）
- [x] **失败分类**（`FailureClass`：timeout / rate_limit / transient / invalid_input / …）
- [x] **运行时控制执行**（conditional / loop_map / retry 策略）
- [x] **恢复状态机**（`execute_with_recovery`：transactional gate + `rule` / `model` 重规划）
- [x] **RuleReplanner**（`OfflineFallbackRule`）
- [x] **ModelRecoveryPlanner**（LLM 生成 `RecoverySubgraph` 补丁）
- [x] **基准套件**（`crates/acos-bench`：condition / loop / retry / recovery / negative 五套 fixture）
- [x] **全 workspace 构建/测试绿**

## P1 里程碑（可靠性与对比评估）🔬

**目标**：通过实证对比，验证 ACOS 的 Cognitive Compiler + Runtime 是否比传统 Agent/Workflow 方法具有真实优势。

### P1-0 Flagship Task ✅

- [x] 旗舰基准任务定义（`tests/benchmarks/p1/flagship_csv_quality/`）
- [x] 4 个 CSV 数据集（含不同扰动模式）
- [x] 期望行为与输出规范（`expected/schema.yaml`）
- [x] ACOS TaskSpec（`acos_task.yaml`）

### P1-1 Golden CIR / Runtime Validation ✅

- [x] 手写 Golden CIR（8 节点：sequence → forEach → validate → conditional → repair → analyze_with_retry → merge_report）
- [x] `run-cir` CLI 子命令（直接执行 CIR，支持 `--env` 注入）
- [x] 环境注入（`execute_with_env`）
- [x] 模板插值增强（`${name}` 在任意字符串中解析）
- [x] Runtime 独立验证（13 项证据，Completed 状态）

### P1-2 Semantic Verification ✅

- [x] Ground Truth 数据集统计（`expected/ground_truth.yaml`）
- [x] 三层确定性验证器（`acos-verify`）：
  - **Structural**：artifact 存在、非空、必需章节
  - **Semantic**：数值声明 vs Ground Truth 一致性
  - **Evidence**：事件日志完整性
- [x] `run-cir --verify` 端到端验证
- [x] 验证器正确拒绝占位报告（证明能识别"坏"输出）

### P1-3 Direct Tool-Loop Baseline ✅

- [x] 最朴素的 LLM Agent：Goal → LLM → Tool Call → Tool Result → LLM → ...
- [x] 相同工具集（read_file, write_file, execute_python）对齐 ACOS 原语
- [x] 相同验证器（acos-verify 三层验证）
- [x] 指标记录（latency, llm_calls, tool_calls, cost）
- [x] CLI `baseline` 子命令
- [x] 修复 `check_evidence` 移除 `artifact.stored` 硬性要求

### P1-4 Fixed Workflow Baseline ⬜

- [ ] 手写确定性脚本（Python/Rust）
- [ ] 使用相同 flagship 任务 + 相同验证器
- [ ] 对比 ACOS vs 手写脚本

### P1-5 ModelCompiler Comparative Evaluation ⬜

- [ ] 启用真正的 LLM Compiler（需 API key）
- [ ] 对比 RuleCompiler vs ModelCompiler 输出质量
- [ ] 回答核心问题："Compiler 到底值不值？"

## P1 实验方法论

```text
                     P1 Flagship Task
                           │
         ┌─────────────────┼─────────────────┐
         ▼                 ▼                 ▼
       ACOS            Direct            Fixed
     (CIR)            Agent            Workflow
         │                 │                 │
         └─────────────────┼─────────────────┘
                           ▼
                  Same Oracle (acos-verify)
                           ▼
            Success / Cost / Latency /
            Recovery / Verification
```

**核心科学问题**：Cognitive Compiler 能不能在没有为每个任务写死 Workflow 的情况下，生成一个可靠的 Cognitive Program？

**实验原则**：
- 冻结基线，无 benchmark 污染
- 先证明组件独立工作，再对比
- 测量语义成功（semantic success），而非仅执行成功

## 环境配置 / Configuration

复制 `.env.example` 为 `.env` 并配置：

```bash
cp .env.example .env
```

关键变量：
- `LONGCAT_API_KEY` / `ANTHROPIC_API_KEY` — Claude API key
- `ACOS_PORT` — Web 服务器端口（默认 8080）
- `ACOS_LLM_MODEL` — 模型 ID（默认 LongCat-2.0）

## 运行方式 / How to run

```bash
# 构建
cargo build --workspace

# 测试
cargo test --workspace

# Web 端（推荐）
cargo run -p acos-server
# 浏览器打开 http://localhost:8080

# 命令行
cargo run -p acos-cli -- run task.yaml           # Claude 规划
cargo run -p acos-cli -- run task.yaml --rules   # 规则规划
cargo run -p acos-cli -- compile task.yaml       # 仅查看规划
cargo run -p acos-cli -- run-cir <cir.json> [--env <env.json>] [--verify <ground_truth.yaml>]
```

## 已知限制 / Known limitations

### P0 已知限制
- `ModelRecoveryPlanner` 需 LLM key；无 key 时相关用例在 `--require-model` 下 FAIL，否则 SKIP
- `acos-expr` 禁止模糊引用与 `null` 字面量
- `for_each` 当前串行（无并发）；`while`/`until` 必须显式 `max_iterations >= 1`
- 重试仅在暂态类生效，且节点原语须 retry-safe

### P1 已知限制
- RuleCompiler 生成的是简单线性流水线（无复杂控制流）——这是 P1-5 要验证的
- Runtime 不发出 `artifact.stored` 事件（验证器已改为不要求此事件）
- 当前 Golden CIR 仅用于 Runtime 验证，不代表 Compiler 能力
- Baseline 端到端测试需 `LONGCAT_API_KEY`（无 key 时跳过）

## 尚未开始 / Not yet started

- P1-4 Fixed Workflow Baseline
- P1-5 ModelCompiler Comparative Evaluation
- Effect System（副作用声明与权限）
- 经验回路（Phase 3，feature flag `experience-feedback`）
- SQLite 持久化存储（Phase 2）
- SDK 稳定化（TypeScript/Python）
- 生产级分布式传输
- WASM 组件 ABI
