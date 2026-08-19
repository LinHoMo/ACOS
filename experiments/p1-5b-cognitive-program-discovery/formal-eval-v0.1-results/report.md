# P1-5B Formal Evaluation v0.1 — 结果报告（2026-08-19）

> 设计冻结：`formal-eval-v0.1-design.md`；代码版本：main @ `eb0d9a8`（Stage Data Contract Phase 1 FROZEN）。
> 实验条件：same task / same dataset / same model（LongCat-2.0）/ same capability set / same timeout(300s) / same oracle（acos-verify 三层 + BR-1..7）。
> 数据集含真实脏数据（sales_q1.csv 第 3 行 6 字段 vs 5 字段 schema drift）——任务要求"detect data-quality issues; repair recoverable issues"。

## 0. 官方结论（v0.1，负结果）

> **P1-5B Formal Evaluation did not support Cognitive Program Discovery under the current single-pass LLM compilation architecture. The model demonstrated partial capability in generating contract-valid CIR programs, but failed to reliably synthesize executable control-aware programs satisfying behavioral requirements.**

失败层级为 **Program Design Failure**（run-005：契约全过但程序设计不满足任务），而非 Compiler Frontend Failure（run-001/002/004：输出可靠性）。两者被明确分离。

**命题判定**：
- 命题 A（程序化编译 + 验证执行 > Direct Tool Loop）：**支持**（RuleCompiler 5/5 vs Baseline 0/5；叠加 P1-R1 Fisher p≈0.0079，证据链成立）。
- 命题 B（LLM 自动发现 Cognitive Program）：**暂不支持**（Compile 1/5 / Contract 1/5 / Execute 0/5 / Adequacy 0/5）。

---

## 1. 结果矩阵（四层独立判定）

| 系统 | Compile | Contract | Execute | Adequacy | Latency(均值) |
|---|---:|---:|---:|---:|---:|
| RuleCompiler（×5） | **5/5 (100%)** | N/A（确定性生成） | **5/5 (100%)** | **5/5 (100%)** | ~秒级 |
| ModelCompiler（×5） | **1/5 (20%)** | **1/5 (20%)** | **0/5 (0%)** | **0/5 (0%)** | ~1.3h/run（含 repair） |
| Direct Tool Loop（×5） | N/A | N/A | N/A | **0/5 (0%)** | 20 turns/run |

## 2. ModelCompiler 逐 run 明细

| Run | Compile | Contract | Execute | Adequacy | 失败分类（主因） | 耗时 |
|---|---|---|---|---|---|---|
| 001 | FAIL | — | — | — | **COMPILE_FAILURE**：repair 耗尽，`merge_results` 引用未绑定 `'${processed_quarters}'` | 19.1 min |
| 002 | FAIL | — | — | — | **COMPILE_FAILURE**：JSON 语法错误（EOF，line 115） | 17.2 min |
| 003 | FAIL | — | — | — | **INFRA_FAILURE**：LLM 请求网络错误（api.longcat.chat） | 1.1 min |
| 004 | FAIL | — | — | — | **COMPILE_FAILURE**：JSON 语法错误（EOF，line 1，空响应） | 30.1 min |
| 005 | **OK** | **PASS** | **FAIL** | — | **EXECUTION_FAILURE**：程序用裸 `pd.read_csv()` 读取，遇脏数据（line 3 6 字段）抛 ParserError；无 conditional/retry 容错 | 7.9 min |

run-005 程序：25 nodes / 24 primitives / 0 loops / 0 conditions / 0 retries；BR 6/7（**BR-6 Control Flow FAIL**——无任何控制结构）；binding_accuracy 4/4；契约 R1–R5 全过。
→ 执行失败非环境故障（PATH 修复后重放 `run-cir` 确认）：是**程序未实现任务要求的 data-quality 容错**。

## 3. Repair Tax（ModelCompiler，trace 提取）

| 指标 | 值 |
|---|---|
| first_pass_compile_success | 1/5（run-005；run-003 首过但网络失败） |
| repair_trigger_rate | 3/5（run-001/002/004） |
| mean_repair_attempts | 1.8（0,3,0,3,0） |
| repair_success_rate | 0%（3 次触发均未救回） |
| final_compile_success | 1/5 |
| repair_latency | ~30–60s/attempt（run-001/004 合计占 50min） |
| repair_tokens | 未独立采集（trace 有原始响应，估算偏高，见局限性） |

## 4. Program Complexity（ModelCompiler 唯一成功编译样本 run-005）

node_count=25 / primitive_count=24 / control_node_count=0 / loop_count=0 / condition_count=0 / retry_count=0 / max_depth=未采集（probe 未输出，v0.1 局限）。
观察：**无控制流的 25 节点线性程序**——复杂度高（24 primitives）但零控制结构，与其 EXECUTION_FAILURE 直接相关（无容错分支）。支持"程序复杂度≠可靠性"的初步证据。

## 5. Q1 / Q2 / Q3

