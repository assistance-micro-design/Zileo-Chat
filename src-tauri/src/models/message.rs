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

//! Message models for conversation persistence.
//!
//! This module provides types for storing and retrieving conversation messages
//! with associated metrics (tokens, cost, duration) for analytics and recovery.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Message role in the conversation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    /// User message
    User,
    /// Assistant response
    Assistant,
    /// System message (errors, notifications)
    System,
}

impl std::fmt::Display for MessageRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MessageRole::User => write!(f, "user"),
            MessageRole::Assistant => write!(f, "assistant"),
            MessageRole::System => write!(f, "system"),
        }
    }
}

/// Message entity representing a conversation message with metrics.
///
/// Includes token counts, model info, cost, and duration for analytics
/// and state recovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier (UUID)
    pub id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Message role (user, assistant, system)
    pub role: MessageRole,
    /// Message content (text)
    pub content: String,
    /// Legacy token count (deprecated, use tokens_input/tokens_output)
    pub tokens: usize,
    /// Input tokens consumed (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<u64>,
    /// Output tokens generated (for assistant messages)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<u64>,
    /// Model used for generation (e.g., "mistral-large-latest")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Provider used (e.g., "Mistral", "Ollama")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Estimated cost in USD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Generation duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Message timestamp
    pub timestamp: DateTime<Utc>,
}

/// Payload for creating a new message.
///
/// ID and timestamp are generated server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageCreate {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Message role
    pub role: String,
    /// Message content
    pub content: String,
    /// Legacy token count (computed from tokens_output, defaults to 0)
    #[serde(default)]
    pub tokens: usize,
    /// Input tokens consumed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_input: Option<u64>,
    /// Output tokens generated
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_output: Option<u64>,
    /// Model used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Provider used
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    /// Cost in USD
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_usd: Option<f64>,
    /// Duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
}

/// Response for paginated message loading.
///
/// Includes pagination metadata for cursor-based navigation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaginatedMessages {
    /// Messages in the current page
    pub messages: Vec<Message>,
    /// Total number of messages available
    pub total: u32,
    /// Current offset (number of messages skipped)
    pub offset: u32,
    /// Page size limit
    pub limit: u32,
    /// Whether more messages are available after this page
    pub has_more: bool,
}

impl MessageCreate {
    /// Creates a new user message (no metrics).
    #[allow(dead_code)]
    pub fn user(workflow_id: String, content: String) -> Self {
        Self {
            workflow_id,
            role: "user".to_string(),
            content,
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
        }
    }

    /// Creates a new assistant message with metrics.
    #[allow(dead_code)]
    pub fn assistant(
        workflow_id: String,
        content: String,
        tokens_input: Option<u64>,
        tokens_output: Option<u64>,
        model: Option<String>,
        provider: Option<String>,
        duration_ms: Option<u64>,
    ) -> Self {
        Self {
            workflow_id,
            role: "assistant".to_string(),
            content,
            tokens: tokens_output.unwrap_or(0) as usize,
            tokens_input,
            tokens_output,
            model,
            provider,
            cost_usd: None, // Cost calculation is provider-specific
            duration_ms,
        }
    }

    /// Creates a new system message (errors, notifications).
    #[allow(dead_code)]
    pub fn system(workflow_id: String, content: String) -> Self {
        Self {
            workflow_id,
            role: "system".to_string(),
            content,
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_role_serialization() {
        let role = MessageRole::User;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, "\"user\"");

        let deserialized: MessageRole = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, MessageRole::User));
    }

    #[test]
    fn test_message_role_all_variants() {
        let variants = vec![
            (MessageRole::User, "\"user\""),
            (MessageRole::Assistant, "\"assistant\""),
            (MessageRole::System, "\"system\""),
        ];

        for (role, expected_json) in variants {
            let json = serde_json::to_string(&role).unwrap();
            assert_eq!(json, expected_json);
        }
    }

    #[test]
    fn test_message_serialization() {
        let message = Message {
            id: "msg_001".to_string(),
            workflow_id: "wf_001".to_string(),
            role: MessageRole::User,
            content: "Hello, assistant!".to_string(),
            tokens: 5,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        let deserialized: Message = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, message.id);
        assert_eq!(deserialized.workflow_id, message.workflow_id);
        assert!(matches!(deserialized.role, MessageRole::User));
        assert_eq!(deserialized.content, message.content);
        assert_eq!(deserialized.tokens, message.tokens);
    }

    #[test]
    fn test_message_with_metrics() {
        let message = Message {
            id: "msg_002".to_string(),
            workflow_id: "wf_001".to_string(),
            role: MessageRole::Assistant,
            content: "Hello! How can I help you today?".to_string(),
            tokens: 10,
            tokens_input: Some(50),
            tokens_output: Some(10),
            model: Some("mistral-large-latest".to_string()),
            provider: Some("Mistral".to_string()),
            cost_usd: Some(0.001),
            duration_ms: Some(1500),
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"tokens_input\":50"));
        assert!(json.contains("\"tokens_output\":10"));
        assert!(json.contains("\"model\":\"mistral-large-latest\""));
        assert!(json.contains("\"provider\":\"Mistral\""));
    }

    #[test]
    fn test_message_create_user() {
        let create = MessageCreate::user("wf_001".to_string(), "Hello".to_string());
        assert_eq!(create.role, "user");
        assert!(create.tokens_input.is_none());
        assert!(create.model.is_none());
    }

    #[test]
    fn test_message_create_assistant() {
        let create = MessageCreate::assistant(
            "wf_001".to_string(),
            "Response".to_string(),
            Some(100),
            Some(50),
            Some("mistral-large".to_string()),
            Some("Mistral".to_string()),
            Some(2000),
        );
        assert_eq!(create.role, "assistant");
        assert_eq!(create.tokens_input, Some(100));
        assert_eq!(create.tokens_output, Some(50));
        assert_eq!(create.tokens, 50); // tokens = tokens_output
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::System.to_string(), "system");
    }

    /// SA-013 #6: Verify MessageCreate always serializes the `tokens` field.
    /// Rust `tokens: usize` is required, so it must always appear in JSON output.
    #[test]
    fn test_message_create_always_serializes_tokens() {
        let create = MessageCreate::user("wf-1".to_string(), "Hello".to_string());
        let json = serde_json::to_string(&create).unwrap();
        assert!(
            json.contains("\"tokens\""),
            "tokens field must always be present in serialized output"
        );
    }

    /// SA-013 #6: Defense-in-depth - MessageCreate should deserialize even without
    /// the `tokens` field, defaulting to 0. This protects against incomplete JSON
    /// from external sources (import, tests).
    #[test]
    fn test_message_create_deserializes_without_tokens() {
        let json = r#"{
            "workflow_id": "wf-1",
            "role": "user",
            "content": "Hello"
        }"#;
        let result: Result<MessageCreate, _> = serde_json::from_str(json);
        assert!(
            result.is_ok(),
            "MessageCreate should deserialize without tokens field"
        );
        assert_eq!(
            result.unwrap().tokens,
            0,
            "Missing tokens should default to 0"
        );
    }

    /// SA-013 #6: Verify tokens value is preserved through serialization roundtrip.
    #[test]
    fn test_message_create_tokens_roundtrip() {
        let create = MessageCreate::assistant(
            "wf-1".to_string(),
            "Response".to_string(),
            Some(100),
            Some(75),
            Some("gpt-4".to_string()),
            None,
            Some(1500),
        );
        assert_eq!(create.tokens, 75);

        let json = serde_json::to_string(&create).unwrap();
        let deserialized: MessageCreate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.tokens, 75, "tokens must survive roundtrip");
    }
}
