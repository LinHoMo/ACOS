# ACOS 项目状态 / Project Status

## 当前阶段 / Current stage

架构已收敛为用户空间认知运行时（user-space cognitive runtime），采用编译器/运行时分离（compiler/runtime split）。

**M0 脚手架已完成（2026-08-16）**：仓库初始化、`acos-core` 接口骨架（types/traits/error/id/schema）、Protobuf schema（cir/task/primitive/events）、CI 配置、第一个 e2e 测试骨架。

下一个实现优先级：ACOS Mini MVP（M1 契约固化 → M2 端到端 demo）。

## 开发起点 / Where to start

1. 安装 Rust 工具链（见 [开发指南 / Development Guide](docs/guides/development_guide.md)）
2. 运行 `cargo build --workspace` 验证脚手架
3. 运行 `cargo test -p acos-core` 验证核心类型与 schema
4. 按 `docs/GUIDANCE.md` § 2.1 三轨道并行推进

## 权威开发顺序 / Canonical development order

1. `docs/cognitive_primitive_spec.md`（认知原语规范）
2. `docs/task_spec.md`（任务规范）
3. `docs/cir_spec.md`（认知中间表示规范）
4. `docs/mvp_spec.md`（MVP 规范）
5. 实现与基准测试（implementation and benchmark）

## 尚未标准化 / Not yet standardized

- 公共 CIR 线上格式（public CIR wire format）
- 本体格式（ontology format）
- 生产级分布式传输（production-grade distributed transport）
- WASM 组件 ABI（WASM component ABI）
- 市场治理（marketplace governance）
- 许可证（license）
