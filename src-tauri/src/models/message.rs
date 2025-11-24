// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Message role in the conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

/// Message entity representing a conversation message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique identifier
    pub id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Message role
    pub role: MessageRole,
    /// Message content
    pub content: String,
    /// Token count
    pub tokens: usize,
    /// Message timestamp
    pub timestamp: DateTime<Utc>,
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
    fn test_message_with_assistant_role() {
        let message = Message {
            id: "msg_002".to_string(),
            workflow_id: "wf_001".to_string(),
            role: MessageRole::Assistant,
            content: "Hello! How can I help you today?".to_string(),
            tokens: 10,
            timestamp: Utc::now(),
        };

        let json = serde_json::to_string(&message).unwrap();
        assert!(json.contains("\"role\":\"assistant\""));
    }
}
