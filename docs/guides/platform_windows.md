# Windows 平台指南 / Windows Platform Guide

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 开发者、运维
- **前置阅读 / Prspecs**: [部署](deployment.md)

## 参考拓扑 / Reference topology

```text
Windows
  ↓
ACOS Host Service / User Process（ACOS 主机服务 / 用户进程）
  ├── Runtime（运行时）
  ├── Local Registry（本地注册表）
  ├── SQLite State（SQLite 状态）
  ├── Event Store（事件存储）
  └── Provider Processes（提供者进程）
```

## 工作区 / Workspace

默认概念：

```text
%USERPROFILE%\.acos\
```

建议文件夹：

- `config`（配置）
- `plugins`（插件）
- `registry`（注册表）
- `state`（状态）
- `events`（事件）
- `artifacts`（工件）
- `logs`（日志）

## 集成目标 / Integration targets

- Windows 进程执行（Windows process execution）
- PowerShell 提供者（PowerShell provider）
- 文件系统 API（filesystem APIs）
- 已安装的浏览器提供者（browser providers where installed）
- 环境/能力发现（environment/capability discovery）

## 环境发现 / Environment discovery

运行时应在规划环境敏感任务之前收集：操作系统版本、架构、可用运行时、已安装的开发工具、GPU 可见性、网络策略和文件系统权限。
