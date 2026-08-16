# API 与兼容性指南 / API and Compatibility Guidelines

- **适用版本 / Since**: v0.1
- **读者 / Audience**: SDK 开发者、插件作者
- **前置阅读 / Prspecs**: [CIR 规范](../specs/cir_spec.md)、[任务规范](../specs/task_spec.md)

## 稳定边界 / Stable boundaries

首个稳定的公共边界是：

- 任务规范 / Task Specification
- 原语清单 / Primitive Manifest
- 事件信封 / Event Envelope
- 提供者 RPC / Provider RPC

CIR 在通过真实编译器/运行时实现验证之前保持实验性。

## 版本控制 / Versioning

使用 major/minor/patch 兼容性。破坏性模式变更需要新的主要 API 版本。

## 错误模型 / Error model

错误应是有类型的、机器可读的。避免依赖自然语言错误消息进行控制流。

最小类别（Minimum classes）：

- `invalid_input`（无效输入）
- `capability_unavailable`（能力不可用）
- `contract_mismatch`（契约不匹配）
- `permission_denied`（权限拒绝）
- `provider_unavailable`（提供者不可用）
- `timeout`（超时）
- `state_conflict`（状态冲突）
- `verification_failed`（验证失败）
- `external_effect_rejected`（外部效果被拒绝）
