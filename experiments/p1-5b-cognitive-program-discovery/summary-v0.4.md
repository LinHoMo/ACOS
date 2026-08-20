# P1-5B Formal Evaluation v0.4 (Serialization Contract & Structured Inputs) — FROZEN

> **状态**: FROZEN（2026-08-20）。本轮按真实结果冻结，**不补跑、不美化**；如需重测 S3，另立 v0.4-R2 并冻结全部实验条件。
> **数据批次声明**: S0 = **历史冻结数据**（v0.3 C 组 `formal-eval-v0.3-results-c/`，commit `f613592`），非本轮重新采样；S1/S2/S3 为本轮新采样 ×5。
> **模型/供应商声明**: S0 基线 = LongCat-2.0；S1/S2/S3 = **deepseek-v4-flash via SenseNova**（OpenAI-compatible transport）——S0 与 S1-S3 **跨模型/供应商**，属记录在案的混杂（见 §6 实验条件）。
> **Spec**: `docs/specs/2026-08-19-p1-5b-v0.4-primitive-invocation-contract-structured-inputs-design.md`（FROZEN）

## 1. 四组因子矩阵

| 组 | 序列化教学 | 结构化 inputs（注入+教学+env 拒绝） | 模型 | Compile | Contract | Execute | Adequacy |
|---|---:|---:|---|---:|---:|---:|---:|
| **S0**（v0.3 C frozen control） | 无 | 无 | LongCat-2.0 | 20% (1/5) | 20% (1/5) | 0/1 | 0% |
| **S1**（+序列化） | 有 | 无 | deepseek-v4-flash | **100% (5/5)** | **100% (5/5)** | 0/5 | 0% |
| **S2**（+结构化 inputs） | 无 | 有 | deepseek-v4-flash | 20% (1/5) | 20% (1/5) | 0/1 | 0% |
| **S3**（+两者） | 有 | 有 | deepseek-v4-flash | 0% (0/5) | 0% (0/5) | 0/0 → **N/A** | **N/A** |

> Adequacy/Execute 分母为 0 时记 **N/A**（spec §8），不记 0%。

## 2. 核心指标

| 指标 | S0 (frozen) | S1 | S2 | S3 |
|---|---|---:|---:|---:|
| serialization_failure_rate（code-as-map / runs） | 4/5 = **80%** | 0/5 = **0%** | 2/5 = 40% | 0/0 → N/A（模型层空响应，从未进入解析） |
| env_failure_rate（env 类失败 / 含 execute_python 的 run） | 1/1 = **100%** | 5/5 = **100%** | 0/1 = **0%** | 0/0 → N/A |
| env_persistence_rate（final plan 含 env / 含 execute_python 的 run） | 1/1 = **100%** | 5/5 = **100%** | 0/1 = **0%** | 0/0 → N/A |
| repair rate（avg repairs/run） | —（frozen） | 0.8 | 2.4 | 3.0 |
| first-pass success | — | 2/5 | 1/5 | 0/5 |

## 3. Per-run detail

### S1（序列化教学，5/5 编译+契约通过）

| run | compile | contract | execute | repairs | 失败类 |
|---|---:|---:|---:|---:|---|
| run-001 | pass | pass | fail | 1 | `NameError: name 'env' is not defined`（运行时，S1 按设计容忍 env） |
| run-002 | pass | pass | fail | 1 | 同上 |
| run-003 | pass | pass | fail | 2 | 同上 |
| run-004 | pass | pass | fail | 0 | 同上（Pandas4Warning 后异常退出） |
| run-005 | pass | pass | fail | 0 | 同上 |

### S2（结构化 inputs，1/5 编译）

| run | compile | contract | execute | repairs | 失败类 |
|---|---:|---:|---:|---:|---|
| run-001 | fail | fail | — | 3 | 模型层空响应（`raw_response=""`，JSON EOF） |
| run-002 | fail | fail | — | 3 | `CIR schema mismatch: foreach aggregate output type 'List' must be 'List<Record>'`（CIR 类型错误） |
| run-003 | **pass** | **pass** | fail | 0 | `TypeError: unhashable type: 'list'`（真实程序 bug，非 env/序列化） |
| run-004 | fail | fail | — | 3 | `invalid type: map, expected a string`（code-as-map，S2 无序列化教学） |
| run-005 | fail | fail | — | 3 | 同上 |

### S3（两者组合，0/5 编译）

