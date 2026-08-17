# ACOS P0 设计：控制语义、失败恢复与 Benchmark / Control Semantics, Failure Recovery & Benchmark

- **状态 / Status**: approved
- **版本 / Version**: 0.1
- **范围 / Scope**: P0（Condition-heavy Benchmark + Failure Recovery）
- **日期 / Date**: 2026-08-17
- **代码锚点 / Code anchors**: `crates/acos-core`、`crates/acos-runtime`、`crates/acos-compiler`、`crates/acos-llm`、`crates/acos-plugin`、`crates/acos-bench`（新增）、`crates/acos-cli`
- **Schema**: `schemas/cir/cir.proto`

---

## 1. 背景 / Background

当前 CIR 已有 `Conditional / LoopMap / Retry` 三种节点类型，但运行时把它们统一当作"透传 children"处理，没有控制语义；失败后无自动重规划（HANDOFF.md 已知限制 #4）。本 spec 将 ACOS 从"能生成图"推进到"具有条件控制、循环、重试、失败恢复和可回归验证的认知程序运行时"。

核心原则：

> **Primitive 提供能力，Runtime 提供控制语义；LLM 只能提交 RecoveryProposal，Runtime 决定是否接受。**

---

## 2. CIR Schema 变更 / CIR Schema Changes

### 2.1 CirNode 增加 `control` 与 `else_children`

```rust
pub struct CirNode {
    pub kind: CirNodeKind,
    pub node_id: String,
    pub capability: Option<String>,
    pub output: Option<String>,
    pub children: Vec<String>,

    #[serde(default)]
    pub else_children: Vec<String>,          // 仅 Conditional 使用

    #[serde(default)]
    pub inputs: HashMap<String, serde_json::Value>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub control: Option<ControlSpec>,        // 控制语义，与业务 inputs 分离
}
```

`inputs` 仍是 Primitive 输入绑定；`control` 承载条件、循环、重试。**二者语义严格分离**。

### 2.2 ControlSpec

```rust
pub struct ControlSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<ConditionSpec>,    // 用于 kind = Conditional 节点
    #[serde(skip_serializing_if = "Option::is_none")]
    pub loop_spec: Option<LoopSpec>,         // 用于 kind = LoopMap 节点
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry: Option<RetryPolicy>,          // 可挂在任意执行节点上
}
```

### 2.3 单一控制模型（重要决策）

`CirNodeKind::Retry` **不再作为语义来源**。P0 统一使用 `control.retry`；任何节点（含 `Retry` kind 的节点）的语义由 `control` 决定。

```text
Conditional → kind = Conditional + control.condition
LoopMap     → kind = LoopMap + control.loop_spec
Retry       → 仅 control.retry，kind 不再承载语义
```

- Rust 侧：`CirNodeKind::Retry` 标记 `#[deprecated(note = "use ControlSpec.retry")]`
- Proto 侧：保留 `CIR_NODE_KIND_RETRY` 避免 breaking change，但注释注明 deprecated
- **P0 约束**：Compiler 不得生成 `kind = Retry` 的节点；Runtime 的 `Retry` 分支仅按 `control.retry` 解释

### 2.4 ConditionSpec

```rust
pub struct ConditionSpec {
    pub expression: String,   // acos-expr 子集，见 §3
}
```

### 2.5 LoopSpec

```rust
pub enum LoopKind { While, Until, ForEach }

pub struct LoopSpec {
    pub kind: LoopKind,
    pub condition: Option<String>,    // while/until 的条件表达式
    pub max_iterations: Option<u32>,  // while/until: Some(n≥1) 必填；for_each: None = 数组长度自然上限
    pub input: Option<String>,        // for_each: env 列表引用（"${list}"）
    pub item_var: Option<String>,     // for_each: 迭代变量名（写入 env）
}
```

**语义必须写死，避免 Compiler/Runtime off-by-one：**

- `While`：先求值条件；`true` → 执行 body；`false` → 退出
- `Until`：先执行 body；再求值条件；`true` → 退出；`false` → 下一轮
- `ForEach`：从 `env[input]` 取数组；对每项将 `env[item_var]` 绑定为该项，执行 body；`max_iterations = Some(n)` 时最多处理前 n 项

