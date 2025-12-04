//! Ollama Tool Adapter
//!
//! Implements the ProviderToolAdapter trait for Ollama's OpenAI-compatible tool API.
//!
//! ## Ollama API Specifics
//!
//! - Uses OpenAI-compatible format but with some differences
//! - `arguments` in tool_calls are JSON objects (not strings!)
//! - No native `tool_call_id` - we generate synthetic IDs
//! - `tool_choice` is not well supported - returns null
//! - Response path: `message.tool_calls` (not choices[0].message)
//! - Tool support varies by model (qwen2.5, llama3.1+, mistral work best)

use crate::llm::tool_adapter::{helpers, ProviderToolAdapter};
use crate::models::function_calling::{FunctionCall, FunctionCallResult, ToolChoiceMode};
use crate::tools::ToolDefinition;
use serde_json::{json, Value};
use tracing::{debug, warn};

/// Adapter for Ollama's OpenAI-compatible tool calling API.
///
/// Handles conversion between our internal tool system and Ollama's API format.
///
/// # Model Compatibility
/// Not all Ollama models support tool calling. Known compatible models:
/// - qwen2.5 (recommended)
/// - llama3.1, llama3.2
/// - mistral, mistral-nemo
/// - command-r
///
/// # Example
/// ```ignore
/// let adapter = OllamaToolAdapter;
/// let tools_json = adapter.format_tools(&[tool_def]);
/// let calls = adapter.parse_tool_calls(&response);
/// ```
#[derive(Debug, Clone, Default)]
pub struct OllamaToolAdapter;

impl OllamaToolAdapter {
    /// Creates a new Ollama tool adapter.
    pub fn new() -> Self {
        Self
    }
}

impl ProviderToolAdapter for OllamaToolAdapter {
    fn format_tools(&self, tools: &[ToolDefinition]) -> Vec<Value> {
        // Ollama uses the same format as OpenAI/Mistral
        tools.iter().map(helpers::tool_definition_to_json).collect()
    }

