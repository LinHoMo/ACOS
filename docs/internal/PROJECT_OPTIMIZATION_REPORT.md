# ACOS 项目优化评估报告 / Project Optimization Report

- 版本（Version）：1.0
- 日期（Date）：2026-08-16
- 范围（Scope）：`ACOS-project-docs-v0.1` 全部文档

---

## 一、总体评价 / Overall Assessment

ACOS 文档集 v0.1 是一个**架构收敛良好、工程意识强、但内容深度不足**的文档基线。

| 维度 | 评分 | 说明 |
|---|---|---|
| 架构清晰度 | ★★★★☆ | 编译器/运行时分离、五层架构、核心抽象明确 |
| 规范完整性 | ★★★☆☆ | 有 Primitive/Task/CIR 三大规范，但深度不够 |
| 工程可行性 | ★★★★☆ | Rust + SQLite + gRPC，MVP 定义清晰 |
| 文档覆盖度 | ★★★☆☆ | 32 个文档覆盖全生命周期，但多数是骨架级 |
| 示例丰富度 | ★★☆☆☆ | 规范缺少完整示例，CIR 只有一个简单序列 |
| 可操作性 | ★★★☆☆ | 有开发指南和测试策略，但缺少快速开始 |
| 国际化 | ★★★★★ | 已完成中英双语化（本次工作） |

**核心判断**：这份文档足以让一个团队理解 ACOS "是什么"，但不足以让一个开发者"开始实现"。下一步的重点应该是**从骨架到血肉**——深化核心规范、补充示例、完善 ADR。

---

## 二、已完成工作 / Completed Work

### 双语化 / Bilingualization

全部 35 个文档（原 32 个 + 新增 3 个）已完成中英双语化：

- 格式规范：标题"中文 / English"，正文中文为主，关键术语首次出现标注英文
- 代码块/YAML/JSON 保持原样
- 内部链接已同步更新

### 新增文档 / New Documents

| 文件 | 价值 |
|---|---|
| `docs/comparison.md` | 与 LangChain/AutoGen/Temporal/ROS/MCP/K8s 的系统对比，明确差异化定位 |
| `docs/adr-0001-user-space-runtime.md` | 首个完整 ADR，作为后续 ADR 的模板 |
| `CHANGELOG.md` | 版本变更记录 |

---

## 三、内容完整性评估 / Content Completeness

### 3.1 已覆盖的领域 / Covered Areas

- ✅ 项目定位与使命（project_overview）
- ✅ 整体架构（architecture）
- ✅ 设计原则（design_principles）
- ✅ 技术栈选型（tech_stack）
- ✅ 核心规范：Primitive / Task / CIR
- ✅ 运行时模型（runtime_model）
- ✅ 执行模型（execution_model）
- ✅ 状态与事件模型（state_and_event_model）
- ✅ 验证架构（verification）
- ✅ 经验系统（experience_system）
- ✅ 安全模型（security）
- ✅ 插件系统（plugin_system）
- ✅ 编译器设计（compiler_design）
- ✅ 部署与三平台指南
- ✅ 开发/测试/贡献/运维
- ✅ 路线图与 MVP 规范
- ✅ 术语表与 ADR 索引

### 3.2 缺失的关键内容 / Missing Critical Content

| 优先级 | 缺失内容 | 影响 |
|---|---|---|
| P0 | **完整的 CIR 示例集** | 没有条件分支、循环、并行、错误处理的 CIR 示例，编译器无法实现 |
| P0 | **Primitive 完整 Schema** | 只有 YAML 示例，没有 JSON Schema/Protobuf 定义，无法做编译时验证 |
| P0 | **ADR 完整内容（剩余 9 个）** | ADR 只有索引，决策背景、理由、后果缺失 |
| P1 | **快速开始指南（Quickstart）** | 新贡献者无法在 30 分钟内跑通一个最小示例 |
| P1 | **端到端示例（End-to-end Example）** | 从 Task Spec → CIR → Execution Graph → 结果的完整走查 |
| P1 | **错误模型详细定义** | api_guidelines 只列了错误类别，没有错误码、恢复策略、用户提示 |
| P2 | **能力本体（Capability Ontology）** | 三级匹配中的 Ontology 层没有定义，能力分类缺失 |
| P2 | **事件类型完整清单** | state_and_event_model 只有信封格式，没有 TaskStarted/CodeGenerated 等具体事件定义 |
| P2 | **权限模型详细设计** | security 只列了权限类别，没有权限申请、审批、审计流程 |
| P3 | **性能基准目标** | testing 列了指标，但没有目标值（如"编译延迟 < 5s"） |
| P3 | **FAQ / 常见问题** | 新用户的常见疑问没有集中回答 |

