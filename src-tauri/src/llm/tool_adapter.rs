//! Provider Tool Adapter Trait
//!
//! This module defines the trait for adapting between our internal tool system
//! and the JSON function calling format used by different LLM providers.
//!
//! Each provider (Mistral, Ollama) has different JSON formats for:
//! - Tool definitions sent to the API
//! - Tool calls received in responses
//! - Tool results sent back
//!
//! The adapters handle these differences transparently.

use crate::models::function_calling::{FunctionCall, FunctionCallResult, ToolChoiceMode};
use crate::tools::ToolDefinition;
use serde_json::Value;

/// Trait for adapting between internal tool system and provider-specific JSON formats.
///
/// Implementors handle the format differences between providers like Mistral and Ollama.
///
/// # Responsibilities
/// - Convert our `ToolDefinition` to provider-specific JSON format
/// - Parse `tool_calls` from provider responses
/// - Format tool results for sending back to the provider
/// - Handle `tool_choice` parameter differences
///
/// # Example
/// ```ignore
/// let adapter = MistralToolAdapter;
/// let tools_json = adapter.format_tools(&tool_definitions);
/// let calls = adapter.parse_tool_calls(&response);
/// ```
#[allow(dead_code)] // Some trait methods are for API completeness and future use
pub trait ProviderToolAdapter: Send + Sync {
    /// Converts internal `ToolDefinition` structs to provider-specific JSON format.
    ///
    /// # Arguments
    /// * `tools` - Slice of tool definitions from our internal system
    ///
    /// # Returns
    /// Vector of JSON objects in the provider's expected format
    ///
    /// # Format (OpenAI/Mistral standard)
    /// ```json
    /// {
    ///   "type": "function",
    ///   "function": {
    ///     "name": "ToolName",
    ///     "description": "Tool description",
    ///     "parameters": { /* JSON Schema */ }
    ///   }
    /// }
    /// ```
    fn format_tools(&self, tools: &[ToolDefinition]) -> Vec<Value>;

    /// Parses tool calls from the provider's response.
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// Vector of parsed `FunctionCall` structs
    ///
    /// # Provider Differences
    /// - **Mistral**: `arguments` is a JSON string that needs parsing
    /// - **Ollama**: `arguments` is already a JSON object
    fn parse_tool_calls(&self, response: &Value) -> Vec<FunctionCall>;

    /// Formats a tool execution result for sending back to the provider.
    ///
    /// # Arguments
    /// * `result` - The execution result from our tool system
    ///
    /// # Returns
    /// JSON object in the provider's expected "tool" message format
    ///
    /// # Format (OpenAI/Mistral standard)
    /// ```json
    /// {
    ///   "role": "tool",
    ///   "tool_call_id": "call_abc123",
    ///   "name": "ToolName",
    ///   "content": "{\"success\": true, ...}"
    /// }
    /// ```
    fn format_tool_result(&self, result: &FunctionCallResult) -> Value;

    /// Converts our `ToolChoiceMode` to the provider's specific format.
    ///
    /// # Arguments
    /// * `mode` - The desired tool choice behavior
    ///
    /// # Returns
    /// Provider-specific JSON value for the `tool_choice` parameter
    ///
    /// # Provider Differences
    /// - **Mistral**: `"auto"`, `"any"` (for required), `"none"`
    /// - **Ollama**: Not used (returns `null`)
    fn get_tool_choice(&self, mode: ToolChoiceMode) -> Value;

    /// Returns the provider name for logging and debugging.
    fn provider_name(&self) -> &'static str;

    /// Extracts the text content from the provider's response.
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// The text content if present, `None` if the response only contains tool calls
    fn extract_content(&self, response: &Value) -> Option<String>;

    /// Checks if the response contains tool calls.
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// `true` if the response contains tool calls
    fn has_tool_calls(&self, response: &Value) -> bool;

    /// Checks if the response indicates tool use is finished.
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// `true` if the finish_reason indicates normal completion (not tool_calls)
    fn is_finished(&self, response: &Value) -> bool {
        // Default implementation: finished if no tool calls
        !self.has_tool_calls(response)
    }