| run | compile | raw_response | repairs | 失败类 |
|---|---:|---:|---:|---|
| run-001 | fail | `""` | 3 | 模型层空响应（JSON EOF） |
| run-002 | fail | `""` | 3 | 同上 |
| run-003 | fail | `""` | 3 | 同上 |
| run-004 | fail | `""` | 3 | 同上 |
| run-005 | fail | `""` | 3 | 同上 |

## 4. 判定（正式冻结结论）

```text
P1-5B v0.4

H-S Serialization Contract
✅ SUPPORTED
   serialization_failure_rate: S0 80% → S1 0%（相对下降 100%，阈值 ≤20%）
   证据：S1 5/5 编译 + 契约通过；code-as-map 类错误零复现

H-E Structured Execution Binding Contract
⚠️ INCONCLUSIVE / PARTIAL MECHANISTIC SUPPORT
   env_failure_rate(S2) = 0/1 ≤ 0.2 ∧ env_persistence_rate(S2) = 0/1 ≤ 0.2
   └─ 但分母 n=1：S2 仅 run-003 进入执行层；4/5 因序列化/类型/模型层问题
      未到达执行层，无法证明结构化 inputs 契约能稳定降低 env 类失败
   └─ 机制级证据（run-003）：模型采纳 inputs[]（3 处）、final plan 零 env 引用，
      剩余失败为真实程序 bug（TypeError: unhashable list）——失败前沿确实推进了

S3 Interaction
❓ NOT IDENTIFIABLE（INTERACTION_UNIDENTIFIABLE）
   5/5 模型层空响应；S1 正常、S2 有输出 → 组合相关，但被模型/供应商/配额/限流
   严重混杂（§6），不能判定"负交互已证明"

Program Discovery（命题 B）
❌ still unresolved（H-S/H-E 达标不宣布命题 B，spec §9）
```

## 5. Root cause analysis（来自 traces）

- **S1：序列化契约教学消除 code-as-map。** S0 中 4/5 失败且 3 次 repair 全部重犯同一错误；S1 中同类错误 0 次复现，first-pass 2/5、repair 平均 0.8。教学段的两个最小正例（§4 spec）足以让模型内化 `code` 必须是 JSON 字符串。
- **S1：纯教学不能修复 env 绑定。** 执行层 5/5 全部 `NameError: name 'env' is not defined`（按设计 S1 容忍 env、不注入）——env 缺陷与序列化缺陷是独立的失败类，需要独立治疗（机制绑定而非 prompt 教学），H-E 方向与 v0.3 结论一致。
- **S2 run-003：结构化 inputs 的机制级证据。** 唯一成功编译样本中，3 个 execute_python 步骤全部使用 `inputs[...]` 读取绑定数据，foreach 体使用 `${item}` 插值（兼容路径），final plan 无 env 引用；执行失败于 `TypeError: unhashable type: 'list'` —— 已不是绑定/序列化错误，而是真实的程序实现缺陷。**这正好把 ACOS 瓶颈推进到"Primitive Invocation 最后一公里"：模型选择能力 + 参数正确，但自己拼装的 Python 逻辑不可靠。**
- **S2 其余失败：序列化（2/5）与 CIR 类型（1/5）问题复现**，符合"无序列化教学 → 契约错误照旧"的因素隔离预期；空响应 1/5。
- **S3：模型层空响应现象（5/5）。** 所有 run 的 `raw_response=""`、repair 3 次全部空响应，耗时长（单 run 总 wall 可达 10.6 min）。在 LongCat 配额耗尽、SenseNova 401、deepseek 429/TPM、flashlite-8k/16k 降级尝试、500 engine error 等条件下，**无法区分"两个 instruction package 组合触发模型级空响应"与"基础设施/模型行为退化"** → 判 INTERACTION_UNIDENTIFIABLE。

## 6. 实验条件记录（Provider/Model/Quota = 实验条件，非基础设施噪声）

| 项 | S0 | S1 | S2 | S3 |
|---|---|---|---|---|
| Provider | LongCat | SenseNova（OpenAI-compatible） | 同左 | 同左 |
| Model | LongCat-2.0 | deepseek-v4-flash | 同左 | 同左 |
| Quota/限流事件 | — | 402（LongCat 配额耗尽）、S1 期间 429 TPM 限流（delete-then-rerun 重试） | 401 SenseNova、deepseek 429、flashlite 降级尝试、500 engine | 同左（隔离样本见 `formal-eval-v0.4-results-s1-quarantine-402/`） |
| Prompt/温度/超时 | v0.3 C（frozen） | §4 教学段 | §5 教学段 | §4+§5 |

