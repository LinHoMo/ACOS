# 认知中间表示（CIR）v0.1 / Cognitive Intermediate Representation (CIR) v0.1

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-core/src/cir.rs`（pending）
- **模式 / Schema**: `schemas/cir/cir.proto`
- **上次验证 / Last verified**: —

## 状态 / Status

实验性（Experimental）。CIR 是编译器产物（compiler artifact），尚未成为稳定的公共语言。

## 目的 / Purpose

CIR 桥接任务理解和可执行图生成。它必须是机器可检查的（machine-checkable），且语义上比任务 JSON 块更丰富。

## 最小概念集 / Minimum concepts

- 有类型的值（typed values）
- 原语调用（primitive invocation）
- 序列（sequence）
- 并行（parallel）
- 条件（conditional）
- 循环/映射（loop/map）
- 重试（retry）
- 检查点（checkpoint）
- 验证义务（verification obligation）
- 效果声明（effect declaration）
- 工件引用（artifact reference）

## 示例 / Example

```yaml
program:
  type: sequence
  steps:
    - op: read_file
      args:
        path: sales.csv
      out: raw_data
    - op: execute_python
      args:
        code_ref: clean_and_analyze.py
        input: raw_data
      out: analysis
    - op: summarize
      args:
        document: analysis
      out: report
```

## 类型系统基线 / Type system baseline

首个实现应支持：

- 基本标量类型（primitive scalar types）
- 记录/结构体（records/structs）
- 列表（lists）
- 可选类型（optionals）
- 结果/错误联合类型（result/error unions）
- 名词性语义类型（nominal semantic types）
- 显式转换运算符（explicit conversion operators）

子类型化（subtyping）和 trait 风格的能力约束是未来扩展。

## 效果 / Effects

效果是 CIR 语义的一部分。示例：

- `fs.read`
- `fs.write`
- `network.read`
- `network.write`
- `process.spawn`
- `secret.read`
- `external.irreversible`

## 图语义 / Graph semantics

数据依赖（Data dependencies）是显式的。控制依赖（Control dependencies）是显式的。证据（Evidence）是一等的值/义务（first-class value/obligation），而不是特殊的边类型。
