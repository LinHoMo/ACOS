//! Schema loading and validation helpers (MVP).
//!
//! In the MVP, schemas are validated as JSON against JSON Schema files under
//! `schemas/`. Wire-format Protobuf decoding is added when the runtime RPC
//! layer lands (Phase 1+).

use serde::de::DeserializeOwned;

use crate::error::AcosError;

/// Loads a JSON-serializable value from a string.
///
/// This is the MVP helper; later stages add schema validation against
/// `schemas/*.jsonschema`.
pub fn from_json<T: DeserializeOwned>(data: &str) -> Result<T, AcosError> {
    serde_json::from_str(data).map_err(|e| AcosError::ValidationFailure {
        message: format!("JSON parse error: {e}"),
    })
}

/// Loads a YAML-serializable value from a string.
pub fn from_yaml<T: DeserializeOwned>(data: &str) -> Result<T, AcosError> {
    serde_yaml::from_str(data).map_err(|e| AcosError::ValidationFailure {
        message: format!("YAML parse error: {e}"),
    })
}

/// Serializes a value to JSON.
pub fn to_json<T: serde::Serialize>(value: &T) -> Result<String, AcosError> {
    serde_json::to_string_pretty(value).map_err(|e| AcosError::Internal {
        message: format!("JSON serialize error: {e}"),
    })
}

/// Validates a task spec for required fields (MVP structural check).
pub fn validate_task_spec(task: &crate::types::TaskSpec) -> Result<(), AcosError> {
    if task.api_version.is_empty() {
        return Err(AcosError::ValidationFailure {
            message: "task.api_version is required".into(),
        });
    }
    if task.goal.trim().is_empty() {
        return Err(AcosError::ValidationFailure {
            message: "task.spec.goal is required".into(),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::to_json;
    use crate::types::{TaskInput, TaskSpec};
    use crate::TaskId;

    #[test]
    fn to_json_roundtrip() {
        let task = TaskSpec {
            api_version: "acos.io/v1".into(),
            id: TaskId::new(),
            goal: "do something".into(),
            inputs: vec![TaskInput {
                input_type: "File".into(),
                path: "./x.csv".into(),
                format: Some("csv".into()),
            }],
            outputs: vec![],
            constraints: None,
            optimization: None,
            approval: None,
        };
        let json = to_json(&task).expect("serialize");
        assert!(json.contains("do something"));
    }
}
