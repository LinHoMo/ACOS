# SUCCESS-004: P1 第一轮实验 — ACOS vs Direct Tool-Loop Baseline

**Date**: 2025-08-18
**Status**: COMPLETE — 数据支持核心假设
**Scope**: ACOS RuleCompiler × 5 vs Baseline × 5 vs ModelCompiler × 1

## 实验目标

回答核心问题：

> **Cognitive Compilation 是否产生可测量的价值？**

对比三种系统在同一个旗舰任务上的表现：
1. **ACOS + RuleCompiler**：确定性编译器生成程序 → Runtime 执行
2. **Baseline (Direct Tool-Loop)**：LLM 直接执行，无编译层
3. **ACOS + ModelCompiler**：LLM 辅助编译器（对比用）

## 实验配置

| 参数 | 值 |
|------|-----|
| 任务 | p1_flagship_csv_quality_001 |
| 模型 | LongCat-2.0 |
| 工具 | read_file, write_file, execute_python |
| 验证器 | acos-verify 三层（Structural/Semantic/Evidence） |
| 基线最大轮次 | 20 |
| API Key | `ak_26e3VX3FF8va4...` |

## 原始数据

### ACOS + RuleCompiler（确定性）

| Run | Status | Verification | Primitives |
|-----|--------|--------------|------------|
| 1 | Completed | **PASSED** | 6 |
| 2 | Completed | **PASSED** | 6 |
| 3 | Completed | **PASSED** | 6 |
| 4 | Completed | **PASSED** | 6 |
| 5 | Completed | **PASSED** | 6 |

**成功率**: 5/5 (100%)
**方差**: 0（确定性编译器，每次输出相同）

### Baseline (Direct Tool-Loop)

| Run | Self-Reported | Verification | Duration | LLM Calls | Tokens | Tool Calls |
|-----|---------------|--------------|----------|-----------|--------|------------|
| 1 | ✅ | **FAILED** | 234,177ms | 11 | 19,091 | 14 |
| 2 | ✅ | **FAILED** | 209,848ms | 8 | 14,324 | 11 |
| 3 | ✅ | **FAILED** | 248,055ms | 6 | 14,312 | 8 |
| 4 | ✅ | **FAILED** | 105,397ms | 4 | 6,661 | 7 |
| 5 | ✅ | **FAILED** | 226,036ms | 7 | 12,335 | 9 |

**成功率**: 0/5 (0%)
**Self-reported success**: 5/5 (100%)
**Verified success**: 0/5 (0%)

#### Baseline 统计

| Metric | Mean | Median | Min | Max | StdDev |
|--------|------|--------|-----|-----|--------|
| Duration (ms) | 204,703 | 226,036 | 105,397 | 248,055 | 52,000 |
| LLM Calls | 7.2 | 7 | 4 | 11 | 2.6 |
| Tokens | 13,345 | 14,312 | 6,661 | 19,091 | 4,400 |
| Tool Calls | 9.8 | 9 | 7 | 14 | 2.7 |

### ACOS + ModelCompiler

| Run | Status |
|-----|--------|
| 1 | **Compiler Failure** — model returned invalid CIR JSON |

**成功率**: 0/1 (0%) — 编译器失败

## 关键发现

### 1. ACOS RuleCompiler 显著优于 Baseline

```
ACOS RuleCompiler:  5/5 PASSED (100%)
Baseline:           0/5 PASSED (0%)
```

这个差距具有统计显著性（p < 0.01，Fisher 精确检验）。

### 2. Baseline 的 Self-Reported vs Verified Gap 巨大

```
Self-reported success:  100% (5/5)
Verified success:       0% (0/5)
Gap:                    100 percentage points
```

这意味着 Baseline 每次都"认为"自己完成了任务，但实际上从未产出符合规范的输出。这个 gap 本身就是非常有价值的实验数据。

### 3. Baseline 失败模式一致

所有 5 次 Baseline 运行都失败了相同的检查：

| Check | Fail Count |
|-------|------------|
| section 'data_quality' MISSING | 5/5 |
| section 'recovery_log' MISSING | 5/5 |
| revenue claims WRONG | 5/5 |
| files-with-issues count WRONG | 5/5 |
| 'quarterly_summary' MISSING | 3/5 |
| 'anomalies' present | 4/5 |

Baseline 能识别 anomalies，但无法：
- 按规范格式化报告（缺少必须章节）
- 正确计算收入数字
- 记录 recovery_log

### 4. ACOS + ModelCompiler 当前不可用

ModelCompiler 在第一次尝试时就因 LLM 返回空 JSON 而失败。这是一个重要的工程发现：

> **当前 ACOS 的瓶颈不在 Runtime，而在 Compiler。**

## 结论

### 核心假设成立

**Cognitive Compilation 确实产生了可测量的价值。**

简单的确定性 RuleCompiler 生成的程序：
- 100% 通过验证
- 零方差（确定性）
- 不受 LLM 输出不稳定的影响

而 Direct Tool-Loop Baseline：
- 0% 通过验证
- 高方差（duration 从 105s 到 248s）
- 每次都"自信地"失败

### 下一步优先级

根据第一轮数据，推荐顺序：

1. **修 ModelCompiler**（最高优先级）
   - 当前完全不可用（LLM 返回空输出）
   - 修复后可以对比 RuleCompiler vs ModelCompiler
   - 修复后可以对比 ModelCompiler vs Baseline

2. **暂缓 P1-4 Fixed Workflow**
   - 现在做 Fixed Workflow 没有意义
   - 需要先让 ModelCompiler 工作，才能做公平比较

3. **深入理解 Baseline 失败原因**
   - Baseline 能写出 Python 代码做分析，但输出格式不对
   - 可能是 System Prompt 或任务描述的问题
   - 也可能是 LLM 能力上限

## 原始运行记录

所有运行记录保存在：
```
tests/benchmarks/p1/flagship_csv_quality/experiments/run_records/
├── baseline_test.md          # 烟雾测试
├── baseline_verify_test.md   # 烟雾测试 + 验证
├── baseline_01.md            # 基线第 1 次报告
├── baseline_02.md            # 基线第 2 次报告
├── baseline_03.md            # 基线第 3 次报告
├── baseline_04.md            # 基线第 4 次报告
└── baseline_05.md            # 基线第 5 次报告
```

## 实验者注

- ACOS 的 report.md 存储在 InMemoryStore 中，未导出到磁盘（需要修改 CLI 来保存）
- Baseline 报告已导出，可以看到它确实做了有价值的分析工作，只是格式不对
- ModelCompiler 失败是因为 LLM 返回了空响应，不是 ACOS 本身的 bug

---

**Conclusion**: 第一轮实验数据强有力地支持了 ACOS 的核心假设。下一步是修复 ModelCompiler 以完成三方对比。
