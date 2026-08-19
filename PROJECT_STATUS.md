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
- [x] **原生 tool calling**（Anthropic `tool_use`，非自定义 XML）
- [x] 相同验证器（acos-verify 三层验证）
- [x] 指标记录（latency, llm_calls, tool_calls, cost）+ 区分 self-reported / verified success
- [x] 跨平台 Python 检测（Windows `where` / Unix `which`）
- [x] 领域无关 System Prompt（不泄露任务类型）
- [x] CLI `baseline` 子命令
- [x] 修复 `check_evidence` 移除 `artifact.stored` 硬性要求
- [x] v0.2 冻结，可开始真实实验

### P1-4 Fixed Workflow Baseline ⬜

- [ ] 手写确定性脚本（Python/Rust）
- [ ] 使用相同 flagship 任务 + 相同验证器
- [ ] 对比 ACOS vs 手写脚本

### P1-5A ModelCompiler Frontend Robustness ✅ FROZEN / PASS

> **目标**：让 ModelCompiler 成为一个可靠的编译器前端，能处理 LLM 的各种异常输出。

- [x] 错误分类体系（7 个具体变体）
- [x] Repair Prompt 机制（把具体 validator error 反馈给模型重试）
- [x] 重试状态机（JSON 提取 → Schema 校验 → CIR 验证 → Repair → 最多 3 次）
- [x] Compiler Robustness Suite（9 个测试用例 + 6 个语义验证测试）
- [x] 明确 CompilerFailure（RepairExhausted 包含完整诊断链）
- [x] Live smoke test 3/3 通过（见 `experiments/smoke/P1-5A-live-smoke-2026-08-18.md`）

**Smoke Test 数据**：
| 指标 | 值 |
|------|----|
| 首次成功率 | 33% (1/3) |
| Repair 成功率 | 100% (2/2) |
| 最终成功率 | 100% (3/3) |
| 平均延迟 | ~112s |

**已知限制**：首次模型响应为空（EOF）概率较高，属于模型/API 稳定性问题，不影响编译器机制正确性。

### P1-5B Cognitive Program Discovery 🔬 IN PROGRESS

> **目标**：验证命题 B——模型能否自动发现合理的 Cognitive Program 结构。

**设计原则**：
- 不要求复制 Golden CIR（语义等价的自由结构均可接受）
- 不给模型工作流提示（只有 Task Spec + Capability Registry + CIR Schema）
- 判断标准是 Task Adequacy（通过 Ground Truth），不是 Structural Equality

**三层正确性模型**：L1 Structural Validity → L2 Executability → L3 Task Adequacy

- [x] 实验设计文档（`experiments/p1-5b-cognitive-program-discovery/design.md`）
- [x] Behavioral Requirements Matrix（`BEHAVIORAL_REQUIREMENTS.md`，7 项行为要求）
- [x] 实现 `ModelCompiler::compile_traced()`（捕获完整 LLM trace + 时序）
- [x] 实现 Discovery Probe 二进制（`cargo run -p acos-cli --bin p1-5b-probe`）
- [x] **Probe-1 完成**（3 runs, LongCat-2.0）— 命题 B **NOT supported** ✅ FROZEN
  - L1 Structural Validity: 3/3 (100%)
  - L2 Executability: 0/3（全部路径幻觉 `/tmp/...`）
  - L3 Task Adequacy: 0/3
  - 分析：`experiments/p1-5b-cognitive-program-discovery/analysis.md`
- [x] **P1-5B-A: Semantic Grounding Prompt + Structured Compile Context**（最小语义对齐修复）
  - 7 条 Semantic Grounding Rules（编译器契约，非工作流提示）
  - 扩展 TaskSpec 为结构化 Compile Context（显式 Input Bindings）
  - 已提交（commit `4ed3f7a`）
- [x] **Probe-2 完成**（2c/2d 修正后）✅ FROZEN
  - 完整分析：`experiments/p1-5b-cognitive-program-discovery/probe-2-analysis.md`
  - 结果：binding 4/4 ×4 runs；BR 5–7/7；compile 4/4；执行 2/4；验证 0/4
  - 结论：> **Probe-2 支持"Prompt/Context Contract 是 Probe-1 主要混淆变量"的判断；尚不足以单独证明 ModelCompiler 已具备通用 Cognitive Program Discovery 能力。**
  - 决策：走 **Formal P1-5B**（正式 Discovery Evaluation），不继续无限调 Prompt
