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
use crate::models::{QuestionOption, UserQuestionCreate, UserQuestionStreamPayload};
use crate::tools::constants::user_question as uq_const;
use crate::tools::utils::{validate_length, validate_not_empty};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AskInput {
    operation: String,
    question: String,
    #[serde(rename = "questionType")]
    question_type: String,
    options: Option<Vec<QuestionOption>>,
    #[serde(rename = "textPlaceholder")]
    text_placeholder: Option<String>,
    #[serde(rename = "textRequired")]
    text_required: Option<bool>,
    context: Option<String>,
}

/// Tool for asking questions to users.
///
/// This tool allows agents to:
/// - Ask users questions with multiple response types
/// - Wait for responses with progressive polling (no timeout)
/// - Receive checkbox selections, text input, or both
///
/// # Scope
///
/// Each UserQuestionTool instance is scoped to a specific workflow and agent.
/// Questions created will be associated with the workflow_id provided at construction.
pub struct UserQuestionTool {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Current workflow ID (scope)
    workflow_id: String,
    /// Agent ID using this tool
    agent_id: String,
    /// Tauri app handle for emitting streaming events
    app_handle: Option<AppHandle>,
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
        Self {
            db,
            workflow_id,
            agent_id,
            app_handle,
        }
    }

    /// Asks a question to the user and waits for response.
    ///
    /// # Arguments
    /// * `input` - Question details including type, options, and context
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id, agent_id = %self.agent_id))]
    async fn ask_question(&self, input: AskInput) -> ToolResult<Value> {
        // Validate input
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

        let question_id = Uuid::new_v4().to_string();
        let options_json = serde_json::to_string(
            &input.options.as_ref().cloned().unwrap_or_default(),
        )
        .map_err(|e| ToolError::ExecutionFailed(format!("Failed to serialize options: {}", e)))?;

        // Create question in DB
        let create_data = UserQuestionCreate {
            workflow_id: self.workflow_id.clone(),
            agent_id: self.agent_id.clone(),
            question: input.question.clone(),
            question_type: input.question_type.clone(),
            options: options_json.clone(),
            text_placeholder: input.text_placeholder.clone(),
            text_required: input.text_required.unwrap_or(false),
            context: input.context.clone(),
            status: "pending".to_string(),
        };

        // Use execute() with JSON encoding (SurrealDB SDK 2.x pattern for writes)
        let json_str = serde_json::to_string(&create_data)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to encode JSON: {}", e)))?;
        let query = format!(
            "CREATE user_question:`{}` CONTENT {}",
            question_id, json_str
        );
        self.db
            .execute(&query)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create question: {}", e)))?;

        info!(question_id = %question_id, "Created user question");

        // Emit streaming event
        if let Some(ref handle) = self.app_handle {
            let payload = UserQuestionStreamPayload {
                question_id: question_id.clone(),
                question: input.question.clone(),
                question_type: input.question_type.clone(),
                options: input.options.clone(),
                text_placeholder: input.text_placeholder.clone(),
                text_required: input.text_required.unwrap_or(false),
                context: input.context.clone(),
            };

            let chunk = json!({
                "workflow_id": self.workflow_id,
                "chunk_type": "user_question_start",
                "user_question": payload
            });

            if let Err(e) = handle.emit("workflow_stream", &chunk) {
                warn!(error = %e, "Failed to emit user_question_start event");
            }
        }

        // Wait for response (progressive polling, no timeout)
        let response = self.wait_for_response(&question_id).await?;

        // Emit completion event
        if let Some(ref handle) = self.app_handle {
            let chunk = json!({
                "workflow_id": self.workflow_id,
                "chunk_type": "user_question_complete",
                "question_id": question_id
            });

            if let Err(e) = handle.emit("workflow_stream", &chunk) {
                warn!(error = %e, "Failed to emit user_question_complete event");
            }
        }

        Ok(response)
    }

    /// Waits for user response with progressive polling.
    ///
    /// Starts with 500ms intervals and gradually increases to 5s.
    /// No timeout - waits indefinitely until user responds or skips.
    #[instrument(skip(self))]
    async fn wait_for_response(&self, question_id: &str) -> ToolResult<Value> {
        let mut interval_idx = 0;

        loop {
            // Query question status
            let query = format!(
                "SELECT status, selected_options, text_response FROM user_question:`{}`",
                question_id
            );

            let result: Vec<serde_json::Value> = self
                .db
                .query_json(&query)
                .await
                .map_err(|e| ToolError::ExecutionFailed(format!("DB query failed: {}", e)))?;

            if let Some(record) = result.first() {
                let status = record
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending");

                match status {
                    "answered" => {
                        let selected_json = record
                            .get("selected_options")
                            .and_then(|v| v.as_str())
                            .unwrap_or("[]");
                        let selected: Vec<String> =
                            serde_json::from_str(selected_json).unwrap_or_default();

                        let text = record
                            .get("text_response")
                            .and_then(|v| v.as_str())
                            .map(String::from);

                        info!(question_id = %question_id, "User answered question");

                        return Ok(json!({
                            "success": true,
                            "selectedOptions": selected,
                            "textResponse": text,
                            "message": "User response received"
                        }));
                    }
                    "skipped" => {
                        warn!(question_id = %question_id, "User skipped question");
                        return Err(ToolError::ExecutionFailed(
                            "Question skipped by user".into(),
                        ));
                    }
                    "pending" => {
                        // Continue polling
                    }
                    _ => {
                        return Err(ToolError::ExecutionFailed(format!(
                            "Invalid question status: {}",
                            status
                        )));
                    }
                }
            }

            // Progressive delay
            let delay = uq_const::POLL_INTERVALS_MS
                .get(interval_idx)
                .copied()
                .unwrap_or(5000);

            debug!(
                question_id = %question_id,
                delay_ms = delay,
                "Waiting for user response"
            );

            tokio::time::sleep(Duration::from_millis(delay)).await;

            if interval_idx < uq_const::POLL_INTERVALS_MS.len() - 1 {
                interval_idx += 1;
            }
        }
    }
}

#[async_trait]
impl Tool for UserQuestionTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "UserQuestionTool".to_string(),
            name: "User Question Tool".to_string(),
            description: "Ask the user a question and wait for their response. Supports checkbox (multiple choice), text input, or mixed (both). No timeout - waits indefinitely for user response.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["ask"],
                        "description": "Operation to perform"
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

    fn requires_confirmation(&self) -> bool {
        false
    }
}
