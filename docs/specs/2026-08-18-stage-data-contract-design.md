# Stage Data Contract — Phase 1 设计

**日期**: 2026-08-18
**状态**: APPROVED（用户拍板，含 7 点修正）
**相关实验**: P1-5B Probe-2（`experiments/p1-5b-cognitive-program-discovery/probe-2-analysis.md`）

## 背景与动机

P1-5B Probe-2 的残余失败集中在一类问题：CIR 层认为某个输出存在，但下游阶段没有一个形式化、可验证的契约保证它存在，且生成代码自行猜测数据结构：

- `NameError: processed_data`（引用未声明绑定）
- `KeyError: total_issues`（阶段间 key 名不一致）
- `NoneType.strip`（生成代码假设不存在的列）

这类失败目前只在 Runtime 暴露。**本设计把数据契约前移到编译期**：CIR 层成为带数据契约的程序，未解析绑定 / 契约违规在 Compile 期报错并进入 repair 循环，而不是运行到 Python 才崩溃。

**正式实验含义**：P1-5B Formal Evaluation 的指标升级为四层：

```
Discovery Success = Compile PASS ∧ Contract PASS ∧ Execute PASS ∧ Adequacy PASS
```

Contract 失败与 Execute 失败分开记录，不再把 Python 错误归因于"模型不会规划"。

## 架构原则（记录，Phase 2 落地）

> **Runtime values should cross stage boundaries as structured data, not source-code interpolation.**

当前 `${all_results}` 通过字符串插值拼进 Python 源码；正确形态是 Binding Resolver → Typed Runtime Value → 结构化传递（stdin/JSON/env）。Phase 1 不实现此原则，仅记录。

## Phase 1 范围

**做**：CIR schema 扩展（强制 schema）、编译期契约检查器（R1–R5）、runtime 点路径解析、探针 Contract 层。
**不做**：类型推导、dependent type、Hindley–Milner、Python AST 类型分析、跨语言 ABI、Python 结构化传递（Phase 2）、动态索引/复杂表达式（`${a[dynamic]}`、`${a[0].b}`）。

## CIR schema 变更（`acos-core/src/types.rs`）

### `output` 结构体化（破坏性，一次性迁移）

```rust
pub struct OutputSpec {
    pub name: String,       // 原 output 名字
    pub type_name: String,  // 如 CsvAnalysisResult / List<CsvAnalysisResult>
    pub fields: Vec<FieldSpec>,
}

pub struct FieldSpec {
    pub name: String,
    pub type_name: String,  // Number | Integer | String | Boolean | List | Record | Any
}
```

- `node.output: Option<String>` → `Option<OutputSpec>`。Rust 类型强制"有输出必有 schema"。
- **拒绝** `output` + `output_schema` 双字段方案（会出现 `output="foo"`, schema=None 或不一致）。
- `FieldSpec` 必须自带 `type_name`——否则 R4 无法做 `${validation_result.total_issues}` 的字段类型兼容检查。
- 新增 `node.input_types: HashMap<String, String>`（input key → 期望类型名，可选；不声明则只查 binding 存在）。
- `LoopSpec.item_var` 保留；loop 聚合类型规则见 §Loop 聚合。

### Loop 聚合输出类型（必须明确）

- 单轮输出：`validation_result: ValidationResult`。
- `loop_map.output`（聚合）类型：**`List<T>`**，其中 T = 每轮最后一个 child 的 output 类型。
- 因此 `${all_results}` 类型为 `List<ValidationResult>`：
  - `${all_results.total_issues}` → **编译期 FAIL**（List 无字段）
  - `${all_results[0].total_issues}` → Phase 1 不支持（Phase 2）
- 此规则写入 `docs/specs/cir_spec.md`。

### 迁移清单

- `tests/benchmarks/p1/flagship_csv_quality/golden_cir.json`（8 节点）
- `acos-compiler` / `acos-runtime` / `acos-bench` 所有构造 `CirNode` 的测试 fixture
- 验收：迁移后全 workspace 测试保持绿

## 契约检查器（`acos-compiler/src/contract.rs`，挂入 `validate_cir_semantic`）

新增错误变体：

```rust
UnresolvedBinding { node_id: String, binding: String }
DataContractViolation { node_id: String, message: String }
```

