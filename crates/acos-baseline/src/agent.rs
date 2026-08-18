//! Direct Tool-Loop Agent.
//!
//! The simplest possible LLM agent: system prompt + tools + LLM + loop.
//! No planner, no compiler, no replanner, no memory, no ACOS.

use acos_core::error::AcosError;
use acos_llm::{ChatMessage, LongCatClient};
use serde_json::Value;

use crate::evidence::EvidenceLog;
use crate::metrics::RunMetrics;
use crate::tools::{self, ToolCall};

/// Configuration for the tool-loop agent.
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Maximum number of LLM turns before giving up.
    pub max_turns: u32,
    /// Maximum output tokens per LLM call.
    pub max_tokens: u32,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_turns: 20,
            max_tokens: 4096,
        }
    }
}

/// Direct tool-loop agent.
pub struct ToolLoopAgent {
    llm: LongCatClient,
    config: AgentConfig,
    tools: Vec<tools::ToolDef>,
}

impl ToolLoopAgent {
    /// Creates a new tool-loop agent with the given LLM client.
    pub fn new(llm: LongCatClient, config: AgentConfig) -> Self {
        Self { llm, config, tools: tools::baseline_tools() }
    }

    /// Runs the agent on a task, returning metrics + final report + evidence.
    pub async fn run(
        &self,
        task_id: &str,
        goal: &str,
    ) -> Result<(RunMetrics, Option<String>, Vec<serde_json::Value>), AcosError> {
        let mut metrics = RunMetrics::new("direct-tool-loop", task_id);
        let mut evidence = EvidenceLog::new(task_id);
        let mut conversation: Vec<ChatMessage> = vec![];

        // Build system prompt with tool definitions
        let system = self.build_system_prompt();

        // Initial user message
        conversation.push(ChatMessage { role: "user".into(), content: goal.to_string() });

        let mut final_report: Option<String> = None;
        let mut distinct_tools = std::collections::HashSet::new();

        for _turn in 0..self.config.max_turns {
            // Call LLM
            let (assistant_text, input_chars, output_chars) = self.llm_call(&system, &conversation).await?;
            metrics.llm_calls += 1;
            metrics.estimated_tokens += (input_chars / 4) as u64; // rough estimate
            metrics.output_chars += output_chars as u64;

            evidence.add(crate::evidence::EvidenceItem::llm_call(
                self.llm.model(),
                (input_chars / 4) as u64,
                output_chars as u64,
            ));

            // Try to parse tool call
            if let Some(tool_call) = parse_tool_call(&assistant_text) {
                // Execute tool
                let result = tools::execute_tool(&tool_call).await;
                metrics.tool_calls += 1;
                distinct_tools.insert(tool_call.name.clone());

                if !result.success {
                    metrics.tool_failures += 1;
                }

                evidence.add(crate::evidence::EvidenceItem::tool_call(
                    &tool_call.name,
                    result.success,
                    result.output.len() as u64,
                ));

                // Add assistant message (tool call) and tool result to conversation
                conversation.push(ChatMessage {
                    role: "assistant".into(),
                    content: assistant_text,
                });
                conversation.push(ChatMessage {
                    role: "user".into(),
                    content: format!("Tool result for `{}`:\n{}", tool_call.name, result.output),
                });
            } else {
                // No tool call → this is the final answer
                final_report = Some(assistant_text.clone());
                metrics.reported_success = true;
                break;
            }
        }

        metrics.distinct_tools_used = distinct_tools.len() as u32;
        metrics.finish();

        // Build evidence for artifacts
        if let Some(ref report) = final_report {
            evidence.add(crate::evidence::EvidenceItem::artifact_stored(
                "report",
                "baseline_report.md",
                report.len() as u64,
            ));
            metrics.artifact_count = 1;
            metrics.final_report = Some(report.clone());
        }

        let evidence_items: Vec<serde_json::Value> = evidence.finish(
            if metrics.reported_success { "completed" } else { "incomplete" }
        ).into_iter().map(|e| serde_json::to_value(&e).unwrap_or_default()).collect();

        Ok((metrics, final_report, evidence_items))
    }

    /// Builds the system prompt describing available tools.
    fn build_system_prompt(&self) -> String {
        let mut prompt = String::from("You are a data analysis assistant. ");
        prompt.push_str("Analyze the task, use tools to read files and run code, ");
        prompt.push_str("then produce a final report.\n\n");
        prompt.push_str(&tools::format_tools_for_prompt(&self.tools));
        prompt.push_str("\nWhen you have completed the task, output the final report as plain text.\n");
        prompt.push_str("Do NOT wrap your final report in tool call markup.\n");
        prompt
    }

    /// Calls the LLM with the current conversation.
    /// Returns (assistant_text, input_chars, output_chars).
    async fn llm_call(
        &self,
        system: &str,
        conversation: &[ChatMessage],
    ) -> Result<(String, usize, usize), AcosError> {
        // For multi-turn, we build a single user message from conversation history
        // (simpler than true multi-turn API for baseline v0.1)
        let mut full_prompt = String::new();
        for msg in conversation {
            match msg.role.as_str() {
                "user" => full_prompt.push_str(&format!("[USER]\n{}\n\n", msg.content)),
                "assistant" => full_prompt.push_str(&format!("[ASSISTANT]\n{}\n\n", msg.content)),
                _ => {}
            }
        }

        let input_chars = system.len() + full_prompt.len();
        let response = self.llm.complete(system, &full_prompt).await?;
        let output_chars = response.len();

        Ok((response, input_chars, output_chars))
    }
}

/// Parses a tool call from LLM output.
/// Expects format: <tool_call>{"name": "...", "arguments": {...}}</tool_call>
/// Returns None if the output doesn't contain a valid tool call.
fn parse_tool_call(text: &str) -> Option<ToolCall> {
    // Look for <tool_call>...</tool_call> pattern
    let start = text.find("<tool_call>")?;
    let end = text.find("</tool_call>")?;
    let json_str = &text[start + "<tool_call>".len()..end];

    // Parse the JSON
    let value: Value = serde_json::from_str(json_str).ok()?;

    let name = value.get("name")?.as_str()?.to_string();
    let arguments = value.get("arguments")?.clone();

    Some(ToolCall { name, arguments })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_call_valid() {
        let text = "Let me read the file. <tool_call>{\"name\": \"read_file\", \"arguments\": {\"path\": \"data.csv\"}}</tool_call>";
        let call = parse_tool_call(text).unwrap();
        assert_eq!(call.name, "read_file");
        assert_eq!(call.arguments["path"], "data.csv");
    }

    #[test]
    fn parse_tool_call_no_call() {
        let text = "Here is my final report.\n\n# Summary\n...";
        assert!(parse_tool_call(text).is_none());
    }

    #[test]
    fn parse_tool_call_invalid_json() {
        let text = "<tool_call>not valid json</tool_call>";
        assert!(parse_tool_call(text).is_none());
    }
}