- [x] **Probe-2 结果分析** → 决定走 B or C → **走 A：Formal P1-5B**
- [x] **Stage Data Contract Phase 1**（编译期 R1–R5，见 `docs/specs/2026-08-18-stage-data-contract-design.md`）✅ FROZEN
  - 实施史料：`docs/specs/2026-08-18-stage-data-contract-phase1-plan.md`（12 commits，每任务独立审查）
  - 范围：OutputSpec/FieldSpec/input_types 全链迁移、R1–R5 契约检查（`crates/acos-compiler/src/contract.rs`）、runtime 点路径 `${a.b.c}`、探针 trace contract 层
  - 验证：`cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets -- -D warnings` clean
  - **冻结声明**：P1-5B Formal 实验期间不再改动 Phase 1 代码（防止实验条件漂移）
- [ ] **P1-5B Formal Evaluation** ✅ COMPLETE（v0.1 有效负结果）
  - 结果矩阵（15 runs，冻结条件 @ `eb0d9a8`）：
    | 系统 | Compile | Contract | Execute | Adequacy |
    |---|---|---:|---:|---:|
    | RuleCompiler ×5 | 5/5 | N/A | 5/5 | 5/5 |
    | ModelCompiler ×5 | 1/5 (20%) | 1/5 (20%) | 0/5 | 0/5 |
    | Direct Tool Loop ×5 | N/A | N/A | N/A | 0/5 |
  - 失败分类：3× COMPILE_FAILURE（repair 耗尽绑定错误 + 2× JSON 截断）、1× INFRA（网络）、1× EXECUTION_FAILURE（run-005：契约全过、BR 6/7、25 节点零控制流，裸 `pd.read_csv()` 遇真实脏数据 ParserError——**Program Design Failure**，非 Frontend Failure）
  - Repair 触发 3/5、成功率 0%；Compile 层延迟巨大（最高 30min/run）
  - **命题 A 支持**（Rule 5/5 vs Baseline 0/5；叠加 P1-R1 Fisher p≈0.0079）；**命题 B 暂不支持**
  - 官方结论与完整分析：`experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.1-results/report.md`
  - **v0.2 方向已拍板**：不优化 prompt，改结构化合成（Task → Plan IR → CIR）；P0 Structured Output Reliability / P1 Program Planning / P2 Code Contract（Phase 2）；三个小实验 A/B/C；P1-4 Fixed Workflow 提前

**ACOS 能力图谱（v0.1 实测后）**：
```text
Runtime          ✅ control semantics / recovery / contract execution
Verification     ✅ structural / semantic / contract
RuleCompiler     ✅ deterministic program synthesis
ModelCompiler    ⚠️ syntax generation / ⚠️ schema compliance / ❌ reliable program discovery
```

**P1-5B 过程中的运行时/编译器修复**（P1-5B-A 之外）：
- [x] `acos-llm`：`max_tokens` 可配置（`ACOS_LLM_MAX_TOKENS`，默认 32768）— 修复 LongCat-2.0 thinking 吞掉输出预算导致空响应
- [x] `build_repair_prompt` 携带完整 Compile Context（任务事实，非工作流提示）
- [x] `NoOpProgram` 校验（零能力图拒绝）+ `UnreachableNodes` 校验（孤儿节点拒绝，P1-5B Probe-2c run-001 案例）
- [x] Runtime `run_loop` 输出聚合：`loop_map.output` 绑定每次迭代最后一个 child 的输出数组（`cir_spec.md` 已更新）
- [x] p1-5b-probe：trace 覆盖保护（拒绝覆写已有 run）、BR-7 排除 loop item_var（避免误报）

## P1 实验方法论

### 命题框架

P1 的核心科学问题被拆分为两个独立命题，分别验证：

| 命题 | 定义 | 验证路径 | 当前状态 |
|------|------|----------|----------|
| **A**: 程序化编译 + 可验证执行 > 直接工具调用 | 给定正确的 Cognitive Program，Runtime + Verifier 的执行可靠性高于 LLM 直接调用工具 | RuleCompiler vs Baseline | ✅ **支持**（P1-R1: 5/5 vs 0/5, p≈0.0079；P1-5B v0.1: 5/5 vs 0/5） |
| **B**: 编译器能自动发现合理程序结构 | 给定任务描述，ModelCompiler 能生成语义合理的 CIR | ModelCompiler vs Ground Truth | ❌ **暂不支持**（P1-5B v0.1: Compile 1/5 / Execute 0/5 / Adequacy 0/5） |

### 实验对比矩阵

