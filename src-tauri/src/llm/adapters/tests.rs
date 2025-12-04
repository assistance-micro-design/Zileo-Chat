//! Integration tests for tool adapters
//!
//! Tests that verify the adapters work correctly with realistic API responses.

use super::*;
use crate::llm::tool_adapter::ProviderToolAdapter;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::tools::ToolDefinition;
use serde_json::json;

/// Creates a realistic MemoryTool definition for testing.
fn memory_tool_definition() -> ToolDefinition {
    ToolDefinition {
        id: "MemoryTool".to_string(),
        name: "Memory Tool".to_string(),
        description: "Store and retrieve contextual memory for the conversation.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "operation": {
                    "type": "string",
                    "enum": ["add", "search", "get", "list", "delete"],
                    "description": "The memory operation to perform"
                },
                "memory_type": {
                    "type": "string",
                    "enum": ["user_pref", "context", "knowledge", "decision"],
                    "description": "Type of memory (for add operation)"
                },
                "content": {
                    "type": "string",
                    "description": "Content to store (for add operation)"
                },
                "query": {
                    "type": "string",
                    "description": "Search query (for search operation)"
                },
                "memory_id": {
                    "type": "string",
                    "description": "Memory ID (for get/delete operations)"
                }
            },
            "required": ["operation"]
        }),
        output_schema: json!({
            "type": "object",
            "properties": {
                "success": {"type": "boolean"},
                "memory_id": {"type": "string"},
                "memories": {"type": "array"}
            }
        }),
        requires_confirmation: false,
    }
}

/// Creates a realistic MCP tool definition for testing.
fn mcp_tool_definition() -> ToolDefinition {
    ToolDefinition {
        id: "mcp__serena__find_symbol".to_string(),
        name: "Find Symbol".to_string(),
        description: "Find symbols in the codebase by name pattern.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name_path_pattern": {
                    "type": "string",
                    "description": "Symbol name or pattern to search for"
                },
                "relative_path": {
                    "type": "string",
                    "description": "Restrict search to this path"
                }
            },
            "required": ["name_path_pattern"]
        }),
        output_schema: json!({}),
        requires_confirmation: false,
    }
}

#[test]
fn test_mistral_realistic_response_single_tool() {
    let adapter = MistralToolAdapter::new();

    // Realistic Mistral API response
    let response = json!({
        "id": "cmpl-abc123",
        "object": "chat.completion",
        "created": 1733234567,
        "model": "mistral-large-latest",
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": "I'll store this information for you.",
                "tool_calls": [{
                    "id": "call_9f8e7d6c5b4a3",
                    "type": "function",
                    "function": {
                        "name": "MemoryTool",
                        "arguments": "{\"operation\":\"add\",\"memory_type\":\"knowledge\",\"content\":\"The user prefers dark mode.\"}"
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

    let calls = adapter.parse_tool_calls(&response);
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].id, "call_9f8e7d6c5b4a3");
    assert_eq!(calls[0].name, "MemoryTool");
    assert_eq!(calls[0].arguments["operation"], "add");
    assert_eq!(calls[0].arguments["memory_type"], "knowledge");

    // Check other adapter methods
    assert!(adapter.has_tool_calls(&response));
    assert!(!adapter.is_finished(&response));
    assert_eq!(
        adapter.extract_content(&response),
        Some("I'll store this information for you.".to_string())
    );
}

#[test]
fn test_mistral_realistic_response_multiple_tools() {
    let adapter = MistralToolAdapter::new();

    // Multiple tool calls in one response
    let response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": null,
                "tool_calls": [
                    {
                        "id": "call_1",
                        "type": "function",
                        "function": {
                            "name": "MemoryTool",
                            "arguments": "{\"operation\":\"search\",\"query\":\"preferences\"}"
                        }
                    },
                    {
                        "id": "call_2",
                        "type": "function",
                        "function": {
                            "name": "mcp__serena__find_symbol",
                            "arguments": "{\"name_path_pattern\":\"UserConfig\"}"
                        }
                    }
                ]
            },
            "finish_reason": "tool_calls"
        }]
    });

    let calls = adapter.parse_tool_calls(&response);
    assert_eq!(calls.len(), 2);

    assert_eq!(calls[0].name, "MemoryTool");
    assert_eq!(calls[1].name, "mcp__serena__find_symbol");

    // Verify MCP tool parsing
    assert!(calls[1].is_mcp_tool());
    assert_eq!(calls[1].parse_mcp_name(), Some(("serena", "find_symbol")));
}

