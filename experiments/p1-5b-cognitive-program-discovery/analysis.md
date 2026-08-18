# P1-5B Discovery Probe 分析报告

**日期**：2026-08-18
**实验**：P1-5B Cognitive Program Discovery v0.1 — Discovery Probe（3 runs）
**模型**：LongCat-2.0
**编译器**：`ModelCompiler::compile_traced`（P1-5A 修复后，max_repair=3）
**任务**：`tests/benchmarks/p1/flagship_csv_quality/acos_task.yaml`

---

## 1. 核心结论

**命题 B 的 Discovery Probe 结果：否定。**

在当前 system prompt 下，ModelCompiler 无法从任务目标自动发现满足任务约束的 Cognitive Program。3 次运行全部编译成功（L1 ✓），但全部执行失败（L2 ✗），无一次达到任务 adequacy（L3 ✗）。

这不是编译器的失败——是**任务理解**的失败。模型生成的 CIR 在语法上完全正确，但在语义上与任务目标严重脱节。

---

## 2. 数据总览

| 指标 | Run 1 | Run 2 | Run 3 | 汇总 |
|------|-------|-------|-------|------|
| **编译成功** | ✓ | ✓ | ✓ | **3/3 (100%)** |
| **首次通过** | ✗ (EOF) | ✗ (EOF) | ✗ (EOF) | **0/3 (0%)** |
| **Repair 次数** | 1 | 1 | 1 | 3/3 成功修复 |
| **编译延迟** | 110.6s | 143.9s | 106.6s | **~120s 均值** |
| **执行成功** | ✗ | ✗ | ✗ | **0/3 (0%)** |
| **验证通过** | — | — | — | **0/3** |
| **节点数** | 3 | 3 | 2 | 2-3 节点 |
| **原语数** | 2 | 3 | 1 | 1-3 原语 |
| **循环数** | 0 | 0 | 0 | **0** |
| **条件数** | 0 | 0 | 0 | **0** |
| **重试数** | 0 | 0 | 0 | **0** |

---

## 3. 生成的 CIR 结构分析

### Run 1 — 2 原语（read_file → write_file）

```
sequence(root)
  ├── primitive_invocation(step_0): read_file { path: "/tmp/data/input.txt" } → raw_0
  └── primitive_invocation(step_1): write_file { path: "output/report.md", content: "${raw_0}" } → report_ref
```

**问题**：
- 路径 `/tmp/data/input.txt` 是**幻觉**——TaskSpec 明确列出 4 个 CSV 文件在 `tests/benchmarks/.../datasets/`
- 无循环——4 个文件需要 `for_each` 或 `loop_map`
- 无数据质量检测、修复、统计、合并步骤
- 直接把输入原样写入输出（identity pipeline）

### Run 2 — 3 原语（read_file → summarize → write_file）

```
sequence(root)
  ├── primitive_invocation(step_0): read_file { path: "/tmp/input.txt" } → raw_doc
  ├── primitive_invocation(step_1): summarize { document: "${raw_doc}" } → summary
  └── primitive_invocation(step_2): write_file { path: "/tmp/report.md", content: "${summary}" } → report_ref
```

**问题**：
- 路径 `/tmp/input.txt` 同样是幻觉
- 使用了 `summarize` 但只是简单总结，不是数据分析
- 仍然无循环、无质量检测、无统计

### Run 3 — 1 原语（read_file only）

```
sequence(root)
  └── primitive_invocation(step_0): read_file { path: "/tmp/data/a.txt" } → raw_0
```

**问题**：
- 最简形式——只读一个文件，无输出
- 路径 `/tmp/data/a.txt` 是幻觉
- 连 write 都没有，不满足"生成报告"的基本要求

---

## 4. 失败模式分类

### 4.1 L1 Structural Validity — ✓ PASS

所有 3 次运行在 repair 后都生成了**语法正确的 CIR**。编译器前端的修复机制（P1-5A）按预期工作：首次 EOF → repair → 有效 JSON → 通过 schema 和语义校验。

### 4.2 L2 Executability — ✗ FAIL (3/3)

所有 3 个 CIR 在运行时第一步就失败了——`read_file` 原语尝试读取不存在的路径。

| Run | 尝试读取的路径 | 实际可用的路径 |
|-----|---------------|---------------|
| 1 | `/tmp/data/input.txt` | `tests/benchmarks/p1/flagship_csv_quality/datasets/sales_q{1,2,3,4}.csv` |
| 2 | `/tmp/input.txt` | 同上 |
| 3 | `/tmp/data/a.txt` | 同上 |

**根因**：模型完全忽略了 TaskSpec 中 `inputs` 字段列出的真实文件路径，生成了自己"想象"的路径。

### 4.3 L3 Task Adequacy — ✗ FAIL (3/3)

即使路径正确，生成的 CIR 也**严重不足**：

