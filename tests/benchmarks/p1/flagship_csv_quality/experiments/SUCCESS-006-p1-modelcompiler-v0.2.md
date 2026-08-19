# SUCCESS-006 — P1-5B ModelCompiler v0.2 Structured Program Synthesis

> **日期**: 2026-08-19 · **代码**: main @ `90c8917` · **协议**: `docs/specs/2026-08-19-modelcompiler-v0.2-structured-program-synthesis-design.md`（APPROVED → FROZEN，用户批准含 total function / 契约内建 / Control Intent Recall / 命题 B 新阈值）

## 结论

**命题 B：NOT SUPPORTED**（Compile 80% ≥ 80% ✓ · Plan completeness 33% < 70% ✗ · Adequacy 0% < 60% ✗）。

但架构升级方向获得**部分支持**——结构性失败层被消除：

> v0.2 首次证明"编译层"有效：`Task → Plan IR → 确定性编译器 → CIR` 把 v0.1 的 Program Design Failure（20% Compile）提升到 80% Compile / 80% Contract / 100% binding 闭合；失败前沿从**程序结构**（控制流/数据流/绑定）转移到**模型生成的步骤代码质量**（运行时环境契约与数据 schema 未知）。

## 结果矩阵（v0.2 vs 冻结 v0.1）

| System | Program Source | Compile | Contract | Execute | Adequacy |
| ------- | -------------- | ------- | -------- | ------- | -------- |
| ModelCompiler v0.1 | LLM → CIR（直接） | 1/5 (20%) | 1/5 | 0/5 | 0/5 |
| **ModelCompiler v0.2** | **LLM → Plan IR → 确定性编译** | **4/5 (80%)** | **4/5** | **0/5** | **0/5** |

逐 run（Experiment A/B/C 同源数据，`formal-eval-v0.2-results/`）：

| run | compile | contract | execute | repairs | control intent | recall | completeness | coverage |
|-----|---------|----------|---------|---------|----------------|--------|--------------|----------|
| run-001 | fail | fail | — | 1 | 2 | 0% | 33% | 67% |
| run-002 | pass | pass | fail | 1 | 1 | 100% | 33% | 33% |
| run-003 | pass | pass | fail | 1 | 1 | 100% | 33% | 33% |
| run-004 | pass | pass | fail | 0 | 1 | 100% | 33% | 33% |
| run-005 | pass | pass | fail | 0 | 1 | 100% | 33% | 33% |

## Plan 指标（Experiment A）

- **Control Intent Recall**: 67%（采纳 4 / 声明 6 控制意图）
- **Plan completeness**（avg）: 33%（6 行为要求：foreach ✓ / conditional ✗ / retry ✗ / write_file ✗ / 多阶段 ✗ / data_flow ≥2 ✗）
- **Control coverage**（avg）: 40%（required foreach+conditional+retry = 3）
- **Plan binding closure**: 20/20 = 100%（Experiment C——模型 Plan 的所有绑定引用均闭合；compile 期未出现未声明引用）

## 失败归因（失败前沿分析）

| run | 层 | 根因 |
|-----|-----|------|
| run-001 | Compile | conditional condition 用了非法表达式（裸 `$` token，repair×3 后仍无效）——模型未掌握 condition 语法（`exists(binding)`） |
| run-002 | Execute | 生成的 Python 引用未定义全局 `env`——模型不知道 execute_python 运行时契约（无预置全局；数据只能经 `${binding}` 插值注入） |
| run-003 | Execute | 裸 `item`（应写 `${item}`）——模板插值是编译期机制，运行时无 `item` 全局 |
| run-004 | Execute | 同 run-002（`env` 未定义） |
| run-005 | Execute | pandas `KeyError: 'quantity'`——未探测 header 就硬编码列名（q2/q3 存在 schema drift） |

**共同模式**：Plan 层（结构/控制流/数据流/绑定）被确定性编译器完整消解；失败全部落在**模型书写的 `execute_python` 代码**——模型既不知道 primitive 运行时契约（env 注入方式、`${...}` 插值语义），也不探测数据 schema。