- **Q1（能否发现程序）**：ModelCompiler 1/5 生成合法程序（20%），且首跑成功率低、repair 无一次成功。
- **Q2（能否执行）**：0/5。唯一合法程序执行失败（脏数据未容错）。
- **Q3（执行是否正确）**：0/5（未到达验证层）。

## 6. 命题 B 判定

**暂不支持**（ModelCompiler）：
- Compile 20% / Contract 20% / Execute 0% / Adequacy 0%
- RuleCompiler 同任务 100% 全绿 vs ModelCompiler 0% 执行：Fisher 精确检验 p ≈ 0.004（5/5 vs 0/5）。
- 数据结论：ModelCompiler 当前既不稳定生成契约闭合的程序（Q1 弱），唯一成功样本也未达到任务要求（Q2/Q3 失败）。**Compiler 的可靠性未通过 ModelCompiler 继承**。

**对照确认**：RuleCompiler（确定性）在 v0.1 条件下维持 SUCCESS-004 的 100%/100%/100%（支持：确定性编译路径完全可靠）；Direct Tool-Loop Baseline 维持 0/5（与 SUCCESS-004 一致）。

## 7. 失败根因观察（供下一轮设计）

1. **repair 0% 成功率**：3 次触发全部失败——repair 提示或上下文不足以纠正绑定错误/JSON 截断。
2. **JSON EOF 截断 ×2**：模型输出被截断（长程序 25+ 节点超输出预算）→ repair 也无法恢复空响应。
3. **绑定错误**（run-001 `${processed_quarters}`）：模型引用与 producer 输出名不匹配——Contract R1 捕获正确（这正是 Stage Data Contract 的价值）。
4. **无控制流**（run-005）：任务明示 "repair recoverable issues / revalidate"，模型却生成线性程序 → BR-6 判定暴露语义缺口。
5. **INFRA 1/5**：网络层不可靠，需重试策略。

## 8. 局限性（如实记录）

- max_depth 未采集（probe 未实现，v0.1 未修代码——冻结纪律）。
- repair_tokens 未独立计量（trace 有 raw 数据可后处理）。
- Baseline 无结构化 trace（文本解析 turns），仅报告 adequacy。
- run-005 首执行被环境（python PATH）拦截，重放后暴露真实程序缺陷——环境修复不改变任何一层的最终判定。
- ModelCompiler 组包含 1 次 INFRA 失败，样本实际有效为 4/5 编译判定（如实标注，未剔除）。

## 9. 下一轮建议（v0.2 候选，未冻结）

1. ModelCompiler 输入中**明确输出长度/结构约束**（防截断）或允许分块生成。
2. repair 提示注入 Contract 错误详情（已具备）+ **失败样例修复模式**。
3. 评估时区分 INFRA 重试：INFRA 自动重试 ≤3 次再计入。
4. Program Complexity 纳入 max_depth 采集。
5. 若 Compile 层不改善，命题 B 判定维持"暂不支持"，并考察是否进入 Optimizer 阶段。

## 10. v0.2 方向（用户拍板：不优化成"更强 Prompt"，结构化合成）

瓶颈排序（P0/P1/P2）：
- **P0 Structured Output Reliability**：3/5 COMPILE_FAILURE（2× JSON truncation）。单次输出完整 CIR JSON 不可靠 → 分阶段生成（Task → CIR draft → incremental validator → completion/repair；或 generate nodes / edges / controls / schemas 分段）。
- **P1 Program Planning Capability**：run-005 25 nodes / 0 control flow —— 模型不把任务抽象成控制结构（ForEach→validate→Conditional(has_issue)→repair→aggregate 缺失），典型 LLM workflow bias（read everything → process → write）。
- **P2 Generated Code Contract**（Phase 2）：Stage Data Contract R1–R5 抓 binding/type/field，抓不到 Python 代码语义契约（`pd.read_csv` 是否允许 dirty CSV / 是否需要容错 / schema inference）。

**ModelCompiler v0.2 架构**（中间 Plan IR 层）：

```text
Task → Task Decomposition → Plan IR → CIR Generation → Contract Validation → Execution
```

**v0.2 三个小实验**（不立即重跑 15 runs）：
- **Experiment A: Control Flow Pressure Test** —— 任务必须自然需要 foreach/conditional/retry，测 `control_node_rate`（生成 control node %），验证模型能否发现控制结构。
- **Experiment B: Two-stage Compiler** —— `Task → CIR`（v0.1）vs `Task → Plan → CIR`（v0.2），只改 compiler 不改 runtime。
- **Experiment C: Output Streaming** —— single JSON vs structured generation，针对 truncation。

**Fixed Workflow（P1-4）提前恢复**：顺序改为 `P1-4 Fixed Workflow Baseline → ModelCompiler v0.2`（不再延期）。P1-4 回答"该任务是否真的需要 AI 编译"，补全 Baseline(0/5) / RuleCompiler(5/5) / ModelCompiler(0/5) 之外的人工显式结构参照。

**核心研究问题（锁定）**：如何让模型从"生成步骤"升级为"生成可验证认知程序"。