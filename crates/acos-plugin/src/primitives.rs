//! The five built-in MVP cognitive primitives.

use async_trait::async_trait;
use serde_json::Value;

use acos_core::error::AcosError;
use acos_core::traits::{CapabilityDesc, Primitive};
use acos_core::types::{EffectDecl, EffectKind};
use acos_core::types::{TypedValue, ValueType};

/// A no-op-ish primitive that always succeeds with the given capability.
///
/// The five MVP primitives are: `search`, `read_file`, `write_file`,
/// `execute_python`, `summarize`.

/// `search` — network read; returns an empty document list for MVP.
pub struct SearchPrimitive;

impl std::fmt::Debug for SearchPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SearchPrimitive")
    }
}

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
        // MVP: return an empty document list.
        Ok(TypedValue {
            value_type: ValueType::List,
            payload: serde_json::json!([]),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `read_file` — filesystem read.
pub struct ReadFilePrimitive;

impl std::fmt::Debug for ReadFilePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ReadFilePrimitive")
    }
}

#[async_trait]
impl Primitive for ReadFilePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "read_file".into(),
            name: "Read File".into(),
            input_type: "FileRef".into(),
            output_type: "Document".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsRead,
            description: "filesystem read".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let path = input
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "read_file requires a 'path' string".into(),
                primitive_id: Some("read_file".into()),
            })?;

        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to read {path}: {e}"),
                primitive_id: Some("read_file".into()),
            })?;

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "path": path, "content": content }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `write_file` — filesystem write (with delete compensation).
pub struct WriteFilePrimitive;

impl std::fmt::Debug for WriteFilePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "WriteFilePrimitive")
    }
}

#[async_trait]
impl Primitive for WriteFilePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "write_file".into(),
            name: "Write File".into(),
            input_type: "ArtifactWriteRequest".into(),
            output_type: "ArtifactRef".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![EffectDecl {
            kind: EffectKind::FsWrite,
            description: "filesystem write".into(),
            reversible: true,
        }]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let path = input
            .payload
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "write_file requires a 'path' string".into(),
                primitive_id: Some("write_file".into()),
            })?;

        let content = input
            .payload
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Ensure parent directory exists.
        if let Some(parent) = std::path::Path::new(path).parent() {
            tokio::fs::create_dir_all(parent).await.ok();
        }

        tokio::fs::write(path, content.as_bytes())
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to write {path}: {e}"),
                primitive_id: Some("write_file".into()),
            })?;

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "path": path }),
        })
    }

    fn has_compensation(&self, effect: &EffectDecl) -> bool {
        effect.kind == EffectKind::FsWrite
    }

    async fn compensate(&self, effect: &EffectDecl, input: TypedValue) -> Result<(), AcosError> {
        if effect.kind != EffectKind::FsWrite {
            return Ok(());
        }
        if let Some(path) = input.payload.get("path").and_then(Value::as_str) {
            tokio::fs::remove_file(path).await.ok();
        }
        Ok(())
    }
}

/// `execute_python` — process execution of Python code (process spawn).
///
/// Requires a `python3`/`python` on PATH. Fails clearly when unavailable.
pub struct ExecutePythonPrimitive;

impl std::fmt::Debug for ExecutePythonPrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "ExecutePythonPrimitive")
    }
}

#[async_trait]
impl Primitive for ExecutePythonPrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "execute_python".into(),
            name: "Execute Python".into(),
            input_type: "PythonExecutionRequest".into(),
            output_type: "ExecutionResult".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![
            EffectDecl {
                kind: EffectKind::ProcessSpawn,
                description: "process execution".into(),
                reversible: true,
            },
            EffectDecl {
                kind: EffectKind::FsRead,
                description: "optional filesystem/network access".into(),
                reversible: true,
            },
        ]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        let code = input
            .payload
            .get("code")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AcosError::PrimitiveFailure {
                message: "execute_python requires a 'code' string".into(),
                primitive_id: Some("execute_python".into()),
            })?;

        // Find a python interpreter.
        let python = ["python3", "python", "py"]
            .iter()
            .find(|cmd| which(cmd))
            .copied()
            .ok_or_else(|| AcosError::ProviderFailure {
                message: "no python interpreter found on PATH".into(),
                provider: "execute_python".into(),
            })?;

        let output = tokio::process::Command::new(python)
            .arg("-c")
            .arg(code)
            .output()
            .await
            .map_err(|e| AcosError::PrimitiveFailure {
                message: format!("failed to spawn python: {e}"),
                primitive_id: Some("execute_python".into()),
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

        if !output.status.success() {
            return Err(AcosError::PrimitiveFailure {
                message: format!("python exited {:?}: {stderr}", output.status.code()),
                primitive_id: Some("execute_python".into()),
            });
        }

        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "stdout": stdout, "stderr": stderr }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// `summarize` — text summarization (model inference for real deployments).
///
/// For the MVP, this performs a deterministic, dependency-free summarization:
/// it concatenates input documents and produces a compact summary. This keeps
/// the MVP verifiable without an LLM provider.
pub struct SummarizePrimitive;

impl std::fmt::Debug for SummarizePrimitive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SummarizePrimitive")
    }
}

#[async_trait]
impl Primitive for SummarizePrimitive {
    fn capability(&self) -> CapabilityDesc {
        CapabilityDesc {
            id: "summarize".into(),
            name: "Summarize".into(),
            input_type: "Document".into(),
            output_type: "Summary".into(),
        }
    }

    fn effects(&self) -> Vec<EffectDecl> {
        vec![]
    }

    async fn invoke(&self, input: TypedValue) -> Result<TypedValue, AcosError> {
        // Accept either a single document or a list of documents.
        let text = if let Some(docs) = input.payload.as_array() {
            docs.iter()
                .filter_map(|d| d.get("content").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n\n")
        } else {
            input
                .payload
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string()
        };

        let summary = summarize_text(&text);
        Ok(TypedValue {
            value_type: ValueType::Scalar,
            payload: serde_json::json!({ "summary": summary }),
        })
    }

    fn has_compensation(&self, _effect: &EffectDecl) -> bool {
        false
    }

    async fn compensate(&self, _effect: &EffectDecl, _input: TypedValue) -> Result<(), AcosError> {
        Ok(())
    }
}

/// Deterministic, dependency-free summarization for the MVP.
///
/// Counts non-empty lines and characters and returns a compact summary. This
/// is intentionally simple so the MVP is verifiable without an LLM.
fn summarize_text(text: &str) -> String {
    let lines: Vec<&str> = text.lines().filter(|l| !l.trim().is_empty()).collect();
    let char_count: usize = lines.iter().map(|l| l.len()).sum();
    let line_count = lines.len();

    let preview = lines
        .iter()
        .take(5)
        .map(|l| l.trim())
        .collect::<Vec<_>>()
        .join(" | ");

    format!(
        "Summary: {line_count} lines, {char_count} chars. Preview: {preview}"
    )
}

/// Returns true if `cmd` is resolvable on PATH.
fn which(cmd: &str) -> bool {
    std::process::Command::new(cmd)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_counts_lines_and_chars() {
        let s = summarize_text("alpha\nbeta\ngamma\n");
        assert!(s.contains("3 lines"));
        assert!(s.contains("alpha"));
    }
}
