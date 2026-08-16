# macOS 平台指南 / macOS Platform Guide

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 开发者、运维
- **前置阅读 / Prspecs**: [部署](deployment.md)

## 参考拓扑 / Reference topology

```text
macOS
  ↓
launchd / user process（launchd / 用户进程）
  ↓
ACOS Runtime（ACOS 运行时）
  ├── Registry（注册表）
  ├── SQLite/Event Store（SQLite/事件存储）
  └── Provider Processes（提供者进程）
```

## 工作区 / Workspace

推荐用户数据位置：

```text
~/Library/Application Support/ACOS/
```

## 集成目标 / Integration targets

- launchd 生命周期（launchd lifecycle）
- 文件系统权限（filesystem permissions）
- 进程执行（process execution）
- 浏览器提供者（browser providers）
- 环境发现（environment discovery）

## 安全 / Security

尊重 macOS 用户隐私控制和权限提示。敏感访问应是显式的，而不是由运行时静默请求。
