# ACOS 项目交接文档 / Project Handoff

> **版本**：ACOS Mini MVP v0.1
> **日期**：2026-08-17
> **状态**：MVP 已完成并验证，含 Web 端与 Claude 集成

---

## 1. 项目概述 / What ACOS is

ACOS（Artificial Cognitive Orchestration System）是一个**具备认知编译层的插件化认知运行时**。

核心命题：
> **目标经 Cognitive Compiler 编译为认知程序（Cognitive Program），并由 Runtime 可靠执行、验证、演化。**

ACOS 的核心抽象是 **Cognitive Program**（一等公民），Agent 只是程序在运行时的动态执行实体。这不是一个 Multi-Agent Framework。

完整链路：
```
Human Intent → Task Specification → Cognitive Compiler → CIR → Cognitive Program → Runtime → Verification + Evidence → Experience
```

---

## 2. 当前状态 / Current State

### 已完成 ✅

| 组件 | Crate | 状态 |
|---|---|---|
| 类型/trait/错误 | `acos-core` | ✅ 完成 |
| 认知编译器 | `acos-compiler` | ✅ 规则优先 + Claude 模型辅助 |
| 运行时执行引擎 | `acos-runtime` | ✅ 图执行、工件、证据 |
| 状态存储 | `acos-state` | ✅ 内存实现（SQLite 待做） |
| 插件系统 | `acos-plugin` | ✅ 5 个内置原语 + 注册表 |
| 验证流水线 | `acos-verify` | ✅ 完成 |
| LLM 提供者 | `acos-llm` | ✅ 龙猫/Anthropic |
| 命令行 | `acos-cli` | ✅ compile / run / bench / --rules |
| 基准套件 | `acos-bench` | ✅ fixture-as-contract 回归（condition/loop/retry/recovery/negative） |
| Web 服务器 | `acos-server` | ✅ actix-web + 单页前端 |
| 测试 | — | ✅ 12 个核心测试 + 5 套 bench 契约 |

### 已验证场景 ✅

1. **单文件总结**：读取文本 → Claude 中文总结 → 写入 report.md → 验证通过
2. **Claude 动态规划**：Claude 根据任务生成 CIR 执行图（非硬编码）
3. **规则规划器**：确定性 read→summarize→write 流水线（无需 API key）
4. **Web 端实时展示**：规划→编译→执行→验证全流程可视化
5. **控制语义（P0）**：conditional 分支 + else_children、loop_map（while/until/for_each）由 `acos-compiler::validate_cir` 编译期校验
6. **失败重试与恢复（P0）**：暂态失败自动重试（retry）；运行时 `execute_with_recovery` 在失败时按 `rule`（`OfflineFallbackRule` 本地回退）→ `model`（`ModelRecoveryPlanner`，需 key）重规划，事务式提交
7. **基准契约（P0）**：`acos bench` 以 fixture 验证上述行为，覆盖 condition/loop/retry/recovery/negative 五套，含负例编译期拒绝

---

## 3. 仓库结构 / Repository Structure

```text
ACOS/
├── .env.example              # 配置模板（复制为 .env 使用）
├── .gitignore
├── Cargo.toml                # Workspace 根（members = crates/*）
├── README.md                 # 项目入口
├── PROJECT_STATUS.md         # 状态与运行方式
├── CHANGELOG.md
├── crates/
│   ├── acos-core/            # 公共类型、trait、错误、schema 辅助
│   ├── acos-compiler/        # RuleCompiler + ModelCompiler
│   ├── acos-runtime/         # RuntimeImpl：图执行引擎
│   ├── acos-state/           # InMemoryStore（EventStore + ArtifactStore）
│   ├── acos-plugin/          # 5 个原语 + BuiltinRegistry
│   ├── acos-verify/          # verify_run()
│   ├── acos-llm/             # LongCatClient（龙猫/Anthropic）
│   ├── acos-cli/             # 命令行入口
│   └── acos-server/          # Web 服务器（actix-web）
├── schemas/                  # Protobuf + JSON Schema
│   ├── cir/cir.proto
│   ├── task/task.proto
│   ├── primitive/primitive.proto
│   └── events/events.proto
├── static/                   # Web 前端
│   └── index.html            # 单页应用（无构建步骤）
├── docs/
│   ├── INDEX.md              # 文档索引
│   ├── GUIDANCE.md           # 全链路指导性建议
│   ├── specs/                # 规范（契约）
│   ├── guides/               # 指南（怎么做）
│   ├── adrs/                 # 架构决策记录（7 个 ADR）
│   └── internal/             # 内部设计备忘
└── tests/fixtures/           # 测试夹具
```

