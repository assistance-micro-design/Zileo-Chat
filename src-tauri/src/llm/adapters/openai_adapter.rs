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

//! OpenAI-Compatible Tool Adapter
//!
//! Implements the ProviderToolAdapter trait for standard OpenAI-compatible APIs.
//!
//! ## API Specifics
//!
//! - Tool definitions follow OpenAI format exactly
//! - `arguments` in tool_calls are JSON strings (need parsing)
//! - `tool_call_id` is provided natively
//! - `tool_choice` supports: "auto", "required", "none"
//! - Response path: `choices[0].message.tool_calls`
//!
//! ## Difference from Mistral
//!
//! The only functional difference is `tool_choice`:
//! - Mistral uses `"any"` for required mode
//! - OpenAI standard uses `"required"` for required mode

use crate::llm::tool_adapter::{helpers, ProviderToolAdapter};
use crate::models::function_calling::{FunctionCall, FunctionCallResult, ToolChoiceMode};
use crate::tools::ToolDefinition;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Adapter for OpenAI-compatible function calling APIs.
///
/// Used by all custom providers (RouterLab, OpenRouter, Together AI, etc.).
/// Identical to MistralToolAdapter except for `tool_choice` values.
#[derive(Debug, Clone, Default)]
pub struct OpenAiToolAdapter;

impl OpenAiToolAdapter {
    /// Creates a new OpenAI-compatible tool adapter.
    pub fn new() -> Self {
        Self
    }
}

impl ProviderToolAdapter for OpenAiToolAdapter {
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
            debug!("No tool calls found in OpenAI-compatible response");
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

                let arguments = match tc.pointer("/function/arguments") {
                    Some(Value::String(args_str)) => {
                        match serde_json::from_str::<Value>(args_str) {
                            Ok(parsed) => parsed,
                            Err(e) => {
                                warn!(
                                    tool = %name,
                                    error = %e,
                                    args = %args_str,
                                    "Failed to parse tool arguments JSON string"
                                );
                                json!({})
                            }
                        }
                    }
                    Some(obj @ Value::Object(_)) => obj.clone(),
                    Some(other) => {
                        warn!(
                            tool = %name,
                            value = %other,
                            "Unexpected arguments type in response"
                        );
                        json!({})
                    }
                    None => {
                        warn!(tool = %name, "Missing arguments in tool call");
                        json!({})
                    }
                };

                debug!(
                    tool = %name,
                    call_id = %id,
                    "Parsed OpenAI-compatible tool call"
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
            ToolChoiceMode::Required => json!("required"), // OpenAI standard (not "any" like Mistral)
            ToolChoiceMode::None => json!("none"),
        }
    }

    fn provider_name(&self) -> &'static str {
        "openai_compatible"
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
        let finish_reason = response
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str());

        match finish_reason {
            Some("tool_calls") => false,
            Some("stop") | Some("end_turn") | Some("length") => true,
            None => !self.has_tool_calls(response),
            _ => true,
        }
    }

    fn build_assistant_message(&self, response: &Value) -> Value {
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
            "Extracted token usage from OpenAI-compatible response"
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
        let adapter = OpenAiToolAdapter::new();
        let tools = vec![sample_tool_definition()];
        let json = adapter.format_tools(&tools);

        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["type"], "function");
        assert_eq!(json[0]["function"]["name"], "MemoryTool");
    }

    #[test]
    fn test_tool_choice_modes() {
        let adapter = OpenAiToolAdapter::new();
        assert_eq!(adapter.get_tool_choice(ToolChoiceMode::Auto), json!("auto"));
        assert_eq!(
            adapter.get_tool_choice(ToolChoiceMode::Required),
            json!("required") // NOT "any" like Mistral
        );
        assert_eq!(adapter.get_tool_choice(ToolChoiceMode::None), json!("none"));
    }

    #[test]
    fn test_parse_tool_calls() {
        let adapter = OpenAiToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call_abc123",
                        "type": "function",
                        "function": {
                            "name": "MemoryTool",
                            "arguments": "{\"operation\":\"add\"}"
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
    }

    #[test]
    fn test_extract_usage() {
        let adapter = OpenAiToolAdapter::new();
        let response = json!({
            "usage": {
                "prompt_tokens": 100,
                "completion_tokens": 50,
                "total_tokens": 150
            }
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 100);
        assert_eq!(output, 50);
    }
}
