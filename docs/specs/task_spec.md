# 认知任务规范 v0.1 / Cognitive Task Specification v0.1

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-core/src/types.rs`（pending）
- **模式 / Schema**: `schemas/task/task.proto`
- **上次验证 / Last verified**: —

任务规范（task specification）是编译器的稳定前端输入（stable front-end input）。

## 推荐结构 / Recommended shape

```yaml
apiVersion: acos.io/v1
kind: CognitiveTask
metadata:
  id: example-task
spec:
  goal: "Analyze sales.csv and generate a quarterly report"
  inputs:
    - type: File
      path: ./sales.csv
      format: csv
  outputs:
    - type: Report
      format: markdown
  constraints:
    timeout_seconds: 300
    max_cost: 5.0
    allowed_network: false
  optimization:
    primary: reliability
    secondary: cost
  approval:
    external_side_effects: required
```

## 为什么采用混合输入 / Why hybrid input

自然语言仍然是面向用户的目标表示，但约束、输入、输出、预算和权限应该是结构化的。这在不强迫用户学习完整编程语言的情况下限制了歧义。

## 验证 / Validation

任务编译器验证：

- 必填字段（required fields）
- 输入存在性（input existence）
- 输出可行性（output feasibility）
- 平台约束（platform constraints）
- 权限兼容性（permission compatibility）
- 预算有效性（budget validity）

自然语言 `goal` 由模型辅助前端（model-assisted front end）解释，但其输出必须根据结构化规范进行验证。