    fn parse_tool_calls(&self, response: &Value) -> Vec<FunctionCall> {
        // Ollama response structure: { "message": { "tool_calls": [...] } }
        // (different from Mistral's choices[0].message.tool_calls)
        let tool_calls = response
            .pointer("/message/tool_calls")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        if tool_calls.is_empty() {
            debug!("No tool calls found in Ollama response");
            return Vec::new();
        }

        tool_calls
            .iter()
            .filter_map(|tc| {
                let name = tc
                    .pointer("/function/name")
                    .and_then(|v| v.as_str())?
                    .to_string();

                // Ollama returns arguments as a JSON OBJECT (not string!)
                // This is a key difference from Mistral
                let arguments = match tc.pointer("/function/arguments") {
                    Some(obj @ Value::Object(_)) => obj.clone(),
                    Some(Value::String(args_str)) => {
                        // Fallback: some versions might return string
                        helpers::parse_arguments_string(args_str)
                    }
                    Some(other) => {
                        warn!(
                            tool = %name,
                            value = %other,
                            "Unexpected arguments type in Ollama response"
                        );
                        json!({})
                    }
                    None => {
                        warn!(tool = %name, "Missing arguments in Ollama tool call");
                        json!({})
                    }
                };

                // Ollama doesn't provide tool_call_id - generate synthetic one
                let id = helpers::generate_call_id("ollama");

                debug!(
                    tool = %name,
                    call_id = %id,
                    "Parsed Ollama tool call (synthetic ID)"
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
        // Ollama format for tool results
        // Note: tool_call_id is included for consistency, though Ollama may ignore it
        json!({
            "role": "tool",
            "content": helpers::result_to_string(result)
        })
    }

    fn get_tool_choice(&self, _mode: ToolChoiceMode) -> Value {
        // Ollama doesn't well support tool_choice parameter
        // Return null to let it use default behavior
        Value::Null
    }

    fn provider_name(&self) -> &'static str {
        "ollama"
    }

    fn extract_content(&self, response: &Value) -> Option<String> {
        // Ollama response: { "message": { "content": "..." } }
        response
            .pointer("/message/content")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    }

    fn has_tool_calls(&self, response: &Value) -> bool {
        response
            .pointer("/message/tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false)
    }

    fn is_finished(&self, response: &Value) -> bool {
        // Ollama uses "done" field to indicate completion
        let is_done = response
            .get("done")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Also check for tool calls
        if self.has_tool_calls(response) {
            // If there are tool calls, we're not finished (need to process them)
            return false;
        }

        is_done
    }

    fn build_assistant_message(&self, response: &Value) -> Value {
        // Ollama: extract message directly (not nested in choices)
        response.get("message").cloned().unwrap_or_else(|| {
            json!({
                "role": "assistant",
                "content": ""
            })
        })
    }

    /// Extracts token usage from Ollama's response format.
    ///
    /// Ollama uses different field names than OpenAI:
    /// - `prompt_eval_count` = input tokens
    /// - `eval_count` = output tokens
    fn extract_usage(&self, response: &Value) -> (usize, usize) {
        let input = response
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let output = response
            .get("eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        debug!(
            prompt_eval_count = input,
            eval_count = output,
            "Extracted token usage from Ollama response"
        );

        (input, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool_definition() -> ToolDefinition {
        ToolDefinition {
            id: "TodoTool".to_string(),
            name: "Todo Tool".to_string(),
            description: "Manage todo items".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "action": {"type": "string", "enum": ["add", "list", "complete"]}
                },
                "required": ["action"]
            }),
            output_schema: json!({}),
            requires_confirmation: false,
        }
    }

    #[test]
    fn test_format_tools() {
        let adapter = OllamaToolAdapter::new();
        let tools = vec![sample_tool_definition()];
        let json = adapter.format_tools(&tools);

        assert_eq!(json.len(), 1);
        assert_eq!(json[0]["type"], "function");
        assert_eq!(json[0]["function"]["name"], "TodoTool");
    }

    #[test]
    fn test_parse_tool_calls_object_arguments() {
        let adapter = OllamaToolAdapter::new();

        // Ollama returns arguments as OBJECT, not string
        let response = json!({
            "model": "qwen2.5",
            "message": {
                "role": "assistant",
                "content": "",
                "tool_calls": [{
                    "function": {
                        "name": "TodoTool",
                        "arguments": {"action": "add", "title": "Test task"}
                    }
                }]
            },
            "done": false
        });

        let calls = adapter.parse_tool_calls(&response);
        assert_eq!(calls.len(), 1);
        assert!(calls[0].id.starts_with("ollama_")); // Synthetic ID
        assert_eq!(calls[0].name, "TodoTool");
        assert_eq!(calls[0].arguments["action"], "add");
        assert_eq!(calls[0].arguments["title"], "Test task");
    }

    #[test]
    fn test_parse_tool_calls_no_calls() {
        let adapter = OllamaToolAdapter::new();
        let response = json!({
            "model": "llama3.1",
            "message": {
                "role": "assistant",
                "content": "Here's your answer."
            },
            "done": true
        });

        let calls = adapter.parse_tool_calls(&response);
        assert!(calls.is_empty());
    }

    #[test]
    fn test_format_tool_result() {
        let adapter = OllamaToolAdapter::new();
        let result = FunctionCallResult::success(
            "ollama_123",
            "TodoTool",
            json!({"task_id": "abc", "status": "created"}),
        );

        let formatted = adapter.format_tool_result(&result);
        assert_eq!(formatted["role"], "tool");
        assert!(formatted["content"].as_str().unwrap().contains("task_id"));
    }

    #[test]
    fn test_tool_choice_returns_null() {
        let adapter = OllamaToolAdapter::new();
        // All modes return null for Ollama
        assert!(adapter.get_tool_choice(ToolChoiceMode::Auto).is_null());
        assert!(adapter.get_tool_choice(ToolChoiceMode::Required).is_null());
        assert!(adapter.get_tool_choice(ToolChoiceMode::None).is_null());
    }

    #[test]
    fn test_has_tool_calls() {
        let adapter = OllamaToolAdapter::new();

        let with_tools = json!({
            "message": {"tool_calls": [{"function": {"name": "test"}}]}
        });
        assert!(adapter.has_tool_calls(&with_tools));

        let without_tools = json!({
            "message": {"content": "Hello"}
        });
        assert!(!adapter.has_tool_calls(&without_tools));
    }

    #[test]
    fn test_is_finished() {
        let adapter = OllamaToolAdapter::new();

        let not_done = json!({
            "message": {"content": "thinking..."},
            "done": false
        });
        assert!(!adapter.is_finished(&not_done));

        let done = json!({
            "message": {"content": "Done!"},
            "done": true
        });
        assert!(adapter.is_finished(&done));

        let with_tool_calls = json!({
            "message": {"tool_calls": [{"function": {"name": "test"}}]},
            "done": false
        });
        assert!(!adapter.is_finished(&with_tool_calls));
    }

    #[test]
    fn test_extract_content() {
        let adapter = OllamaToolAdapter::new();

        let with_content = json!({
            "message": {"content": "Hello world"}
        });
        assert_eq!(
            adapter.extract_content(&with_content),
            Some("Hello world".to_string())
        );

        let empty_content = json!({
            "message": {"content": ""}
        });
        assert!(adapter.extract_content(&empty_content).is_none());
    }

    #[test]
    fn test_extract_usage_with_tokens() {
        let adapter = OllamaToolAdapter::new();

        // Ollama response format with token counts
        let response = json!({
            "model": "llama3.2",
            "message": {"role": "assistant", "content": "Hello!"},
            "done": true,
            "prompt_eval_count": 42,
            "eval_count": 15
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 42);
        assert_eq!(output, 15);
    }

    #[test]
    fn test_extract_usage_without_tokens() {
        let adapter = OllamaToolAdapter::new();

        // Response without token fields
        let response = json!({
            "model": "llama3.2",
            "message": {"role": "assistant", "content": "Hello!"},
            "done": true
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 0);
        assert_eq!(output, 0);
    }

    #[test]
    fn test_extract_usage_partial_tokens() {
        let adapter = OllamaToolAdapter::new();

        // Response with only output tokens
        let response = json!({
            "model": "llama3.2",
            "message": {"content": "Hello!"},
            "eval_count": 25
        });

        let (input, output) = adapter.extract_usage(&response);
        assert_eq!(input, 0);
        assert_eq!(output, 25);
    }
}
