# 插件系统 / Plugin System

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-plugin/src/registry.rs`（pending）
- **模式 / Schema**: `schemas/primitive/plugin_manifest.proto`
- **上次验证 / Last verified**: —

## 目标 / Goal

插件应该感觉像可附加的能力（attachable capabilities），而不是隐藏的包管理器副作用（hidden package-manager side effects）。

## 生命周期 / Lifecycle

```text
Discover（发现）
  ↓
Fetch（获取）
  ↓
Verify manifest/signature（验证清单/签名）
  ↓
Install to user-owned ACOS workspace（安装到用户拥有的 ACOS 工作区）
  ↓
Register（注册）
  ↓
Health check（健康检查）
  ↓
Enable（启用）
```

## 本地优先的工作区 / Local-first workspace

推荐：

```text
~/.acos/
  kernel/
  plugins/
  registry/
  state/
  events/
  artifacts/
  config/
```

工作区必须可检查且可移植。插件代码不应静默消失到框架拥有的缓存路径中。

## 插件包 / Plugin package

一个插件包含：

- manifest（清单）
- implementation/provider（实现/提供者）
- schemas（模式）
- compatibility requirements（兼容性要求）
- permissions/effects（权限/效果）
- tests/health check（测试/健康检查）
- license metadata（许可证元数据）

## 版本控制 / Versioning

使用语义版本控制（semantic versioning）加显式协议兼容性范围（explicit protocol compatibility ranges）。运行时升级不得静默加载不兼容的插件。

## 安装模型 / Installation model

包管理器是 ACOS 的关注点。npm/pip 可以在插件构建过程中*内部*使用，但用户可见的安装应将结果插件注册到 ACOS，而不是暴露原始语言包语义。

## 热加载 / Hot loading

插件必须支持运行时加载和卸载，无需重启 ACOS 运行时。

### 加载流程

```text
plugin install <source>
  ↓
Fetch + verify manifest/signature
  ↓
Register capabilities into live registry
  ↓
Emit PluginLoaded event
  ↓
Active plugins updated for subsequent compilation
```

### 卸载流程

```text
plugin uninstall <id>
  ↓
Execute plugin-defined compensation（执行插件定义的补偿）
  ↓
Unregister capabilities（注销能力）
  ↓
Emit PluginUnloaded event
  ↓
Pending tasks using this plugin trigger replan（待处理任务触发重新规划）
```

### 约束

- 正在使用被卸载插件的飞行中任务（in-flight tasks）进入 replan 或 graceful degradation
- 卸载不得导致运行时崩溃——所有注册的效果必须有对应的逆操作
- 热加载是 ACOS 和传统包管理器的关键区别：能力变更即时生效

## 能力接缝模型 / Capability Seam Model

借鉴 DSH 的三角色模式，ACOS 的每个能力接缝（capability seam）由三个角色组成：

### 三角色 / Three roles

| 角色 / Role | 职责 / Responsibility | 示例 / Example |
|---|---|---|
| **Service Definition**（服务定义） | 声明能力的接口契约：输入/输出模式、效果集、前置/后置条件 | `FileRead` 定义：输入 `FileRef`，输出 `Document`，效果 `fs.read` |
| **Service Provider**（服务提供者） | 能力的具体实现，绑定到运行时/进程 | 本地文件系统读取器、S3 读取器、内存缓存读取器 |
| **Consumer**（消费者） | 使用能力的组件：原语、验证器、编译器 Pass | `read_file` 原语、路径安全检查器、CIR 优化器 |

### 设计规则

1. **一个定义，多个提供者**：同一能力可以有多个提供者实现，运行时根据约束选择
2. **提供者可替换**：切换提供者不应影响消费者逻辑
3. **消费者通过定义绑定**：消费者只依赖服务定义，不直接依赖具体提供者
4. **接缝是扩展点**：添加新能力 = 设计完整的三个角色，而不是仅添加一个函数

### 接缝注册示例

```yaml
apiVersion: acos.io/v1
kind: CapabilitySeam
metadata:
  id: filesystem.read
spec:
  definition:
    interface: acos.io/filesystem.read/v1
    input: FileRef
    output: Document
    effects: [fs.read]
    preconditions: [path_exists, permission_granted]
    postconditions: [output_not_empty]
  providers:
    - id: local-fs
      runtime: process
      command: "acos-provider-fs-local"
    - id: s3-fs
      runtime: process
      command: "acos-provider-fs-s3"
  consumers:
    - primitive: read_file
    - verifier: path_safety_checker
```

## Profile 与 Bundle 分层组合 / Profile and Bundle Composition

借鉴 DSH 的 Profile/Bundle 模型，ACOS 使用分层组合来管理插件集合。

### 概念 / Concepts

| 概念 / Concept | 说明 / Description |
|---|---|
| **Bundle**（束） | 一组相关插件的分发格式。声明自身提供的插件列表和配置 |
| **Profile**（配置文件） | 命名的 Bundle 组合。定义一个完整运行环境的插件堆叠顺序 |
| **Patch**（补丁） | 对任意 Bundle 配置的用户覆盖。用于定制而不修改原始 Bundle |

### 分层规则

运行时启动时，按以下顺序应用层（从底到顶）：

```text
Base Profile（基础配置）
  ↓
Bundle A（基础能力：模型适配、工具、持久化）
  ↓
Bundle B（Web UI、Chat 节点）
  ↓
User Patch（用户自定义覆盖）
  ↓
CLI --patch overlay（命令行临时覆盖）
```

后加载的层可以覆盖或扩展先加载层的配置。

### Profile 示例

```yaml
apiVersion: acos.io/v1
kind: Profile
metadata:
  id: web
  description: "ACOS with Web UI for interactive task management"
spec:
  bundles:
    - acos-bundle-core      # 核心运行时、编译器、状态存储
    - acos-bundle-builtin   # 内置原语（search, read_file, write_file, execute_python, summarize）
    - acos-bundle-web       # Web UI、REST API
  patches:
    - path: ./my-config.patch.yml
```

### 内置 Profile

| Profile | 包含 | 适用场景 |
|---|---|---|
| `minimal` | core + builtin | 最小运行，仅 CLI |
| `web` | core + builtin + web | 完整交互式体验 |
| `headless` | core + builtin | 无 UI，脚本/CI 环境 |

### Patch 机制

Patch 允许用户在不修改 Bundle 原始内容的情况下定制行为：

```yaml
# my-config.patch.yml
patches:
  - target: primitive.execute_python  # 按 ID 定位
    config:
      timeout_seconds: 600             # 覆盖默认值
      sandbox: strict                  # 增强沙盒
  - target: provider.model
    replace: true
    config:
      provider: anthropic              # 替换模型提供者
```

Patch 是可组合的：Profile 级 Patch + 用户级 Patch + CLI 级 Patch 按顺序叠加。
