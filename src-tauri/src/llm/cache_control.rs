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

//! Prompt cache control for OpenAI-compatible providers.
//!
//! Applies `cache_control: { "type": "ephemeral" }` breakpoints to messages
//! to maximize prompt caching with providers like Anthropic Claude via OpenRouter.

/// Applies prompt cache control markers at strategic positions to maximize cache hits.
///
/// Places up to 3 `cache_control: { "type": "ephemeral" }` breakpoints:
/// - **BP1**: System message (always, stable across iterations)
/// - **BP2**: Last assistant message before current iteration (near-end of stable prefix)
/// - **BP3**: Last tool message (exact boundary of stable prefix)
///
/// Only BP1 is applied for short conversations (< 3 messages).
/// Required for Anthropic Claude models via OpenRouter. Harmlessly ignored
/// by providers that cache automatically (OpenAI, DeepSeek, Gemini).
pub fn apply_prompt_cache_control(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    // Find indices for BP2 and BP3 within the stable prefix.
    // The last message is always new content (current iteration) and must NOT be marked.
    let mut last_assistant_idx: Option<usize> = None;
    let mut last_tool_idx: Option<usize> = None;

    if messages.len() > 2 {
        let stable_prefix = &messages[..messages.len() - 1];
        for (i, msg) in stable_prefix.iter().enumerate().rev() {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
            if last_tool_idx.is_none() && role == "tool" {
                last_tool_idx = Some(i);
            }
            if last_assistant_idx.is_none()
                && role == "assistant"
                && (last_tool_idx.is_none() || i < last_tool_idx.unwrap_or(usize::MAX))
            {
                last_assistant_idx = Some(i);
                break;
            }
        }
    }

    messages
        .iter()
        .enumerate()
        .map(|(i, msg)| {
            let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

            let should_mark = match role {
                "system" => true,
                "assistant" => Some(i) == last_assistant_idx,
                "tool" => Some(i) == last_tool_idx,
                _ => false,
            };

            if should_mark {
                if let Some(content) = msg.get("content").and_then(|c| c.as_str()) {
                    let mut marked = serde_json::json!({
                        "role": role,
                        "content": [{
                            "type": "text",
                            "text": content,
                            "cache_control": { "type": "ephemeral" }
                        }]
                    });
                    // Preserve tool-specific fields
                    if role == "tool" {
                        if let Some(tool_call_id) = msg.get("tool_call_id") {
                            marked["tool_call_id"] = tool_call_id.clone();
                        }
                        if let Some(name) = msg.get("name") {
                            marked["name"] = name.clone();
                        }
                    }
                    marked
                } else {
                    // Already multipart or non-string content: skip
                    msg.clone()
                }
            } else {
                msg.clone()
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_control_system_only_short_conversation() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "You are a helpful assistant."}),
            serde_json::json!({"role": "user", "content": "Hello"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        let system = &result[0];
        assert_eq!(system["role"], "system");
        let content = system["content"]
            .as_array()
            .expect("content should be array");
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "You are a helpful assistant.");
        assert_eq!(content[0]["cache_control"]["type"], "ephemeral");

        let user = &result[1];
        assert_eq!(user["role"], "user");
        assert_eq!(user["content"], "Hello");
    }

    #[test]
    fn test_cache_control_no_system() {
        let messages = vec![
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Hi there"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        assert_eq!(result[0]["content"], "Hello");
        assert_eq!(result[1]["content"], "Hi there");
    }

    #[test]
    fn test_cache_control_already_multipart() {
        let messages = vec![serde_json::json!({
            "role": "system",
            "content": [{"type": "text", "text": "Already multipart"}]
        })];

        let result = apply_prompt_cache_control(&messages);

        let content = result[0]["content"]
            .as_array()
            .expect("should remain array");
        assert_eq!(content[0]["text"], "Already multipart");
        assert!(content[0].get("cache_control").is_none());
    }

    #[test]
    fn test_cache_control_multi_breakpoint_with_tools() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Do something"}),
            serde_json::json!({"role": "assistant", "content": "I will use a tool"}),
            serde_json::json!({"role": "tool", "content": "Tool result 1", "tool_call_id": "call_1", "name": "MyTool"}),
            serde_json::json!({"role": "assistant", "content": "Based on results..."}),
            serde_json::json!({"role": "tool", "content": "Tool result 2", "tool_call_id": "call_2", "name": "MyTool"}),
            serde_json::json!({"role": "assistant", "content": "Final answer"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // BP1: System message marked
        assert!(result[0]["content"].is_array());
        assert_eq!(
            result[0]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );

        // User: unchanged
        assert_eq!(result[1]["content"], "Do something");

        // First assistant: unchanged
        assert_eq!(result[2]["content"], "I will use a tool");

        // First tool: unchanged
        assert_eq!(result[3]["content"], "Tool result 1");

        // BP2: Last assistant before last tool (index 4)
        assert!(result[4]["content"].is_array());
        assert_eq!(
            result[4]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(result[4]["content"][0]["text"], "Based on results...");

        // BP3: Last tool message (index 5)
        assert!(result[5]["content"].is_array());
        assert_eq!(
            result[5]["content"][0]["cache_control"]["type"],
            "ephemeral"
        );
        assert_eq!(result[5]["content"][0]["text"], "Tool result 2");
        assert_eq!(result[5]["tool_call_id"], "call_2");
        assert_eq!(result[5]["name"], "MyTool");

        // Last assistant: unchanged (new content)
        assert_eq!(result[6]["content"], "Final answer");
    }

    #[test]
    fn test_cache_control_no_tool_messages() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Response 1"}),
            serde_json::json!({"role": "user", "content": "Follow up"}),
            serde_json::json!({"role": "assistant", "content": "Response 2"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        // BP1: System marked
        assert!(result[0]["content"].is_array());

        // BP2: assistant at index 2
        assert!(result[2]["content"].is_array());
        assert_eq!(result[2]["content"][0]["text"], "Response 1");

        // Other messages unchanged
        assert_eq!(result[1]["content"], "Hello");
        assert_eq!(result[3]["content"], "Follow up");
        assert_eq!(result[4]["content"], "Response 2");
    }

    #[test]
    fn test_cache_control_last_message_is_assistant() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Do something"}),
            serde_json::json!({"role": "assistant", "content": "Using tool"}),
            serde_json::json!({"role": "tool", "content": "Result", "tool_call_id": "call_1"}),
            serde_json::json!({"role": "assistant", "content": "New content"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        assert!(result[0]["content"].is_array());
        assert!(result[2]["content"].is_array());
        assert_eq!(result[2]["content"][0]["text"], "Using tool");
        assert!(result[3]["content"].is_array());
        assert_eq!(result[3]["content"][0]["text"], "Result");
        assert_eq!(result[4]["content"], "New content");
    }

    #[test]
    fn test_cache_control_idempotent() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System prompt"}),
            serde_json::json!({"role": "user", "content": "Hello"}),
            serde_json::json!({"role": "assistant", "content": "Using tool"}),
            serde_json::json!({"role": "tool", "content": "Result", "tool_call_id": "call_1"}),
            serde_json::json!({"role": "assistant", "content": "Final answer"}),
        ];

        let first_pass = apply_prompt_cache_control(&messages);
        let second_pass = apply_prompt_cache_control(&first_pass);

        assert!(second_pass[0]["content"].is_array());
        let system_content = second_pass[0]["content"].as_array().unwrap();
        assert_eq!(system_content.len(), 1);

        assert!(second_pass[2]["content"].is_array());
        assert!(second_pass[3]["content"].is_array());
        assert_eq!(second_pass[4]["content"], "Final answer");
    }

    #[test]
    fn test_cache_control_max_three_breakpoints() {
        let messages = vec![
            serde_json::json!({"role": "system", "content": "System"}),
            serde_json::json!({"role": "user", "content": "Go"}),
            serde_json::json!({"role": "assistant", "content": "A1"}),
            serde_json::json!({"role": "tool", "content": "T1", "tool_call_id": "c1"}),
            serde_json::json!({"role": "assistant", "content": "A2"}),
            serde_json::json!({"role": "tool", "content": "T2", "tool_call_id": "c2"}),
            serde_json::json!({"role": "assistant", "content": "A3"}),
            serde_json::json!({"role": "tool", "content": "T3", "tool_call_id": "c3"}),
            serde_json::json!({"role": "assistant", "content": "A4"}),
        ];

        let result = apply_prompt_cache_control(&messages);

        let marked_count = result
            .iter()
            .filter(|msg| {
                msg["content"]
                    .as_array()
                    .map(|arr| arr.iter().any(|part| part.get("cache_control").is_some()))
                    .unwrap_or(false)
            })
            .count();

        assert_eq!(marked_count, 3);

        assert!(result[0]["content"].is_array()); // System BP1
        assert_eq!(result[6]["content"][0]["text"], "A3"); // BP2
        assert_eq!(result[7]["content"][0]["text"], "T3"); // BP3
        assert_eq!(result[8]["content"], "A4"); // Not marked
    }
}
