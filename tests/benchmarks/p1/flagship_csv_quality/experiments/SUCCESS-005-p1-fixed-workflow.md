# SUCCESS-005 — P1-4 Fixed Workflow Baseline

> **日期**: 2026-08-19 · **代码**: main @ `b44d00c` · **协议**: `docs/specs/2026-08-19-p1-4-fixed-workflow-design.md`（APPROVED → FROZEN）

## 结论

**Case A 命中**（Fixed Workflow 5/5 · RuleCompiler 5/5）：

> ACOS 当前证明的是**可靠执行架构**（Runtime + Verification + 确定性编译），而非自动程序发现（ModelCompiler 0/5）。Fixed Workflow 作为人工显式结构参照证明：给定正确程序，执行与验证层完全可靠；自动发现仍开放。

## 结果矩阵（四方终态）

| System           | Program Source        | Compile | Contract | Execute | Adequacy |
| ---------------- | --------------------- | ------- | -------- | ------- | -------- |
| Direct Tool Loop | LLM runtime decisions | N/A     | N/A      | N/A     | 0/5      |
| **Fixed Workflow** | **Human authored**    | **N/A** | **5/5**  | **5/5** | **5/5**  |
| RuleCompiler     | Rule-generated CIR    | 5/5     | 5/5      | 5/5     | 5/5      |
| ModelCompiler    | LLM generated CIR     | 1/5     | 1/5      | 0/5     | 0/5      |

Fixed Workflow 逐 run（确定性，零方差）：

| Run | Contract | Execute | Adequacy | Latency |
|-----|----------|---------|----------|---------|
| 1   | PASS     | PASS    | PASS     | 53ms    |
| 2   | PASS     | PASS    | PASS     | 58ms    |
| 3   | PASS     | PASS    | PASS     | 50ms    |
| 4   | PASS     | PASS    | PASS     | 50ms    |
| 5   | PASS     | PASS    | PASS     | 52ms    |

验证明细（每 run 相同）：Structural 4/4（data_quality / quarterly_summary / anomalies / recovery_log）· Semantic 6/6（q1–q4 revenue + grand total + files-with-issues 全匹配 Ground Truth）· Evidence 3/3（run.started / run.finished / 4 primitives）。数值与 GT 全一致（33850 / 24250 / 22500 / 2118550 / 2199150 / 3）。

## Engineering Cost

| System         | LOC | Nodes | Author Time | External Knowledge Required |
| -------------- | --- | ----- | ----------- | ---------------------------- |
| **Fixed Workflow** | **264**（Python） | N/A | ~45 min（估计） | human understanding of CSV task（领域 + 工程） |
| RuleCompiler   | 规则量（编译期） | 9 节点 | N/A（代码已存） | compiler rule implementation |
| ModelCompiler  | prompt（编译期） | 25 节点(最佳) | N/A（代码已存） | prompt + model capability |

> 注：Fixed Workflow 的 264 LOC 是一次性成本；RuleCompiler 的规则与 ModelCompiler 的 prompt 同样是一次性工程产物——三者的"每任务增量成本"才构成真正的对比（Fixed Workflow 每任务重写，Rule 规则库可复用）。

## 实现

- `crates/acos-fixed-workflow/`（flagship 注册 + python 定位/执行 + 指标 + 验证对接）
- `workflows/flagship.py`（264 LOC，纯 Python stdlib，人类编写）
- CLI：`acos fixed-workflow P1-FLAGSHIP-001 [--dataset-dir] [--report-out] [--gt] [--author-time]`
- **关键边界**：不生成 CIR、不走 ACOS Runtime/Compiler；仅共享验证 oracle（acos-verify）

## Fixed Workflow 工作流（通用工程逻辑）

容错加载（未引号化货币字段合并）→ schema 对齐（语义别名列映射）→ 校验（缺失 / 重复 / 负值 / 离群 median×10 / 列漂移 / 货币格式）→ 修复（去重、货币清洗、NULL 行排除）→ 重验 → 统计（清洗后求和，缺失记 0）→ 聚合 → 四章节报告 + evidence log。

## Fairness Review（§7 清单，逐项通过）

| 禁止项 | 状态 |
|--------|------|
| hardcode GT 数值 | ✅ 零数值字面量；离群阈值 median×10、缺失记 0 为通用规则 |
| 读取 expected/*.yaml | ✅ 仅读 dataset 目录 CSV |
| 文件名特判（`if file == q2` 类） | ✅ `glob("*.csv")` 全通配；无任何按名分支 |
| 调用 ACOS runtime/compiler/CIR | ✅ 仅执行 python + 共享 oracle |

**诚实记录的设计决策**：
1. **NULL 行排除**（units/revenue/date 任一字面 NULL → 整行无效，SQL NULL 语义；NA/空 → 字段级缺失标记、保留）。该区分是工程惯例，非按文件名特判。
2. **报告章节下划线名**（`data_quality` 等）：来自任务公开输出规范 `expected/schema.yaml`（required_sections），oracle 兼容。
3. **GT 数值反推为零**：开发中两次数值偏差（Q2 货币未清洗 12150→24250；Q3 NULL 行 26700→22500）均为清洗语义修正，未引入任何 GT 字面量。

## 实验解读

- **命题 A 强化**：确定性编译 + 可验证执行 = 可靠（Rule 5/5、Fixed 5/5、Baseline 0/5；Fisher p≈0.004 对任意一方）。
- **Fixed Workflow = RuleCompiler 上界参照**：RuleCompiler 5/5 与人类手写程序 5/5 持平——RuleCompiler 的确定性规则路径已达人工水平，但**未超越**（Case A 定义）。
- **ACOS 价值判定**：执行 + 验证架构可靠（Cost 低：ms 级、无 LLM）；自动程序合成是核心开放问题（ModelCompiler 0/5、程序 Design Failure 层级）。
- **故事**：Execution substrate solved / Verification solved / Program synthesis **unsolved**（开放问题，v0.2 Structured Program Synthesis 方向已冻结在 roadmap）。

## 复现

```bash
# PATH 前置 python（Windows）
$env:PATH = "C:\Users\Lin\AppData\Local\Programs\Python\Python312;" + $env:PATH
cargo run -p acos-cli --bin acos -- fixed-workflow P1-FLAGSHIP-001 --report-out report.md
# 输出 JSON：layers.{contract,execute,adequacy} 全 true
```

原始数据：`experiments/p1-4-fixed-workflow/run-00{1..5}.json` + `run-00{1..5}-report.md`