```text
                         P1 Flagship Task
                               │
         ┌─────────────────────┼─────────────────────┐
         ▼                     ▼                     ▼
   Direct Tool Loop      Expert Workflow      ACOS ModelCompiler
   (无结构)              (人工显式结构)        (机器自动发现结构)
         │                     │                     │
         └─────────────────────┼─────────────────────┘
                               ▼
                    Same Oracle (acos-verify)
                               ▼
              Reliability / Adaptability / Cost / Latency
```

**实验原则**：
- 冻结基线，无 benchmark 污染（P1-R1 已冻结，见 `SUCCESS-004`）
- 先证明组件独立工作，再对比
- 测量语义成功（semantic success），而非仅执行成功
- 命题 A 和命题 B 分别报告，不混为一谈

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
- RuleCompiler 生成的是简单线性流水线（read → summarize → write，无复杂控制流）——这是 P1-5B 要验证的
- Runtime 不发出 `artifact.stored` 事件（验证器已改为不要求此事件）
- 当前 Golden CIR 仅用于 Runtime 验证，不代表 Compiler 能力
- Baseline 端到端测试需 `LONGCAT_API_KEY`（无 key 时跳过）
- Baseline 当前使用 flat conversation（非 true multi-turn API），v0.2 足够但长期可改进
- **ModelCompiler 前端已修复**（P1-5A 完成，FROZEN/PASS）
- P1-5B Discovery Probe 完成（Probe-1 不支持命题 B；Probe-2 支持"Prompt/Context Contract 是主要混淆变量"的判断，尚不足以单独证明通用 Cognitive Program Discovery；决策走 Formal P1-5B）
- **Generated-code data contract**：`execute_python` 阶段间无 schema/type 契约（`${all_results}`、`${processed_data}`、key 名、列名假设），`KeyError`/`NameError`/`NoneType.strip` 属此层；作为独立工程问题处理（P1-5B Formal Branch 前置）。**Phase 1 已实施静态契约部分**（编译期 R1–R5：binding 存在性、结构可达性、类型对齐、字段路径、输出 schema 完整性），**Phase 2 structured transport（stdin/JSON/env 结构化传递）待做**。P1-5B v0.1 进一步定位 **P2：Python 代码语义契约**（`pd.read_csv` 是否容错 dirty CSV / schema inference）——Stage Data Contract 抓不到代码级语义，属 Phase 2 范围

## 第一轮实验结果（P1-R1，已冻结）

```text
ACOS RuleCompiler:  5/5 PASSED (100%)  [确定性输出，零方差]
Baseline:           0/5 PASSED (0%)    [高方差，self-reported 5/5 vs verified 0/5]
ModelCompiler:      0/1 (编译器失败，LLM 返回空/无效输出)
```

**结论（修订版）**：P1-R1 支持命题 A——对于结构化分析任务，确定性编译 + 可验证执行显著优于直接工具调用。尚未验证命题 B。

**关键发现**：Baseline 的 Completion Illusion（self-reported 100% vs verified 0%）成为设计原则 #9 的实证基础。

详细记录：`tests/benchmarks/p1/flagship_csv_quality/experiments/SUCCESS-004-p1-acos-vs-baseline-round1.md`

## 尚未开始 / Not yet started

### 当前优先级：P1-5B Cognitive Program Discovery

- [x] 设计 Compiler Discovery Probe（旗舰任务，无 Workflow 提示）
- [x] 定义 Behavioral Requirements（非结构要求）
- [x] 实现 compile_traced（Compile Success + Repair Tax + 完整 trace）
- [x] 实现 Discovery Probe 二进制（3 runs + 执行 + 验证）
- [x] 运行 Discovery Probe（Probe-1/2/2b/2c/2d，结果归档 `probe-results/`、`probe-2*-results/`）
- [x] 分析结果（`probe-2-analysis.md`：决策 = Formal P1-5B）
- [ ] **Formal P1-5B**（正式 Discovery Evaluation；前置 Generated-code data contract 已修复——Stage Data Contract Phase 1 完成并冻结）

### 后续计划

- **P1-4 Fixed Workflow Baseline**（**提前恢复**，先于 ModelCompiler v0.2——回答"该任务是否真的需要 AI 编译"，补全人工显式结构参照）
- **ModelCompiler v0.2: Structured Program Synthesis**（Task → Task Decomposition → Plan IR → CIR Generation → Contract Validation；三个小实验：A Control Flow Pressure Test / B Two-stage Compiler / C Output Streaming）
- Effect System（副作用声明与权限）
- 经验回路（Phase 3，feature flag `experience-feedback`）
- SQLite 持久化存储（Phase 2）
- SDK 稳定化（TypeScript/Python）
- 生产级分布式传输
- WASM 组件 ABI
