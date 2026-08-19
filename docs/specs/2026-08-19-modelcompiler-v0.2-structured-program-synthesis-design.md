# ModelCompiler v0.2 — Structured Program Synthesis 实验设计（spec FROZEN）

> **状态**: FROZEN（用户批准冻结，2026-08-19；批准后直接进入实现，未做进一步修改）。
> **日期**: 2026-08-19 · **代码版本**: main @ `5f5525a`（P1-4 FROZEN）。
> **动机**: P1-5B v0.1 负结果——`Task → LLM → CIR` 单次生成失败层级为 Program Design Failure（run-005：契约全过但零控制流、程序不满足任务）。失败原因**不是 JSON 输出**，而是**缺少中间认知结构**。
> **批准时新增的硬性要求（用户确认，全部纳入冻结）**：
> 1. **映射是 total function**：合法 Plan → 合法 CIR（validator 接受 ⇒ compiler 必成功；compile_plan 末尾以 validate_cir_semantic + validate_data_contract 作为内部哨兵）。
> 2. **契约内建**：undefined binding = Plan compilation error（非 runtime error）。
> 3. **Experiment A 新增指标 Control Intent Recall**（模型生成的 foreach/conditional/retry 意图中真正被编译器采纳的比例）。
> 4. **命题 B 判定（新阈值）**：Compile ≥ 80% ∧ Plan completeness ≥ 70% ∧ Adequacy ≥ 60%。
> 5. **禁止项**：prompt 不得注入 Golden Plan；PlanIR 不得 hardcode 旗舰内容；PlanIR 不得包含 CIR 节点。
> 6. **实施顺序**：T1 Plan IR schema → T2 validator → T3 Plan→CIR compiler → T4 frontend switch（先不改 LLM prompt）→ T5 Contract → T6 harness。

---

## 1. 核心假设与架构升级

### 假设

> LLM 无法直接生成可验证 CIR（单次、单层、序列化 JSON）；但 LLM 可以生成 **intent-level 程序（Plan IR）**，由**确定性编译器**降级为 CIR——"编译层"在架构上真正出现。

### 架构

```text
v0.1（失败）:                    v0.2（本设计）:
TaskSpec                          TaskSpec
   |                                 |
   v                                 v
LLM → CIR                        LLM → Plan IR（intent-level）
   |                                 |
   v                                 v
Contract Validation           deterministic Plan→CIR compiler
   |                                 |
   v                                 v
Runtime                         Contract Validation
                                     |
                                     v
                                 Runtime
```

**不变**：Runtime / acos-verify / Stage Data Contract R1–R5 / Capability Registry / 任务与数据集 / 模型。

**不做什么**：不修 prompt、不堆 repair 机制（v0.1 的 repair 保留但不再是救赎路径）。

## 2. Plan IR Schema（冻结候选）

```yaml
plan:
  goal: "<task goal>"
  steps:
    - name: analyze_files
      kind: foreach            # primitive | foreach | conditional | retry
      over: input_files        # binding of a List
      body:
        - {name: load_csv, kind: primitive, capability: execute_python,
           input_bindings: [{source: analyze_files, binding: file_path}],
           output: {name: records, type: List}}
        - {name: detect_issues, kind: primitive, capability: execute_python,
           input_bindings: [{source: load_csv, binding: records}],
           output: {name: issues, type: List}}
        - {name: repair_if_needed, kind: conditional,
           condition: "has_issues", input_bindings: [{source: detect_issues, binding: issues}]}
    - name: aggregate
      kind: primitive
      capability: execute_python
      input_bindings: [{source: analyze_files, binding: per_file_results}]
      output: {name: totals, type: Record}
    - name: generate_report
      kind: primitive
      capability: write_file
      input_bindings: [{source: aggregate, binding: totals}]
  data_flow:
    - {from: analyze_files, to: aggregate, binding: per_file_results}
    - {from: aggregate, to: generate_report, binding: totals}
  control_flow:
    - {kind: foreach, target: analyze_files, over: input_files}
    - {kind: conditional, target: repair_if_needed}
```

