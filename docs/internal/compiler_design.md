# 认知编译器设计 / Cognitive Compiler Design

## 编译器流水线 / Compiler pipeline

```text
Task Specification（任务规范）
  ↓
Intent Analysis（意图分析）
  ↓
Task Model（任务模型）
  ↓
Capability Resolution（能力解析）
  ↓
Hybrid Planning（混合规划）
  ↓
CIR Synthesis（CIR 综合）
  ↓
Static Validation（静态验证）
  ↓
Optimization（优化）
  ↓
Execution Graph（执行图）
```

## 规划策略 / Planning strategy

ACOS 使用混合策略（hybrid strategy）：

1. 模型辅助分解（Model-assisted decomposition）以推断候选子任务。
2. 受约束的规划器（constrained planner）检查前置条件、效果和依赖。
3. 能力解析（Capability resolution）将抽象动作绑定到具体原语/提供者。
4. 优化（Optimization）以满足用户优先级和预算。

## 能力匹配 / Capability matching

三级解析器（Three-stage resolver）：

1. 精确能力/契约匹配（exact capability/contract match）；
2. 本体/分类匹配（ontology/taxonomy match）；
3. 模型辅助语义匹配（model-assisted semantic match）。

模型是提议者（proposer），不是最终权威。选定的原语必须通过契约、效果和资源验证。

## 方法来源问题 / Method source problem

规划方法（Planning methods）来源于：

- 内置领域方法（built-in domain methods）
- 任务模板（task templates）
- 已验证的历史经验（validated historical experience）
- 模型提议的候选方法（model-proposed candidate methods）

模型提议的方法必须在执行前验证。

## 优化 / Optimization

默认目标顺序（Default objective order）：

1. 满足硬约束（satisfy hard constraints）；
2. 最大化任务成功可能性（maximize task success likelihood）；
3. 尊重预算（respect budget）；
4. 最小化延迟（minimize latency）。

用户优先级可以覆盖次要目标。

## 编译器输出 / Compiler outputs

编译器发出：

- CIR
- 已解析的能力/提供者（resolved capabilities/providers）
- 执行图（execution graph）
- 所需权限（required permissions）
- 估算成本/延迟（estimated cost/latency）
- 验证义务（verification obligations）
