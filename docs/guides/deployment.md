# 部署 / Deployment

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 运维、开发者
- **前置阅读 / Prspecs**: [平台指南](platform_windows.md)

## 支持平台 / Supported platforms

- Windows 10/11 x64/ARM64（依赖允许时）
- 带有 systemd 的 Linux 发行版（用于参考部署）
- macOS 13+ 作为参考目标；确切最低版本可能变化

## 部署模式 / Deployment modes

### 本地开发者模式 / Local developer mode

所有组件从源码检出运行。插件从本地目录注册。

### 用户运行时模式 / User runtime mode

ACOS 运行时作为用户管理的服务/后台进程安装。数据位于用户的 ACOS 目录中。

### 隔离模式 / Isolated mode

高风险原语在可用时使用容器/沙盒进程。

## 数据目录 / Data directories

特定平台的目录记录在平台指南中。

## 升级策略 / Upgrade strategy

- 运行时二进制文件有版本（Runtime binaries are versioned）。
- 模式迁移是显式的（Schema migrations are explicit）。
- 事件格式在主要版本内向后兼容（Event formats are backward-compatible within a major version）。
- 插件兼容性在激活前检查（Plugin compatibility is checked before activation）。