**方法论（自本轮起生效，写入 PROJECT_STATUS 实验原则）**：任何涉及模型供应商、模型版本、API 配额、限流、网络的变化，均视为**实验条件变化**而非普通基础设施噪声；必须进入实验 metadata；跨条件对比只能作为混杂记录，不能作为因果证据。

### 时间戳异常记录（历史元数据异常，不改动原始 trace）

```yaml
timestamp_note:
  status: anomalous
  observed_dates:
    - 2026-09-05
    - 2026-09-06
  expected_local_date: 2026-08-20
  interpretation: "Probe date-conversion bug (days_to_date 365-day approximation drifted ~2 weeks); host clock was NOT the cause. Raw trace timestamps preserved."
  corrective_action: "v0.4-R2 probe uses correct proleptic-Gregorian UTC conversion; future metadata must record clock sanity (ACOS_EXP_CLOCK_SANITY)."
```

原始 trace 时间戳（S2: 2026-09-05T16:xxZ、S3: 2026-09-06T01:xxZ）为实验原始证据，**保留不修改**；异常根因已在 v0.4-R2 中定位为探针 `days_to_date()` 日期换算 bug（非主机时钟偏差）并在 R2 探针修复。本记录仅用于标注异常，供论文/复现时知晓时间戳未经人为篡改。

## 7. Findings

1. **序列化契约被证明有效**（H-S SUPPORTED）：显式的 `code: string` 契约 + 最小正例消除了 code-as-map 失败类（80% → 0%），且 repair 不再重复同一错误。
2. **env 类失败未被序列化教学触及**（S1 100% 复现），与 v0.3 归因一致——绑定契约与序列化契约是两个独立失败类。
3. **结构化 inputs 仅获机制级支持**（n=1）：run-003 证明"inputs[] → 无 env → 失败前沿推进到真实程序 bug"的因果链可行，但端到端证据不足，H-E 判 INCONCLUSIVE。
4. **S3 交互不可判定**：空响应 5/5 被模型/供应商/配额混杂污染，不构成负交互证据。
5. **瓶颈收敛**：ACOS 的问题已从"CIR 编译"（v0.2/v0.3 主要失败层）推进到"认知程序调用原语的最后一公里——Primitive Invocation / implementation binding 不可靠"。

## 8. Next steps

1. **v0.4 冻结，不再补跑。** S1 结果已提交（`3194f9f`）；S2/S3 traces 保留为未追踪目录，随本报告归档。
2. **若需重测 S3 交互**：单独立项 `v0.4-R2`，冻结模型、供应商、额度、Prompt、温度、超时等全部条件（§6 全部入 metadata），不污染 v0.4。
3. **下一研究目标：Primitive Invocation 从"提示模型"走向"机器可约束接口"。** 模型只选择 capability + arguments（如 `csv.inspect_schema → SchemaInfo → 下一节点`），不再自行拼装 `pd.read_csv(...)` / `env[...]` / `inputs[...]` 级代码——即回到 Cognitive Primitive 定义本身。
4. **H-S / H-E 与命题 B 分离报告**（spec §9 维持）：本轮不宣布命题 B。

## 9. v0.4-R2 实验元数据模板（FROZEN，每个 run 至少保存）

> 目标：把 H-E 的证据从 `n=1` 提升到可判断的样本量；单模型、固定配额、固定时间源。任何 run 缺字段即视为 metadata 不完整，不得进入因果结论。

```yaml
experiment:
  commit:
  provider:
  model:
  temperature:
  max_tokens:
  timeout_seconds:

environment:
  os:
  timezone:
  clock_utc:
  clock_sanity_check:

limits:
  quota_status:
  rate_limit_status:

security:
  key_present: true
  raw_key_persisted: false
```

- `clock_utc` + `clock_sanity_check`：记录执行时主机 UTC 时间与基准（如 NTP）比对结果，杜绝 v0.4 时间戳异常复现
- `security.raw_key_persisted` 恒为 `false`；任何 run 不得保存原始 key（凭据卫生原则）

## 10. Artifacts

- Traces: `formal-eval-v0.4-results-s1/`（已提交）、`formal-eval-v0.4-results-s2/`、`formal-eval-v0.4-results-s3/`（未追踪，保留原始数据）
- 基础设施隔离样本: `formal-eval-v0.4-results-s1-quarantine-402/`（402/401/429/500/flashlite 降级）
- Harness: `formal-eval-v0.4.ps1`（S0–S3 模式开关）
- Spec: `docs/specs/2026-08-19-p1-5b-v0.4-primitive-invocation-contract-structured-inputs-design.md`（FROZEN）
- S1 提交: `3194f9f`（test(exp): P1-5B v0.4 S1 complete）
