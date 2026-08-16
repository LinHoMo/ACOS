# 测试策略 / Testing Strategy

- **适用版本 / Since**: v0.1
- **读者 / Audience**: 开发者
- **前置阅读 / Prspecs**: [开发指南](development_guide.md)

## 测试级别 / Test levels

### 单元测试 / Unit tests
模式（schemas）、类型检查、效果验证、状态转换、原语包装器。

### 编译器测试 / Compiler tests
任务 → CIR 夹具（fixtures）、能力解析、规划器验证、优化决策。

### 运行时集成测试 / Runtime integration tests
检查点、重试、取消、事件重放、效果强制执行、**补偿执行**（验证补偿操作正确回滚副作用）。

### 插件测试 / Plugin tests
清单验证、兼容性、健康检查、安装/卸载。

### 端到端测试 / End-to-end tests
完整用户目标 → 编译程序 → 执行 → 验证 → 经验。

## 基准测试套件 / Benchmark suite

基准测试应包括：

1. 简单线性任务（simple linear task）
2. 条件分支（conditional branching）
3. 多输入映射（map over multiple inputs）
4. 失败/恢复（failure/recovery）
5. 带审批的外部副作用（external side effect with approval）
6. 动态能力选择（dynamic capability selection）
7. 跨平台环境适配（cross-platform environment adaptation）

## 关键指标 / Key metrics

- 编译成功率（compile success rate）
- 执行成功率（execution success rate）
- 验证通过率（verification pass rate）
- 人工干预率（human intervention rate）
- 延迟（latency）
- 总成本（total cost）
- 图大小（graph size）
- 原语复用（primitive reuse）
- 恢复成功率（recovery success rate）
- 基线编写工作量（baseline authoring effort）

## 基线比较 / Baseline comparisons

与以下比较：

- 直接的 LLM/工具调用 Agent（a direct LLM/tool-calling agent）
- 手写工作流（a hand-authored workflow）
- 合适时的强工作流引擎（a strong workflow engine where appropriate）
