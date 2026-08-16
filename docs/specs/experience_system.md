# 经验系统 / Experience System

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-state/src/experience.rs`（pending）
- **模式 / Schema**: `schemas/events/experience.proto`
- **上次验证 / Last verified**: —

## 目的 / Purpose

将完成的运行转化为结构化证据，用于未来的编译器决策，而无需存储每个内部推理步骤。

## 经验记录 / Experience Record

```json
{
  "task_signature": "...",
  "program_hash": "...",
  "capabilities": ["..."],
  "outcome": "success",
  "metrics": {
    "latency_ms": 120000,
    "cost": 0.75,
    "quality": 0.92
  },
  "failures": [],
  "lessons": ["provider X was more reliable for this input"]
}
```

## 用途 / Uses

经验可以改善：

- 能力/提供者排名（capability/provider ranking）
- 估算成本和延迟（estimated cost and latency）
- 模板选择（template selection）
- 方法选择（method selection）
- 重试/回退策略（retry/fallback strategies）

## 安全 / Safety

经验不得自动重写核心运行时语义。它应在有界策略约束内影响排名和规划。
