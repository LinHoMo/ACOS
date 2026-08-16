# 贡献指南 / Contribution Guide

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 贡献者
- **前置阅读 / Prspecs**: [开发指南](development_guide.md)

## 贡献者可以添加什么 / What contributors can add

- 新原语/提供者（new primitives/providers）
- 补偿策略（compensation strategies for effects）
- 编译器方法（compiler methods）
- 验证检查器（verification checkers）
- Profile/Bundle 定义（Profile/Bundle definitions）
- 平台集成（platform integrations）
- SDK 改进（SDK improvements）
- 基准测试和评估任务（benchmarks and evaluation tasks）
- 文档（documentation）

## 贡献新原语之前 / Before contributing a new primitive

记录：

- capability（能力）
- input/output schema（输入/输出模式）
- effects（效果）
- supported platforms（支持平台）
- dependencies（依赖）
- tests（测试）
- security considerations（安全考虑）

## 兼容性 / Compatibility

不要在没有 ADR 和迁移说明的情况下更改公共模式。

## 审查标准 / Review standard

架构审查应优先考虑可衡量的行为而非概念主张。当变更添加可复现的测试或基准测试时，它更有说服力。