## 实现交付（T1–T6）

- **T1** Plan IR schema：`crates/acos-compiler/src/plan.rs`（PlanIR/PlanStep/StepKind/BindingRef/StepOutput/DataFlowDecl/ControlDecl/RetrySpec；`writePath` 输出路径语义）
- **T2** `validate_plan`：全局唯一名、绑定作用域闭合（siblings+outer）、foreach aggregate 类型、conditional 无 output、retry maxAttempts≥2、data/control_flow 交叉校验、保留名保护
- **T3** `compile_plan`（total function）：合法 Plan ⇒ 合法 CIR；`over:"inputs"` → 编译器生成 `task_inputs` 注入节点（路径来自 TaskSpec 非 LLM，P1-5B-A 路径幻觉防护）；compile 末尾 `validate_cir_semantic` + `validate_data_contract` 哨兵
- **T4** frontend 切换：`PLAN_SYSTEM_PROMPT`（Plan IR 教学，无 Golden Plan）、`compile_plan_traced`（initial + repair loop）、`parse_plan`、`CompileTrace.plan` 字段
- **T5** 契约内建：undefined binding / missing field = PlanCompileError（编译期，非运行时）
- **T6** harness：`p1-5b-probe --plan`、`formal-eval-v0.2.ps1`（A/B/C 聚合 + 命题 B 判定）、`p1-5b-plan-smoke`（免 LLM 全管线冒烟：validate→compile→contract→execute→structural 4/4→semantic 6/6→evidence 3/3 ALL PASSED）
- Runtime 信封语义（编译层契约的运行时支撑）：`execute_python` 输出 `{stdout,stderr}` 信封；List/Record 插值输出可嵌入 Python 字面量的转义 JSON（消费方 `json.loads`）；ForEach 可迭代信封 stdout 的 JSON 数组；write_file content 信封解包取 stdout

## 契约（Experiment C 结论）

- undefined binding / missing field 全部在编译期拦截：**零运行时契约失败**
- compile-time contract failures surfaced: 1（run-001）；repair 捕获 3 次（run-001/002/003）
- Plan binding closure 100%——契约内建的构造性保证在真实模型输出上成立

## 工程成本

| 项 | 量 |
|----|----|
| Plan 编译器 + 验证器 | plan.rs ~1300 LOC（含测试） |
| 冒烟管线 | plan-smoke.rs + gen_plan_smoke.py（手写黄金 Plan，测试基建，禁入 prompt） |
| v0.2 harness | formal-eval-v0.2.ps1 |
| LLM 成本 | 5 runs ×（1 initial + 0.6 repair）≈ 8 LLM 调用 |

## 复现

```powershell
# PATH 前置 python（Windows）
$env:Path = "C:\Users\Lin\AppData\Local\Programs\Python\Python312;$env:Path"
# 免 LLM 冒烟（全管线 ALL PASSED）
cargo run -q -p acos-cli --bin p1-5b-plan-smoke
# 实验 A/B/C ×5（需 LONGCAT_API_KEY）
powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2.ps1
# 聚合（不重跑）
powershell -ExecutionPolicy Bypass -File experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2.ps1 -AggregateOnly
```

原始数据：`experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.2-results/{run-001..run-005}.trace.json` + `summary.md`。

## 故事

- **Execution substrate / Verification**：已解（v0.1 定论，未重跑）。
- **Program synthesis**：仍未解，但**首次出现结构性进展**——"中间认知结构 + 确定性编译"把失败层级从"程序设计"推后到"步骤代码"。
- **v0.3 方向**（spec §6 判定分支 + 失败归因）：Plan 层已验证器 + **execute_python 运行时契约与 schema 探测的教学**（prompt 内教 `${...}` 插值、header 探测、无预置全局）；或把步骤代码收窄为编译器模板/受限 DSL。