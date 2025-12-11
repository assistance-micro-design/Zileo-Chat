// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Mistral Tool Adapter
//!
//! Implements the ProviderToolAdapter trait for Mistral AI's function calling API.
//!
//! ## Mistral API Specifics
//!
//! - Tool definitions follow OpenAI format exactly
//! - `arguments` in tool_calls are JSON strings (need parsing)
//! - `tool_call_id` is provided natively
//! - `tool_choice` supports: "auto", "any" (required), "none"
//! - Response path: `choices[0].message.tool_calls`

use crate::llm::tool_adapter::{helpers, ProviderToolAdapter};
use crate::models::function_calling::{FunctionCall, FunctionCallResult, ToolChoiceMode};
use crate::tools::ToolDefinition;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Adapter for Mistral AI's function calling API.
///
/// Handles conversion between our internal tool system and Mistral's API format.
///
/// # Example
/// ```ignore
/// let adapter = MistralToolAdapter;
/// let tools_json = adapter.format_tools(&[tool_def]);
/// let calls = adapter.parse_tool_calls(&response);
/// ```
#[derive(Debug, Clone, Default)]
pub struct MistralToolAdapter;

impl MistralToolAdapter {
    /// Creates a new Mistral tool adapter.
    pub fn new() -> Self {
        Self
    }
}