---

## 四、目录结构优化 / Directory Structure Optimization

### 4.1 当前问题 / Current Issues

所有 33 个文档平铺在 `docs/` 下，没有分类。随着文档增长，查找和维护会越来越困难。

### 4.2 推荐结构 / Recommended Structure

```text
docs/
├── INDEX.md                          # 文档总索引
├── comparison.md                     # 相关系统对比
│
├── concepts/                         # 核心概念
│   ├── project_overview.md
│   ├── architecture.md
│   ├── design_principles.md
│   └── glossary.md
│
├── specs/                            # 规范（最核心）
│   ├── cognitive_primitive_spec.md
│   ├── task_spec.md
│   ├── cir_spec.md
│   ├── api_guidelines.md
│   └── plugin_system.md
│
├── compiler/                         # 编译器
│   └── compiler_design.md
│
├── runtime/                          # 运行时
│   ├── runtime_model.md
│   ├── execution_model.md
│   ├── state_and_event_model.md
│   ├── verification.md
│   ├── experience_system.md
│   └── security.md
│
├── platforms/                        # 平台与部署
│   ├── deployment.md
│   ├── platform_windows.md
│   ├── platform_linux.md
│   └── platform_macos.md
│
├── development/                      # 开发与运维
│   ├── development_guide.md
│   ├── quickstart.md                 # 新增
│   ├── testing.md
│   ├── contribution.md
│   ├── repository_structure.md
│   ├── tech_stack.md
│   └── operations.md
│
├── roadmap/                          # 路线图
│   ├── roadmap.md
│   └── mvp_spec.md
│
├── adr/                              # 架构决策记录
│   ├── README.md                     # 原 adr_index.md
│   ├── 0001-user-space-runtime.md
│   ├── 0002-rust-core.md             # 待写
│   └── ...
│
└── examples/                         # 示例（新增）
    ├── task_examples.md
    ├── cir_examples.md
    └── primitive_examples.md
```

### 4.3 重组注意事项 / Migration Notes

- 重组后需要更新所有内部链接（约 40+ 处）
- 建议使用 Git 移动（`git mv`）以保留历史
- 可以分阶段进行：先创建新目录，逐步移动，最后删除空文件
- README.md 中的文档导航链接需要同步更新

---

## 五、内容深化建议 / Content Deepening Recommendations

### 5.1 P0：核心规范深化（必须在实现前完成）

#### CIR Specification 深化

当前 CIR 只有概念列表和一个简单序列示例。需要补充：

1. **完整的类型系统定义**：基本类型、结构体、联合类型、列表、可选类型的语法和语义
2. **控制流结构的 CIR 表示**：
   - 条件分支（if/else）怎么表示？
   - 循环/映射（loop/map）怎么表示？
   - 并行（parallel）怎么表示？
   - 错误处理（try/catch 或 Result 类型）怎么表示？
3. **效果系统的精确语义**：效果如何组合？如何检查冲突？
4. **至少 5 个完整示例**：从简单线性到条件密集型任务

#### Primitive Schema 形式化

当前只有 YAML 示例。需要：

1. 提供 JSON Schema 或 Protobuf 定义
2. 定义每个字段的类型、可选性、默认值、验证规则
3. 提供 5 个 MVP 原语的完整清单（每个都有完整的 input/output schema）

#### ADR 补全

剩余 9 个 ADR 需要补全为完整文档（参考已创建的 ADR-0001 模板）：

- ADR-0002: Rust 核心
- ADR-0003: 隔离进程提供者
- ADR-0004: SQLite 状态存储
- ADR-0005: 事件溯源
- ADR-0006: 混合任务输入
- ADR-0007: 三级能力匹配
- ADR-0008: 确定性验证优先
- ADR-0009: ACOS 管理的插件安装
- ADR-0010: WASM 推迟

### 5.2 P1：可操作性提升（实现开始时需要）

