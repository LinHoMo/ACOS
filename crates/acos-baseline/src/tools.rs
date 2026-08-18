//! Tool implementations for the baseline agent.
//!
//! These mirror ACOS primitives to ensure fair comparison:
//! - `read_file` matches ACOS `read_file`
//! - `write_file` matches ACOS `write_file`
//! - `execute_python` matches ACOS `execute_python`

use serde::{Deserialize, Serialize};
use std::path::Path;

/// A tool definition (sent to LLM as JSON).
#[derive(Debug, Clone, Serialize)]
pub struct ToolDef {
    /// Tool name.
    pub name: String,
    /// Description for the LLM.
    pub description: String,
    /// Parameter schema (JSON Schema).
    pub parameters: serde_json::Value,
}

/// A tool call produced by the LLM.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolCall {
    /// Tool name.
    pub name: String,
    /// Tool arguments.
    pub arguments: serde_json::Value,
}

/// Result of executing a tool.
#[derive(Debug, Clone, Serialize)]
pub struct ToolResult {
    /// Whether execution succeeded.
    pub success: bool,
    /// Output text (stdout or error message).
    pub output: String,
}

/// Returns the baseline tool set (matches ACOS primitives).
pub fn baseline_tools() -> Vec<ToolDef> {
    vec![
        ToolDef {
            name: "read_file".into(),
            description: "Read a file from disk and return its contents. Use for CSV, text, or markdown files.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "Absolute or relative file path" }
                },
                "required": ["path"]
            }),
        },
        ToolDef {
            name: "write_file".into(),
            description: "Write content to a file on disk. Use to save reports or processed data.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": { "type": "string", "description": "File path to write" },
                    "content": { "type": "string", "description": "Content to write" }
                },
                "required": ["path", "content"]
            }),
        },
        ToolDef {
            name: "execute_python".into(),
            description: "Execute Python code and return stdout. Use for data analysis, CSV processing.".into(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "Python code to execute" }
                },
                "required": ["code"]
            }),
        },
    ]
}

/// Executes a tool call and returns the result.
pub async fn execute_tool(call: &ToolCall) -> ToolResult {
    match call.name.as_str() {
        "read_file" => read_file(&call.arguments).await,
        "write_file" => write_file(&call.arguments).await,
        "execute_python" => execute_python(&call.arguments).await,
        other => ToolResult {
            success: false,
            output: format!("unknown tool: {other}"),
        },
    }
}

async fn read_file(args: &serde_json::Value) -> ToolResult {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult {
            success: false,
            output: "read_file: missing 'path'".into(),
        };
    }
    match tokio::fs::read_to_string(path).await {
        Ok(content) => ToolResult {
            success: true,
            output: content,
        },
        Err(e) => ToolResult {
            success: false,
            output: format!("read_file error: {e}"),
        },
    }
}

async fn write_file(args: &serde_json::Value) -> ToolResult {
    let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
    if path.is_empty() {
        return ToolResult {
            success: false,
            output: "write_file: missing 'path'".into(),
        };
    }
    // Ensure parent directory exists
    if let Some(parent) = Path::new(path).parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    match tokio::fs::write(path, content).await {
        Ok(_) => ToolResult {
            success: true,
            output: format!("wrote {} bytes to {}", content.len(), path),
        },
        Err(e) => ToolResult {
            success: false,
            output: format!("write_file error: {e}"),
        },
    }
}

async fn execute_python(args: &serde_json::Value) -> ToolResult {
    let code = args.get("code").and_then(|v| v.as_str()).unwrap_or("");
    if code.is_empty() {
        return ToolResult {
            success: false,
            output: "execute_python: missing 'code'".into(),
        };
    }

    // Find python interpreter (cross-platform)
    let python = find_python();

    let python = match python {
        Some(p) => p,
        None => {
            return ToolResult {
                success: false,
                output: "execute_python: no python interpreter found".into(),
            }
        }
    };

    match tokio::process::Command::new(python)
        .arg("-c")
        .arg(code)
        .output()
        .await
    {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            if output.status.success() {
                ToolResult {
                    success: true,
                    output: stdout,
                }
            } else {
                ToolResult {
                    success: false,
                    output: format!("python exited {:?}: {}", output.status.code(), stderr),
                }
            }
        }
        Err(e) => ToolResult {
            success: false,
            output: format!("execute_python error: {e}"),
        },
    }
}

/// Cross-platform Python interpreter detection.
#[cfg(windows)]
fn find_python() -> Option<&'static str> {
    ["python", "python3", "py"]
        .iter()
        .find(|cmd| {
            std::process::Command::new("where")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .copied()
}

/// Cross-platform Python interpreter detection.
#[cfg(not(windows))]
fn find_python() -> Option<&'static str> {
    ["python3", "python"]
        .iter()
        .find(|cmd| {
            std::process::Command::new("which")
                .arg(cmd)
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .copied()
}

/// Formats tool definitions for the system prompt.
pub fn format_tools_for_prompt(tools: &[ToolDef]) -> String {
    let mut s = String::from("## Available Tools\n\n");
    s.push_str("To use a tool, call it using the native tool calling interface.\n\n");
    for tool in tools {
        s.push_str(&format!("### {}\n", tool.name));
        s.push_str(&format!("Description: {}\n", tool.description));
        s.push_str(&format!(
            "Parameters: {}\n\n",
            serde_json::to_string_pretty(&tool.parameters)
                .unwrap_or_default()
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn baseline_tools_has_three() {
        let tools = baseline_tools();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0].name, "read_file");
        assert_eq!(tools[1].name, "write_file");
        assert_eq!(tools[2].name, "execute_python");
    }

    #[tokio::test]
    async fn read_file_missing_path() {
        let call = ToolCall {
            name: "read_file".into(),
            arguments: serde_json::json!({}),
        };
        let result = execute_tool(&call).await;
        assert!(!result.success);
    }

    #[tokio::test]
    async fn write_file_roundtrip() {
        let path = "_test_output.txt";
        let call = ToolCall {
            name: "write_file".into(),
            arguments: serde_json::json!({ "path": path, "content": "hello" }),
        };
        let result = execute_tool(&call).await;
        assert!(result.success);
        let _ = tokio::fs::remove_file(path).await;
    }

    #[tokio::test]
    async fn execute_python_hello() {
        let call = ToolCall {
            name: "execute_python".into(),
            arguments: serde_json::json!({ "code": "print('hello')" }),
        };
        let result = execute_tool(&call).await;
        // Skip if python not available in test environment
        if !result.success
            && (result.output.contains("no python")
                || result.output.contains("Python was not found")
                || result.output.contains("exited Some(9009)"))
        {
            eprintln!("SKIP: python not available");
            return;
        }
        assert!(result.success, "output: {}", result.output);
        assert!(result.output.contains("hello"));
    }
}
