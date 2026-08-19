//! ACOS LLM provider.
//!
//! A minimal client for the Anthropic Messages API, targeted at the LongCat
//! proxy (`https://api.longcat.chat/anthropic`), plus an OpenAI-compatible
//! Chat Completions transport for other providers (e.g. SenseNova). It is
//! intentionally dependency-light (reqwest only).
//!
//! Supports both plain completions and native tool calling (Anthropic
//! `tool_use` / OpenAI `tool_calls`).

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

/// Request body for `POST /v1/messages` (Anthropic).
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

/// Request body for `POST /chat/completions` (OpenAI-compatible).
#[derive(Debug, Serialize)]
struct ChatCompletionsRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<OpenAiMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OpenAiToolDef>>,
}

/// A message in the OpenAI Chat Completions format.
#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

/// A tool definition in the OpenAI format.
#[derive(Debug, Serialize)]
struct OpenAiToolDef {
    #[serde(rename = "type")]
    tool_type: String,
    function: OpenAiFunction,
}

#[derive(Debug, Serialize)]
struct OpenAiFunction {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

/// Response body from the OpenAI Chat Completions API.
#[derive(Debug, Deserialize)]
struct ChatCompletionsResponse {
    choices: Vec<ChatCompletionChoice>,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionChoice {
    message: OpenAiResponseMessage,
}

#[derive(Debug, Deserialize, Default)]
struct OpenAiResponseMessage {
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<OpenAiResponseToolCall>,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseToolCall {
    id: String,
    function: OpenAiResponseFunction,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponseFunction {
    name: String,
    #[serde(default)]
    arguments: String,
}

/// LLM provider transport flavor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    /// Anthropic Messages API (`POST {base}/v1/messages`).
    Anthropic,
    /// OpenAI-compatible Chat Completions API (`POST {base}/chat/completions`).
    OpenAi,
}

impl Provider {
    /// Parses `ACOS_LLM_PROVIDER` (`anthropic` | `openai`), defaulting to Anthropic.
    pub fn from_env() -> Self {
        match std::env::var("ACOS_LLM_PROVIDER").as_deref() {
            Ok("openai") => Provider::OpenAi,
            _ => Provider::Anthropic,
        }
    }
}

/// Default maximum output tokens.
///
/// P1-5B Probe-2b finding: LongCat-2.0 is a reasoning model that consumes
/// most of the output budget on `thinking` blocks. With `max_tokens = 4096`
/// the model hit `stop_reason: max_tokens` with *zero* text for complex
/// compile tasks (12/12 empty responses in Probe-2b). 16384 helped but is
/// still occasionally overrun (empty or truncated responses); 32768 leaves
/// reliable room for thinking + the CIR JSON.
const DEFAULT_MAX_TOKENS: u32 = 32768;

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

/// LLM client (Anthropic Messages or OpenAI-compatible transport).
///
/// Configure via environment (see [`LongCatClient::from_env`]):
/// - `ACOS_LLM_PROVIDER` — `anthropic` (default) | `openai`
/// - `LONGCAT_API_KEY` (or `ANTHROPIC_API_KEY`; OpenAI mode also accepts `OPENAI_API_KEY`) — bearer key
/// - `LONGCAT_BASE_URL` — Anthropic mode defaults to `https://api.longcat.chat/anthropic`;
///   OpenAI mode defaults to `https://api.openai.com/v1` (e.g. SenseNova: `https://token.sensenova.cn/v1`)
/// - `ACOS_LLM_MODEL` — defaults to `LongCat-2.0`
/// - `ACOS_LLM_MAX_TOKENS` — max output tokens, defaults to 16384
#[derive(Debug, Clone)]
pub struct LongCatClient {
    http: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    max_tokens: u32,
    provider: Provider,
}

impl LongCatClient {
    /// Builds a client from environment variables.
    ///
    /// # Errors
    /// Returns [`AcosError::ValidationFailure`] if no API key is configured.
    pub fn from_env() -> Result<Self, AcosError> {
        let provider = Provider::from_env();

        let api_key = match provider {
            Provider::OpenAi => std::env::var("OPENAI_API_KEY")
                .or_else(|_| std::env::var("LONGCAT_API_KEY"))
                .or_else(|_| std::env::var("ANTHROPIC_API_KEY")),
            Provider::Anthropic => std::env::var("LONGCAT_API_KEY")
                .or_else(|_| std::env::var("ANTHROPIC_API_KEY")),
        }
        .map_err(|_| AcosError::ValidationFailure {
            message: "set LONGCAT_API_KEY (or ANTHROPIC_API_KEY / OPENAI_API_KEY) to use the model planner".into(),
        })?;

        let base_url = match provider {
            Provider::Anthropic => {
                std::env::var("LONGCAT_BASE_URL")
                    .unwrap_or_else(|_| "https://api.longcat.chat/anthropic".into())
            }
            Provider::OpenAi => {
                std::env::var("LONGCAT_BASE_URL")
                    .unwrap_or_else(|_| "https://api.openai.com/v1".into())
            }
        };

        let model =
            std::env::var("ACOS_LLM_MODEL").unwrap_or_else(|_| "LongCat-2.0".into());

        let max_tokens = std::env::var("ACOS_LLM_MAX_TOKENS")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(DEFAULT_MAX_TOKENS);

        Ok(Self {
            http: reqwest::Client::builder()
                // SenseNova gateway rejects HTTP/2 requests (lowercase
                // `authorization` header not recognized -> 401 Forbidden).
                // Force HTTP/1.1; harmless for Anthropic/LongCat.
                .http1_only()
                .build()
                .map_err(|e| AcosError::ExternalSystemFailure {
                    message: format!("http client init failed: {e}"),
                    system: Some("llm".into()),
                })?,
            api_key,
            base_url,
            model,
            max_tokens,
            provider,
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
            max_tokens: DEFAULT_MAX_TOKENS,
            provider: Provider::Anthropic,
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
        match self.provider {
            Provider::Anthropic => self.chat_anthropic(system, messages, tools).await,
            Provider::OpenAi => self.chat_openai(system, messages, tools).await,
        }
    }

    async fn chat_anthropic(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<ChatResponse, AcosError> {
        let req = MessagesRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
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

    async fn chat_openai(
        &self,
        system: &str,
        messages: &[ChatMessage],
        tools: Option<&[ToolDefinition]>,
    ) -> Result<ChatResponse, AcosError> {
        let mut openai_messages = Vec::with_capacity(messages.len() + 1);
        openai_messages.push(OpenAiMessage {
            role: "system".into(),
            content: Some(system.to_string()),
            tool_call_id: None,
        });
        for m in messages {
            let mapped = match &m.content {
                MessageContent::Text(text) => vec![OpenAiMessage {
                    role: m.role.clone(),
                    content: Some(text.clone()),
                    tool_call_id: None,
                }],
                MessageContent::Blocks(blocks) => blocks
                    .iter()
                    .filter(|b| b.block_type == "tool_result")
                    .map(|b| OpenAiMessage {
                        role: "tool".into(),
                        content: b.content.clone(),
                        tool_call_id: b.tool_use_id.clone(),
                    })
                    .collect(),
            };
            openai_messages.extend(mapped);
        }

        let tool_defs = tools.map(|ts| {
            ts.iter()
                .map(|t| OpenAiToolDef {
                    tool_type: "function".into(),
                    function: OpenAiFunction {
                        name: t.name.clone(),
                        description: t.description.clone(),
                        parameters: t.input_schema.clone(),
                    },
                })
                .collect()
        });

        let req = ChatCompletionsRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: openai_messages,
            tools: tool_defs,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("content-type", "application/json")
            .json(&req)
            .send()
            .await
            .map_err(|e| AcosError::ExternalSystemFailure {
                message: format!("LLM request failed: {e}"),
                system: Some("openai".into()),
            })?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(AcosError::ExternalSystemFailure {
                message: format!("LLM API error {status}: {body}"),
                system: Some("openai".into()),
            });
        }

        let parsed: ChatCompletionsResponse =
            resp.json()
                .await
                .map_err(|e| AcosError::ExternalSystemFailure {
                    message: format!("LLM response parse error: {e}"),
                    system: Some("openai".into()),
                })?;

        let message = parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message)
            .unwrap_or_default();

        let mut tool_calls = Vec::new();
        for tc in &message.tool_calls {
            let input = serde_json::from_str(&tc.function.arguments)
                .unwrap_or_else(|_| serde_json::json!({ "raw": tc.function.arguments }));
            tool_calls.push(LlmToolCall {
                id: tc.id.clone(),
                name: tc.function.name.clone(),
                input,
            });
        }

        Ok(ChatResponse {
            text: message.content.unwrap_or_default(),
            tool_calls,
        })
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

    #[test]
    fn provider_from_env_defaults_to_anthropic() {
        std::env::remove_var("ACOS_LLM_PROVIDER");
        assert_eq!(Provider::from_env(), Provider::Anthropic);
    }

    #[test]
    fn provider_from_env_parses_openai() {
        std::env::set_var("ACOS_LLM_PROVIDER", "openai");
        assert_eq!(Provider::from_env(), Provider::OpenAi);
        std::env::remove_var("ACOS_LLM_PROVIDER");
    }

    #[test]
    fn openai_response_parses_tool_calls() {
        let json = r#"{
            "choices": [{
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": { "name": "read_file", "arguments": "{\"path\": \"data.csv\"}" }
                    }]
                }
            }]
        }"#;
        let parsed: ChatCompletionsResponse = serde_json::from_str(json).unwrap();
        let message = parsed.choices.into_iter().next().unwrap().message;
        assert!(message.content.is_none());
        assert_eq!(message.tool_calls.len(), 1);
        assert_eq!(message.tool_calls[0].id, "call_1");
        assert_eq!(message.tool_calls[0].function.name, "read_file");
    }

    #[test]
    fn openai_request_serializes_system_and_tool_result() {
        let req = ChatCompletionsRequest {
            model: "sensenova-6.8-flash-lite".into(),
            max_tokens: 8192,
            messages: vec![
                OpenAiMessage { role: "system".into(), content: Some("sys".into()), tool_call_id: None },
                OpenAiMessage { role: "user".into(), content: Some("hi".into()), tool_call_id: None },
                OpenAiMessage { role: "tool".into(), content: Some("result".into()), tool_call_id: Some("t1".into()) },
            ],
            tools: Some(vec![OpenAiToolDef {
                tool_type: "function".into(),
                function: OpenAiFunction {
                    name: "read_file".into(),
                    description: "Read a file".into(),
                    parameters: serde_json::json!({"type": "object"}),
                },
            }]),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["messages"][0]["role"], "system");
        assert_eq!(json["messages"][2]["role"], "tool");
        assert_eq!(json["messages"][2]["tool_call_id"], "t1");
        assert_eq!(json["tools"][0]["type"], "function");
        assert_eq!(json["tools"][0]["function"]["name"], "read_file");
    }
}