Rust 对应（示意）：

```rust
struct PlanIR {
    goal: String,
    steps: Vec<PlanStep>,
    data_flow: Vec<DataDependency>,
    control_flow: Vec<ControlNode>,
}
enum StepKind { Primitive, Foreach, Conditional, Retry }
struct PlanStep { name: String, kind: StepKind, capability: Option<String>,
                  input_bindings: Vec<Binding>, output: Option<StepOutput>,
                  body: Vec<PlanStep> }
```

**设计要点**：
- Plan 表达**意图**（步骤 + 数据流 + 控制流），不表达运行时细节（env 注入、模板、event 构造）。
- 绑定引用步骤名（`analyze_files`），非任意字符串——语义闭合由**确定性编译器**保证。
- 控制结构显式声明（foreach/conditional/retry），由编译器生成对应 CIR 控制节点。

## 3. Plan → CIR 确定性映射（冻结候选）

| Plan IR | CIR 生成规则 |
|---|---|
| `step(kind=primitive, capability=c)` | PrimitiveInvocation 节点（capability=c） |
| `step(kind=foreach, over=X, body=B)` | LoopMap 节点（loop_spec 引用 X 声明，item_var 由编译器生成）+ body 子节点 |
| `step(kind=conditional, cond=C)` | Conditional 节点（condition 由编译器翻译）+ 子节点 + else_children |
| `step(kind=retry, ...)` | control.retry（max_attempts 规则默认 3，暂态类限定） |
| `data_flow` | OutputSpec（name/type/fields）+ 消费者 input_types（Contract R1–R5 直接对齐） |
| `step.output` | OutputSpec 声明（type_name 映射 Plan 类型 → CIR 类型） |
| 顶层 sequence | Sequence 根节点 |

**契约内建**：编译过程即契约生成过程——undefined binding 在**编译器层**成为不可能（绑定要么声明于 data_flow 要么来自步骤 output）。

## 4. 三个实验（不跑大 benchmark，逐个小步验证）

### Experiment A: Control Flow Discovery
- 目标：验证 LLM 能否发现控制结构（foreach/conditional/retry）。
- 输入：P1-FLAGSHIP-001（天然需要 foreach 多文件 + conditional 按问题修复 + retry 暂态）。
- 指标：**Plan completeness**（Plan 步骤覆盖任务行为要求的比例）、**Control coverage**（required 控制结构 vs 生成的 foreach/conditional/retry 数量）、**Control Intent Recall**（模型声明的控制意图中被编译器采纳的比例）、binding 引用闭合率。
- 运行：×5，同 v0.1 条件。成功标准不预设。

### Experiment B: Two-stage Compilation（核心实验）
- 比较：`Task → CIR`（v0.1 数据，冻结）vs `Task → Plan IR → CIR`（v0.2）。
- 只改编译器前端，runtime/contract/oracle 不变。
- 指标：四层（Compile/Contract/Execute/Adequacy）+ Repair Tax + Latency/Cost。
- 运行：×5。

### Experiment C: Data Contract Integration
- 验证 Plan→CIR 编译 + Contract R1–R5 后：undefined binding / missing field / wrong data flow 的出现率 vs v0.1。
- 指标：contract_violation_count、binding_accuracy、CIR 结构合法率。

## 5. 指标（沿用 + 新增）

- 沿用：四层成功模型（Compile/Contract/Execute/Adequacy）、Repair Tax、Program Complexity、失败分类（COMPILE/CONTRACT/EXECUTION/ADEQUACY/INFRA）。
- 新增：Plan completeness、Control coverage、Plan-level binding 闭合率（Plan 内数据流一致性）。
- Engineering Cost 参照：Plan IR prompt（≈ ModelCompiler prompt 的升级量）vs Fixed Workflow 264 LOC vs Rule 规则。

## 6. 判定（命题 B 复查）

