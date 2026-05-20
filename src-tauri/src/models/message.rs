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

/// Multimodal attachment carried by a user message (v1: images only).
///
/// `data_base64` is the raw base64 payload (NO `data:` URI prefix). The prefix
/// is reconstructed at the boundaries that need it:
/// - Frontend preview: `data:${mime_type};base64,${data_base64}`
/// - Adapter image_url: per-provider shape (Mistral/Ollama want a string data URL,
///   OpenAI/Custom want an object with `{url: ...}`)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MessageAttachment {
    /// Attachment kind. Always "image" for v1.
    pub kind: String,
    /// IANA MIME type (e.g., "image/png", "image/jpeg", "image/webp", "image/gif").
    pub mime_type: String,
    /// Base64-encoded payload (raw, without the `data:image/...;base64,` prefix).
    pub data_base64: String,
    /// Original filename (display only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// File size in bytes (display only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
}

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
    /// Thinking/reasoning tokens (for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u64>,
    /// Cached prompt tokens (cache reads) when the provider exposes them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    /// Cache-write prompt tokens (first request that primes the cache).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// `llm_model.id` of the model that produced this assistant message.
    /// Captured at write time so cross-workflow restoration uses the exact
    /// pricing snapshot of the moment, not the agent's current configuration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id_used: Option<String>,
    /// Optional multimodal attachments (images attached by the user).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<MessageAttachment>>,
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
    /// Thinking/reasoning tokens (for reasoning models)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking_tokens: Option<u64>,
    /// Cached prompt tokens (cache reads) when the provider exposes them.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u64>,
    /// Cache-write prompt tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_tokens: Option<u64>,
    /// `llm_model.id` of the model that produced this assistant message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id_used: Option<String>,
    /// Optional multimodal attachments (images attached by the user).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<MessageAttachment>>,
}

