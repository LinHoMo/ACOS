# 开发指南 / Development Guide

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 开发者
- **前置阅读 / Prspecs**: [仓库结构](../internal/repository_structure.md)

## 前置条件 / Prerequisites

- Rust stable 工具链
- Python 3.11+（用于提供者开发）
- Node.js LTS（用于 SDK/工具）
- Git

## 本地设置 / Local setup

1. 克隆仓库（Clone repository）。
2. 构建 Rust 工作区（Build Rust workspace）。
3. 安装 SDK 开发依赖（Install SDK development dependencies）。
4. 启动本地 ACOS 运行时（Start local ACOS runtime）。
5. 注册内置示例原语（Register built-in example primitives）。
6. 运行集成测试（Run integration tests）。

## 开发原则 / Development principle

保持运行时小巧。新的领域功能应该是原语/提供者或编译器扩展，而不是核心中的硬编码分支。

## 变更流程 / Change process

1. 编写/修改相关规范（Write/modify the relevant spec）。
2. 如果架构变更，添加 ADR（Add an ADR if architecture changes）。
3. 在接口后实现（Implement behind an interface）。
4. 添加测试（Add tests）。
5. 运行跨平台 CI（Run cross-platform CI）。
6. 更新文档和兼容性说明（Update docs and compatibility notes）。

## 提交指南 / Commit guidance

优先使用 Conventional Commits 或其他一致的方案；在仓库初始化后决定。
