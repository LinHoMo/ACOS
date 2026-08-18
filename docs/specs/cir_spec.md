# 认知中间表示（CIR）v0.1 / Cognitive Intermediate Representation (CIR) v0.1

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-core/src/types.rs`（`CirProgram` / `CirNode` / `ControlSpec` / `LoopSpec` / `RetryPolicy`）
- **模式 / Schema**: `schemas/cir/cir.proto`
- **上次验证 / Last verified**: 2026-08-17 (P0 控制语义)

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

## CirNode.control（P0 新增）

控制语义从业务 `inputs` 中分离，挂在节点的 `control` 字段上。校验由 `acos_compiler::validate_cir` 在编译期完成。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `condition` | `ConditionSpec` | `conditional` 节点的分支条件（acos-expr 安全子集） |
| `loop_spec` | `LoopSpec` | `loop_map` 节点的循环语义 |
| `retry` | `RetryPolicy` | 任意可执行节点的失败重试（需 retry-safe） |
| `else_children` | `[]string` | `conditional` 节点的假分支（真分支为 `children`） |

语义规则：

- `conditional` 节点必须有 `control.condition`，且其表达式中引用的标识符必须在程序已声明的 `output` 中。
- `loop_map` 节点必须有 `control.loop_spec`：
  - `while`：先求值条件再执行；`until`：先执行再求值（避免 off-by-one）。
  - `for_each`：以数组长度自然终止；`while`/`until` **必须**显式提供 `max_iterations >= 1`（终止性保证）。
  - `max_iterations == 0` 在编译期被拒绝。
- `loop_map` 节点的 `output` 语义（P1-5B Probe-2c 补充）：每次迭代结束后，取**最后一个 child** 声明的 `output` 绑定值收集为数组；循环结束后绑定到该 `output` 名（无迭代时为空数组）。下游节点通过 `"${loop_output}"` 引用聚合结果。
- `retry` 仅对暂态类（`timeout` / `rate_limit` / `transient`）生效，且节点原语必须 retry-safe
  （`idempotent()` 为真，或所有效果为纯读）；`ExternalIrreversible` 效果**禁止**重试。
  请求在非 retry-safe 原语上重试会在编译期被拒绝（提示 `irreversible`）。
- 未声明 `kind` 的节点（如 `primitive_invocation`）若携带 `control.loop_spec` / `control.condition` 会被忽略——
  控制语义只在 `loop_map` / `conditional` 节点上生效。

### 序列化约定（camelCase）

CIR 使用 `serde(rename_all = "camelCase")`，注意转换规则只作用于下划线后的字母：

- 程序 id 字段为 `id`（不是 `programId`）；`task_id` → `taskId`，`node_id` → `nodeId`。
- 控制字段：`control.condition.expression`、`control.loopSpec`（`kind` / `condition` / `maxIterations` / `input` / `itemVar`）、
  `control.retry`（`maxAttempts` / `backoffMs` / `strategy` / `retryOn`）、`elseChildren`。

### 恢复（Recovery）

运行时 `execute_with_recovery` 在失败时依次尝试：

1. `rule` 重规划器（`RuleReplanner` + `OfflineFallbackRule`）：匹配暂态类失败，将失败节点替换为 `read_file` 等本地回退。
2. `model` 重规划器（`ModelRecoveryPlanner`）：仅在配置了 LLM key 时可用，处理无法用规则修复的失败（如 `Unknown`）。

每次重规划提交为事务式补丁（`replan.started` / `replan.completed` / `replan.rejected`），补丁根节点必须复用被替换节点的 `node_id`。
`bench` 套件以 fixture 为契约验证上述行为（见 `crates/acos-bench/fixtures/`）。
