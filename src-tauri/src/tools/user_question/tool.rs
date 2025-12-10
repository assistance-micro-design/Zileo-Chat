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
use crate::tools::user_question::circuit_breaker::UserQuestionCircuitBreaker;
use crate::tools::utils::{validate_length, validate_not_empty};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::{Arc, RwLock};
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
/// - Wait for responses with progressive polling (5-minute timeout)
/// - Receive checkbox selections, text input, or both
/// - Circuit breaker protection against repeated timeouts (OPT-UQ-12)
///
/// # Scope
///
/// Each UserQuestionTool instance is scoped to a specific workflow and agent.
/// Questions created will be associated with the workflow_id provided at construction.
///
/// # Circuit Breaker (OPT-UQ-12)
///
/// The tool tracks consecutive timeouts per workflow. After 3 consecutive timeouts,
/// the circuit opens and new questions are rejected immediately for 60 seconds.
/// This prevents spamming questions when users are unresponsive.
pub struct UserQuestionTool {
    /// Database client for persistence
    db: Arc<DBClient>,
    /// Current workflow ID (scope)
    workflow_id: String,
    /// Agent ID using this tool
    agent_id: String,
    /// Tauri app handle for emitting streaming events
    app_handle: Option<AppHandle>,
    /// Circuit breaker for timeout resilience (OPT-UQ-12)
    circuit_breaker: RwLock<UserQuestionCircuitBreaker>,
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
    fn validate_ask_input(&self, input: &AskInput) -> ToolResult<()> {
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

    /// Creates a question record in the database.
    ///
    /// # Arguments
    /// * `question_id` - UUID of the question
    /// * `input` - Question details
    ///
    /// # Errors
    /// Returns `ToolError::ExecutionFailed` if DB operation fails
    async fn create_question_record(&self, question_id: &str, input: &AskInput) -> ToolResult<()> {
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

        // Use execute_with_params for CREATE (CLAUDE.md SurrealDB SDK 2.x pattern)
        let json_data = serde_json::to_value(&create_data)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to encode JSON: {}", e)))?;
        let query = format!("CREATE user_question:`{}` CONTENT $data", question_id);

        info!(question_id = %question_id, "Creating user question in DB");

        self.db
            .execute_with_params(&query, vec![("data".to_string(), json_data)])
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to create question: {}", e)))?;

        // Verify the question was created
        let verify_query = format!("SELECT status FROM user_question:`{}`", question_id);
        let verify_result: Vec<serde_json::Value> =
            self.db.query_json(&verify_query).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to verify question creation: {}", e))
            })?;

        if verify_result.is_empty() {
            return Err(ToolError::ExecutionFailed(format!(
                "Question was not created in DB: {}",
                question_id
            )));
        }

        info!(question_id = %question_id, verify_result = ?verify_result, "Created and verified user question");