两者都进 repair 循环（模型收到具体契约错误文本）。

### R1 Binding 存在性

扫描所有 input 值（含 `execute_python` 的 code 字符串内嵌 `${ref}`）+ control（loop input / condition / item_var）中的引用；每个名字必须解析到某个 producer 的 `OutputSpec.name` 或 loop 的 `item_var`。未解析 → `UnresolvedBinding`。

> **边界（写入 spec，防止误读）**: Phase 1 只验证引用的绑定存在；不保证把绑定嵌入任意源码是语义/语法安全的。`${all_results}` 可能拼成 `df = [{'a':1}]`，也可能拼成未转义字符串——代码生成正确性不属于 Phase 1 职责（Phase 2 structured transport 处理）。

### R2 Producer ordering（结构可达性，非数组顺序）

> **Producer must be structurally reachable before consumer use.**

即 producer → consumer 存在合法程序执行先后关系。Phase 1 规则：

- **Sequence**：`A → B`，A 的输出可被 B 使用。
- **Parallel**：`A ∥ B → C`，A/B 均完成后 C 才可用它们的输出。
- **Conditional**：分支内部产生的绑定**不能在 conditional 节点外部假设存在**（`if x: result=A` 后外部 `print(result)` 在 x=false 时无定义 → **拒绝**）。
- 明确不使用"节点物理数组顺序"作为判据。

### R3 Type alignment

consumer 声明了 `input_types` 的 key → 类型名与 producer `type_name` 严格一致，或 number/integer 宽松兼容。未声明 `input_types` 的 input 只查 binding 存在。

### R4 Field path（静态字段路径）

只允许静态字段路径：`identifier.field.field`。`${validation_result.total_issues}`、`${validation_result.issues}` 允许；`${validation_result[dynamic_key]}`、`${foo.bar[0].value}` 拒绝（Phase 1 不引入表达式语言）。字段必须存在于 producer `fields` 且类型兼容，否则 `DataContractViolation`。

### R5 Output declaration completeness

有输出就必须有完整 schema：`output != None → name 非空 ∧ type_name 非空`。拒绝 `type_name: ""` 等半合法状态。

### item_var 作用域（Phase 1 简化）

- `item_var` 在 loop body 内可见；loop 外引用 → unresolved。
- **`item_var` 不能覆盖已存在的顶层 binding**（如 env 已有 `file_path`，loop item_var 也叫 `file_path`）→ `DataContractViolation`。
- 严格 shadowing / lexical scope 留 Phase 2。

## Runtime 最小支持

`resolve_value` 增加点路径解析（`${a.b.c}` → env 记录中嵌套 payload 字段）。Python 传递方式不动。

## 探针与正式指标

`p1-5b-probe` trace 增加 `contract` 字段（pass/fail + 违规详情）。Formal P1-5B 四层指标：

| 层 | 问题 | 判定 |
|----|------|------|
| Compile | 能否生成合法 CIR | compile success |
| Contract | 节点间数据能否正确衔接 | R1–R5 全过 |
| Execute | Runtime 能否完成 | run status |
| Adequacy | 结果是否满足任务 | acos-verify |

## 测试策略

- 契约检查器单测：UnresolvedBinding / 类型不匹配 / 字段缺失 / 顺序违规（sequence 非法反向、conditional 分支外引用、parallel 可用）/ 点路径通过 / 静态路径拒绝 / item_var 作用域 / item_var 遮蔽顶层绑定 / loop 聚合 List 类型 / R5 空 schema
- 迁移后全 workspace 测试保持绿（迁移正确性验收）

## 落地顺序

1. `acos-core` schema 变更（`OutputSpec`/`FieldSpec`/`input_types`）
2. 存量迁移（golden CIR + fixtures，保持全绿）
3. `acos-compiler/src/contract.rs`：R1–R5 + 错误变体 + repair 接入
4. Runtime 点路径解析
5. 探针 Contract 层 + `cir_spec.md` 更新（loop 聚合 List<T>、点路径、契约规则）
6. 提交 → writing-plans 出实施计划

## 已知限制（Phase 2 候选）

- Python structured transport（stdin/JSON/env）
- 动态索引 / 复杂表达式（`${a[0].b}`）
- 严格 lexical scope / shadowing
- 字段级 runtime 动态验证（Python 输出是否符合声明 schema）