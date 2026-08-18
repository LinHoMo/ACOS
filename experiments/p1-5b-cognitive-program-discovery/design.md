# P1-5B: Cognitive Program Discovery v0.1

**Date**: 2026-08-18
**Status**: DESIGNED — 待执行
**Depends**: P1-5A (FROZEN), P1-R1 (FROZEN)
**Core Research Question**: Proposition B

## 核心研究命题

### Proposition A (已初步验证)
> 结构化程序 + 可靠执行 + 外部验证 > 直接 Tool Loop

P1-R1 数据支持此命题：5/5 vs 0/5。

### Proposition B (P1-5B 目标)
> 机器能否从任务目标中自动发现满足约束的 Cognitive Program？

这不是测试"模型能否输出 JSON"（P1-5A 已验证），而是测试：
- 模型能否理解任务语义
- 模型能否选择合适的程序结构
- 模型生成的程序能否通过 Ground Truth 验证

## 实验原则

### 1. 不要求复制 Golden CIR

Golden CIR (8 节点) 只是**一种**正确实现。模型可以自由选择：
- ForEach vs Parallel vs Sequential
- Conditional + Repair vs 直接 execute_python
- 不同的语义等价实现

判断标准是 **Task Adequacy**，不是 **Structural Equality**。

### 2. 不给模型"答案提示"

输入只包含：
- Task Specification（目标、输入、输出约束）
- Capability Registry（可用原语及其 contract）
- CIR Schema（格式约束）

**禁止**在 Prompt 中出现：
- "使用 ForEach"
- "添加 data_quality 章节"
- "使用 retry"
- 任何旗舰任务的具体业务流程描述

### 3. Capability Registry 提供"能力"，不是"工作流"

允许：
- `read_file`, `write_file`, `execute_python`, `summarize`, `search`

禁止：
- `csv_quality_analysis`（任务专用业务原语）
- `analyze_dataset`
- `repair_csv`

这些把整个任务封装成一个 Primitive，会使 Program Discovery 退化为"选择预定义工作流"。

## 三层正确性模型

| Level | 名称 | 判断方式 | P1-5A 覆盖 |
|-------|------|----------|-----------|
| L1 | Structural Validity | CIR schema / reference / capability / control 校验 | ✅ |
| L2 | Executability | Runtime 执行完毕（无死循环、无不可执行节点） | ❌ 本轮验证 |
| L3 | Task Adequacy | 输出通过 Ground Truth Oracle | ❌ 本轮验证 |

**只有三层全部通过，才算真正的 Program Discovery 成功。**

## 实验设计

### 任务：P1-FLAGSHIP-001

使用与 P1-R1 完全相同的旗舰任务：
- 4 个 CSV 文件（含不同扰动模式）
- 期望输出：`data_quality`, `recovery_log`, `quarterly_summary`, `anomalies`, `revenue_by_category`
- Ground Truth 数值（`expected/ground_truth.yaml`）

### 运行配置

| 参数 | 值 |
|------|-----|
| 模型 | LongCat-2.0 |
| 任务 | P1-FLAGSHIP-001 |
| 验证器 | acos-baseline 三层（Structural/Semantic/Evidence） |
| 运行次数 | 3（Discovery Probe） |
| 每次保存 | 完整 trace（见下） |
| API Key | LONGCAT_API_KEY (from .env) |

### 每次运行保存的数据

```yaml
run:
  run_id: uuid
  timestamp: ISO8601
  model: LongCat-2.0

input:
  task_spec: acos_task.yaml
  prompt_sent: <full prompt text>

output:
  initial_raw_response: <model's first response verbatim>
  initial_parse_error: <error type or null>
  repair_count: <0-3>
  repair_traces:
    - attempt: 1
      raw_response: <model response>
      parse_error: <error type or null>
      validation_error: <error or null>
  final_cir: <complete CIR JSON>
  compile_success: boolean

execution:
  execution_success: boolean
  execution_error: <or null>
  artifacts: <list of produced files>

verification:
  structural: boolean
  semantic: boolean
  evidence: boolean
  overall: boolean

program_metrics:
  node_count: int
  primitive_count: int
  control_node_count: int
  loop_count: int
  condition_count: int
  retry_count: int
  capability_types: <list>

timing:
  first_llm_call_ms: int
  total_compile_ms: int
  execution_ms: int
  verification_ms: int
  total_wall_ms: int

repair_tax:
  first_pass_success: boolean
  repair_attempts_used: int
  repair_latency_ms: int
```

## Behavioral Requirements Matrix

旗舰任务的行为要求（不是结构要求）：

```yaml
behavioral_requirements:

  multi_file_processing:
    description: "处理所有输入文件（4 个 CSV）"
    check: "所有 4 个文件都被读取"
    required: true

  data_quality_analysis:
    description: "检测数据质量问题（缺失值、重复、格式不一致）"
    check: "data_quality 章节存在且非空"
    required: true

  anomaly_detection:
    description: "识别异常值（负数量、极端收入等）"
    check: "anomalies 章节存在且包含具体异常"
    required: true

  repair_or_recovery:
    description: "执行修复或恢复操作"
    check: "recovery_log 章节存在且记录修复动作"
    required: true

  structured_report:
    description: "生成结构化报告（含必需章节）"
    check: "所有必需章节存在: data_quality, recovery_log, quarterly_summary, anomalies, revenue_by_category"
    required: true

  ground_truth_accuracy:
    description: "数值声明与 Ground Truth 一致"
    check: "revenue 数值误差 < 1%"
    required: true

  evidence:
    description: "提供可审计的证据链"
    check: "事件日志完整，可追溯每个输出"
    required: true
```

## 成功标准（三级）

### Discovery Success（完全成功）
- L1: CIR valid ✅
- L2: Execution completed ✅
- L3: 所有 7 个 Behavioral Requirements 满足 ✅

### Discovery Partial（部分成功）
- L1: CIR valid ✅
- L2: Execution completed ✅
- L3: 至少 4/7 Behavioral Requirements 满足 ⚠️

### Discovery Failure（失败）
- L1: Compile failed ❌
- 或 L2: Execution failed ❌
- 或 L3: < 4/7 Behavioral Requirements ❌

## 验收线

### 通过条件（P1-5B 第一轮）
- 至少 1/3 runs 达到 Discovery Success
- 或 2/3 runs 达到 Discovery Partial+
- 所有 runs 无 panic / 无限重试 / 绕过 Validator

### 如果 0/3 成功
- 分析失败模式（compile vs execution vs verification）
- 判断是 Prompt 问题、模型能力问题、还是 Runtime 问题
- 决定是否需要调整 ModelCompiler 的 system prompt

## 后续实验路线

```text
P1-5B Discovery Probe (3 runs)
        ↓
   结果分析
        ↓
   ┌────┴────┐
   │         │
 ≥1/3 成功   0/3 成功
   │         │
   ↓         ↓
 正式实验   诊断修复
(5 runs)   (调整 prompt /
   │        修复 Runtime)
   ↓         ↓
 三方比较 ←──┘
(Rule / Model / Baseline)
   │
   ↓
+ Fixed Workflow
   │
   ↓
 完整四方实验
```

## 文件结构

```
experiments/p1-5b-cognitive-program-discovery/
├── design.md                          # 本文件
├── probe-results/                     # 每次运行的原始结果
│   ├── run-001.md
│   ├── run-002.md
│   └── run-003.md
├── analysis.md                        # 结果分析
└── behavioral-requirements.yaml        # 行为要求定义
```
