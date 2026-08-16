# Linux 平台指南 / Linux Platform Guide

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 开发者、运维
- **前置阅读 / Prspecs**: [部署](deployment.md)

## 参考拓扑 / Reference topology

```text
Linux
  ↓
systemd
  ↓
acosd
  ├── Runtime（运行时）
  ├── Registry（注册表）
  ├── State/Event Store（状态/事件存储）
  └── Provider Processes（提供者进程）
```

## 工作区 / Workspace

默认用户工作区：

```text
$HOME/.acos/
```

系统级部署可以使用 `/opt/acos` 存放二进制文件，同时将用户特定状态保留在用户的主目录中。

## 集成目标 / Integration targets

- systemd 生命周期（systemd lifecycle）
- POSIX 文件系统/进程接口（POSIX filesystem/process interfaces）
- shell 提供者（shell providers）
- 容器/沙盒提供者（containers/sandbox providers）
- 环境和包发现（environment and package discovery）

## 安全 / Security

Linux 部署应优先使用非特权服务执行（unprivileged service execution）和操作系统级沙盒（OS-level sandboxing）来处理有风险的提供者。
