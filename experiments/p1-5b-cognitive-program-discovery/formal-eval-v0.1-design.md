# P1-5B Formal Evaluation v0.1 — 实验设计（冻结）

> **状态**: FROZEN（2026-08-19，用户拍板）。实验期间**不修改**下方冻结清单中的任何一项。
> **前置**: Stage Data Contract Phase 1 ✅ FROZEN（commit `eb0d9a8` 起，GitHub main 已确认）。
> **回答**: 命题 B——给定任务目标、输入绑定、能力集合和 CIR 约束，ModelCompiler 能否自动发现一个**可执行、数据契约闭合、并最终满足任务要求**的 Cognitive Program？

---

## 1. 实验系统（三组）

| 系统 | 运行数 | 入口 | 备注 |
|---|---|---|---|
| ACOS RuleCompiler | 5 | `acos run <task.yaml> --rules`（确定性） | 5 次同构，验证确定性 + 作为参照 |
| ACOS ModelCompiler | 5 | `p1-5b-probe --runs 5`（随机性保留） | 核心被测对象 |
| Direct Tool-Loop Baseline | 5 | `acos baseline <goal> --verify` | 对照组 |

`Fixed Workflow` 保持 Pending——先确认 ModelCompiler 是否具备基本 Program Discovery 能力，再决定是否投入第四组。

**同条件约束**（5 次之间不得变更）：same task / same dataset / same model / same capability set / same timeout / same verification oracle。

## 2. 四层成功模型（严格执行，分别报告）

```
Discovery Success = Compile PASS ∧ Contract PASS ∧ Execute PASS ∧ Adequacy PASS
```

| 层 | 定义 | 数据来源 |
|---|---|---|
| Compile | ModelCompiler 最终产出合法 CIR（含 repair 后） | probe trace `compile` |
| Contract | Stage Data Contract R1–R5 通过 | probe trace `contract.pass` |
| Execute | Runtime 执行成功（Completed，产出 artifact） | probe trace `run` |
| Adequacy | acos-verify 三层验证通过（Structural/Semantic/Evidence） | probe trace `verify` |

**分别报告**：Compile Success Rate / Contract Success Rate / Execution Success Rate / Adequacy Success Rate / Overall Discovery Success Rate。

**禁止过度推断**：`CIR valid → "Compiler 成功"` 不再成立；四层独立判定。

## 3. 禁止 Golden CIR 结构比较

禁止 `Model CIR != Golden CIR → FAIL`。允许任意结构（节点数、控制流、顺序自由），只要满足 Contract + Execution + Behavioral Requirements + Ground Truth 即算成功。

> 评价的是程序语义，不是程序长相。

## 4. Behavioral Requirements（正式裁判，冻结）

冻结基准：`experiments/p1-5b-cognitive-program-discovery/BEHAVIORAL_REQUIREMENTS.md`（BR-1..BR-7）。

任务行为要求（用户 v0.1 指定 7 项）与 BR 映射：

| 行为要求 | 对应 |
|---|---|
| multi_file_processing | BR-1（binding_accuracy == 1.0） |
| data_quality_analysis | BR-2 |
| anomaly_detection + repair_or_recovery | BR-3 |
| aggregation | Ground Truth `aggregate`（grand_total_revenue 等，P1-2 验证器） |
| structured_report | BR-4 |
| evidence | BR-5 |

补充程序行为：BR-6（控制流）、BR-7（无幻觉资源）。以上均为行为检查，**不是**结构要求（不要求必须 ForEach / Conditional / Retry / 8 节点）。

## 5. ModelCompiler 输入（冻结）

- Task Specification（`acos_task.yaml`）
- Input Bindings
- Capability Registry
- CIR Schema
- Compiler Rules（P1-5B-A 的 7 条 Semantic Grounding + 编译期校验 + Contract R1–R5 反馈）

**禁止**：Golden CIR、Fixed Workflow、"请使用 ForEach/Conditional"、"请生成 8 个节点" 等任何结构引导。

## 6. 失败分类（每次失败归入一个主要阶段，可附 secondary cause）

| 分类 | 示例 |
|---|---|
| COMPILE_FAILURE | 无合法 CIR（含 repair 耗尽） |
| CONTRACT_FAILURE | CIR 合法但违反 R1–R5 |
| EXECUTION_FAILURE | 契约通过但 Python NameError/KeyError/NoneType |
| ADEQUACY_FAILURE | 执行成功但验证失败 |
| INFRA_FAILURE | API 错误、超时、工具故障 |

## 7. ModelCompiler 额外记录（Repair Tax + Program Complexity）

**Repair Tax**：first_pass_compile_success / repair_trigger_rate / mean_repair_attempts / repair_success_rate / final_compile_success / repair_latency / repair_tokens。

**Program Complexity**（每次 ModelCompiler 输出）：node_count / primitive_count / control_node_count / loop_count / condition_count / retry_count / max_depth。
观察 `Program Complexity vs Task Success`——为 Optimizer 实验奠基（判断认知程序是否过度复杂）。

## 8. 统一 Oracle

```
Ground Truth ──► ACOS / Model / Baseline 各自产出 ──► 同一 P1 Verifier（acos-verify 三层）
```

`verified_success` 三系统同口径可比。Baseline 的 Compile/Contract 记为 **N/A**（非 0）。

## 9. 第一轮成功标准（不预设阈值，数据说话）

- **Q1 能否发现程序**：final_compile_success
- **Q2 程序能否执行**：execution_success
- **Q3 执行是否正确**：adequacy_success

## 10. 命题 B 判定（三级）

| 级别 | 条件 |
|---|---|
| 支持 | 稳定生成 + 执行 + 过 Ground Truth，且多随机运行非偶然 |
| 初步支持 | 能成功但 variance high / repair dependency high / cost high |
| 暂不支持 | 如 Compile 100% / Contract 90% / Execute 20% / Adequacy 10% |

## 11. 结果矩阵（最终交付）

| 系统 | Compile | Contract | Execute | Adequacy | Overall | Latency | Cost |
|---|---|---|---:|---:|---:|---:|---:|---:|---:|
| RuleCompiler | ? | ? | ? | ? | ? | ? | ? |
| ModelCompiler | ? | ? | ? | ? | ? | ? | ? |
| Direct Tool Loop | — | — | ? | ? | ? | ? | ? |

## 12. 实验冻结点（冻结清单）

- TaskSpec: `tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml`
- Dataset: `datasets/sales_q{1..4}.csv`
- Ground Truth: `expected/ground_truth.yaml`
- Behavioral Requirements: `BEHAVIORAL_REQUIREMENTS.md`（BR-1..7）
- Compiler Prompt: P1-5B-A 冻结版（含 Compile Context）
- Capability Registry: 冻结版
- Verifier: acos-verify 三层
- Model: LongCat-2.0（`ACOS_LLM_MODEL`）
- Retry limit: 编译 repair ≤ 3 次（P1-5A 冻结）
- Runtime/Compiler 版本: main @ `eb0d9a8`

> **冻结纪律**：5 次实验中途不得修改以上任何一项；Stage Data Contract Phase 1 代码不再改动。

## 13. 输出与归档

- 结果目录：`experiments/p1-5b-cognitive-program-discovery/formal-eval-v0.1-results/`
- 每 run 归档：probe trace JSON（model）、baseline 报告、rule 报告
- 汇总：`formal-eval-v0.1-results/summary.md`（矩阵 + Repair Tax + Complexity + 失败分类 + Q1/Q2/Q3 + 判定）