//! The built-in plugin registry.
//!
//! Holds the five MVP primitives and implements the `PluginRegistry` trait.

use async_trait::async_trait;

use acos_core::error::AcosError;
use acos_core::id::PrimitiveId;
use acos_core::traits::{CapabilityDesc, PluginRegistry, Primitive, PrimitiveManifest};
use crate::primitives::{
    CsvAggregatePrimitive, CsvInspectSchemaPrimitive, ExecutePythonPrimitive, ReadFilePrimitive,
    SearchPrimitive, SummarizePrimitive, WriteFilePrimitive,
};

/// A registry preloaded with the built-in MVP primitives.
#[derive(Debug, Clone)]
pub struct BuiltinRegistry {
    capabilities: Vec<CapabilityDesc>,
}

impl BuiltinRegistry {
    /// Creates a registry with all built-in primitives registered.
    pub fn new() -> Self {
        // SummarizePrimitive now carries an optional LLM client, so build an
        // instance to read its capability descriptor.
        let summarize = SummarizePrimitive::new();
        let capabilities = vec![
            SearchPrimitive.capability(),
            ReadFilePrimitive.capability(),
            WriteFilePrimitive.capability(),
            ExecutePythonPrimitive.capability(),
            CsvInspectSchemaPrimitive.capability(),
            CsvAggregatePrimitive.capability(),
            summarize.capability(),
        ];
        Self { capabilities }
    }
}

impl Default for BuiltinRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl PluginRegistry for BuiltinRegistry {
    fn list(&self) -> Vec<CapabilityDesc> {
        self.capabilities.clone()
    }

    async fn resolve(&self, capability_id: &str) -> Result<Box<dyn Primitive>, AcosError> {
        match capability_id {
            "search" => Ok(Box::new(SearchPrimitive)),
            "read_file" => Ok(Box::new(ReadFilePrimitive)),
            "write_file" => Ok(Box::new(WriteFilePrimitive)),
            "execute_python" => Ok(Box::new(ExecutePythonPrimitive)),
            "csv.inspect_schema" => Ok(Box::new(CsvInspectSchemaPrimitive)),
            "csv.aggregate" => Ok(Box::new(CsvAggregatePrimitive)),
            "summarize" => Ok(Box::new(SummarizePrimitive::new())),
            other => Err(AcosError::ValidationFailure {
                message: format!("unknown primitive capability: {other}"),
            }),
        }
    }

    async fn load(&self, manifest: PrimitiveManifest) -> Result<PrimitiveId, AcosError> {
        let id = &manifest.id;
        if self.capabilities.iter().any(|c| &c.id == id) {
            Ok(PrimitiveId::new())
        } else {
            Err(AcosError::ValidationFailure {
                message: format!("cannot load unknown primitive: {id}"),
            })
        }
    }

    async fn unload(&self, _id: PrimitiveId) -> Result<(), AcosError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn resolve_and_invoke_read_file_and_summarize() {
        std::env::remove_var("LONGCAT_API_KEY");
        std::env::remove_var("ANTHROPIC_API_KEY");
        let reg = BuiltinRegistry::new();
        assert_eq!(reg.list().len(), 7);

        let summarize = reg.resolve("summarize").await.unwrap();
        let input = acos_core::types::TypedValue {
            value_type: acos_core::types::ValueType::Scalar,
            payload: serde_json::json!({ "content": "hello\nworld" }),
        };
        let out = summarize.invoke(input).await.unwrap();
        let summary = out.payload.get("summary").unwrap().as_str().unwrap();
        assert!(summary.contains("hello"));
    }
}
