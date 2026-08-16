# ADR-0007: 经验反馈回路在 MVP 中剥离 / Experience Feedback Loop Deferred in MVP

- 状态 / Status：已接受 / Accepted
- 日期 / Date：2026-08-16
- 决策者 / Deciders：ACOS 架构委员会

## 背景 / Context

ACOS 的核心差异化之一是"经验反馈回路"：完成的运行转化为结构化证据，用于优化未来编译决策（能力排名、历史图模板、成本/延迟估算）。这是 Phase 3 的目标。但在 MVP 阶段就接入此回路，会显著增加设计与验证复杂度。

## 决策 / Decision

MVP 中 **ExperienceStore 仅做记录（append-only）**，**不接入编译回路**。经验反馈回路通过 feature flag `experience-feedback` 隔离，Phase 3 再启用。

具体含义：

- `acos-state` 仍提供 `ExperienceStore` trait 与实现，保证架构完整性。
- 运行时在每个运行完成后发出经验记录，但编译器不消费经验。
- 能力排名、历史图模板、成本估算等"消费经验"的功能，在 feature flag 关闭时编译为空/无操作实现。
- Phase 3 启用 feature flag，接通经验 → 编译器的反馈路径。

## 理由 / Rationale

1. **聚焦 MVP 验证目标**：MVP 的首要目标是证明"目标可编译为可执行图并可靠执行"，而非证明经验优化有效。
2. **降低风险**：经验反馈回路涉及能力排名、成本模型等未经验证的设计，MVP 中接入会引入大量不确定性与调试负担。
3. **架构完整性**：保留 `ExperienceStore` trait 与存储路径，确保 Phase 3 接通时无需重构状态层。
4. **与架构不变量一致**：MVP 保留"经验记录被发出"的验收标准（见 `specs/mvp_spec.md`），但不提前接通消费侧。

## 后果 / Consequences

### 正面 / Positive

- MVP 设计更简洁、更易验证。
- 经验数据从首个运行即开始积累，Phase 3 启用时已有历史数据可用。

### 负面 / Negative

- 需要在 `acos-compiler` 中维护 feature flag 与条件编译（通过 Cargo features 管理）。
- Phase 3 接通时需谨慎验证反馈回路不会引入不稳定或偏见。

## 参考 / References

- [经验系统 / Experience System](specs/experience_system.md)
- [ACOS Mini MVP 规范 / ACOS Mini MVP Specification](specs/mvp_spec.md)
- [路线图 / Roadmap](guides/roadmap.md)
