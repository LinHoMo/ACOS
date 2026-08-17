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

## 下一步 / Next

- [ ] 条件密集型任务验证（branch/loop/recovery）
- [ ] Effect System（副作用声明与权限）
- [ ] 经验回路（Phase 3，feature flag `experience-feedback`）
- [ ] SQLite 持久化存储（Phase 2）
- [ ] 失败恢复与重规划（Test C）
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