    /// Builds an assistant message from the provider's response for conversation history.
    ///
    /// This extracts the assistant message (including tool_calls) from the response
    /// to be added to the messages array for multi-turn conversations.
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// JSON object representing the assistant message in provider-specific format
    fn build_assistant_message(&self, response: &Value) -> Value;

    /// Extracts token usage from the provider's response.
    ///
    /// Different providers return token counts in different formats:
    /// - **Mistral**: `usage.prompt_tokens`, `usage.completion_tokens`
    /// - **Ollama**: `prompt_eval_count`, `eval_count` (at root level)
    ///
    /// # Arguments
    /// * `response` - The raw JSON response from the provider
    ///
    /// # Returns
    /// Tuple of (input_tokens, output_tokens). Returns (0, 0) if not available.
    fn extract_usage(&self, response: &Value) -> (usize, usize) {
        // Default implementation: try OpenAI/Mistral format
        let input = response
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        let output = response
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;
        (input, output)
    }
}

/// Helper functions for common adapter operations.
pub mod helpers {
    use super::*;
    use serde_json::json;
    use uuid::Uuid;

    /// Converts a `ToolDefinition` to OpenAI-style JSON format.
    ///
    /// This is the standard format used by Mistral and Ollama (OpenAI-compatible).
    pub fn tool_definition_to_json(tool: &ToolDefinition) -> Value {
        json!({
            "type": "function",
            "function": {
                "name": &tool.id,
                "description": &tool.description,
                "parameters": &tool.input_schema
            }
        })
    }

    /// Generates a synthetic tool call ID for providers that don't provide one.
    ///
    /// Format: `{provider_prefix}_{uuid}`
    pub fn generate_call_id(provider_prefix: &str) -> String {
        format!("{}_{}", provider_prefix, Uuid::new_v4())
    }

    /// Formats a tool result as a JSON string.
    ///
    /// This is used by providers that expect `content` as a string.
    pub fn result_to_string(result: &FunctionCallResult) -> String {
        if result.success {
            serde_json::to_string(&result.result).unwrap_or_else(|_| "{}".to_string())
        } else {
            json!({
                "error": result.error.as_deref().unwrap_or("Unknown error")
            })
            .to_string()
        }
    }

    /// Parses JSON arguments from a string.
    ///
    /// Returns empty object if parsing fails.
    pub fn parse_arguments_string(args_str: &str) -> Value {
        serde_json::from_str(args_str).unwrap_or(json!({}))
    }
}

#[cfg(test)]
mod tests {
    use super::helpers::*;
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_definition_to_json() {
        let tool = ToolDefinition {
            id: "TestTool".to_string(),
            name: "Test Tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                },
                "required": ["param"]
            }),
            output_schema: json!({}),
            requires_confirmation: false,
        };

        let json = tool_definition_to_json(&tool);

        assert_eq!(json["type"], "function");
        assert_eq!(json["function"]["name"], "TestTool");
        assert_eq!(json["function"]["description"], "A test tool");
        assert!(json["function"]["parameters"].is_object());
    }

    #[test]
    fn test_generate_call_id() {
        let id1 = generate_call_id("mistral");
        let id2 = generate_call_id("ollama");

        assert!(id1.starts_with("mistral_"));
        assert!(id2.starts_with("ollama_"));
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_result_to_string_success() {
        let result = FunctionCallResult::success("id", "TestTool", json!({"data": "value"}));
        let string = result_to_string(&result);
        assert!(string.contains("data"));
        assert!(string.contains("value"));
    }

    #[test]
    fn test_result_to_string_failure() {
        let result = FunctionCallResult::failure("id", "TestTool", "Something went wrong");
        let string = result_to_string(&result);
        assert!(string.contains("error"));
        assert!(string.contains("Something went wrong"));
    }

    #[test]
    fn test_parse_arguments_string() {
        let valid = parse_arguments_string(r#"{"key": "value"}"#);
        assert_eq!(valid["key"], "value");

        let invalid = parse_arguments_string("not json");
        assert!(invalid.is_object());
        assert!(invalid.as_object().unwrap().is_empty());
    }
}
