//! ACOS LLM provider.
//!
//! A minimal client for the Anthropic Messages API, targeted at the LongCat
//! proxy (`https://api.longcat.chat/anthropic`). It is intentionally
//! dependency-light (reqwest only) so the same code works against the real
//! Anthropic endpoint by changing the base URL.
//!
//! Supports both plain completions and native tool calling (Anthropic `tool_use`).

#![warn(missing_docs)]

use acos_core::error::AcosError;
use serde::{Deserialize, Serialize};

/// A chat message in the Messages API format.
///
/// `content` can be plain text (for user/assistant messages) or structured
/// blocks (for tool results).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    /// `"user"` or `"assistant"`.
    pub role: String,
    /// Text content, or structured content blocks for tool results.
    #[serde(with = "content_serde")]
    pub content: MessageContent,
}

/// Message content that can be text or structured blocks.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text content.
    Text(String),
    /// Structured content blocks (for tool results).
    Blocks(Vec<ContentBlockOut>),
}

/// A content block in an assistant's response or tool result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlockOut {
    /// Block type (`"text"` or `"tool_result"`).
    #[serde(rename = "type")]
    pub block_type: String,
    /// Text payload when `type == "text"`.
    #[serde(default)]
    pub text: Option<String>,
    /// Tool use id when `type == "tool_result"`.
    #[serde(default)]
    pub tool_use_id: Option<String>,
    /// Tool result content when `type == "tool_result"`.
    #[serde(default)]
    pub content: Option<String>,
}

/// Helper module for serializing/deserializing MessageContent.
mod content_serde {
    use super::{ContentBlockOut, MessageContent};
    use serde::{self, Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S>(content: &MessageContent, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match content {
            MessageContent::Text(text) => serializer.serialize_str(text),
            MessageContent::Blocks(blocks) => <Vec<ContentBlockOut> as Serialize>::serialize(blocks, serializer),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<MessageContent, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;
        if value.is_string() {
            let text = value.as_str().unwrap_or("").to_string();
            Ok(MessageContent::Text(text))
        } else {
            let blocks = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            Ok(MessageContent::Blocks(blocks))
        }
    }
}

/// Creates a user message with plain text.
pub fn user_message(text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".into(),
        content: MessageContent::Text(text.into()),
    }
}

/// Creates an assistant message with plain text.
pub fn assistant_message(text: &str) -> ChatMessage {
    ChatMessage {
        role: "assistant".into(),
        content: MessageContent::Text(text.into()),
    }
}

/// Creates a tool result message (user role with structured content).
pub fn tool_result_message(tool_use_id: &str, result_text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".into(),
        content: MessageContent::Blocks(vec![ContentBlockOut {
            block_type: "tool_result".into(),
            text: None,
            tool_use_id: Some(tool_use_id.into()),
            content: Some(result_text.into()),
        }]),
    }
}

/// A tool definition for the Anthropic tool-use API.
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    /// Tool name (must match `^[a-zA-Z0-9_-]{1,64}$`).
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for the input.
    pub input_schema: serde_json::Value,
}

/// A tool call returned by the LLM (Anthropic `tool_use` content block).
#[derive(Debug, Clone, Deserialize)]
pub struct LlmToolCall {
    /// Unique tool call id.
    pub id: String,
    /// Tool name.
    pub name: String,
    /// Tool input (JSON object).
    pub input: serde_json::Value,
}

/// Response from a chat completion that may include tool calls.
#[derive(Debug, Clone, Default)]
pub struct ChatResponse {
    /// Text content (may be empty if only tool calls).
    pub text: String,
    /// Tool calls requested by the LLM (empty if none).
    pub tool_calls: Vec<LlmToolCall>,
}

impl ChatResponse {
    /// Returns `true` if the response contains at least one tool call.
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }
}

/// Request body for `POST /v1/messages`.
#[derive(Debug, Serialize)]
struct MessagesRequest {
    /// Model id (e.g. `"LongCat-2.0"`).
    model: String,
    /// Maximum tokens to generate.
    max_tokens: u32,
    /// System prompt.
    system: String,
    /// Conversation messages.
    messages: Vec<ChatMessage>,
    /// Tool definitions (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ToolDefinition>>,
}

/// Response body from the Messages API.
#[derive(Debug, Deserialize)]
struct MessagesResponse {
    /// Generated content blocks.
    content: Vec<ContentBlock>,
}

/// A single content block in the response.
#[derive(Debug, Deserialize)]
struct ContentBlock {
    /// Block type (`"text"` or `"tool_use"`).
    #[serde(rename = "type")]
    block_type: String,
    /// Text payload when `type == "text"`.
    text: Option<String>,
    /// Tool call id when `type == "tool_use"`.
    id: Option<String>,
    /// Tool name when `type == "tool_use"`.
    name: Option<String>,
    /// Tool input when `type == "tool_use"`.
    input: Option<serde_json::Value>,
}

