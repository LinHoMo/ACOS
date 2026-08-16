//! ACOS LLM provider.
//!
//! A minimal client for the Anthropic Messages API, targeted at the LongCat
//! proxy (`https://api.longcat.chat/anthropic`). It is intentionally
//! dependency-light (reqwest only) so the same code works against the real
//! Anthropic endpoint by changing the base URL.

#![warn(missing_docs)]

use acos_core::error::AcosError;
use serde::{Deserialize, Serialize};

/// A chat message in the Messages API format.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    /// `"user"` or `"assistant"`.
    pub role: String,
    /// Text content.
    pub content: String,
}

/// Request body for `POST /v1/messages`.
#[derive(Debug, Serialize)]
struct MessagesRequest {
    /// Model id (e.g. `"claude-sonnet-4-5-20250929"`).
    model: String,
    /// Maximum tokens to generate.
    max_tokens: u32,
    /// System prompt.
    system: String,
    /// Conversation (single user turn is enough for planning).
    messages: Vec<ChatMessage>,
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
    /// Block type (e.g. `"text"`).
    #[serde(rename = "type")]
    block_type: String,
    /// Text payload when `type == "text"`.
    text: Option<String>,
}

/// LongCat / Anthropic Messages API client.
///
/// Configure via environment (see [`LongCatClient::from_env`]):
/// - `LONGCAT_API_KEY` (or `ANTHROPIC_API_KEY`) — bearer <_REDACTED>
/// - `LONGCAT_BASE_URL` — defaults to `https://api.longcat.chat/anthropic`
/// - `ACOS_LLM_MODEL` — defaults to `claude-sonnet-4-5-20250929`
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
            std::env::var("ACOS_LLM_MODEL").unwrap_or_else(|_| "claude-sonnet-4-5-20250929".into());

        Ok(Self {
            http: reqwest::Client::new(),
            api_key,
            base_url,
            model,
        })
    }

    /// Sends a single-turn chat completion and returns the assistant text.
    ///
    /// `system` is the system prompt; `user` is the user instruction.
    pub async fn complete(&self, system: &str, user: &str) -> Result<String, AcosError> {
        let req = MessagesRequest {
            model: self.model.clone(),
            max_tokens: 4096,
            system: system.to_string(),
            messages: vec![ChatMessage {
                role: "user".into(),
                content: user.to_string(),
            }],
        };

        let url = format!("{}/v1/messages", self.base_url);
        let resp = self
            .http
            .post(&url)
            .header("x-api-key", &self.api_key)
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

        let text = parsed
            .content
            .iter()
            .find(|c| c.block_type == "text")
            .and_then(|c| c.text.clone())
            .unwrap_or_default();

        Ok(text)
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
        let msg = ChatMessage {
            role: "user".into(),
            content: "hello".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"role\":\"user\""));
        assert!(json.contains("\"content\":\"hello\""));
    }
}
