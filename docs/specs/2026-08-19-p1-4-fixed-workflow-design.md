# P1-4 Fixed Workflow Baseline — 实验设计（spec）

> **状态**: **APPROVED → FROZEN**（2026-08-19 用户批准）。
> **日期**: 2026-08-19 · **代码版本**: main @ `7f663d6`（P1-5B v0.1 已冻结，历史数据不修改）。

---

## 1. Background & Motivation

实验矩阵缺口：

```text
P1-R1:  Direct Tool Loop 0/5 · RuleCompiler 5/5
P1-5B:  ModelCompiler    0/5 · RuleCompiler 5/5

缺少：  Fixed Workflow（Human authored）?
```

没有 Fixed Workflow，无法区分 RuleCompiler 的优势来自 **Cognitive Compilation**，还是仅因任务存在一个简单确定性的最佳流程。P1-4 直接回答该问题。

## 2. Research Question

> 对于结构化分析任务，一个人工设计的固定程序是否已经足够达到 ACOS RuleCompiler 的可靠性？ACOS 的价值在于自动生成 Cognitive Program，还是单纯执行预定义 workflow？

## 3. 实验矩阵（最终形态）

| System           | Program Source        | Compile | Contract | Execute | Adequacy |
| ---------------- | --------------------- | ------- | -------- | ------- | -------- |
| Direct Tool Loop | LLM runtime decisions | N/A     | N/A      | ?       | 0/5      |
| Fixed Workflow   | Human authored        | N/A     | PASS?    | ?       | ?        |
| RuleCompiler     | Rule-generated CIR    | PASS    | PASS     | 5/5     | 5/5      |
| ModelCompiler    | LLM generated CIR     | 1/5     | 1/5      | 0/5     | 0/5      |

**列定义**（Fixed Workflow 专属）：
- **Compile**：N/A（无编译产物）。
- **Contract**：输出契约检查——artifact 满足任务 `outputs` 声明（report.md 存在、非空、含必需章节；即 verifier Structural 层）。记为 PASS/FAIL，非 N/A。
  - ⚠️ **明确区分**：`Fixed Workflow Contract Check ≠ ACOS CIR Data Contract`（Stage Data Contract R1–R5 是编译期 CIR 契约；Fixed Workflow 无 CIR，其 Contract 仅是 **artifact-level contract compliance** 的可比代理。避免二者混淆）。
- **Execute**：固定脚本执行完成且无未捕获异常。
- **Adequacy**：同一 acos-verify 三层 oracle（Structural / Semantic / Evidence vs Ground Truth）。

## 4. Implementation Boundary（关键）

**Fixed Workflow 不生成 CIR，不走 Runtime**：

```text
Fixed Workflow
      ↓
Direct Execution Layer（Python 脚本 + Rust 执行壳）
      ↓
Artifact (report.md)
      ↓
Same Verification (acos-verify 三层)
```

原因：P1-4 的问题是"**人类是否可以直接写一个可靠程序**"，不是"人类是否可以写一个 CIR"。若走 CIR+Runtime，则与 RuleCompiler 混淆。

**不改动**：`acos-compiler` / `acos-runtime` / `acos-verify` / Stage Data Contract Phase 1 代码（冻结纪律）。

## 5. 实现形态

```
crates/acos-fixed-workflow/
  src/
    flagship.rs    # flagship 任务注册 + 工作流定义（脚本路径、输入、输出声明）
    tools.rs       # python 定位（where/which）与执行（对齐 acos-baseline 跨平台检测）
    report.rs      # 报告装配（artifact 落盘）
    metrics.rs     # 指标记录（execution_time_ms, loc, 等）
```

CLI 子命令（`crates/acos-cli/src/main.rs`）：

```
acos fixed-workflow <task>
```

- `<task>` 当前仅支持 `P1-FLAGSHIP-001`（其他任务返回明确错误）。
- 输出统一 JSON（与 `acos baseline` / `acos run-cir` / `acos bench` 同一实验接口）：

```json
{
  "system": "fixed_workflow",
  "task": "P1-FLAGSHIP-001",
  "execution_time_ms": 12345,
  "verification": {
    "structural": true,
    "semantic": true,
    "evidence": true
  }
}
```

## 6. Fixed Workflow 内容定义（通用工程逻辑，非 benchmark 特判）

工作流（对 4 个输入 CSV 依次处理 + 聚合 + 报告）：

```text
for each csv:
    load（容错解析：严格读取失败 → 记录解析异常并降级容错读取）
    normalize schema（列名漂移检测：与标准 schema 对齐；货币格式 $ , 剥离）
    validate（缺失 / 重复 / 负值 / 离群 / 列漂移检测）
    repair recoverable（去重；货币解析；缺失标记；负值与离群标记但保留）
    revalidate
    compute statistics（total_revenue、total_units、问题清单）
aggregate（grand_total_revenue、files_with_issues、total_issues）
generate report（data_quality / quarterly_summary / anomalies / recovery_log + evidence log）
```

