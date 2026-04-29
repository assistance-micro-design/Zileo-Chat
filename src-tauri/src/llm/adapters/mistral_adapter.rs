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

    fn extract_thinking(&self, response: &Value) -> Option<String> {
        // Delegate to the shared extractor which handles all known formats:
        //   - message.reasoning (OpenRouter)
        //   - message.reasoning_content (vLLM/LM Studio)
        //   - message.reasoning_details[]
        //   - message.thinking (Ollama/proxy + Mistral top-level variant)
        //   - <think>...</think> tags in content string
        //   - content blocks array with {type:"thinking", thinking:string|array}
        //     covering Magistral (array) AND mistral-small reasoning_effort (string)
        // Mistral's response shape varies by model and reasoning_effort, so we
        // must accept any of these shapes - the previous content-blocks-only
        // extractor missed reasoning_content / thinking-top-level variants on
        // mistral-medium-3.5 and mistral-small (no thinking visible in UI).
        let message = response.pointer("/choices/0/message")?;
        let extracted = crate::llm::utils::extract_thinking_from_message(message);
        if extracted.is_none() {
            // Diagnostic: when reasoning_effort was sent but no thinking was
            // surfaced, log the message shape so we can extend the extractor
            // for new model variants without flying blind.
            if let Some(content) = message.get("content") {
                let shape = match content {
                    Value::String(_) => "string".to_string(),
                    Value::Array(arr) => format!(
                        "array(types={:?})",
                        arr.iter()
                            .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                    ),
                    Value::Null => "null".to_string(),
                    _ => "other".to_string(),
                };
                debug!(
                    provider = "mistral",
                    content_shape = %shape,
                    has_reasoning_field = message.get("reasoning").is_some(),
                    has_reasoning_content_field = message.get("reasoning_content").is_some(),
                    has_thinking_field = message.get("thinking").is_some(),
                    "No thinking extracted from Mistral response"
                );
            }
        }
        extracted
    }

    fn extract_content(&self, response: &Value) -> Option<String> {
        let content = response.pointer("/choices/0/message/content")?;

        // Standard format: content is a string
        if let Some(s) = content.as_str() {
            return Some(s.to_string());
        }

        // Reasoning format: content is an array of blocks, extract text blocks
        if let Some(blocks) = content.as_array() {
            let mut text_parts = String::new();
            for block in blocks {
                if block.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                        if !text_parts.is_empty() {
                            text_parts.push('\n');
                        }
                        text_parts.push_str(text);
                    }
                }
            }
            if !text_parts.is_empty() {
                return Some(text_parts);
            }
        }

        None
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
        // Mistral reasoning models return `content` as an array containing
        // ThinkChunk-shaped blocks plus text blocks, with extra fields
        // `signature` and `closed`. The Mistral API REJECTS that exact shape
        // when it appears in the input messages of a follow-up request:
        //   - `signature` is `extra_forbidden` on input ThinkChunk
        //   - `content` is expected to be a string OR a sanitized chunk list
        // We therefore rebuild a clean assistant message: keep `role` and
        // `tool_calls`, flatten `content` to the visible text portion using
        // `extract_content` (drops thinking blocks entirely). When the model
        // only emitted thinking + tool_calls, content becomes "" so the
        // tool_calls-only message stays valid for replay.
        let message = match response.pointer("/choices/0/message") {
            Some(m) => m,
            None => {
                return json!({"role": "assistant", "content": ""});
            }
        };

        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("assistant")
            .to_string();

        let content = match message.get("content") {
            Some(Value::String(s)) => s.clone(),
            Some(Value::Array(_)) => self.extract_content(response).unwrap_or_default(),
            _ => String::new(),
        };

        let mut out = json!({
            "role": role,
            "content": content,
        });

        if let Some(tool_calls) = message.get("tool_calls") {
            if let Some(obj) = out.as_object_mut() {
                obj.insert("tool_calls".to_string(), tool_calls.clone());
            }
        }

        out
    }

    // extract_usage: uses trait default from ProviderToolAdapter
    // which extracts all fields including cache_write_tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_tool_definition() -> ToolDefinition {
        ToolDefinition {
            id: "MemoryTool".to_string(),
            name: "Memory Tool".to_string(),
            summary: "Store and retrieve memory".to_string(),
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

        let usage = adapter.extract_usage(&response);
        assert_eq!(usage.input_tokens, 24);
        assert_eq!(usage.output_tokens, 27);
        assert_eq!(usage.cached_tokens, None);
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

        let usage = adapter.extract_usage(&response);
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
        assert_eq!(usage.cached_tokens, None);
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

        let usage = adapter.extract_usage(&response);
        assert_eq!(usage.input_tokens, 150);
        assert_eq!(usage.output_tokens, 45);
        assert_eq!(usage.cached_tokens, None);
    }

    #[test]
    fn test_extract_usage_with_cached_tokens() {
        let adapter = MistralToolAdapter::new();

        let response = json!({
            "usage": {
                "prompt_tokens": 1000,
                "completion_tokens": 200,
                "prompt_tokens_details": {
                    "cached_tokens": 800
                }
            }
        });

        let usage = adapter.extract_usage(&response);
        assert_eq!(usage.input_tokens, 1000);
        assert_eq!(usage.output_tokens, 200);
        assert_eq!(usage.cached_tokens, Some(800));
        assert_eq!(usage.cache_write_tokens, None);
    }

    /// Reproduces the exact `content` shape returned by mistral-medium-3.5
    /// (and Magistral) when reasoning is active. extract_thinking MUST find
    /// the thinking text so iteration.rs:127 can emit StreamChunk::thinking_block
    /// to the frontend (renders as "Reflexion" bloc).
    #[test]
    fn test_extract_thinking_magistral_array_format_with_signature() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": [
                                {"type": "text", "text": "L'utilisateur demande de lister les memoires."}
                            ],
                            "signature": null,
                            "closed": true
                        },
                        {"type": "text", "text": "Je vais lister."}
                    ],
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "MemoryTool", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let thinking = adapter.extract_thinking(&response);
        assert_eq!(
            thinking.as_deref(),
            Some("L'utilisateur demande de lister les memoires."),
            "extract_thinking must surface the visible thinking text from Magistral-shape blocks (signature/closed extras must not block extraction)"
        );
    }

    /// Some Mistral variants (and OpenRouter-relayed Mistral) surface the
    /// reasoning trace at `choices[0].message.reasoning` (string) instead of
    /// inside the content array. The Mistral adapter must accept this shape
    /// or the "Reflexion" bloc never reaches the frontend.
    #[test]
    fn test_extract_thinking_reasoning_field_string() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Final answer.",
                    "reasoning": "Step 1: I parse the request. Step 2: I call MemoryTool."
                }
            }]
        });
        assert_eq!(
            adapter.extract_thinking(&response).as_deref(),
            Some("Step 1: I parse the request. Step 2: I call MemoryTool.")
        );
    }

    /// vLLM-style reasoning_content surfaced by some Mistral deployments.
    #[test]
    fn test_extract_thinking_reasoning_content_field() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "OK",
                    "reasoning_content": "Internal trace here."
                }
            }]
        });
        assert_eq!(
            adapter.extract_thinking(&response).as_deref(),
            Some("Internal trace here.")
        );
    }

    /// Multi-text-chunk Magistral thinking: ensure all chunks are concatenated
    /// (not just the first). Real reasoning often spans multiple text chunks.
    #[test]
    fn test_extract_thinking_magistral_multiple_text_chunks_concatenated() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": [
                                {"type": "text", "text": "First step."},
                                {"type": "text", "text": "Second step."}
                            ],
                            "signature": null,
                            "closed": true
                        }
                    ]
                }
            }]
        });

        let thinking = adapter.extract_thinking(&response);
        assert_eq!(thinking.as_deref(), Some("First step.\nSecond step."));
    }

    /// Reproduces ERR_LLM_008: Mistral API rejects an assistant message
    /// echoed back with thinking blocks. The Magistral response shape is
    /// `content = [{type:"thinking", thinking:[{type:"text",text:"..."}], signature:null, closed:true}, {type:"text", text:"..."}]`
    /// and the next request must send a string `content` (or a sanitized
    /// list without ThinkChunk fields like `signature`/`closed`).
    #[test]
    fn test_build_assistant_message_strips_magistral_thinking_blocks() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": [{"type": "text", "text": "internal trace"}],
                            "signature": null,
                            "closed": true
                        },
                        {"type": "text", "text": "Final answer."}
                    ],
                    "tool_calls": [{
                        "id": "call_1",
                        "type": "function",
                        "function": {"name": "MemoryTool", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let msg = adapter.build_assistant_message(&response);

        assert_eq!(msg["role"], "assistant", "role must be preserved");
        assert!(
            msg["content"].is_string(),
            "content must be a string when echoing back to Mistral, got: {}",
            msg["content"]
        );
        assert_eq!(
            msg["content"].as_str().unwrap(),
            "Final answer.",
            "content must contain only the text portion, thinking stripped"
        );
        assert!(
            msg.get("tool_calls").is_some(),
            "tool_calls must be preserved"
        );
        assert_eq!(msg["tool_calls"][0]["id"], "call_1");
    }

    /// mistral-small with reasoning_effort returns thinking as a string
    /// (not an array). Same problem: signature/closed fields are rejected.
    #[test]
    fn test_build_assistant_message_strips_small_string_thinking_blocks() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": "internal reasoning",
                            "signature": null,
                            "closed": true
                        },
                        {"type": "text", "text": "Visible answer"}
                    ]
                },
                "finish_reason": "stop"
            }]
        });

        let msg = adapter.build_assistant_message(&response);

        assert!(msg["content"].is_string());
        assert_eq!(msg["content"].as_str().unwrap(), "Visible answer");
        // Thinking field must NOT bubble up (Mistral input schema rejects it).
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(
            !serialized.contains("\"signature\""),
            "signature must not appear in echoed assistant message"
        );
        assert!(
            !serialized.contains("\"thinking\""),
            "thinking blocks must be stripped"
        );
    }

    /// When the model only emits thinking + tool_calls (no visible text),
    /// the echoed `content` must be an empty string (Mistral input rejects
    /// arrays where ThinkChunk has signature/closed).
    #[test]
    fn test_build_assistant_message_only_thinking_yields_empty_content() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": [
                        {
                            "type": "thinking",
                            "thinking": [{"type": "text", "text": "trace only"}],
                            "signature": null,
                            "closed": true
                        }
                    ],
                    "tool_calls": [{
                        "id": "call_x",
                        "type": "function",
                        "function": {"name": "MemoryTool", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let msg = adapter.build_assistant_message(&response);

        assert!(msg["content"].is_string());
        assert_eq!(msg["content"].as_str().unwrap(), "");
        assert!(msg.get("tool_calls").is_some());
    }

    /// Standard string content (non-reasoning models) must pass through
    /// unchanged.
    #[test]
    fn test_build_assistant_message_passes_string_content_through() {
        let adapter = MistralToolAdapter::new();
        let response = json!({
            "choices": [{
                "message": {
                    "role": "assistant",
                    "content": "Plain text answer",
                    "tool_calls": [{
                        "id": "call_2",
                        "type": "function",
                        "function": {"name": "MemoryTool", "arguments": "{}"}
                    }]
                },
                "finish_reason": "tool_calls"
            }]
        });

        let msg = adapter.build_assistant_message(&response);

        assert_eq!(msg["role"], "assistant");
        assert_eq!(msg["content"], "Plain text answer");
        assert_eq!(msg["tool_calls"][0]["id"], "call_2");
    }

    /// Missing `choices[0].message` must still produce a valid empty
    /// assistant message (regression guard for the existing fallback).
    #[test]
    fn test_build_assistant_message_missing_message_fallback() {
        let adapter = MistralToolAdapter::new();
        let response = json!({"choices": []});
        let msg = adapter.build_assistant_message(&response);
        assert_eq!(msg["role"], "assistant");
        assert_eq!(msg["content"], "");
    }

    #[test]
    fn test_extract_usage_with_cache_write_tokens() {
        let adapter = MistralToolAdapter::new();

        let response = json!({
            "usage": {
                "prompt_tokens": 3000,
                "completion_tokens": 400,
                "prompt_tokens_details": {
                    "cached_tokens": 1500,
                    "cache_write_tokens": 1500
                }
            }
        });

        let usage = adapter.extract_usage(&response);
        assert_eq!(usage.input_tokens, 3000);
        assert_eq!(usage.output_tokens, 400);
        assert_eq!(usage.cached_tokens, Some(1500));
        assert_eq!(usage.cache_write_tokens, Some(1500));
    }
}
