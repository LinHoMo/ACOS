# P1-5A: ModelCompiler Frontend Robustness

**Date**: 2026-08-18
**Status**: PLANNED
**Depends**: P1-R1 (FROZEN, `SUCCESS-004`)
**Enables**: P1-5B (Cognitive Program Discovery)

## 目标

让 ModelCompiler 成为一个**可靠的编译器前端**，能处理 LLM 的各种异常输出，而不是在遇到无效 JSON 时直接崩溃。

**明确限定范围**：P1-5A 不追求"更聪明的规划"，只解决"编译器前端鲁棒性"。

## 当前问题

`crates/acos-compiler/src/lib.rs` 中 `ModelCompiler::parse_cir` 方法（第 120-149 行）在遇到以下情况时直接失败：

- LLM 返回空字符串
- LLM 返回纯文本（无 JSON）
- JSON 格式错误（多余逗号、缺少引号等）
- JSON 结构不符合 CIR schema
- 缺少必需字段
- 引用了不存在的 capability
- 节点引用不存在
- 控制语义违规（如 while 无 max_iterations）

当前没有任何修复/重试路径。

## 设计方案

### 1. 错误分类体系

定义 `CompilerError` 枚举。**注意**：标注 `[当前已有]` 的类是 `parse_cir` / `validate_cir` 当前已经产生的错误；标注 `[新增校验]` 的类需要在 Step 2 中新增校验逻辑。

```rust
pub enum CompilerError {
    // JSON 层 [当前已有]
    JsonSyntaxError {
        message: String,
        raw_excerpt: String,
    },
    // Schema 层 [当前已有]
    JsonShapeError {
        message: String,
    },
    MissingRequiredField {  // [当前已有]
        field: String,
    },
    // 语义层
    UnknownCapability {  // [新增校验] 当前代码不检查 capability 是否在白名单内
        node_id: String,
        capability: String,
    },
    InvalidReference {  // [当前已有]
        node_id: String,
        referenced: String,
    },
    InvalidControlSemantics {  // [当前已有]
        node_id: String,
        message: String,
    },
    InvalidEffect {  // [新增校验] 当前代码不检查 effect kind 是否合法
        message: String,
    },
    // 重试耗尽
    RepairExhausted {
        attempts: u32,
        last_error: Box<CompilerError>,
    },
}
```

**与当前 `lib.rs` 错误来源的映射**：

| CompilerError 变体 | 当前代码来源 |
|---|---|
| `JsonSyntaxError` | `serde_json::from_str` 失败（lib.rs:122-125） |
| `JsonShapeError` | `serde_json::from_value` 失败（lib.rs:140-145） |
| `MissingRequiredField` | `validate_cir` 检查 entry 为空（lib.rs:384-388） |
| `InvalidReference` | `validate_cir` 检查 entry/child/identifier 引用（lib.rs:394-412） |
| `InvalidControlSemantics` | `validate_control_semantics` 全部检查（lib.rs:423-511） |
| `UnknownCapability` | **当前不检查**，需在 Step 2 新增 |
| `InvalidEffect` | **当前不检查**，需在 Step 2 新增 |

### 2. 编译状态机

```text
Model Response
      │
      ▼
JSON Extraction (extract_json_object)
      │
   ┌──┴──────────┐
   │             │
valid JSON    invalid JSON
   │             │
   ▼             ▼
Schema Parse   Repair Prompt
   │             │
   │           Retry (max 2-3)
   │             │
   │         still invalid
   │             │
   │           CompilerFailure
   │
   ▼
CIR Validation (validate_cir)
      │
   ┌──┴──────────┐
   │             │
  valid       invalid
   │             │
   ▼             ▼
  OK        Repair Prompt
               │
             Retry (max 2-3)
               │
           still invalid
               │
             CompilerFailure
```

### 3. Repair Prompt 机制

每次 repair 必须包含：

1. **原始输出**（或错误相关片段）
2. **错误类别**（人类可读）
3. **具体 validator error**（来自 serde_json 或 validate_cir）
4. **要求修复的字段/结构**