### 2.6 FailureClass 与 RetryPolicy

```rust
pub enum FailureClass {
    Timeout,
    RateLimit,
    TransientNetworkError,
    InvalidInput,
    PermissionDenied,
    SyntaxError,
    Unknown,
}

pub struct RetryPolicy {
    pub max_attempts: u32,                  // ≥ 1；0 在编译校验拒绝
    pub backoff_ms: u64,
    pub strategy: RetryStrategy,            // MVP 仅 Fixed
    #[serde(default)]
    pub retry_on: Vec<FailureClass>,        // 空 = 全部可重试类
}
```

可重试类：`Timeout / RateLimit / TransientNetworkError`。`InvalidInput / PermissionDenied / SyntaxError` 重试无意义，不可自动重试。

### 2.7 Proto 同步

`schemas/cir/cir.proto` 增加对应可选字段：`else_children`、`control`（含 condition/loop_spec/retry 的消息定义），字段编号从 6 起。注释同步说明 Retry 已 deprecated。

---

## 3. acos-expr：最小条件表达式 / Minimal Condition Expression

新模块 `acos-core::expr`。**只做安全子集，绝不执行任意 Rust/Python/JS。**

语法：

```text
expr        := or_expr
or_expr     := and_expr ( "||" and_expr )*
and_expr    := not_expr ( "&&" not_expr )*
not_expr    := "!" not_expr | primary
primary     := comparison | exists_expr | "(" expr ")"
comparison  := operand ( "==" | "!=" | ">" | "<" | ">=" | "<=" ) operand
operand     := literal | identifier_path
literal     := number | string | "true" | "false"
exists_expr := identifier_path ("exists" | "not_exists")
```

- `identifier_path`：`name` 或 `name.sub.field`（`.` 路径遍历 JSON）
- 标识符从 env（`HashMap<String, TypedValue>`）取根值；`TypedValue.payload` 为 JSON

**规则：条件引用禁止模糊匹配。** 与 `inputs` 的模糊引用解析不同，Condition 表达式必须**静态可证**：

- 编译期校验（`validate_cir`）：表达式中的所有顶层标识符必须命中以下集合之一
  - 某个节点的 `output` 名（精确匹配）
  - 任务级保留绑定（如 `goal`）
- 未命中 → `AcosError::ValidationFailure`（Negative Benchmark 必须覆盖，如 `test.exit_cod` 拼写错误）

---

## 4. 运行时控制语义 / Runtime Control Semantics

`crates/acos-runtime/src/lib.rs` 的 `run_node` 兜底分支替换为真实语义：

### 4.1 Conditional

```text
求值 control.condition.expression
    true  → 执行 children
    false → 执行 else_children
```

节点事件沿用 `node.start`（payload 记录 `kind: "conditional"` 与求值结果 `branch: "then" | "else"`）。

### 4.2 LoopMap

- 按 §2.5 的 While/Until/ForEach 语义执行
- 每轮迭代事件：`iteration.started` / `iteration.completed`（payload 含 `index`）
- 达到 `max_iterations` 而条件未满足 → 以 `RuntimeInfrastructureFailure`（或新增 `LoopLimitReached` 语义）终止该节点并向上传播失败
- 迭代间 env 共享：子节点输出按名覆盖

### 4.3 Retry（含重试安全性门）

```text
可自动重试
    = FailureClass ∈ retry_on（或 retry_on 为空）
  AND retry-safe（effect 层面，见下）
```

**重试安全性规则（防副作用重复执行）：**

```rust
fn retry_safe(primitive: &dyn Primitive) -> bool {
    primitive.idempotent()
        || primitive.effects().iter().all(|e| e.reversible || matches!(e.kind,
            EffectKind::FsRead | EffectKind::NetworkRead))
}
```

- `Primitive` trait 新增默认方法：`fn idempotent(&self) -> bool { false }`（builtin 原语可覆写；MVP 无需 manifest 变更）
- `timeout + network_read` → 可重试；`timeout + external_irreversible` → 禁止自动重试（编译期即拒绝，见 §7.2）；`timeout + write_file`（FsWrite 非幂等）→ 运行时禁止自动重试
- 不满足安全性 → 视为非可重试，按原失败路径处理

重试事件：`retry.started`（attempt 数）/ `retry.exhausted`。