        Ok(())
    }

    /// Emits a question start event to the frontend.
    ///
    /// # Arguments
    /// * `question_id` - UUID of the question
    /// * `input` - Question details
    fn emit_question_event(&self, question_id: &str, input: &AskInput) {
        if let Some(ref handle) = self.app_handle {
            let payload = UserQuestionStreamPayload {
                question_id: question_id.to_string(),
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
    }

    /// Emits a question completion event to the frontend.
    ///
    /// # Arguments
    /// * `question_id` - UUID of the question
    fn emit_completion_event(&self, question_id: &str) {
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
    }

    /// Asks a question to the user and waits for response.
    ///
    /// # Arguments
    /// * `input` - Question details including type, options, and context
    ///
    /// # Circuit Breaker (OPT-UQ-12)
    ///
    /// Before asking, checks if the circuit breaker allows new questions.
    /// If the circuit is open (too many recent timeouts), returns an error immediately.
    /// After receiving a response, updates the circuit breaker state:
    /// - Success: resets timeout count
    /// - Timeout: increments count, may open circuit
    /// - Skip: treated as success (user actively responded)
    #[instrument(skip(self), fields(workflow_id = %self.workflow_id, agent_id = %self.agent_id))]
    async fn ask_question(&self, input: AskInput) -> ToolResult<Value> {
        // OPT-UQ-12: Check circuit breaker before asking
        {
            let mut cb = self.circuit_breaker.write().map_err(|e| {
                ToolError::ExecutionFailed(format!("Circuit breaker lock poisoned: {}", e))
            })?;
            if !cb.allow_question() {
                let remaining = cb
                    .remaining_cooldown()
                    .map(|d| d.as_secs())
                    .unwrap_or(uq_const::CIRCUIT_COOLDOWN_SECS);
                warn!(
                    workflow_id = %self.workflow_id,
                    circuit_state = ?cb.state(),
                    timeout_count = cb.timeout_count(),
                    remaining_cooldown_secs = remaining,
                    "Circuit breaker open - rejecting question"
                );
                return Err(ToolError::ExecutionFailed(format!(
                    "User appears unresponsive ({} consecutive timeouts). \
                     Question rejected. Retry in {} seconds.",
                    cb.timeout_count(),
                    remaining
                )));
            }
        }

        self.validate_ask_input(&input)?;

        let question_id = Uuid::new_v4().to_string();
        self.create_question_record(&question_id, &input).await?;
        self.emit_question_event(&question_id, &input);

        // Wait for response and update circuit breaker based on result
        let response = self.wait_for_response(&question_id).await;

        // OPT-UQ-12: Update circuit breaker based on response
        match &response {
            Ok(_) => {
                if let Ok(mut cb) = self.circuit_breaker.write() {
                    cb.record_success();
                    debug!(workflow_id = %self.workflow_id, "Circuit breaker: recorded success");
                }
            }
            Err(e) => {
                let error_msg = e.to_string();
                if error_msg.contains("Timeout") || error_msg.contains("timed out") {
                    if let Ok(mut cb) = self.circuit_breaker.write() {
                        cb.record_timeout();
                        warn!(
                            workflow_id = %self.workflow_id,
                            circuit_state = ?cb.state(),
                            timeout_count = cb.timeout_count(),
                            "Circuit breaker: recorded timeout"
                        );
                    }
                } else if error_msg.contains("skipped") {
                    // Skip is an active user choice, treat like success
                    if let Ok(mut cb) = self.circuit_breaker.write() {
                        cb.record_skip();
                        debug!(workflow_id = %self.workflow_id, "Circuit breaker: recorded skip");
                    }
                }
                // Other errors don't affect circuit breaker
            }
        }

        self.emit_completion_event(&question_id);

        response
    }

    /// Waits for user response with progressive polling and configurable timeout.
    ///
    /// Starts with 500ms intervals and gradually increases to 5s.
    /// Times out after `DEFAULT_TIMEOUT_SECS` (5 minutes) and updates DB status to "timeout".
    #[instrument(skip(self))]
    async fn wait_for_response(&self, question_id: &str) -> ToolResult<Value> {
        let timeout = Duration::from_secs(uq_const::DEFAULT_TIMEOUT_SECS);
        let start = std::time::Instant::now();
        let mut interval_idx = 0;

        loop {
            // Check timeout first (OPT-UQ-7)
            if start.elapsed() > timeout {
                warn!(
                    question_id = %question_id,
                    elapsed_secs = start.elapsed().as_secs(),
                    timeout_secs = uq_const::DEFAULT_TIMEOUT_SECS,
                    "User question timeout"
                );

                // Update DB status to timeout
                let update_query = format!(
                    "UPDATE user_question:`{}` SET status = 'timeout'",
                    question_id
                );
                if let Err(e) = self.db.execute(&update_query).await {
                    warn!(error = %e, "Failed to update question status to timeout");
                }

                return Err(ToolError::ExecutionFailed(format!(
                    "Timeout waiting for user response after {} seconds",
                    uq_const::DEFAULT_TIMEOUT_SECS
                )));
            }

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
                debug!(question_id = %question_id, record = ?record, "Poll result");

                let status = record
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("pending");

                debug!(question_id = %question_id, status = %status, "Parsed status");

                match status {
                    "answered" => {
                        let selected_json = record
                            .get("selected_options")
                            .and_then(|v| v.as_str())
                            .unwrap_or("[]");
                        let selected: Vec<String> =
                            serde_json::from_str(selected_json).map_err(|e| {
                                ToolError::ExecutionFailed(format!(
                                    "Failed to parse selected_options JSON: {}",
                                    e
                                ))
                            })?;

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
                    "timeout" => {
                        // Status was already set to timeout (possibly by another process)
                        return Err(ToolError::ExecutionFailed("Question timed out".into()));
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
                elapsed_secs = start.elapsed().as_secs(),
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
            description: r#"Asks the user a question and waits for their response with configurable input types.

USE THIS TOOL WHEN:
- You need user input to proceed (clarification, choice, confirmation)
- A decision cannot be made autonomously
- User preferences or validation is required

IMPORTANT CONSTRAINTS:
- Timeout: 5 minutes (returns error if no response)
- Circuit breaker: After 3 consecutive timeouts, tool blocks for 60 seconds
- Maximum 20 options for checkbox type
- Question length: max 2000 characters

QUESTION TYPES:
- checkbox: Multiple choice with predefined options (user selects one or more)
- text: Free-form text input with optional placeholder
- mixed: Both options AND text input available

OPERATIONS:
- ask: Present question to user and wait for response

BEST PRACTICES:
- Keep questions clear and concise
- Provide meaningful option labels for checkbox type
- Use context parameter to explain why you're asking
- Handle timeout errors gracefully (circuit may be open)

EXAMPLES:
1. Checkbox question:
   {"operation": "ask", "question": "Which database should we use?", "questionType": "checkbox", "options": [{"id": "pg", "label": "PostgreSQL"}, {"id": "mysql", "label": "MySQL"}]}

2. Text input:
   {"operation": "ask", "question": "What should be the API endpoint name?", "questionType": "text", "textPlaceholder": "e.g., /api/v1/users"}

3. Mixed (options + text):
   {"operation": "ask", "question": "Select a template or describe custom:", "questionType": "mixed", "options": [{"id": "basic", "label": "Basic template"}], "textPlaceholder": "Custom description..."}"#.to_string(),
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

    fn requires_confirmation(&self) -> bool {
        false
    }
}
