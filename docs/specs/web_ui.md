# Web UI / Web 用户界面

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `packages/web-ui/`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 目标 / Goal

ACOS Web UI 提供认知程序的可视化、可检查、可交互的管理界面。它不是 CLI 的替代品，而是互补：CLI 用于脚本和自动化，Web UI 用于理解和控制。

## 核心视图 / Core Views

### 1. 执行图视图 / Execution Graph View

可视化编译器生成的执行图（Execution Graph）。

- 节点表示原语调用，边表示数据/控制依赖
- 颜色编码节点状态：pending / running / succeeded / failed / paused
- 点击节点查看：输入/输出、证据引用、耗时、成本
- 支持缩放、筛选、搜索
- 导出为 PNG / SVG / DOT

### 2. 任务面板 / Task Panel

管理认知任务的完整生命周期。

```text
Task List → Task Detail → Execution Graph → Artifacts → Experience
```

- 任务列表：状态、进度、耗时、成本
- 任务详情：原始目标、编译产物（CIR）、执行追踪
- 操作：提交新任务、暂停、恢复、取消、重新规划

### 3. 事件日志 / Event Log

实时流式显示事件日志。

- 按任务/运行/节点过滤
- 事件类型分组：Task / Agent / Primitive / Verification / Plugin
- 时间线视图和列表视图可切换
- 事件详情展开：完整 payload、生产者、时间戳

### 4. 插件管理 / Plugin Management

可视化的插件生命周期管理。

- 已安装插件列表：版本、能力、健康状态
- 安装新插件：从目录/URL/registry
- 卸载插件：触发补偿流程
- 能力接缝视图：Definition → Provider → Consumer 关系图

### 5. 工件与证据 / Artifacts & Evidence

浏览和管理执行产生的工件。

- 工件索引：按任务、类型、时间
- 证据链：从原始输入到最终输出的完整溯源
- 内容预览：Markdown、代码、数据表

### 6. 经验洞察 / Experience Insights

展示经验系统的聚合数据。

- 能力/提供者排名：成功率、延迟、成本
- 任务历史趋势：完成率、平均耗时、人工干预率
- 教训标签云：自动提取的 pattern

## 技术架构 / Technical Architecture

```text
Browser (React/TypeScript)
  ↓
REST API + WebSocket（事件流）
  ↓
ACOS Runtime (Rust)
  ↓
SQLite / Event Store
```

- 前端：React + TypeScript，通过 REST 和 WebSocket 与运行时通信
- 后端：ACOS Runtime 暴露 HTTP API（与 CLI 共享同一套 API）
- 实时更新：WebSocket 推送事件变更，无需轮询

## API 端点 / API Endpoints

| 方法 | 路径 | 用途 |
|---|---|---|
| `POST` | `/api/v1/tasks` | 提交新任务 |
| `GET` | `/api/v1/tasks` | 列出任务 |
| `GET` | `/api/v1/tasks/{id}` | 获取任务详情 |
| `POST` | `/api/v1/tasks/{id}/cancel` | 取消任务 |
| `POST` | `/api/v1/tasks/{id}/resume` | 恢复任务 |
| `GET` | `/api/v1/tasks/{id}/graph` | 获取执行图（JSON/DOT） |
| `GET` | `/api/v1/events` | 事件流（WebSocket） |
| `GET` | `/api/v1/plugins` | 列出插件 |
| `POST` | `/api/v1/plugins` | 安装插件 |
| `DELETE` | `/api/v1/plugins/{id}` | 卸载插件 |
| `GET` | `/api/v1/artifacts` | 列出工件 |
| `GET` | `/api/v1/experience` | 经验聚合数据（Phase 3，MVP 返回空） |

## 用户交互原则 / UX Principles

1. **图是第一公民**：打开任何任务，默认展示执行图——这是 ACOS 的核心差异化
2. **状态可见**：任何时刻都能看到"系统现在在做什么"
3. **错误可操作**：失败不是终点，提供 replan / retry / skip 选项
4. **历史可审计**：任何过去运行的完整状态都可重建和检查
5. **实时但不焦虑**：事件流实时更新，但 UI 不自动跳转——用户控制注意力

## 与 CLI 的关系 / Relationship with CLI

| 场景 | CLI | Web UI |
|---|---|---|
| 提交任务 | ✅ `acos run task.yaml` | ✅ 表单上传 |
| 查看执行图 | 导出 DOT 文件后外部渲染 | ✅ 内嵌交互式可视化 |
| 安装插件 | ✅ `acos plugin install` | ✅ 拖拽/点击 |
| 查看事件日志 | ✅ `acos events --follow` | ✅ 实时流式面板 |
| 批量操作 | ✅ 脚本化 | ❌ 不擅长 |
| CI/CD 集成 | ✅ 无头模式 | ❌ 不适用 |

两者共享同一套运行时 API，功能等价，只是交互界面不同。

## 路线图 / Roadmap

### MVP（Phase 1）

- 执行图可视化（只读）
- 任务列表和详情
- 事件日志（实时流）
- 基础插件管理

### Phase 2（可靠性）

- 交互式任务控制（暂停/恢复/取消）
- 插件热加载的 UI 反馈
- 执行图导出（PNG/SVG）

### Phase 3（经验优化）

- 经验洞察仪表盘
- 能力排名可视化
- 历史趋势图表
