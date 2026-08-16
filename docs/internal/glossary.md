# 术语表 / Glossary

| 术语 / Term | 定义 / Definition |
|---|---|
| ACOS | 人工认知编排系统（Artificial Cognitive Orchestration System）；在操作层面，是认知运行时（cognitive runtime）而非主机操作系统。核心定位：具备认知编译层的插件化认知运行时。 |
| 认知运行时 / Cognitive Runtime | 执行认知程序的用户空间运行时（user-space runtime）。 |
| 认知程序 / Cognitive Program | 为 ACOS 生成或编写的、面向目标的可执行程序。 |
| 认知原语 / Cognitive Primitive | 具有显式效果和元数据的原子类型化能力（atomic typed capability）。 |
| 能力 / Capability | 原语能做什么，独立于具体实现。 |
| 提供者 / Provider | 一个能力的具体实现（concrete implementation）。广义概念，对应能力接缝模型中的 Service Provider 角色。 |
| 认知任务规范 / Cognitive Task Specification | 目标、输入、输出、约束和优化偏好的结构化描述。 |
| CIR | 认知中间表示（Cognitive Intermediate Representation），用于规划和执行之间。 |
| 执行图 / Execution Graph | 表示节点、依赖、条件、循环和检查点的运行时图。 |
| 世界状态 / World State | 任务或项目的当前物化状态（materialized state）。 |
| 证据 / Evidence | 支持主张或结果的来源或执行工件（source or execution artifact）。 |
| 工件 / Artifact | 执行产生的持久输出，如代码、报告、图像或数据集。 |
| 经验记录 / Experience Record | 包含任务、图、结果、指标和教训的紧凑运行后记录（compact post-run record）。 |
| 墓碑 / Tombstone | 临时 Agent/进程终止后的持久记录。 |
| 验证 / Verification | 验证证据、策略、正确性和质量的过程。 |
| 效果 / Effect | 原语声明的副作用或资源交互（declared side effect or resource interaction）。 |
| 编译器 / Compiler | 将任务意图转换为可执行认知程序的组件。 |
| Agent | 认知程序的临时运行时执行实例（temporary runtime execution instance）。 |
| 插件 / Plugin | 向 ACOS 暴露原语/提供者的独立分发包。 |
| 注册表 / Registry | 已安装插件、能力、版本和健康状态的本地目录。 |
| 主机操作系统 / Host OS | ACOS 运行其上的 Windows、Linux 或 macOS。 |
| 黑板 / Blackboard | 共享项目/世界状态抽象；实现为物化状态、事件、知识和工件的组合。 |
| 稳态 / Homeostasis | 早期的生物学隐喻；在工程文档中使用"认知控制循环（Cognitive Control Loop）"。 |
| 能力接缝 / Capability Seam | 由服务定义、服务提供者和消费者三角色组成的可替换能力单元。 |
| 服务定义 / Service Definition | 声明能力的接口契约：输入/输出模式、效果集、前置/后置条件。 |
| 服务提供者 / Service Provider | 能力的具体实现，绑定到运行时/进程。 |
| 消费者 / Consumer | 使用能力的组件：原语、验证器、编译器 Pass。 |
| Bundle（束） | 一组相关插件的分发格式，声明自身提供的插件列表和配置。 |
| Profile（配置文件） | 命名的 Bundle 组合，定义一个完整运行环境的插件堆叠顺序。 |
| Patch（补丁） | 对任意 Bundle 配置的用户覆盖，用于定制而不修改原始 Bundle。 |
| 补偿 / Compensation | 效果的逆操作，用于在任务失败或验证未通过时回滚副作用。 |
| 热加载 / Hot Loading | 运行时加载和卸载插件，无需重启 ACOS 运行时。 |
| 模型可见即已记录 / Model-visible means logged | 核心不变量：任何到达模型请求的输入都必须能从事件日志重建。 |
| Web UI | ACOS 的 Web 用户界面，提供认知程序的可视化、可检查、可交互管理。 |