---

## 4. 快速上手 / Quick Start

### 前置条件

- Rust 1.81+（通过 [rustup](https://rustup.rs/) 安装）
- API key（可选，用于 Claude 模型辅助规划）

### 配置

```bash
cp .env.example .env
# 编辑 .env 填入 LONGCAT_API_KEY
```

### 运行

```bash
# 构建
cargo build --workspace

# 测试（12 个测试）
cargo test --workspace

# Web 端（推荐）
cargo run -p acos-server
# 浏览器打开 http://localhost:8080

# 命令行
cargo run -p acos-cli -- run task.yaml           # Claude 规划
cargo run -p acos-cli -- run task.yaml --rules   # 规则规划
```

---

## 5. 架构要点 / Architecture Highlights

### 5.1 编译器（acos-compiler）

两种规划器：

- **RuleCompiler**：确定性规则，硬编码 read→summarize→write 流水线。无需外部依赖。
- **ModelCompiler**：调用 Claude（LongCat-2.0）生成 CIR JSON。系统提示词教授 Claude CIR 格式与原语契约。

关键设计：
- 引用解析支持**模糊匹配**（LLM 命名不一致时回退）
- JSON 响应自动提取（容忍 markdown 包裹）
- 图验证（entry/children 引用完整性）

### 5.2 运行时（acos-runtime）

`RuntimeImpl` 执行 CIR 图：
- 支持 sequence/parallel/conditional 容器节点
- primitiveInvocation 调用注册表中的原语
- 数据通过命名输出 + `${reference}` 引用传递
- write_file 同时写入磁盘和工件存储

### 5.3 原语（acos-plugin）

5 个内置 MVP 原语：

| 原语 | 输入 | 输出 | 效果 |
|---|---|---|---|
| `search` | `{query}` | DocumentList | network read |
| `read_file` | `{path}` | Document | fs read |
| `write_file` | `{path, content}` | ArtifactRef | fs write |
| `execute_python` | `{code}` | ExecutionResult | process spawn |
| `summarize` | `{document}` 或 `{documents}` | Summary | 无（调 Claude） |

`summarize` 在有 API key 时调 Claude，否则回退本地文本统计。

### 5.4 Web 端（acos-server）

- `actix-web` HTTP 服务器
- 单页前端（`static/index.html`，无构建步骤）
- API：
  - `POST /api/run` — 编译并执行任务
  - `POST /api/write-file` — 写入文件
  - `GET /api/artifact?name=report.md` — 读取工件
  - `GET /api/health` — 健康检查
- 自动加载 `.env`（via dotenvy）

### 5.5 控制语义与恢复（P0）

`CirNode` 新增 `control` 字段（`condition` / `loop_spec` / `retry`）与 `else_children`：

- **编译期**：`acos_compiler::validate_cir` 校验条件标识符、循环 `max_iterations >= 1`、重试 `max_attempts >= 1`，并拒绝在 `ExternalIrreversible` 原语上重试。
- **运行期**：`RuntimeImpl::execute_with_recovery` 执行图；节点 `control.retry` 触发暂态重试；失败时按以下顺序恢复：
  1. `rule` 重规划（`RuleReplanner` + `OfflineFallbackRule`）：暂态类失败将失败节点替换为本地 `read_file` 回退。
  2. `model` 重规划（`ModelRecoveryPlanner`）：LLM 生成 `RecoverySubgraph` 补丁，**仅当配置了 `LONGCAT_API_KEY`**。
- 每次重规划提交为事务式补丁（事件 `replan.started` / `replan.completed` / `replan.rejected`），最多 `MAX_RECOVERY_ATTEMPTS = 3` 次。
- 恢复标签（rule/model/retry）可由 `acos bench` 的 `Recover` 列观测。

> **操作细节**：`--require-model` 把未配置 model 的 `model` 恢复用例从 SKIP 翻转为 FAIL，供 CI 严格模式使用。`ModelRecoveryPlanner::from_env()` 在无 key 时返回 `Err`，runner 据此判定 SKIP。

---

## 6. 配置 / Configuration

`.env` 文件：

```bash
# LLM 提供者（龙猫代理）
LONGCAT_API_KEY=your_key_here

# 可选
# LONGCAT_BASE_URL=https://api.longcat.chat/anthropic
# ACOS_LLM_MODEL=LongCat-2.0
# ACOS_PORT=8080
```

---

## 7. 测试 / Testing

```bash
# 全部测试
cargo test --workspace

# 单个 crate
cargo test -p acos-core
cargo test -p acos-compiler
cargo test -p acos-runtime
```

当前 12 个测试覆盖：
- 类型序列化 roundtrip
- 编译器规划（规则 + 模型）
- 运行时执行（含工件产出）
- 原语调用（read_file + summarize）
- 验证流水线
- JSON 提取辅助函数

---

## 8. 已知限制 / Known Limitations

1. **LLM 命名不一致**：Claude 生成的引用名可能与 output 名不精确匹配。已通过模糊引用匹配缓解，但非根本解决。
2. **经验反馈回路**：已剥离到 Phase 3（feature flag `experience-feedback`）。
3. **SQLite 存储**：未实现，默认内存存储。
4. **失败恢复**：已由 P0 的 `execute_with_recovery` + `rule`/`model` 重规划覆盖（Test C 完成）；`model` 恢复仍需 LLM key。
5. **Effect System**：副作用声明与权限模型待完善。
6. **分布式**：无多主机支持。

---

## 9. 下一步 / Next Steps

按优先级：

### 高优先级（验证核心命题）
1. **条件密集型任务**：branch/loop/recovery 已由 P0 完成（Test B ✅）
2. **失败恢复任务**：运行时失败 → 重规划 已由 P0 完成（Test C ✅）
3. **Effect System**：每个原语声明 effects/permissions

### 中优先级（工程化）
4. **SQLite 持久化**：替换内存存储
5. **引用一致性**：根本解决 LLM 命名问题（或改用位置绑定）
6. **SDK**：TypeScript/Python SDK

### 低优先级（生态）
7. **经验回路**：Phase 3
8. **WASM 插件沙盒**：Phase 4
9. **跨平台安装器**

---

## 10. 关键决策记录 / Key ADRs

- **ADR-0001**：用户空间运行时（非 OS 内核）
- **ADR-0002**：编译器/运行时分离
- **ADR-0003**：Rust 核心
- **ADR-0004**：SQLite 为 MVP 状态存储
- **ADR-0005**：Protobuf 用于 RPC，JSON 用于 manifest
- **ADR-0006**：原生进程优先的插件运行时
- **ADR-0007**：经验回路在 MVP 中剥离

详见 `docs/adrs/`。

---

## 11. 参考 / References

- 架构真理来源：`docs/`
- 全链路建议：`docs/GUIDANCE.md`
- 项目状态：`PROJECT_STATUS.md`
- 本文档的维护者：项目架构委员会

---

*本文档用于项目交接与新成员 onboarding。如有疑问，请查阅 `docs/` 或提交 issue。*
