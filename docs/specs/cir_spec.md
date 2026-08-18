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

## Stage Data Contract（P1-5B Formal / Phase 1 新增）

**代码锚点**: `crates/acos-compiler/src/contract.rs`（`validate_data_contract`，经 `validate_cir_semantic` 挂入编译期校验）
**设计**: `docs/specs/2026-08-18-stage-data-contract-design.md`

数据契约把"阶段间存在某个输出"从运行时假设前移到编译期校验：未解析绑定 / 契约违规在 Compile 期报错并进入 repair 循环，而不是运行到 Python 才崩溃（`NameError` / `KeyError` / `NoneType.strip` 属此层）。校验规则 R1–R5：

| 规则 | 内容 |
| --- | --- |
| R1 | Binding 存在性：所有 `${...}` 引用（input 值 + control 字段）必须解析到某个 producer 的 `output.name` 或 loop 的 `item_var`；未解析 → `UnresolvedBinding` |
| R2 | Producer ordering（结构可达性，非节点数组顺序）：Sequence 渐进（前序输出对后序可见）；Parallel 分支间不共享，但块完成后输出对后续可见；Conditional 分支内产生的绑定**不能**在节点外部假设存在 |
| R3 | Type alignment：声明了 `inputTypes` 的 input 必须与 producer `typeName` 严格一致（number/integer 互相宽松兼容）；未声明的只查 binding 存在 |
| R4 | Field path：只允许静态字段路径 `identifier.field.field`，字段必须存在于 producer `fields` 且类型兼容；含 `[`/`]` 的动态索引**拒绝**（Phase 2） |
| R5 | Output completeness：有输出必有完整 schema（`name` 与 `typeName` 均非空），拒绝 `typeName: ""` 等半合法状态 |

### `output` 结构体化

`output` 现在是 `OutputSpec { name, typeName, fields }`（`fields: Vec<FieldSpec { name, typeName }>`）。Rust 类型强制"有输出必有 schema"。新增 `inputTypes`（input key → 期望类型名，可选）。

### Loop 聚合输出类型

`loop_map.output`（聚合）类型必须是 **`List<T>`**，T = body **最后一个**声明 output 的 child 类型。例如 `all_results: List<ValidationResult>`：

- `${all_results.total_issues}` → **编译期 FAIL**（List 无字段）
- `${all_results[0].total_issues}` → Phase 1 不支持（Phase 2）

### 点路径（R4 细节与 Phase 1 边界）

`${a.b.c}` 是静态字段路径，按 producer 的 `fields` 表逐层校验。**Phase 1 边界：路径不下降**——字段表是平面的，`${a.b.c}` 第二层以 `a` 的顶层字段表校验。含 `[`/`]` 的动态索引被拒绝。

### R3/R4 与运行时一致性（Phase 1 简化，记录）

- R3 对点路径引用 `${a.b}` 按 producer 的**顶层** `typeName` 比对（不深入字段级类型）。
- loop `input` 引用在运行时（`resolve_ref_value`）Phase 1 不支持点路径解析——契约接受 `${x.y}` 作 loop input 时，运行时解析为 None 且**静默零次迭代**。实务上 loop input 应为列表绑定名（如 `${file_list}`）；正式评估以不写点路径的 loop input 为准。
- 容器节点（sequence / parallel / conditional）声明 `output` 被**拒绝**（运行时只为 primitive 与 loop 绑定输出，契约与运行时保持一致）。

### item_var 作用域

- `item_var` 在 loop body 内可见；loop 外引用 → unresolved。
- `item_var` 不能覆盖已存在的顶层 binding（遮蔽 → `DataContractViolation`）。
- 严格 shadowing / lexical scope 留 Phase 2。

### control 引用校验

control 中的引用（`loopSpec.input`、condition 表达式、retry 配置等）与 input 值**同样受 R1 校验**：dangling loop input 会在编译期报 `UnresolvedBinding`，不再拖到 runtime。

### 架构原则（记录，Phase 2 落地）

> Runtime values should cross stage boundaries as structured data, not source-code interpolation.

当前 `${all_results}` 通过字符串插值拼进 Python 源码；正确形态是 Binding Resolver → Typed Runtime Value → 结构化传递（stdin/JSON/env）。Phase 1 仅记录此原则；同时 Phase 1 不保证把绑定嵌入任意源码的语义/语法安全（代码生成正确性属 Phase 2 structured transport 职责）。
