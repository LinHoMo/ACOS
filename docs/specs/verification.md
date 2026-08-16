# 验证架构 / Verification Architecture

- **状态 / Status**: draft
- **代码锚点 / Code anchor**: `crates/acos-verify/src/lib.rs`（pending）
- **模式 / Schema**: —
- **上次验证 / Last verified**: —

## 目标 / Goal

验证应减少错误接受（false acceptance），而不成为无界的第二个 Agent。

## 层次 / Layers

### 证据验证 / Evidence verification
检查主张是否有足够的来源/出处支持（source/provenance support）。

### 契约验证 / Contract verification
检查输入/输出模式和原语后置条件（primitive postconditions）。

### 执行验证 / Execution verification
尽可能使用确定性检查：测试、哈希、模式、类型检查、linting。

### 策略验证 / Policy verification
检查权限、安全规则和用户定义的约束。

### 语义审查 / Semantic review
在确定性方法不足时使用模型辅助审查（model-assisted review）。

## 验证原则 / Verification principle

优先使用确定性验证而非模型判断。仅当不存在足够可靠的确定性检查时，才使用模型作为批评者（critics）。

## 验证结果 / Verification result

每个验证结果应包括：

- status: pass/fail/warn（状态：通过/失败/警告）
- checker identity/version（检查器身份/版本）
- checked object（被检查对象）
- evidence references（证据引用）
- explanation（解释）
- timestamp（时间戳）