**口径**：以任务描述 + 验证器要求为准（如"revenue 为列求和、q2 货币格式需清洗、q3 重复行排除、q4 离群保留求和"）——这是任务语义（RuleCompiler 规则同样知道要求），**不是 Ground Truth 数值**。

**允许**：pandas / csv 标准库 / 确定性规则。

## 7. Fairness Rules（防作弊）

**禁止**：
- hardcode Ground Truth 数值（任何 `grand_total_revenue = 2199150.00` 之类字面量）
- 读取 `expected/ground_truth.yaml` 或 `schema.yaml`
- 按文件名特判（`if file == sales_q2.csv: ...` 或针对具体扰动模式的分支，如 `if quarter=="Q2": fix_schema()`）
- **调用 ACOS runtime / CIR executor / compiler 组件**（`acos-runtime` / `acos-compiler` / `ModelCompiler` / `RuleCompiler` / CIR 任何路径）——防止边界污染

**允许**（engineering logic，非 benchmark 特判）：
- 通用列映射推断：`if missing_column: infer_column_mapping()`（语义关键词对齐，非按文件名）
- 通用数据修复：去重、货币格式清洗、缺失标记、容错解析（CSV 未引号化逗号修复）
- pandas / csv 标准库 / 确定性规则

**审查**：实现完成后逐行对照本清单审查；任何违规 → 修改直至合规方可运行实验。

## 8. 指标

四层（§3）+ **Engineering Cost**（新增维度——Fixed Workflow 的代价是"人类写多少"）：

| System         | LOC   | Nodes | Author Time | External Knowledge Required |
| -------------- | ----- | ----- | ----------- | ---------------------------- |
| Fixed Workflow | ?     | N/A   | ?           | human understanding of CSV task（领域 + 工程） |
| RuleCompiler   | 规则量 | 程序节点 | N/A        | compiler rule implementation |
| ModelCompiler  | prompt | 程序节点 | N/A        | prompt + model capability |

否则 Fixed Workflow 永远是强者：人类提前知道答案。

## 9. 预期结果解释

- **Case A**（Fixed 5/5 · Rule 5/5）：ACOS 当前证明的是**可靠执行架构**，而非自动程序发现——仍是成功结果。
- **Case B**（Fixed 4/5 · Rule 5/5）：最有价值——编译层产生了超过简单人工 workflow 的可靠性。
- **Case C**（Fixed 5/5 · Rule 5/5 · Model 0/5）：论文叙事清晰：Execution substrate solved / Verification solved / Program synthesis unsolved。

## 10. 验收标准

1. 固定脚本 ×5 独立 runs，结果确定性（variance ≈ 0，5/5 或一致失败）。
2. 同一 oracle（acos-verify 三层 vs Ground Truth），不与任何 LLM 交互。
3. `cargo test --workspace` 全绿 + `cargo clippy --workspace --all-targets -- -D warnings` clean。
4. Fairness 审查通过（§7 清单）。
5. 报告 `SUCCESS-005-p1-fixed-workflow.md`：矩阵 + Engineering Cost + Case A/B/C 判定 + 结论（"ACOS value = execution?" 或 "compilation?"）。
6. 推送 GitHub main。

## 11. 冻结清单（批准后生效）

- TaskSpec / Dataset（4 CSV）/ Ground Truth / acos-verify oracle：与 P1-5B v0.1 完全相同
- 代码版本：main @ `6b91984` 之上新增 acos-fixed-workflow（不改既有 crate 行为）
- P1-5B v0.1 历史数据不修改
- 运行环境：Python 前置 PATH（`C:\Users\Lin\AppData\Local\Programs\Python\Python312`）

## 12. 风险与对策

| 风险 | 对策 |
|------|------|
| 脚本被写成 benchmark 特判（作弊） | §7 清单审查 + 拒绝运行 |
| python 环境未就绪 | 复用 acos-baseline 的跨平台检测；实验前 PATH 校验 |
| 口径与验证器不一致导致误判 | 先跑 smoke 验证 artifact 结构，再正式 ×5 |
| Contract 层定义含糊 | §3 列定义固定：Contract = 输出契约检查（Structural 层），且明确 ≠ Stage Data Contract |

## 13. 冻结声明（用户批准，LOCKED）

- Status: **APPROVED → FROZEN**（commit `7f663d6`）
- 实验协议 LOCKED，**禁止修改**：workflow logic（runs 开始后）、verification oracle、ground truth、metrics definition
- 实施顺序：spec 冻结 → `acos-fixed-workflow` crate → Fairness review → cargo test → clippy → ×5 runs → SUCCESS-005 → freeze