| 任务要求 | Run 1 | Run 2 | Run 3 |
|---------|-------|-------|------|
| 多文件处理（4 个 CSV） | ✗ | ✗ | ✗ |
| 数据质量检测 | ✗ | ✗ | ✗ |
| 修复可恢复问题 | ✗ | ✗ | ✗ |
| 重新验证 | ✗ | ✗ | ✗ |
| 计算季度统计 | ✗ | ✗ | ✗ |
| 合并结果 | ✗ | ✗ | ✗ |
| 质量审查 | ✗ | ✗ | ✗ |
| 生成 Markdown 报告 | ✗ | ✗ | ✗ |
| 证据日志 | ✗ | ✗ | ✗ |

---

## 5. 关键发现

### 发现 1：首次响应 100% 为空（EOF）

3/3 次运行的首次 LLM 调用返回空响应（EOF），全部依赖 repair 才成功。这与 P1-5A smoke test 的 2/3 EOF 率一致，确认了 LongCat-2.0 API 的稳定性问题。

**影响**：每次编译都多消耗一次 LLM 调用（~16-47s 额外延迟），但不影响最终结果。

### 发现 2：模型"假装理解"任务

模型生成的 CIR 看起来像一个"合理的程序"（有 sequence、有 read/write），但实际上**完全忽略了任务的具体内容**。它似乎在使用一个"通用模板"：

```
read something → maybe transform → write output
```

而不是真正解析 TaskSpec 中的 goal 和 inputs。

### 发现 3：幻觉路径是系统性问题

3/3 次运行都生成了 `/tmp/...` 路径，这不是随机错误——模型在"猜测"输入文件的位置，而不是从 TaskSpec 中提取。

### 发现 4：任务复杂度被完全忽略

TaskSpec 的 goal 包含 8 个明确步骤（detect → repair → revalidate → compute → merge → review → report → evidence log），但生成的 CIR 最多只有 3 个节点，且没有任何控制流（无 loop、无 conditional）。

---

## 6. 与 Golden CIR 的对比（参考，非评分标准）

| 维度 | Golden CIR | Run 1 | Run 2 | Run 3 |
|------|-----------|-------|-------|------|
| 节点数 | 8 | 3 | 3 | 2 |
| 原语数 | 6 | 2 | 3 | 1 |
| 循环 | 1 (forEach) | 0 | 0 | 0 |
| 条件 | 1 | 0 | 0 | 0 |
| 重试 | 1 | 0 | 0 | 0 |
| 使用真实路径 | ✓ | ✗ | ✗ | ✗ |

这不是说"生成的 CIR 必须和 Golden CIR 一样"——但 Golden CIR 展示了任务实际需要的复杂度，而生成的 CIR 完全不在同一个量级。

---

## 7. 根因分析

### 7.1 System Prompt 的问题

当前 system prompt 只告诉模型：
- "Compile the following ACOS task into a CIR execution graph"
- 给出 TaskSpec JSON
- 给出 CIR schema 和 capability registry

**缺失的关键信息**：
1. **没有明确指示使用 TaskSpec.inputs 中的路径**——模型不知道这些路径是真实可用的
2. **没有强调 goal 中的每个步骤都应在 CIR 中体现**——模型可以"自由发挥"地简化
3. **没有给出"输入文件应被循环处理"的提示**——模型不知道需要 for_each

### 7.2 模型能力问题

LongCat-2.0 可能在"从复杂任务描述中提取结构化执行计划"方面能力有限。它倾向于生成"看起来合理但实际空洞"的程序——这是 Completion Illusion 的另一种表现。

---

## 8. 下一步建议

### 方案 A：增强 System Prompt（推荐，先试这个）

在 system prompt 中加入：

```
CRITICAL RULES:
1. Use the EXACT file paths from taskSpec.inputs — do NOT invent paths
2. For each input file, create a loop_map or for_each node
3. The CIR must reflect ALL steps mentioned in the goal
4. Include data validation, repair, and quality analysis nodes
5. Generate a final report node that writes to the specified output path
```

### 方案 B：提供 Few-shot Example

在 prompt 中放一个简化版的"正确 CIR"示例（不是 Golden CIR，而是一个通用的"如何处理多文件分析任务"的模板）。

### 方案 C：降级到 RuleCompiler

如果 ModelCompiler 在 prompt 增强后仍然无法生成合理 CIR，则 P1-5B 的结论为"当前 LLM 能力不足以自动发现复杂认知程序"，需要依赖 RuleCompiler 或人工辅助。

---

## 9. 命题 B 的当前状态

| 标准 | 结果 |
|------|------|
| L1 Structural Validity | ✓ 100% (3/3) |
| L2 Executability | ✗ 0% (0/3) |
| L3 Task Adequacy | ✗ 0% (0/3) |
| Behavioral Requirements (7 项) | 0/7 满足 |

**结论**：命题 B 在**当前 system prompt 下不成立**。但这是 prompt 问题还是模型能力问题，需要进一步实验（方案 A/B）才能确定。

---

## 10. 原始数据

- `probe-results/run-001.trace.json` — 完整 LLM trace（prompt + raw response + repair + CIR）
- `probe-results/run-002.trace.json`
- `probe-results/run-003.trace.json`

所有原始 LLM 输出已保存，可审计。
