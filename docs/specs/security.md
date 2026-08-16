# 安全模型 / Security Model

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-core/src/policy.rs`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 原则 / Principles

- 最小权限（least privilege）
- 显式副作用（explicit side effects）
- 默认隔离（isolation by default）
- 密钥永不出现在普通任务上下文中（secrets never appear in ordinary task context）
- 外部写入需要策略检查（external writes require policy checks）
- 插件身份和完整性可验证（plugin identity and integrity are verifiable）

## 权限类别 / Permission classes

- filesystem read/write（文件系统读/写）
- process spawn（进程生成）
- network read/write（网络读/写）
- device access（设备访问）
- secret access（密钥访问）
- OS integration（操作系统集成）
- irreversible external side effect（不可逆外部副作用）

## 信任层级 / Trust tiers

1. 内置可信原语（Built-in trusted primitive）
2. 本地开发插件（Locally developed plugin）
3. 已验证签名插件（Verified signed plugin）
4. 未验证外部插件（Unverified external plugin）

不可信插件应在可行时运行在隔离的进程/容器中。

## 威胁模型 / Threat model

主要威胁：

- 恶意插件（malicious plugins）
- 通过外部内容的提示注入（prompt injection through external content）
- 凭证泄露（credential exfiltration）
- 不安全的工具使用（unsafe tool use）
- 状态损坏（state corruption）
- 依赖泄露（dependency compromise）
- 供应链攻击（supply-chain attacks）

## 防御者模型 / Defender model

安全通过显式策略执行和验证实现。"免疫系统（Immune system）"仅作为概念隐喻。
