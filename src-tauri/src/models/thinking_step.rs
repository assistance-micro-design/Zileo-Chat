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

//! Thinking step models for persistence.
//!
//! This module provides types for storing and retrieving agent reasoning steps
//! captured during workflow execution. Thinking steps represent the agent's
//! internal reasoning process before generating a response.
//!
//! Phase 4: Thinking Steps Persistence

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Thinking step entity representing a single reasoning step.
///
/// Captures the agent's thought process during response generation,
/// useful for debugging, transparency, and understanding agent behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingStep {
    /// Unique identifier (UUID)
    pub id: String,
    /// Associated workflow ID
    pub workflow_id: String,
    /// Associated message ID (the assistant message this thinking belongs to)
    pub message_id: String,
    /// Agent ID that generated this thinking step
    pub agent_id: String,
    /// Step number within the reasoning sequence (0-indexed)
    pub step_number: u32,
    /// Content of the thinking step (the actual reasoning text)
    pub content: String,
    /// Duration to generate this step in milliseconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Number of tokens in this step (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
    /// Timestamp when the step was recorded
    pub created_at: DateTime<Utc>,
}

/// Payload for creating a new thinking step record.
///
/// ID and created_at are generated server-side.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingStepCreate {
    /// Associated workflow ID
    pub workflow_id: String,
    /// Associated message ID
    pub message_id: String,
    /// Agent ID
    pub agent_id: String,
    /// Step number (0-indexed)
    pub step_number: u32,
    /// Reasoning content
    pub content: String,
    /// Duration in milliseconds (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    /// Token count (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<u64>,
}

impl ThinkingStepCreate {
    /// Creates a new thinking step payload.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `message_id` - Associated message ID
    /// * `agent_id` - Agent ID generating the thinking
    /// * `step_number` - Step number in sequence
    /// * `content` - Reasoning content
    #[allow(dead_code)]
    pub fn new(
        workflow_id: String,
        message_id: String,
        agent_id: String,
        step_number: u32,
        content: String,
    ) -> Self {
        Self {
            workflow_id,
            message_id,
            agent_id,
            step_number,
            content,
            duration_ms: None,
            tokens: None,
        }
    }

    /// Creates a thinking step with timing information.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `message_id` - Associated message ID
    /// * `agent_id` - Agent ID generating the thinking
    /// * `step_number` - Step number in sequence
    /// * `content` - Reasoning content
    /// * `duration_ms` - Time to generate this step
    /// * `tokens` - Token count for this step
    #[allow(dead_code)]
    pub fn with_metrics(
        workflow_id: String,
        message_id: String,
        agent_id: String,
        step_number: u32,
        content: String,
        duration_ms: Option<u64>,
        tokens: Option<u64>,
    ) -> Self {
        Self {
            workflow_id,
            message_id,
            agent_id,
            step_number,
            content,
            duration_ms,
            tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_step_serialization() {
        let step = ThinkingStep {
            id: "step_001".to_string(),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_001".to_string(),
            agent_id: "agent_001".to_string(),
            step_number: 0,
            content: "Analyzing the user request to understand the intent.".to_string(),
            duration_ms: Some(150),
            tokens: Some(20),
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&step).unwrap();
        assert!(json.contains("\"step_number\":0"));
        assert!(json.contains("\"content\":\"Analyzing the user request"));
        assert!(json.contains("\"duration_ms\":150"));
        assert!(json.contains("\"tokens\":20"));
    }

    #[test]
    fn test_thinking_step_without_optional_fields() {
        let step = ThinkingStep {
            id: "step_002".to_string(),
            workflow_id: "wf_001".to_string(),
            message_id: "msg_001".to_string(),
            agent_id: "agent_001".to_string(),
            step_number: 1,
            content: "Formulating a response based on available data.".to_string(),
            duration_ms: None,
            tokens: None,
            created_at: Utc::now(),
        };

        let json = serde_json::to_string(&step).unwrap();
        assert!(!json.contains("\"duration_ms\"")); // Should be skipped
        assert!(!json.contains("\"tokens\"")); // Should be skipped
        assert!(json.contains("\"step_number\":1"));
    }

    #[test]
    fn test_thinking_step_deserialization() {
        let json = r#"{
            "id": "step_003",
            "workflow_id": "wf_001",
            "message_id": "msg_001",
            "agent_id": "agent_001",
            "step_number": 2,
            "content": "Preparing the final response.",
            "created_at": "2025-11-27T10:00:00Z"
        }"#;

        let step: ThinkingStep = serde_json::from_str(json).unwrap();
        assert_eq!(step.id, "step_003");
        assert_eq!(step.step_number, 2);
        assert!(step.duration_ms.is_none());
        assert!(step.tokens.is_none());
    }

    #[test]
    fn test_thinking_step_create_new() {
        let create = ThinkingStepCreate::new(
            "wf_001".to_string(),
            "msg_001".to_string(),
            "agent_001".to_string(),
            0,
            "Initial reasoning step.".to_string(),
        );

        assert_eq!(create.step_number, 0);
        assert!(create.duration_ms.is_none());
        assert!(create.tokens.is_none());
    }

    #[test]
    fn test_thinking_step_create_with_metrics() {
        let create = ThinkingStepCreate::with_metrics(
            "wf_001".to_string(),
            "msg_001".to_string(),
            "agent_001".to_string(),
            1,
            "Second reasoning step with timing.".to_string(),
            Some(200),
            Some(35),
        );

        assert_eq!(create.step_number, 1);
        assert_eq!(create.duration_ms, Some(200));
        assert_eq!(create.tokens, Some(35));
    }

    #[test]
    fn test_thinking_step_create_serialization() {
        let create = ThinkingStepCreate::new(
            "wf_001".to_string(),
            "msg_001".to_string(),
            "agent_001".to_string(),
            0,
            "Test content.".to_string(),
        );

        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"workflow_id\":\"wf_001\""));
        assert!(json.contains("\"step_number\":0"));
        // Optional fields should be skipped when None
        assert!(!json.contains("\"duration_ms\""));
        assert!(!json.contains("\"tokens\""));
    }
}