示例 repair prompt：

```text
Your previous output failed CIR validation.

Error type: MissingRequiredField
Details: field 'entry' is missing at CIR root level

Original output excerpt:
{"nodes": [...]}

Please return a complete CIR JSON that includes the 'entry' field.
Respond with ONLY the corrected JSON.
```

### 4. 实施步骤

#### Step 1: 定义错误类型

文件：`crates/acos-compiler/src/error.rs`（新建）或扩展 `lib.rs`

- [ ] 定义 `CompilerError` 枚举
- [ ] 实现 `Display` trait
- [ ] 实现 `std::error::Error` trait

#### Step 2: 重构 parse_cir

文件：`crates/acos-compiler/src/lib.rs`

- [ ] 将 `parse_cir` 改为返回 `Result<CirProgram, CompilerError>`
- [ ] 区分 JSON 解析错误 vs schema 错误 vs 验证错误
- [ ] 保留原始 raw 输出用于 repair

#### Step 3: 实现 Repair 逻辑

文件：`crates/acos-compiler/src/lib.rs`（`ModelCompiler` impl）

- [ ] 新增 `compile_with_repair` 方法
- [ ] 实现 repair prompt 构建器
- [ ] 实现重试循环（max_attempts = 3）
- [ ] 每次重试记录 diagnostic

#### Step 4: 更新 compile 方法

文件：`crates/acos-compiler/src/lib.rs`

- [ ] `Compiler::compile` 调用 `compile_with_repair`
- [ ] 重试耗尽时返回明确的 `CompilerFailure`（包含诊断链）

#### Step 5: 编写 Robustness Suite

文件：`crates/acos-compiler/src/lib.rs`（tests 模块）或独立测试文件

至少 9 个测试用例：

| # | 输入类型 | 预期行为 |
|---|----------|----------|
| 1 | 合法 CIR JSON | 一次通过 |
| 2 | 带 markdown fence 的合法 JSON | 一次通过（extract_json 处理） |
| 3 | 带前后文的合法 JSON | 一次通过（extract_json 处理） |
| 4 | 格式错误的 JSON（多余逗号） | 进入 repair → 修复或失败 |
| 5 | 空 JSON 对象 `{}` | MissingRequiredField → repair |
| 6 | 缺少 entry 字段 | MissingRequiredField → repair |
| 7 | 未知 capability | UnknownCapability → repair |
| 8 | 无效节点引用 | InvalidReference → repair |
| 9 | while 无 max_iterations | InvalidControlSemantics → repair |

#### Step 6: 集成测试

- [ ] 使用 mock LLM client 测试各种错误场景
- [ ] 验证 repair 后能生成合法 CIR
- [ ] 验证重试耗尽时返回明确错误

### 5. 明确不做的事

- **不做部分采纳 + 自动补全**：Compiler 不应该悄悄替模型改变程序语义。P1-5A 严格：valid → accept, invalid → repair, still invalid → fail。
- **不做更聪明的 prompt engineering**：P1-5A 不优化 system prompt，只修复编译器前端。
- **不做命题 B 实验**：那是 P1-5B 的事。

## 验收标准

1. 所有现有测试继续通过
2. 新增 9 个 robustness 测试全部通过
3. 手动测试：用真实 LLM API 运行 5 次，记录首次通过率、repair 成功率、最终失败率
4. 代码审查：错误分类完整、repair prompt 信息充分、重试逻辑清晰

## 文件变更清单

| 文件 | 操作 |
|------|------|
| `crates/acos-compiler/src/lib.rs` | 重构 parse_cir、新增 compile_with_repair、新增测试 |
| `crates/acos-compiler/src/error.rs` | 新建（可选，也可内联在 lib.rs） |
| `crates/acos-core/src/error.rs` | 可能需要扩展 CompilerFailure 变体 |

## 时间估计

- Step 1-2: 2-3 小时
- Step 3-4: 3-4 小时
- Step 5-6: 2-3 小时
- 总计: 1-2 个工作日