/// Lightweight metrics from the most recent assistant message of a workflow.
///
/// Used by the frontend to restore the session display when the user switches
/// to a workflow that has no live execution running. Lets the UI show
/// "what the last run cost" rather than blank zeros.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageMetrics {
    pub tokens_input: Option<u64>,
    pub tokens_output: Option<u64>,
    pub cached_tokens: Option<u64>,
    pub cache_write_tokens: Option<u64>,
    pub thinking_tokens: Option<u64>,
    pub cost_usd: Option<f64>,
    /// `llm_model.id` of the model that produced the message (for pricing lookup).
    pub model_id_used: Option<String>,
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
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            attachments: None,
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
            thinking_tokens: Some(25),
            cached_tokens: Some(20),
            cache_write_tokens: Some(15),
            model_id_used: Some("model-uuid-123".to_string()),
            attachments: None,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
        assert!(json.contains("\"tokens_input\":50"));
        assert!(json.contains("\"tokens_output\":10"));
        assert!(json.contains("\"model\":\"mistral-large-latest\""));
        assert!(json.contains("\"provider\":\"Mistral\""));
        assert!(json.contains("\"cached_tokens\":20"));
        assert!(json.contains("\"cache_write_tokens\":15"));
        assert!(json.contains("\"model_id_used\":\"model-uuid-123\""));
    }

    #[test]
    fn test_message_create_omits_cache_fields_when_none() {
        let create = MessageCreate {
            workflow_id: "wf-1".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            attachments: None,
        };
        let json = serde_json::to_string(&create).unwrap();
        assert!(!json.contains("cached_tokens"));
        assert!(!json.contains("cache_write_tokens"));
        assert!(!json.contains("model_id_used"));
    }

    #[test]
    fn test_message_create_serializes_cache_fields_when_some() {
        let create = MessageCreate {
            workflow_id: "wf-1".to_string(),
            role: "assistant".to_string(),
            content: "hi".to_string(),
            tokens: 0,
            tokens_input: Some(100),
            tokens_output: Some(50),
            model: Some("m".to_string()),
            provider: Some("Mistral".to_string()),
            cost_usd: Some(0.001),
            duration_ms: Some(1200),
            thinking_tokens: None,
            cached_tokens: Some(40),
            cache_write_tokens: Some(60),
            model_id_used: Some("mid".to_string()),
            attachments: None,
        };
        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"cached_tokens\":40"));
        assert!(json.contains("\"cache_write_tokens\":60"));
        assert!(json.contains("\"model_id_used\":\"mid\""));
    }

    #[test]
    fn test_message_deserializes_legacy_payload_without_cache_fields() {
        // Pre-Phase-5 rows lack cache fields → must default to None.
        let json = r#"{
            "id":"m1","workflow_id":"wf","role":"user","content":"hi","tokens":1,
            "timestamp":"2026-05-02T00:00:00Z"
        }"#;
        let msg: Message = serde_json::from_str(json).expect("legacy parses");
        assert_eq!(msg.cached_tokens, None);
        assert_eq!(msg.cache_write_tokens, None);
        assert_eq!(msg.model_id_used, None);
    }

    #[test]
    fn test_message_role_display() {
        assert_eq!(MessageRole::User.to_string(), "user");
        assert_eq!(MessageRole::Assistant.to_string(), "assistant");
        assert_eq!(MessageRole::System.to_string(), "system");
    }

    /// Defense-in-depth - MessageCreate should deserialize even without
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

    fn sample_attachment() -> MessageAttachment {
        MessageAttachment {
            kind: "image".into(),
            mime_type: "image/png".into(),
            data_base64: "iVBORw0KGgo=".into(),
            name: Some("test.png".into()),
            size_bytes: Some(1234),
        }
    }

    #[test]
    fn test_message_attachment_serializes_with_all_fields() {
        let att = sample_attachment();
        let json = serde_json::to_string(&att).unwrap();
        assert!(json.contains("\"kind\":\"image\""));
        assert!(json.contains("\"mime_type\":\"image/png\""));
        assert!(json.contains("\"data_base64\":\"iVBORw0KGgo=\""));
        assert!(json.contains("\"name\":\"test.png\""));
        assert!(json.contains("\"size_bytes\":1234"));
    }

    #[test]
    fn test_message_attachment_omits_optional_fields_when_none() {
        let att = MessageAttachment {
            kind: "image".into(),
            mime_type: "image/jpeg".into(),
            data_base64: "abc".into(),
            name: None,
            size_bytes: None,
        };
        let json = serde_json::to_string(&att).unwrap();
        assert!(!json.contains("\"name\""));
        assert!(!json.contains("\"size_bytes\""));
    }

    #[test]
    fn test_message_attachment_roundtrip() {
        let original = sample_attachment();
        let json = serde_json::to_string(&original).unwrap();
        let restored: MessageAttachment = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, original);
    }

    #[test]
    fn test_message_serializes_with_attachments() {
        let message = Message {
            id: "msg_att".to_string(),
            workflow_id: "wf_att".to_string(),
            role: MessageRole::User,
            content: "What is in this image?".to_string(),
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            attachments: Some(vec![sample_attachment()]),
            timestamp: Utc::now(),
        };
        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"attachments\":["));
        assert!(json.contains("\"image/png\""));

        let restored: Message = serde_json::from_str(&json).unwrap();
        let atts = restored.attachments.expect("attachments preserved");
        assert_eq!(atts.len(), 1);
        assert_eq!(atts[0].mime_type, "image/png");
    }

    #[test]
    fn test_message_deserializes_legacy_payload_without_attachments() {
        // Pre-vision rows lack attachments -> must default to None (no error).
        let json = r#"{
            "id":"m1","workflow_id":"wf","role":"user","content":"hi","tokens":1,
            "timestamp":"2026-05-02T00:00:00Z"
        }"#;
        let msg: Message = serde_json::from_str(json).expect("legacy parses");
        assert!(
            msg.attachments.is_none(),
            "legacy rows should have attachments=None"
        );
    }

    #[test]
    fn test_message_create_omits_attachments_when_none() {
        let create = MessageCreate {
            workflow_id: "wf-1".to_string(),
            role: "user".to_string(),
            content: "hi".to_string(),
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            attachments: None,
        };
        let json = serde_json::to_string(&create).unwrap();
        assert!(!json.contains("attachments"));
    }

    #[test]
    fn test_message_create_serializes_with_attachments() {
        let create = MessageCreate {
            workflow_id: "wf-1".to_string(),
            role: "user".to_string(),
            content: "look".to_string(),
            tokens: 0,
            tokens_input: None,
            tokens_output: None,
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            model_id_used: None,
            attachments: Some(vec![sample_attachment()]),
        };
        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"attachments\":["));
    }
}
