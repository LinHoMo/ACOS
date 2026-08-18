//! Bench-only primitives and the composite [`BenchRegistry`].
//!
//! The registry wraps [`acos_plugin::BuiltinRegistry`] and overrides a handful
//! of capabilities with deterministic stubs so fixtures can rely on exact
//! behavior: `search` (always `[]`), `flaky_search` (fails once with a
//! configurable [`FailureClass`]), `list_source` (fixed item list), and
//! `irreversible` (external irreversible effect, used by negative fixtures).

use std::sync::Mutex;

use acos_core::error::AcosError;
use acos_core::id::PrimitiveId;
use acos_core::traits::{CapabilityDesc, PluginRegistry, Primitive, PrimitiveManifest};
use acos_core::types::{EffectDecl, EffectKind, FailureClass, TypedValue, ValueType};
use acos_plugin::BuiltinRegistry;
use async_trait::async_trait;

/// Fails the first `failures` invocations with `class`, then returns `[]`.
#[derive(Debug)]
pub struct FlakySearchPrimitive {
    failures: usize,
    class: FailureClass,
    count: Mutex<usize>,
}

impl FlakySearchPrimitive {
    /// Creates a flaky search that fails `failures` times before succeeding.
    pub fn new(failures: usize, class: FailureClass) -> Self {
        Self {
            failures,
            class,
            count: Mutex::new(0),
        }
    }
}

#[async_trait]
impl Primitive for FlakySearchPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "flaky_search".into(),
            name: "Flaky Search".into(),
            input_type: "SearchQuery".into(),
            output_type: "DocumentList".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::NetworkRead,
            description: "network read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        let mut c = self.count.lock().unwrap();
        if *c < self.failures {
            *c += 1;
            return Err(AcosError::PrimitiveFailure {
                message: format!("stub failure ({:?})", self.class),
                primitive_id: Some("flaky_search".into()),
                class: self.class.clone(),
            });
        }
        Ok(TypedValue {
            value_type: ValueType::List,
            payload: serde_json::json!([]),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(
        &self,
        _effect: &EffectDecl,
        _input: TypedValue,
    ) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Always-succeeding search stub returning an empty results list.
#[derive(Debug)]
pub struct SearchPrimitive;

#[async_trait]
impl Primitive for SearchPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "search".into(),
            name: "Search".into(),
            input_type: "SearchQuery".into(),
            output_type: "DocumentList".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::NetworkRead,
            description: "network read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        Ok(TypedValue {
            value_type: ValueType::List,
            payload: serde_json::json!([]),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(
        &self,
        _effect: &EffectDecl,
        _input: TypedValue,
    ) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Returns a fixed list of items for foreach sources.
#[derive(Debug)]
pub struct ListSourcePrimitive {
    items: Vec<String>,
}

impl ListSourcePrimitive {
    /// Creates a list source that yields `items`.
    pub fn new(items: Vec<String>) -> Self {
        Self { items }
    }
}

#[async_trait]
impl Primitive for ListSourcePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "list_source".into(),
            name: "List Source".into(),
            input_type: "Void".into(),
            output_type: "StringList".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        Ok(TypedValue {
            value_type: ValueType::List,
            payload: serde_json::to_value(&self.items).expect("serialize items"),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(
        &self,
        _effect: &EffectDecl,
        _input: TypedValue,
    ) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Rejects retry-on-failure (ExternalIrreversible effect) — used by negative
/// retry fixtures.
#[derive(Debug)]
pub struct IrreversiblePrimitive;

#[async_trait]
impl Primitive for IrreversiblePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "irreversible".into(),
            name: "Irreversible".into(),
            input_type: "Void".into(),
            output_type: "Void".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::ExternalIrreversible,
            description: "irreversible external side effect".into(),
            reversible: false,
        }]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!("done"),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(
        &self,
        _effect: &EffectDecl,
        _input: TypedValue,
    ) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Fails its first invocation with an `Unknown` class — used by the model
/// recovery fixture so the rule replanner (which only matches transient
/// classes) cannot fix it and the runtime must fall back to the model
/// replanner (which is unavailable without an LLM key → SKIP).
#[derive(Debug)]
pub struct UnstableSearchPrimitive;

#[async_trait]
impl Primitive for UnstableSearchPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "unstable_search".into(),
            name: "Unstable Search".into(),
            input_type: "SearchQuery".into(),
            output_type: "DocumentList".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::NetworkRead,
            description: "network read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, _input: TypedValue) -> Result<TypedValue, AcosError> {
        Err(AcosError::PrimitiveFailure {
            message: "stub failure (Unknown)".into(),
            primitive_id: Some("unstable_search".into()),
            class: FailureClass::Unknown,
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(
        &self,
        _effect: &EffectDecl,
        _input: TypedValue,
    ) -> Result<(), AcosError> {
        Ok(())
    }
}

/// BuiltinRegistry plus bench stubs. `search` always succeeds (so
/// non-retry fixtures that need search don't break); `flaky_search` carries the
/// transient failure used by retry fixtures.
#[derive(Debug)]
pub struct BenchRegistry {
    inner: BuiltinRegistry,
    search_failure_class: FailureClass,
}

impl BenchRegistry {
    /// Creates a registry with the default flaky-search failure class (Timeout).
    pub fn new() -> Self {
        Self {
            inner: BuiltinRegistry::new(),
            search_failure_class: FailureClass::Timeout,
        }
    }

    /// Overrides the stub failure class (used by recovery suites).
    pub fn with_search_failure_class(mut self, class: FailureClass) -> Self {
        self.search_failure_class = class;
        self
    }
}

impl Default for BenchRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginRegistry for BenchRegistry {
    fn list(&self) -> Vec<CapabilityDesc> {
        let mut v = self.inner.list();
        v.push(SearchPrimitive.capability());
        v.push(FlakySearchPrimitive::new(1, FailureClass::Timeout).capability());
        v.push(ListSourcePrimitive::new(vec![]).capability());
        v.push(IrreversiblePrimitive.capability());
        v.push(UnstableSearchPrimitive.capability());
        v
    }

    async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError> {
        match capability_id {
            "search" => Ok(Box::new(SearchPrimitive)),
            "flaky_search" => Ok(Box::new(FlakySearchPrimitive::new(
                1,
                self.search_failure_class.clone(),
            ))),
            "list_source" => Ok(Box::new(ListSourcePrimitive::new(vec![
                "alpha".into(),
                "beta".into(),
                "gamma".into(),
            ]))),
            "irreversible" => Ok(Box::new(IrreversiblePrimitive)),
            "unstable_search" => Ok(Box::new(UnstableSearchPrimitive)),
            _ => self.inner.resolve(capability_id).await,
        }
    }

    async fn load(&self, m: PrimitiveManifest) -> Result<PrimitiveId, AcosError> {
        self.inner.load(m).await
    }

    async fn unload(&self, id: PrimitiveId) -> Result<(), AcosError> {
        self.inner.unload(id).await
    }
}
