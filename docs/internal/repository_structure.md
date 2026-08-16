# 仓库结构 / Repository Structure

推荐的 monorepo：

```text
acos/
├── README.md
├── LICENSE
├── Cargo.toml
├── pyproject.toml
├── package.json
├── crates/
│   ├── acos-core/
│   ├── acos-runtime/
│   ├── acos-compiler/
│   ├── acos-state/
│   ├── acos-verify/
│   ├── acos-plugin/
│   └── acos-cli/
├── packages/
│   ├── sdk-python/
│   ├── sdk-typescript/
│   └── web-ui/
├── schemas/
│   ├── task/
│   ├── primitive/
│   ├── cir/
│   └── events/
├── plugins/
│   ├── builtin/
│   └── examples/
├── tests/
│   ├── unit/
│   ├── integration/
│   ├── compiler/
│   ├── runtime/
│   └── benchmarks/
├── examples/
├── docs/
└── scripts/
```

## 规则 / Rule

核心模式独立于应用示例进行版本控制。每个模式变更都应有 ADR 和兼容性说明。
