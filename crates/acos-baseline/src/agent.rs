//! Direct Tool-Loop Agent.
//!
//! The simplest possible LLM agent: system prompt + tools + LLM + loop.
//! No planner, no compiler, no replanner, no memory, no ACOS.
//!
//! Uses **native tool calling** (Anthropic `tool_use`) — not custom XML parsing.

use acos_core::error::AcosError;
use acos_llm::{
    assistant_message, tool_result_message, user_message, ChatMessage, LongCatClient, ToolDefinition,
};

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
        Self {
            llm,
            config,
            tools: tools::baseline_tools(),
        }
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
        conversation.push(user_message(goal));

        let mut final_report: Option<String> = None;
        let mut distinct_tools = std::collections::HashSet::new();

        // Build tool definitions for native tool calling
        let tool_defs: Vec<ToolDefinition> = self.tools.iter().map(|t| ToolDefinition {
            name: t.name.clone(),
            description: t.description.clone(),
            input_schema: t.parameters.clone(),
        }).collect();

        for _turn in 0..self.config.max_turns {
            // Call LLM with native tool calling
            let response = self
                .llm
                .chat_with_tools(&system, &conversation, Some(&tool_defs))
                .await?;

            metrics.llm_calls += 1;
            // Estimate tokens (rough: 1 token ≈ 4 chars)
            let input_chars: usize = conversation.iter().map(|m| {
                match &m.content {
                    acos_llm::MessageContent::Text(t) => t.len(),
                    acos_llm::MessageContent::Blocks(_) => 500, // rough estimate for blocks
                }
            }).sum();
            metrics.input_tokens += (input_chars / 4) as u64;
            metrics.output_tokens += (response.text.len() / 4) as u64
                + response.tool_calls.iter().map(|tc| tc.input.to_string().len() / 4).sum::<usize>() as u64;
            metrics.output_chars += response.text.len() as u64;

            evidence.add(crate::evidence::EvidenceItem::llm_call(
                self.llm.model(),
                (input_chars / 4) as u64,
                response.text.len() as u64,
            ));

            // If there are tool calls, execute them
            if response.has_tool_calls() {
                // Add assistant message to conversation
                conversation.push(assistant_message(&response.text));

                for tool_call in &response.tool_calls {
                    let call = ToolCall {
                        name: tool_call.name.clone(),
                        arguments: tool_call.input.clone(),
                    };
                    let result = tools::execute_tool(&call).await;
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

                    // Add tool result as user message (native format)
                    conversation.push(tool_result_message(&tool_call.id, &result.output));
                }
            } else {
                // No tool calls → this is the final answer
                final_report = Some(response.text.clone());
                metrics.self_reported_success = true;
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

        let evidence_items: Vec<serde_json::Value> = evidence
            .finish(if metrics.self_reported_success { "completed" } else { "incomplete" })
            .into_iter()
            .map(|e| serde_json::to_value(&e).unwrap_or_default())
            .collect();

        Ok((metrics, final_report, evidence_items))
    }

    /// Builds the system prompt — defines agent capabilities, NOT solution approach.
    fn build_system_prompt(&self) -> String {
        let mut prompt = String::from(
            "You are a general-purpose task agent.\n\n\
             You may use the following tools when needed.\n\
             Use tools only when necessary to complete the task accurately.\n\
             Return the final result when the task is complete.\n\n",
        );
        prompt.push_str(&tools::format_tools_for_prompt(&self.tools));
        prompt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_default() {
        let config = AgentConfig::default();
        assert_eq!(config.max_turns, 20);
        assert_eq!(config.max_tokens, 4096);
    }
}
