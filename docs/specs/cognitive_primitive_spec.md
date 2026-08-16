# 认知原语规范 v0.1 / Cognitive Primitive Specification v0.1

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-core/src/traits.rs`（pending）
- **模式 / Schema**: `schemas/primitive/primitive.proto`
- **上次验证 / Last verified**: —

## 目的 / Purpose

认知原语（Cognitive Primitive）是 ACOS 愿意作为独立操作进行调度的、最小的可复用认知工作单元（smallest reusable unit of cognitive work）。

## 原语设计规则 / Primitive design rules

1. 一个有意义的认知/系统动作（One meaningful cognitive/system action）。
2. 有类型的输入和输出（Typed input and output）。
3. 显式效果（Explicit effects）。
4. 可独立测试（Independently testable）。
5. 可替换的实现（Replaceable implementation）。
6. 在可行时可衡量的结果（Measurable outcome where practical）。

## 清单示例 / Manifest example

```yaml
apiVersion: acos.io/v1
kind: CognitivePrimitive
metadata:
  id: summarize
  version: 1.0.0
spec:
  capability: information.summarization
  input: Document
  output: Summary
  effects: []
  compensations: []  # 与 effects 一一对应的逆操作
  resources:
    cpu: low
    network: false
  quality:
    measurable: true
  provider:
    runtime: process
    command: "acos-provider-summarize"
```

> **注**：`compensations` 字段与 `effects` 一一对应。每个声明的效果必须有补偿操作（详见 [执行模型 - 补偿机制](execution_model.md#补偿机制)）。不可逆效果标记为 `external.irreversible`，无需补偿但需审批。

## MVP 原语 / MVP primitives

### search
输入（Input）：`SearchQuery`
输出（Output）：`DocumentList`
效果（Effects）：network read（网络读取）

### read_file
输入：`FileRef`
输出：`Document`
效果：filesystem read（文件系统读取）

### write_file
输入：`ArtifactWriteRequest`
输出：`ArtifactRef`
效果：filesystem write（文件系统写入）

### execute_python
输入：`PythonExecutionRequest`
输出：`ExecutionResult`
效果：process execution（进程执行）；可选 filesystem/network（文件系统/网络）

### summarize
输入：`Document`
输出：`Summary`
效果：model inference（模型推理）

## 原语元数据 / Primitive metadata

必填字段（Required fields）：

- capability（能力）
- input schema（输入模式）
- output schema（输出模式）
- effect set（效果集合）
- runtime/provider identity（运行时/提供者身份）
- semantic version（语义版本）

推荐字段（Recommended fields）：

- expected latency（预期延迟）
- estimated cost（估算成本）
- reliability score（可靠性评分）
- resource requirements（资源需求）
- supported platforms（支持平台）
- deterministic/non-deterministic flag（确定性/非确定性标志）

## 提供者抽象 / Provider abstraction

多个实现可以满足同一个能力。编译器/运行时可以根据约束和历史性能选择提供者。

### 能力接缝三角色 / Capability Seam Three Roles

每个完整的能力接缝由三个角色组成（详见 [插件系统 - 能力接缝模型](plugin_system.md#能力接缝模型-capability-seam-model)）：

| 角色 | 说明 |
|---|---|
| **Service Definition** | 声明能力的接口契约：输入/输出模式、效果集、前置/后置条件 |
| **Service Provider** | 能力的具体实现（本规范的"提供者"） |
| **Consumer** | 使用能力的组件：原语、验证器、编译器 Pass |

> 添加新能力 = 设计完整的三个角色，而不仅仅是添加一个提供者函数。
