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

//! UserQuestionTool implementation for asking users questions.
//!
//! This tool allows agents to ask questions to users through a modal interface.

use crate::db::DBClient;
use crate::models::QuestionOption;
use crate::tools::constants::user_question as uq_const;
use crate::tools::user_question::circuit_breaker::UserQuestionCircuitBreaker;
use crate::tools::utils::{validate_length, validate_not_empty};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tauri::AppHandle;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AskInput {
    pub(crate) operation: String,
    pub(crate) question: String,
    #[serde(rename = "questionType")]
    pub(crate) question_type: String,
    pub(crate) options: Option<Vec<QuestionOption>>,
    #[serde(rename = "textPlaceholder")]
    pub(crate) text_placeholder: Option<String>,
    #[serde(rename = "textRequired")]
    pub(crate) text_required: Option<bool>,
    pub(crate) context: Option<String>,
}

/// Tool for asking questions to users.
///
/// This tool allows agents to:
/// - Ask users questions with multiple response types
/// - Wait for responses with progressive polling (5-minute timeout)
/// - Receive checkbox selections, text input, or both
/// - Circuit breaker protection against repeated timeouts
///
/// # Scope
///
/// Each UserQuestionTool instance is scoped to a specific workflow and agent.
/// Questions created will be associated with the workflow_id provided at construction.
///
/// # Circuit Breaker
///
/// The tool tracks consecutive timeouts per workflow. After 3 consecutive timeouts,
/// the circuit opens and new questions are rejected immediately for 60 seconds.
/// This prevents spamming questions when users are unresponsive.
pub struct UserQuestionTool {
    /// Database client for persistence
    pub(crate) db: Arc<DBClient>,
    /// Current workflow ID (scope)
    pub(crate) workflow_id: String,
    /// Agent ID using this tool
    pub(crate) agent_id: String,
    /// Tauri app handle for emitting streaming events
    pub(crate) app_handle: Option<AppHandle>,
    /// Circuit breaker for timeout resilience
    pub(crate) circuit_breaker: RwLock<UserQuestionCircuitBreaker>,
}