#[test]
fn test_ollama_realistic_response_with_tool() {
    let adapter = OllamaToolAdapter::new();

    // Realistic Ollama API response (qwen2.5 model)
    let response = json!({
        "model": "qwen2.5:latest",
        "created_at": "2024-12-03T10:30:00.000Z",
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{
                "function": {
                    "name": "MemoryTool",
                    "arguments": {
                        "operation": "add",
                        "memory_type": "context",
                        "content": "User is working on a Rust project"
                    }
                }
            }]
        },
        "done": false
    });

    let calls = adapter.parse_tool_calls(&response);
    assert_eq!(calls.len(), 1);
    assert!(calls[0].id.starts_with("ollama_"));
    assert_eq!(calls[0].name, "MemoryTool");
    // Ollama returns arguments as object, not string
    assert_eq!(calls[0].arguments["operation"], "add");
    assert_eq!(calls[0].arguments["memory_type"], "context");

    assert!(adapter.has_tool_calls(&response));
    assert!(!adapter.is_finished(&response));
}

#[test]
fn test_ollama_realistic_final_response() {
    let adapter = OllamaToolAdapter::new();

    let response = json!({
        "model": "llama3.1",
        "message": {
            "role": "assistant",
            "content": "I've completed the task. The memory has been stored successfully."
        },
        "done": true,
        "total_duration": 2500000000_i64,
        "prompt_eval_count": 100,
        "eval_count": 25
    });

    assert!(!adapter.has_tool_calls(&response));
    assert!(adapter.is_finished(&response));
    assert_eq!(
        adapter.extract_content(&response),
        Some("I've completed the task. The memory has been stored successfully.".to_string())
    );
}

#[test]
fn test_tool_result_formatting_both_adapters() {
    let mistral = MistralToolAdapter::new();
    let ollama = OllamaToolAdapter::new();

    let success_result = FunctionCallResult::success(
        "call_123",
        "MemoryTool",
        json!({
            "success": true,
            "memory_id": "mem_abc123",
            "message": "Memory stored successfully"
        }),
    );

    // Mistral format includes tool_call_id
    let mistral_formatted = mistral.format_tool_result(&success_result);
    assert_eq!(mistral_formatted["role"], "tool");
    assert_eq!(mistral_formatted["tool_call_id"], "call_123");
    assert_eq!(mistral_formatted["name"], "MemoryTool");

    // Ollama format is simpler
    let ollama_formatted = ollama.format_tool_result(&success_result);
    assert_eq!(ollama_formatted["role"], "tool");
    // Ollama doesn't use tool_call_id in same way

    // Test error result
    let error_result =
        FunctionCallResult::failure("call_456", "MemoryTool", "Database unavailable");

    let mistral_error = mistral.format_tool_result(&error_result);
    assert!(mistral_error["content"]
        .as_str()
        .unwrap()
        .contains("Database unavailable"));

    let ollama_error = ollama.format_tool_result(&error_result);
    assert!(ollama_error["content"]
        .as_str()
        .unwrap()
        .contains("Database unavailable"));
}

#[test]
fn test_tool_definitions_formatted_identically() {
    let mistral = MistralToolAdapter::new();
    let ollama = OllamaToolAdapter::new();

    let tools = vec![memory_tool_definition(), mcp_tool_definition()];

    let mistral_json = mistral.format_tools(&tools);
    let ollama_json = ollama.format_tools(&tools);

    // Both should produce identical tool definitions
    // (the format is the same, only response parsing differs)
    assert_eq!(mistral_json.len(), ollama_json.len());

    for (m, o) in mistral_json.iter().zip(ollama_json.iter()) {
        assert_eq!(m["type"], o["type"]);
        assert_eq!(m["function"]["name"], o["function"]["name"]);
        assert_eq!(m["function"]["description"], o["function"]["description"]);
    }
}

#[test]
fn test_mcp_tool_name_parsing() {
    // Test that MCP tool names are parsed correctly
    let mcp_call = FunctionCall::new(
        "call_1",
        "mcp__serena__find_symbol",
        json!({"pattern": "Foo"}),
    );

    assert!(mcp_call.is_mcp_tool());
    assert_eq!(mcp_call.parse_mcp_name(), Some(("serena", "find_symbol")));

    // Test with double underscore in tool name
    let mcp_call2 = FunctionCall::new(
        "call_2",
        "mcp__context7__get_library_docs",
        json!({"lib": "react"}),
    );

    assert!(mcp_call2.is_mcp_tool());
    assert_eq!(
        mcp_call2.parse_mcp_name(),
        Some(("context7", "get_library_docs"))
    );

    // Regular tool should not be MCP
    let local_call = FunctionCall::new("call_3", "MemoryTool", json!({}));
    assert!(!local_call.is_mcp_tool());
    assert!(local_call.parse_mcp_name().is_none());
}