/// LongCat / Anthropic Messages API client.
///
/// Configure via environment (see [`LongCatClient::from_env`]):
/// - `LONGCAT_API_KEY` (or `ANTHROPIC_API_KEY`) — bearer <_REDACTED>
/// - `LONGCAT_BASE_URL` — defaults to `https://api.longcat.chat/anthropic`
/// - `ACOS_LLM_MODEL` — defaults to `LongCat-2.0`
#[derive(Debug, Clone)]
pub struct LongCatClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl LongCatClient {
    /// Builds a client from environment variables.
    ///
    /// # Errors
    /// Returns [`AcosError::ValidationFailure`] if no API key is configured.
    pub fn from_env() -> Result<Self, AcosError> {
        let api_key = std::env::var("LONGCAT_API_KEY")
            .or_else(|_| std::env::var("ANTHROPIC_API_KEY"))
            .map_err(|_| AcosError::ValidationFailure {
                message: "set LONGCAT_API_KEY (or ANTHROPIC_API_KEY) to use the model planner".into(),
            })?;

        let base_url = std::env::var("LONGCAT_BASE_URL")
            .unwrap_or_else(|_| "https://api.longcat.chat/anthropic".into());

        let model =
            std::env::var("ACOS_LLM_MODEL").unwrap_or_else(|_| "LongCat-2.0".into());

        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url,
            model,
        })
    }

    /// Creates a client that always fails on `complete` (for tests that only
    /// exercise parsing/prompt-building without network access).
    pub fn dummy() -> Self {
        Self {
            http: reqwest::Client::new(),
            api_key: String::new(),
            base_url: "http://127.0.0.1:1".into(),
            model: "dummy".into(),
        }
    }

    /// Sends a single-turn chat completion and returns the assistant text.
    ///
    /// `system` is the system prompt; `user` is the user instruction.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String, AcosError> {
        let resp = self
            .chat_with_tools(
                system,
                &[ChatMessage {
                    role: "user".into(),
                    content: MessageContent::Text(user.into()),
                }],
                None,
            )
            .await?;
        Ok(resp.text)
    }

    /// Sends a chat completion with native tool calling support.
    ///
    /// Returns a [`ChatResponse`] containing both text and any tool calls.
    /// If `tools` is `None` or empty, behaves like a plain completion.
    pub async fn chat_with_tools(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<ChatResponse, AcosError> {
        let req = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system.to_string(),
            messages: messages.to_vec(),
            tools: tools.map(|t| t.to_vec()),
        };

        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| AcosError::ExternalSystemFailure {
                message: format!("LLM request failed: {e}"),
                system: Some("longcat".into()),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AcosError::ExternalSystemFailure {
                message: format!("LLM API error {status}: {body}"),
                system: Some("longcat".into()),
            });
        }

        let parsed: MessagesResponse =
            resp.json()
                .await
                .map_err(|e| AcosError::ExternalSystemFailure {
                    message: format!("LLM response parse error: {e}"),
                    system: Some("longcat".into()),
                })?;

        let mut text = String::new();
        let mut tool_calls = Vec::new();

        for block in &parsed.content {
            match block.block_type.as_str() {
                "text" => {
                    if let Some(t) = &block.text {
                        text.push_str(t);
                    }
                }
                "tool_use" => {
                    if let (Some(id), Some(name), Some(input)) = (&block.id, &block.name, &block.input) {
                        tool_calls.push(LlmToolCall {
                            id: id.clone(),
                            name: name.clone(),
                            input: input.clone(),
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(ChatResponse { text, tool_calls })
    }

    /// Returns the configured model id.
    pub fn model(&self) -> &str {
        &self.model
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_message_serializes() {
        let msg = user_message("hello");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"hello\""));
    }

    #[test]
    fn chat_message_tool_result_serializes() {
        let msg = tool_result_message("tool_1", "file contents here");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"type\":\"tool_result\""));
        assert!(json.contains("\"tool_use_id\":\"tool_1\""));
        assert!(json.contains("\"content\":\"file contents here\""));
    }

    #[test]
    fn chat_response_has_tool_calls() {
        let resp = ChatResponse::default();
        assert!(!resp.has_tool_calls());

        let resp = ChatResponse {
            text: "".into(),
            tool_calls: vec![LlmToolCall {
                id: "tool_1".into(),
                name: "read_file".into(),
                input: serde_json::json!({"path": "data.csv"}),
            }],
        };
        assert!(resp.has_tool_calls());
    }
}