impl UserQuestionTool {
    /// Creates a new UserQuestionTool for a specific workflow.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `workflow_id` - Workflow ID to scope questions to
    /// * `agent_id` - Agent ID using this tool
    /// * `app_handle` - Optional Tauri app handle for emitting events
    ///
    /// # Circuit Breaker
    ///
    /// Initializes with a circuit breaker configured from constants:
    /// - Threshold: 3 consecutive timeouts
    /// - Cooldown: 60 seconds
    ///
    /// # Example
    /// ```ignore
    /// let tool = UserQuestionTool::new(
    ///     db.clone(),
    ///     "wf_001".into(),
    ///     "agent_id".into(),
    ///     Some(app_handle)
    /// );
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        workflow_id: String,
        agent_id: String,
        app_handle: Option<AppHandle>,
    ) -> Self {
        let circuit_breaker = UserQuestionCircuitBreaker::new(
            workflow_id.clone(),
            uq_const::CIRCUIT_FAILURE_THRESHOLD,
            Duration::from_secs(uq_const::CIRCUIT_COOLDOWN_SECS),
        );

        Self {
            db,
            workflow_id,
            agent_id,
            app_handle,
            circuit_breaker: RwLock::new(circuit_breaker),
        }
    }

    /// Validates input for ask operation.
    ///
    /// # Arguments
    /// * `input` - Question input to validate
    ///
    /// # Errors
    /// Returns `ToolError::ValidationFailed` if validation fails
    pub(crate) fn validate_ask_input(&self, input: &AskInput) -> ToolResult<()> {
        validate_not_empty(&input.question, "question")?;
        validate_length(&input.question, uq_const::MAX_QUESTION_LENGTH, "question")?;

        if !uq_const::VALID_TYPES.contains(&input.question_type.as_str()) {
            return Err(ToolError::ValidationFailed(format!(
                "Invalid question type: {}. Valid types: {:?}",
                input.question_type,
                uq_const::VALID_TYPES
            )));
        }

        // Validate options for checkbox/mixed types
        if input.question_type == "checkbox" || input.question_type == "mixed" {
            let options = input.options.as_ref().ok_or_else(|| {
                ToolError::ValidationFailed("Options required for checkbox/mixed types".into())
            })?;

            if options.is_empty() {
                return Err(ToolError::ValidationFailed(
                    "At least one option required".into(),
                ));
            }

            if options.len() > uq_const::MAX_OPTIONS {
                return Err(ToolError::ValidationFailed(format!(
                    "Too many options: {}. Maximum: {}",
                    options.len(),
                    uq_const::MAX_OPTIONS
                )));
            }

            for opt in options {
                validate_not_empty(&opt.id, "option.id")?;
                validate_length(&opt.id, uq_const::MAX_OPTION_ID_LENGTH, "option.id")?;
                validate_not_empty(&opt.label, "option.label")?;
                validate_length(
                    &opt.label,
                    uq_const::MAX_OPTION_LABEL_LENGTH,
                    "option.label",
                )?;
            }
        }

        // Validate context if provided
        if let Some(ref ctx) = input.context {
            validate_length(ctx, uq_const::MAX_CONTEXT_LENGTH, "context")?;
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for UserQuestionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "UserQuestionTool".to_string(),
            name: "User Question Tool".to_string(),
            summary: "Ask the user a question and wait for their response".to_string(),
            description: r#"Asks the user a question and waits for their response with configurable input types.

USE THIS TOOL WHEN:
- You need user input to proceed (clarification, choice, confirmation)
- A decision cannot be made autonomously
- Confirming potentially destructive or irreversible actions

DO NOT USE THIS TOOL WHEN:
- The answer is already in the conversation context
- You can make a reasonable default choice

OPERATIONS:
- ask: Present question to user and wait for response
  Question types: checkbox (multiple choice), text (free-form), mixed (options + text)

EXAMPLES:
1. Checkbox: {"operation": "ask", "question": "Which database?", "questionType": "checkbox", "options": [{"id": "pg", "label": "PostgreSQL"}, {"id": "mysql", "label": "MySQL"}]}
2. Text: {"operation": "ask", "question": "API endpoint name?", "questionType": "text", "textPlaceholder": "e.g., /api/v1/users"}
3. Mixed: {"operation": "ask", "question": "Select or describe:", "questionType": "mixed", "options": [{"id": "basic", "label": "Basic"}], "textPlaceholder": "Custom..."}"#
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["ask"],
                        "description": "Operation: 'ask' presents question to user and waits for response"
                    },
                    "question": {
                        "type": "string",
                        "description": "The question to ask the user"
                    },
                    "questionType": {
                        "type": "string",
                        "enum": ["checkbox", "text", "mixed"],
                        "description": "Type of question: checkbox (multiple choice), text (free text), or mixed (both)"
                    },
                    "options": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "label": { "type": "string" }
                            },
                            "required": ["id", "label"]
                        },
                        "description": "Options for checkbox/mixed type questions"
                    },
                    "textPlaceholder": {
                        "type": "string",
                        "description": "Placeholder text for the text input"
                    },
                    "textRequired": {
                        "type": "boolean",
                        "default": false,
                        "description": "Whether text response is required (for mixed type)"
                    },
                    "context": {
                        "type": "string",
                        "description": "Additional context to display to the user"
                    }
                },
                "required": ["operation", "question", "questionType"]
            }),
            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": { "type": "boolean" },
                    "selectedOptions": {
                        "type": "array",
                        "items": { "type": "string" }
                    },
                    "textResponse": { "type": "string" },
                    "message": { "type": "string" }
                }
            }),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        let parsed: AskInput = serde_json::from_value(input.clone())
            .map_err(|e| ToolError::ValidationFailed(format!("Invalid input: {}", e)))?;

        if parsed.operation != "ask" {
            return Err(ToolError::ValidationFailed(format!(
                "Unknown operation: {}. Only 'ask' is supported.",
                parsed.operation
            )));
        }

        self.ask_question(parsed).await
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        // Validate required fields exist
        let obj = input
            .as_object()
            .ok_or_else(|| ToolError::ValidationFailed("Input must be an object".into()))?;

        if !obj.contains_key("operation") {
            return Err(ToolError::ValidationFailed(
                "Missing 'operation' field".into(),
            ));
        }
        if !obj.contains_key("question") {
            return Err(ToolError::ValidationFailed(
                "Missing 'question' field".into(),
            ));
        }
        if !obj.contains_key("questionType") {
            return Err(ToolError::ValidationFailed(
                "Missing 'questionType' field".into(),
            ));
        }

        Ok(())
    }
}