- **命题 B 判定（用户批准阈值）**：Compile ≥ 80%（×5 中 ≥ 4）∧ Plan completeness ≥ 70% ∧ Adequacy ≥ 60%（×5 中 ≥ 3）——三者同时满足即命题 B **支持**（架构升级方向有效）。
- 若 Compile 改善但 Execute/Adequacy 未动：命题 B 维持暂不支持，v0.3 方向 = Plan 验证器（Plan 层语义检查）。
- 若 Compile 未改善：支持"LLM 无法生成可验证程序"的更强结论，转向 Optimizer/人工引导路径。

## 7. 验收标准

1. Plan IR schema + 确定性 Plan→CIR 编译器实现（纯 Rust，无 LLM 参与降级）。
2. 单测：Plan 样例 → CIR 结构断言（节点数/控制节点/binding）。
3. `cargo test --workspace` 绿 + clippy -D warnings clean。
4. 三实验完成（A×5 / B×5×2 / C 集成到 B 数据）。
5. 报告：`SUCCESS-006-p1-modelcompiler-v0.2.md`（矩阵 + Plan 指标 + 命题 B 判定）。
6. 推送 GitHub main。

## 8. 冻结清单（已批准生效）

- Plan IR schema（§2）、Plan→CIR 映射（§3）
- 任务/数据集/GT/oracle/模型/超时：与 P1-5B v0.1 相同
- Runtime/Contract/Verifier 代码不修改
- v0.1 历史数据不修改
- 模型温度等配置与 v0.1 相同（保留随机性）
- **total function**：合法 Plan ⇒ compile_plan 必成功（validate_cir_semantic + validate_data_contract 作为 compile_plan 内部哨兵）
- **契约内建**：undefined binding / missing field 在编译期报错（PlanCompileError），不产生运行时失败
- **禁止项**：prompt 不注入 Golden Plan；PlanIR 不含旗舰任务内容 hardcode；PlanIR 不含 CIR 节点
- **实施顺序**：T1 schema → T2 validator → T3 compiler → T4 frontend → T5 contract → T6 harness

### 实现记录（FROZEN 后，按批准顺序实现时的设计落地）

- `over: "inputs"` 特例：编译器生成 `task_inputs` 注入节点（execute_python，代码为编译器模板、路径来自 TaskSpec 而非 LLM——P1-5B-A 路径幻觉防护），绑定 `input_files: List<String>`，作为根序列第一个 child。
- item_var 固定 `"item"`（ITEM_VAR），禁止 shadow；保留名 `item` / `task_inputs` / `plan_root`。
- 步骤作用域：siblings + outer 双 map（body 步骤可解析外层绑定）。
- `writePath`：write_file 步骤必填输出路径（输出路径属 Plan 所有；输入路径 Plan 永不书写）。
- Parallel 容器步骤不支持（契约拒绝容器声明 output）。
- 运行时信封语义（Plan 作者的契约基础）：execute_python 输出为 `{stdout, stderr}` record；模板插值 `${binding}` 对 List/Record 产出**可嵌入 Python 字符串字面量的转义 JSON**（消费方 `json.loads`）；ForEach 输入若为 record 信封则迭代 stdout 中的 JSON 数组；write_file 的 content 若为信封则取其 stdout。
- 冒烟验证：`plan-smoke.json`（手写黄金 Plan，测试基建，禁止入 prompt）经全管线（validate→compile→contract→execute→structural 4/4→semantic 6/6→evidence 3/3）通过。

## 9. 风险与对策

| 风险 | 对策 |
|---|---|
| Plan 仍含幻觉绑定 | 确定性编译器只接受 data_flow/step.output 声明引用；Plan 层校验（编译器拒绝未声明引用） |
| Plan 控制结构仍缺失 | Experiment A 专门测；若缺失 → v0.3 Plan 验证器 + 示例驱动 |
| 判定受 repair 干扰 | repair 数据单独报告（Repair Tax），成功判定以 first-pass 数据为主 |
| Plan→CIR 映射过度工程 | 映射表冻结在 §3，最小实现（旗舰任务所需子集） |