1. **Quickstart 指南**：从安装到运行第一个任务的完整步骤，包含命令和预期输出
2. **端到端示例**：以"分析 5 个 CSV 文件"为例，展示 Task Spec → CIR → Execution Graph → 验证 → 经验记录的完整流程
3. **错误模型详细定义**：每个错误类别的错误码、触发条件、恢复策略、用户可见消息
4. **事件类型清单**：定义所有运行时事件（TaskSubmitted、CompilationStarted、PrimitiveExecuted、VerificationPassed 等）的 payload schema

### 5.3 P2：生态准备（MVP 验证后需要）

1. **能力本体**：定义 capability 的分类体系（如 `information.retrieval`、`code.generation`、`data.analysis`）
2. **权限流程**：权限申请、审批、审计的完整流程设计
3. **插件开发教程**：从零开发一个 Primitive Provider 的完整教程
4. **SDK 文档**：Python/TypeScript SDK 的 API 参考

### 5.4 P3： polish（公开发布前需要）

1. 性能基准目标值
2. FAQ
3. 架构图补充（每个核心组件一张图）
4. 视频/图文教程

---

## 六、术语一致性检查 / Terminology Consistency

本次双语化统一了以下术语翻译，建议后续文档严格遵守：

| 英文 | 中文 | 备注 |
|---|---|---|
| Cognitive Primitive | 认知原语 | 不译"认知基元" |
| Cognitive Runtime | 认知运行时 | |
| Cognitive Program | 认知程序 | |
| Cognitive IR (CIR) | 认知中间表示 | 保留缩写 CIR |
| Cognitive Compiler | 认知编译器 | |
| Effect | 效果 | 不译"效应" |
| Evidence | 证据 | |
| Artifact | 工件 | 不译"制品" |
| Experience Record | 经验记录 | |
| Provider | 提供者 | 不译"提供商" |
| Capability | 能力 | |
| Task Specification | 任务规范 | 不译"任务规格" |
| Execution Graph | 执行图 | |
| World Model | 世界模型 | |
| Tombstone | 墓碑 | Agent 终止后的持久记录 |
| Homeostasis | 稳态 | 工程文档中用"认知控制循环"替代 |

---

## 七、优先级行动清单 / Priority Action List

### 立即执行（本周）

1. ✅ 完成全部文档双语化（已完成）
2. ✅ 补充 comparison.md、ADR-0001、CHANGELOG（已完成）
3. ⬜ 执行目录结构重组（按 4.2 方案）
4. ⬜ 更新所有内部链接

### 短期（2 周内，实现前必须完成）

5. ⬜ 深化 CIR Spec：补充类型系统定义 + 5 个完整示例
6. ⬜ 形式化 Primitive Schema：提供 JSON Schema 定义
7. ⬜ 补全剩余 9 个 ADR
8. ⬜ 编写 Quickstart 指南

### 中期（MVP 开发期间）

9. ⬜ 编写端到端示例文档
10. ⬜ 详细定义错误模型
11. ⬜ 定义事件类型清单
12. ⬜ 定义能力本体初版

### 长期（MVP 验证后）

13. ⬜ 权限流程详细设计
14. ⬜ 插件开发教程
15. ⬜ SDK 文档
16. ⬜ 性能基准与 FAQ

---

## 八、风险提示 / Risks

1. **规范先行 vs 实现先行的平衡**：过度追求规范完美会延迟实现。建议规范做到"足以指导实现"即可，在实现中迭代。
2. **CIR 设计风险**：CIR 是整个系统的支点，如果类型系统设计不当，会导致编译器无法实现或生成的图无法执行。建议先做最小可用 CIR，在 MVP 中验证。
3. **文档与代码的同步**：规范文档容易与实际实现脱节。建议在 CI 中加入 schema 验证，确保文档中的示例与代码一致。
4. **双语维护成本**：双语文档意味着每次变更都要更新两种语言。建议建立"中文为主、英文术语标注"的模式，而不是逐句翻译，降低维护成本。

---

## 九、结论 / Conclusion

ACOS v0.1 文档集是一个**有潜力的架构基线**，它正确地收敛了定位（用户空间认知运行时）、选择了务实的技术栈（Rust + SQLite + gRPC）、定义了清晰的核心抽象（Primitive / Program / Runtime）。

当前的主要差距不是"方向错了"，而是"深度不够"。32 个文档覆盖了全生命周期，但多数停留在骨架级。**下一步的关键不是增加更多文档，而是深化最核心的 3 个规范（CIR / Primitive / Task）和补全 ADR。**

如果能在 2 周内完成 P0 项，ACOS 就可以从"设计文档"进入"可实现的规范"阶段。