### 4.4 失败分类入口 / FailureClassifier

```text
Primitive.invoke()
      ↓
AcosError::PrimitiveFailure { message, primitive_id, class }
      ↓
FailureClassifier::classify(err) -> FailureClass
```

- `AcosError::PrimitiveFailure` 增加 `class: FailureClass`（serde `#[serde(default)]` = `Unknown`，向后兼容）
- 原语可直接构造带 `class` 的错误；分类器集中兜底（`acos-core::error::classify`，**唯一**允许字符串模式匹配的位置），runtime 其余代码不得散落 `error.to_string()` 匹配

### 4.5 错误传播

`run_primitive` 失败仍沿 `run_node → run_nodes → execute` 传播，但 `execute` 的错误路径扩展为恢复状态机（§5）。

---

## 5. 失败恢复状态机 / Failure Recovery

### 5.1 状态机（最终确认）

```text
Primitive Failure
      ↓
① RetryPolicy（确定性，仅暂态类 + retry-safe）
      ↓ 仍失败
② RuleReplanner（确定性，无 API key）
      ↓ 无法修复
③ ModelReplanner（LLM 生成 RecoverySubgraph；无 key → 优雅跳过）
      ↓
④ 事务式提交门（结构/能力/契约/效果/权限校验）
      ↓ 通过
⑤ 替换节点，重新执行
      ↓ 仍失败
⑥ 运行失败（等待人类审批/终止）
```

### 5.2 类型与 Trait（置于 acos-core）

```rust
pub struct FailureContext {
    pub run_id: RunId,
    pub node_id: String,
    pub error_class: FailureClass,
    pub error_message: String,
    pub attempts: u32,
    pub recent_events: Vec<Event>,
}

pub struct RecoveryProposal {
    pub replace_node: String,   // 原节点 id（见 5.4）
    pub subgraph: Vec<CirNode>,
    pub reason: String,
}

pub trait Replanner: Send + Sync {                 // 同步、确定性
    fn propose(&self, failure: &FailureContext, program: &CirProgram) -> Option<RecoveryProposal>;
}

#[async_trait]
pub trait ModelReplanner: Send + Sync {            // LLM
    async fn propose(&self, failure: &FailureContext, program: &CirProgram)
        -> Result<Option<RecoveryProposal>, AcosError>;
}
```

### 5.3 运行时集成

```rust
pub async fn execute(&self, program: CirProgram) -> Result<RunReport, AcosError>;   // 现状，recovery = None
pub async fn execute_with_recovery(&self, program: CirProgram,
    recovery: Option<&RecoveryContext<'_>>) -> Result<RunReport, AcosError>;
// RecoveryContext { rule: Option<&dyn Replanner>, model: Option<&dyn ModelReplanner> }
```

失败路径：① 节点有 `control.retry` 且满足 §4.3 → 重试；② 否则若 `rule` 存在 → propose → 校验 → 提交执行；③ 否则若 `model` 存在 → 同上；④ 否则按现状返回错误。

事件：`replan.started` / `replan.completed`（payload 含 proposal 摘要）/ `replan.rejected`（payload 含校验错误）。

### 5.4 RecoveryProposal 的 Patch 语义（事务式提交）

**替换根节点必须保留原 node_id**：

```text
原来：  A → B → C
补丁：  B 保留 id，subgraph 根节点复用 "B"，B1/B2/B3 挂为子树
结果：  A → B → C   （B 内部变为 B1 → B2 → B3）
```

- 上游引用不变、下游引用不变、Event Log 连续、Recovery 可审计
- 提交管道（全部通过才替换，否则 `replan.rejected`，不修改任何当前 Program 状态）：

```text
① 结构校验：node id 唯一；entry/children 引用完整；subgraph 根 node_id == replace_node
② 能力校验：subgraph 内 capability 全部可被 registry.resolve
③ 契约校验：输入/输出类型与 CapabilityDesc（input_type/output_type）一致
④ 效果校验：subgraph 声明效果 ⊆ program.effects 已声明集合
⑤ 权限校验：MVP 无权限系统（P1），本步留接口，P0 恒通过
⑥ 提交：node_map 中替换/插入节点，替换失败节点后重新执行
```

