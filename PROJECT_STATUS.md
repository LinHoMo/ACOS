# ACOS 项目状态 / Project Status

## 当前阶段 / Current stage

架构已收敛为用户空间认知运行时（user-space cognitive runtime），采用编译器/运行时分离（compiler/runtime split）。

**ACOS Mini MVP 已完成并验证**：
- 7 个核心 Rust crate 实现
- 认知编译器（规则优先 + Claude 模型辅助）
- 运行时执行引擎
- Web 端（`acos-server`，端口 8080）
- 12 个测试全部通过
- 端到端验证：目标 → 编译 → 执行 → 工件 → 验证

## 环境配置 / Configuration

复制 `.env.example` 为 `.env` 并配置：

```bash
cp .env.example .env
```

关键变量：
- `LONGCAT_API_KEY` / `ANTHROPIC_API_KEY` — Claude API key
- `ACOS_PORT` — Web 服务器端口（默认 8080）
- `ACOS_LLM_MODEL` — 模型 ID（默认 LongCat-2.0）

## 运行方式 / How to run

```bash
# 构建
cargo build --workspace

# 测试
cargo test --workspace

# Web 端（推荐）
cargo run -p acos-server
# 浏览器打开 http://localhost:8080

# 命令行
cargo run -p acos-cli -- run task.yaml           # Claude 规划
cargo run -p acos-cli -- run task.yaml --rules   # 规则规划
cargo run -p acos-cli -- compile task.yaml       # 仅查看规划
```

## 开发顺序 / Canonical development order

1. ~~`docs/cognitive_primitive_spec.md`~~ ✅
2. ~~`docs/task_spec.md`~~ ✅
3. ~~`docs/cir_spec.md`~~ ✅
4. ~~`docs/mvp_spec.md`~~ ✅
5. ~~实现与基准测试~~ ✅
6. ~~Web 端 + Claude 集成~~ ✅
7. **下一步**：条件密集型任务验证、Effect System、经验回路

## 已完成 / Done

- [x] M0 脚手架（仓库、workspace、CI、schema）
- [x] 7 个核心 crate 实现
- [x] 认知编译器（RuleCompiler + ModelCompiler）
- [x] 运行时执行引擎（图执行、工件、证据）
- [x] 插件系统（5 个内置原语 + BuiltinRegistry）
- [x] 验证流水线
- [x] LLM 集成（龙猫/Anthropic，LongCat-2.0）
- [x] Web 端（actix-web + 单页前端）
- [x] 命令行接口
- [x] 12 个测试全部通过
- [x] 端到端验证（Claude 规划 → 执行 → 工件 → 验证）

## P0 里程碑（控制语义 + 失败恢复 + 基准）✅

**目标**：让 CIR 支持条件分支、循环映射、暂态重试，并在运行时失败时有可验证的恢复路径；以 fixture 为契约的回归套件守护这些行为。

- [x] **控制语义类型**（`CirNode.control`：`condition` / `loop_spec` / `retry`，`else_children`）
- [x] **编译期校验**（`acos_compiler::validate_cir`：条件标识符、循环 `max_iterations >= 1`、`retry.max_attempts >= 1`、不可逆原语禁止重试）
- [x] **失败分类**（`FailureClass`：timeout / rate_limit / transient / invalid_input / …）
- [x] **运行时控制执行**（conditional / loop_map / retry 策略）
- [x] **恢复状态机**（`execute_with_recovery`：transactional gate + `rule` / `model` 重规划，`MAX_RECOVERY_ATTEMPTS = 3`）
- [x] **RuleReplanner**（`OfflineFallbackRule`：暂态失败替换为本地 `read_file` 回退）
- [x] **ModelRecoveryPlanner**（LLM 生成 `RecoverySubgraph` 补丁；需 `LONGCAT_API_KEY`）
- [x] **基准套件**（`crates/acos-bench`：condition / loop / retry / recovery / negative 五套 fixture，CLI `acos bench`）
- [x] **全 workspace 构建/测试绿**：移除 `actix-web` `compress` 默认特性（其经 `zstd-sys`/`brotli-sys` 拉入需 gcc 的 C 依赖），并修复 `[workspace.lints]` 的 `lint_groups_priority` 冲突 → `cargo test --workspace` 与 `cargo clippy --workspace --all-targets` 现在可运行且无非新增 warning。

### P0 已知限制 / P0 known limitations

- `ModelRecoveryPlanner` 需 LLM key；无 key 时相关用例在 `--require-model` 下 FAIL，否则 SKIP。
- `acos-expr` 禁止模糊引用与 `null` 字面量；条件需用 `exists(...)` / `not_exists(...)` / 显式比较。
- `for_each` 当前串行（无并发）；`while`/`until` 必须显式 `max_iterations >= 1`。
- 重试仅在暂态类生效，且节点原语须 retry-safe；`ExternalIrreversible` 效果禁止重试。
- CIR proto（`schemas/cir/cir.proto`）已补齐 `control` / `else_children` 字段，但 `primitive_id` ↔ `capability` 命名尚未统一到 Rust。

## 下一步 / Next

- [x] 条件密集型任务验证（branch/loop/recovery）→ P0 已完成
- [x] 失败恢复与重规划（Test C）→ P0 已完成
- [x] 恢复事件可视化（bench 报告新增 `Detail` 列：`replan:rule` / `retry(xN)`）→ P1 已完成
- [ ] **P1 待办**：expr 增强、任务级保留绑定、ForEach 并发
- [ ] Effect System（副作用声明与权限）
- [ ] 经验回路（Phase 3，feature flag `experience-feedback`）
- [ ] SQLite 持久化存储（Phase 2）
- [ ] SDK 稳定化（TypeScript/Python）

## 尚未标准化 / Not yet standardized

- 生产级分布式传输（production-grade distributed transport）
- WASM 组件 ABI（WASM component ABI）
- 市场治理（marketplace governance）
- 许可证（license）

## 已知限制 / Known limitations

- LLM 规划可能存在命名不一致（已通过模糊引用匹配缓解）
- 经验反馈回路已剥离（Phase 3）
- SQLite 存储尚未实现（默认内存存储）
- 无分布式多主机支持
- 见上方「P0 已知限制」