impl ProviderToolAdapter for MistralToolAdapter {
    fn format_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        tools.iter().map(helpers::tool_definition_to_json).collect()
    }

    fn parse_tool_calls(&self, response: &Value) -> Vec<FunctionCall> {
        let tool_calls = response
            .pointer("/choices/0/message/tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if tool_calls.is_empty() {
            debug!("No tool calls found in Mistral response");
            return Vec::new();
        }

        tool_calls
            .iter()
            .filter_map(|tc| {
                let id = tc.get("id").and_then(|v| v.as_str())?.to_string();
                let name = tc
                    .pointer("/function/name")
                    .and_then(|v| v.as_str())?
                    .to_string();

                // Mistral returns arguments as a JSON STRING - need to parse it
                let arguments = match tc.pointer("/function/arguments") {
                    Some(Value::String(args_str)) => {
                        match serde_json::from_str::<Value>(args_str) {
                            Ok(parsed) => parsed,
                            Err(e) => {
                                warn!(
                                    tool = %name,
                                    error = %e,
                                    args = %args_str,
                                    "Failed to parse Mistral tool arguments JSON string"
                                );
                                json!({})
                            }
                        }
                    }
                    Some(obj @ Value::Object(_)) => {
                        // Some Mistral models might return object directly
                        obj.clone()
                    }
                    Some(other) => {
                        warn!(
                            tool = %name,
                            value = %other,
                            "Unexpected arguments type in Mistral response"
                        );
                        json!({})
                    }
                    None => {
                        warn!(tool = %name, "Missing arguments in Mistral tool call");
                        json!({})
                    }
                };

                debug!(
                    tool = %name,
                    call_id = %id,
                    "Parsed Mistral tool call"
                );

                Some(FunctionCall {
                    id,
                    name,
                    arguments,
                })
            })
            .collect()
    }

    fn format_tool_result(&self, result: &FunctionCallResult) -> Value {
        json!({
            "role": "tool",
            "tool_call_id": &result.call_id,
            "name": &result.function_name,
            "content": helpers::result_to_string(result)
        })
    }

    fn get_tool_choice(&self, mode: ToolChoiceMode) -> Value {
        match mode {
            ToolChoiceMode::Auto => json!("auto"),
            ToolChoiceMode::Required => json!("any"), // Mistral uses "any" for required
            ToolChoiceMode::None => json!("none"),
        }
    }

    fn provider_name(&self) -> &'static str {
        "mistral"
    }

    fn extract_content(&self, response: &Value) -> Option<String> {
        response
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    }

    fn has_tool_calls(&self, response: &Value) -> bool {
        response
            .pointer("/choices/0/message/tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    }

    fn is_finished(&self, response: &Value) -> bool {
        // Mistral uses finish_reason: "tool_calls" when tools are called
        let finish_reason = response
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str());

        match finish_reason {
            Some("tool_calls") => false, // More tool calls expected
            Some("stop") | Some("end_turn") | Some("length") => true, // Generation finished
            None => !self.has_tool_calls(response), // Fallback: check for tool calls
            _ => true,                   // Unknown finish reason - assume finished
        }
    }

    fn build_assistant_message(&self, response: &Value) -> Value {
        // Mistral: extract choices[0].message which contains role, content, and tool_calls
        response
            .pointer("/choices/0/message")
            .cloned()
            .unwrap_or_else(|| {
                json!({
                    "role": "assistant",
                    "content": ""
                })
            })
    }

    /// Extracts token usage from Mistral's response format.
    ///
    /// Mistral uses OpenAI-compatible format:
    /// - `usage.prompt_tokens` = input tokens
    /// - `usage.completion_tokens` = output tokens
    fn extract_usage(&self, response: &Value) -> (usize, usize) {
        let input = response
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let output = response
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        debug!(
            prompt_tokens = input,
            completion_tokens = output,
            "Extracted token usage from Mistral response"
        );

        (input, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool_definition() -> ToolDefinition {
        ToolDefinition {
            id: "MemoryTool".to_string(),
            name: "Memory Tool".to_string(),
            description: "Store and retrieve memory".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string", "enum": ["add", "search"]}
                },
                "required": ["operation"]
            }),
            output_schema: json!({}),
            requires_confirmation: false,
        }
    }

    #[test]
    fn test_format_tools() {
        let adapter = MistralToolAdapter::new();
        let tools = vec![sample_tool_definition()];
        let json = adapter.format_tools(&tools);

        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["type"], "function");
        assert_eq!(json[0]["function"]["name"], "MemoryTool");
    }

    #[test]
    fn test_parse_tool_calls_string_arguments() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "I'll help you.",
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "MemoryTool",
                            "arguments": "{\"operation\":\"add\",\"content\":\"test\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let calls = adapter.parse_tool_calls(&response);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_abc123");
        assert_eq!(calls[0].name, "MemoryTool");
        assert_eq!(calls[0].arguments["operation"], "add");
    }

    #[test]
    fn test_parse_tool_calls_no_calls() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello, how can I help?"
                },
                "finish_reason": "stop"
            }]
        });

        let calls = adapter.parse_tool_calls(&response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_format_tool_result() {
        let adapter = MistralToolAdapter::new();
        let result =
            FunctionCallResult::success("call_abc123", "MemoryTool", json!({"memory_id": "123"}));

        let formatted = adapter.format_tool_result(&result);
        assert_eq!(formatted["role"], "tool");
        assert_eq!(formatted["tool_call_id"], "call_abc123");
        assert_eq!(formatted["name"], "MemoryTool");
    }

    #[test]
    fn test_tool_choice_modes() {
        let adapter = MistralToolAdapter::new();
        assert_eq!(adapter.get_tool_choice(ToolChoiceMode::Auto), json!("auto"));
        assert_eq!(
            adapter.get_tool_choice(ToolChoiceMode::Required),
            json!("any")
        );
        assert_eq!(adapter.get_tool_choice(ToolChoiceMode::None), json!("none"));
    }

    #[test]
    fn test_has_tool_calls() {
        let adapter = MistralToolAdapter::new();

        let with_tools = json!({
            "choices": [{"message": {"tool_calls": [{"id": "1"}]}}]
        });
        assert!(adapter.has_tool_calls(&with_tools));

        let without_tools = json!({
            "choices": [{"message": {"content": "Hello"}}]
        });
        assert!(!adapter.has_tool_calls(&without_tools));
    }

    #[test]
    fn test_is_finished() {
        let adapter = MistralToolAdapter::new();

        let tool_calls_response = json!({
            "choices": [{"finish_reason": "tool_calls"}]
        });
        assert!(!adapter.is_finished(&tool_calls_response));

        let stop_response = json!({
            "choices": [{"finish_reason": "stop"}]
        });
        assert!(adapter.is_finished(&stop_response));
    }

    #[test]
    fn test_extract_usage_with_tokens() {
        let adapter = MistralToolAdapter::new();

        // Mistral response format with usage object
        let response = json!({
            "id": "chat-abc123",
            "object": "chat.completion",
            "model": "mistral-large-latest",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 24,
                "completion_tokens": 27,
                "total_tokens": 51
            }
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 24);
        assert_eq!(output, 27);
    }

    #[test]
    fn test_extract_usage_without_tokens() {
        let adapter = MistralToolAdapter::new();

        // Response without usage object
        let response = json!({
            "id": "chat-abc123",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Hello!"
                },
                "finish_reason": "stop"
            }]
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 0);
        assert_eq!(output, 0);
    }

    #[test]
    fn test_extract_usage_with_tool_calls() {
        let adapter = MistralToolAdapter::new();

        // Response with tool calls and usage
        let response = json!({
            "id": "chat-abc123",
            "model": "mistral-large-latest",
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_123",
                        "function": {
                            "name": "MemoryTool",
                            "arguments": "{\"operation\":\"add\"}"
                        }
                    }]
                },
                "finish_reason": "tool_calls"
            }],
            "usage": {
                "prompt_tokens": 150,
                "completion_tokens": 45,
                "total_tokens": 195
            }
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 150);
        assert_eq!(output, 45);
    }
}