### 5.5 RuleReplanner（acos-runtime，确定性、无 key）

**能力专属恢复规则**，不做全局字符串嗅探：

```rust
// 规则注册：capability -> Vec<RecoveryRule>
trait RecoveryRule: Send + Sync {
    fn matches(&self, failure: &FailureContext) -> bool;
    fn propose(&self, failure: &FailureContext, program: &CirProgram) -> Option<RecoveryProposal>;
}
```

MVP 规则集（可扩展）：

- `execute_python`：stderr 命中 Python 缺失依赖特征 → 修改参数重试（例：`missing_module` → 降级代码路径）
- 任意原语：capability 无法 resolve → 注册表中找**同 capability** 且 Input/Output Contract 兼容、Effect 兼容的替代原语（§5.6）
- 其余失败 → 不匹配（返回 None，交给 ModelReplanner 或最终失败）

### 5.6 替代原语的兼容性检查

capability 相同 ≠ 可直接替换。必须全部兼容：

```text
capability id 相同
+ input_type 兼容（CapabilityDesc.input_type 一致或子集）
+ output_type 兼容
+ effects ⊆ 原声明集合
+ 权限声明兼容（P0 无权限系统，留接口）
```

### 5.7 ModelReplanner（acos-compiler，复用 LongCatClient）

- 新 system prompt：**只生成 RecoverySubgraph**（`replace_node + subgraph + reason`），禁止重写整图
- 输入上下文：FailureContext + 当前 Program + 可用 capabilities 清单
- 无 API key（`from_env` 失败）→ `Ok(None)`，由调用方优雅跳过（bench 显示 SKIP）
- 输出经 §5.4 提交门校验，任何不合法输出 → `replan.rejected`，不执行

---

## 6. 编译期校验扩展 / Compile-Time Validation Extensions

`validate_cir`（acos-compiler）新增程序级校验（供 bench 的 `mode: cir` 直接调用）：

| 规则 | 结果 |
|---|---|
| `kind = Conditional` 但无 `control.condition` | **拒绝**（ValidationFailure） |
| `kind = LoopMap` 但无 `control.loop_spec` | **拒绝** |
| `kind = LoopMap` + While/Until 且 `max_iterations = None` | **拒绝** |
| 非 Conditional 节点使用 `else_children` | **拒绝** |
| While/Until 缺 `condition`；ForEach 缺 `input` 或 `item_var` | **拒绝** |
| `max_iterations = Some(0)` | **拒绝** |
| `RetryPolicy.max_attempts = 0` | **拒绝** |
| Condition 表达式标识符未静态命中（§3） | **拒绝** |
| 节点含 `control.retry` 且其原语声明 `ExternalIrreversible` 效果 | **拒绝**（不可安全重试） |
| 引用解析失败（现有模糊匹配之外的悬空引用） | **拒绝** |

ModelCompiler 系统提示词同步更新：教会模型输出 `control`（condition/loop_spec/retry）字段，且不得生成 `kind = Retry`。

---

## 7. Benchmark：crates/acos-bench / Benchmark

### 7.1 定位

独立 crate（lib + bin），是 **ACOS 行为契约与长期回归基础设施**，不与用户 CLI 生命周期绑定。`acos-cli` 增加 `bench` 子命令作为控制面入口：

```bash
acos bench                        # 全部 suite
acos bench --suite condition      # 指定 suite
acos bench --case retry_timeout   # 指定 case
acos bench --require-model        # model_replan SKIP 视为 FAIL
```

### 7.2 Fixture = 行为契约

```
crates/acos-bench/fixtures/
├── condition/  basic.yaml、repair_branch.yaml
├── loop/       foreach.yaml、quality_loop.yaml
├── retry/      timeout.yaml
└── recovery/   rule_replan.yaml、model_replan.yaml
└── negative/   语法类负例（mode: cir）
```

**`mode: run`（端到端 编译→执行→验证）：**

```yaml
id: retry_timeout_001
mode: run
goal: "..."
expected:
  compile: pass                 # pass | fail
  execution: pass               # pass | fail
  verification: pass            # pass | fail
  recovery: retry               # none | retry | rule_replan | model_replan
  final_status: success         # success | failed
```

**`mode: cir`（直接校验 CIR，不经过 Compiler）：**

```yaml
id: negative_retry_irreversible_001
mode: cir
cir: { ... }                    # 内联或引用 CIR 片段
expected:
  validation: reject            # accept | reject
  final_status: invalid
```

### 7.3 四组正向用例 + 负例

1. **Condition**：`IF file.valid → summarize ELSE → repair`（验证 condition/branch 与 then/else 分支选择）
2. **Loop**：`for_each file → analyze(file)`；`while quality < threshold → improve`（验证迭代、迭代上限、终止）
3. **Retry**：`execute → timeout → retry → success`；并验证非重试类不重试（retry 事件计数为 0）
4. **Recovery**：`A → B(fail) → RuleReplanner → B' → C`；`model_replan` 无 key 时 SKIP
5. **Negative（mode: cir）**：
   - loop While 无 `max_iterations` → `validation: reject`
   - `retry.max_attempts = 0` → `validation: reject`
   - Condition 引用未声明标识符（如 `test.exit_cod`）→ `validation: reject`
   - **retry + `ExternalIrreversible` 效果 → `validation: reject`**

### 7.4 报告与状态

```text
ACOS Benchmark v0.1

Suite                  Result   Compile   Execute   Recover
------------------------------------------------------------
condition_basic        PASS       ✓         ✓         -
loop_foreach           PASS       ✓         ✓         -
retry_timeout          PASS       ✓         ✓         ✓
failure_rule_replan    PASS       ✓         ✓         ✓
failure_model_replan   SKIP      ✓         -         -
------------------------------------------------------------
5 cases / 4 passed / 0 failed / 1 skipped
```

状态三值：`PASS / FAIL / SKIP`。`--require-model` 将 SKIP 视为 FAIL（CI 无 key 时默认不红）。

### 7.5 代码结构

```text
crates/acos-bench/
├── src/
│   ├── main.rs        # bin 入口（--suite/--case/--require-model）
│   ├── lib.rs         # bench API（供 acos-cli 复用）
│   ├── runner.rs      # fixture 解析 + 执行管道
│   ├── report.rs      # 表格输出
│   └── metrics.rs     # 预留：latency/token_cost/replan_count
├── fixtures/          # 行为契约（§7.2）
└── tests/             # condition.rs loop.rs retry.rs recovery.rs（复用 expected 字段断言）
```

`acos-cli` 依赖 `acos-bench`（lib），提供 `bench` 子命令。

---

## 8. 实施顺序 / Implementation Order

Benchmark Harness **提前**，尽早驱动控制语义验证，不等 Replanner：

1. **acos-core**：`ControlSpec/LoopSpec/RetryPolicy/FailureClass` 类型 + `acos-core::expr` + `validate_cir` 扩展 + `FailureClassifier` + Replanner traits/`FailureContext`/`RecoveryProposal` + `Primitive::idempotent()` + `PrimitiveFailure.class`；cir.proto 同步
2. **acos-runtime**：Conditional/LoopMap/Retry 控制语义 + 重试安全门 + 新事件（iteration/retry/replan）
3. **acos-bench**：crate 骨架 + condition/loop/retry fixtures + harness + 集成测试 → **立即验证控制语义**
4. **RuleReplanner**（acos-runtime）+ 替代原语兼容性检查 + negative fixtures（validation 类）
5. **ModelReplanner**（acos-compiler）+ RecoverySubgraph 提示词 + recovery suite + `--require-model`
6. **收尾**：全量 `cargo test --workspace` + `acos bench` 全 suite；更新 HANDOFF.md / README / docs

## 9. 验证 / Verification

```bash
cargo build --workspace
cargo test --workspace                 # 含 acos-bench/tests 集成测试
cargo run -p acos-cli -- bench          # 无 key：model_replan = SKIP
cargo run -p acos-cli -- bench --require-model   # 有 key：全部 PASS
```

## 10. 明确不做（YAGNI）/ Out of Scope

- 真正的并行执行（Parallel 保持顺序遍历）
- `kind = Retry` 语义与生成（deprecated，CIR 2.0 移除）
- Permission 系统落地（仅留接口，P1 Effect + Permission）
- 表达式语言扩展（`language` 字段、任意函数调用）
- SQLite、Plugin SDK、SSE 事件流（P1/P2）
