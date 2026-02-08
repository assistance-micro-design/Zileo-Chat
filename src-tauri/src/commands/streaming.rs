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

//! Streaming workflow execution with real-time events.
//!
//! Provides Tauri commands for executing workflows with streaming
//! responses via Tauri events.

use crate::{
    agents::core::agent::Task,
    db::queries::workflow as wf_queries,
    llm::pricing::calculate_cost,
    models::{
        llm_models::LLMModel, streaming::events, Message, StreamChunk, ThinkingStepCreate,
        ToolExecutionCreate, Workflow, WorkflowComplete, WorkflowMetrics, WorkflowResult,
        WorkflowToolExecution,
    },
    security::Validator,
    tools::constants::workflow as wf_const,
    AppState,
};
use tauri::{Emitter, State, Window};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Executes a workflow with streaming events.
///
/// Emits events during execution:
/// - `workflow_stream`: For each token/tool/reasoning chunk
/// - `workflow_complete`: When execution finishes
///
/// # Arguments
/// * `window` - Tauri window for event emission
/// * `workflow_id` - Associated workflow ID
/// * `message` - User message to process
/// * `agent_id` - Agent to execute with
/// * `locale` - User's selected language (e.g., "en", "fr")
///
/// # Returns
/// Final workflow result after streaming completes
#[tauri::command]
#[instrument(
    name = "execute_workflow_streaming",
    skip(window, state, message),
    fields(
        workflow_id = %workflow_id,
        agent_id = %agent_id,
        message_len = message.len(),
        locale = %locale
    )
)]
pub async fn execute_workflow_streaming(
    window: Window,
    workflow_id: String,
    message: String,
    agent_id: String,
    locale: String,
    state: State<'_, AppState>,
) -> Result<WorkflowResult, String> {
    info!("Starting streaming workflow execution");

    // Validate inputs
    let validated_workflow_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    let validated_message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // Safety net: enforce concurrent workflow limit
    // Frontend enforces this too, but backend provides race condition protection
    let running_count = state.streaming_cancellations.lock().await.len();
    let max_concurrent: usize = 3; // Maximum concurrent workflows (frontend also enforces per-mode limits)
    if running_count >= max_concurrent {
        return Err(format!(
            "Maximum concurrent workflows ({}) reached. Please wait for a workflow to complete.",
            max_concurrent
        ));
    }

    // Create cancellation token for this workflow (enables real cancel functionality)
    let cancellation_token = state
        .create_cancellation_token(&validated_workflow_id)
        .await;

    // OPT-WF-1: Use centralized query constant
    let query = format!(
        "{} WHERE meta::id(id) = '{}'",
        wf_queries::SELECT_BASIC,
        validated_workflow_id
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow");
        emit_error(
            &window,
            &validated_workflow_id,
            &format!("Failed to load workflow: {}", e),
        );
        format!("Failed to load workflow: {}", e)
    })?;

    let workflows: Vec<Workflow> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<std::result::Result<Vec<Workflow>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize workflow");
            emit_error(
                &window,
                &validated_workflow_id,
                &format!("Failed to deserialize workflow: {}", e),
            );
            format!("Failed to deserialize workflow: {}", e)
        })?;

    let _workflow = workflows.first().ok_or_else(|| {
        warn!(workflow_id = %validated_workflow_id, "Workflow not found");
        emit_error(&window, &validated_workflow_id, "Workflow not found");
        "Workflow not found".to_string()
    })?;

    // Generate a message ID for this execution (the assistant response)
    // This is generated early so thinking steps can reference it
    let message_id = Uuid::new_v4().to_string();

    // Counter for thinking steps
    let mut thinking_step_number: u32 = 0;

    // Emit and persist initial reasoning step
    let initial_reasoning = "Analyzing request and preparing response...".to_string();
    emit_chunk(
        &window,
        StreamChunk::reasoning(validated_workflow_id.clone(), initial_reasoning.clone()),
    );

    // Emit initial content to show user something is happening
    emit_chunk(
        &window,
        StreamChunk::token(
            validated_workflow_id.clone(),
            "Processing your request...\n\n".to_string(),
        ),
    );

    // Persist the initial thinking step
    let initial_step = ThinkingStepCreate {
        workflow_id: validated_workflow_id.clone(),
        message_id: message_id.clone(),
        agent_id: validated_agent_id.clone(),
        step_number: thinking_step_number,
        content: initial_reasoning,
        duration_ms: None,
        tokens: None,
    };
    let step_id = Uuid::new_v4().to_string();
    if let Err(e) = state
        .db
        .create("thinking_step", &step_id, initial_step)
        .await
    {
        warn!(error = %e, "Failed to persist initial thinking step");
    }
    thinking_step_number += 1;

    // Load conversation history for context (API-native format for continuation)
    // Messages are stored with role: system|user|assistant
    let history_query = format!(
        r#"SELECT
            meta::id(id) AS id,
            workflow_id,
            role,
            content,
            tokens,
            tokens_input,
            tokens_output,
            model,
            provider,
            cost_usd,
            duration_ms,
            timestamp
        FROM message
        WHERE workflow_id = '{}'
        ORDER BY timestamp ASC
        LIMIT {}"#, // OPT-WF-3: Use centralized constant
        validated_workflow_id,
        wf_const::MESSAGE_HISTORY_LIMIT
    );

    let history_json = state
        .db
        .query_json(&history_query)
        .await
        .unwrap_or_default();
    let conversation_history: Vec<Message> = history_json
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    // Check if we have a system message (indicates existing context)
    let has_system_message = conversation_history
        .iter()
        .any(|msg| matches!(msg.role, crate::models::MessageRole::System));

    // Build conversation context for the LLM
    // If we have existing messages with system prompt, pass them as conversation_messages
    // for direct reuse (no reconstruction needed)
    // Note: locale is always passed for system prompt injection (first message only uses it)
    let history_context = if has_system_message && !conversation_history.is_empty() {
        // Continuation: format messages for API-native reuse
        let api_messages: Vec<serde_json::Value> = conversation_history
            .iter()
            .map(|msg| {
                serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                })
            })
            .collect();
        serde_json::json!({
            "conversation_messages": api_messages,
            "is_primary_agent": true,
            "workflow_id": validated_workflow_id.clone(),
            "locale": locale.clone()
        })
    } else {
        // First message or no system prompt: let agent build the context
        serde_json::json!({
            "is_primary_agent": true,
            "workflow_id": validated_workflow_id.clone(),
            "locale": locale.clone()
        })
    };

    info!(
        history_count = conversation_history.len(),
        has_system_message = has_system_message,
        is_continuation = has_system_message && !conversation_history.is_empty(),
        "Loaded conversation history for context"
    );

    // Create task with conversation history
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for streaming workflow");

    let task = Task {
        id: task_id.clone(),
        description: validated_message,
        context: history_context,
    };

    // Emit tool start (agent execution)
    emit_chunk(
        &window,
        StreamChunk::tool_start(validated_workflow_id.clone(), validated_agent_id.clone()),
    );

    let start_time = std::time::Instant::now();

    // Execute via orchestrator with MCP support, racing against cancellation token
    // Using tokio::select! allows the execution to be cancelled immediately when the user clicks Cancel
    let execution_future = state.orchestrator.execute_with_mcp(
        &validated_agent_id,
        task,
        Some(state.mcp_manager.clone()),
    );

    let report = tokio::select! {
        // Execution branch - runs the actual LLM call
        result = execution_future => {
            match result {
                Ok(report) => report,
                Err(e) => {
                    error!(error = %e, task_id = %task_id, "Streaming workflow execution failed");
                    emit_chunk(
                        &window,
                        StreamChunk::error(validated_workflow_id.clone(), e.to_string()),
                    );
                    emit_complete(
                        &window,
                        WorkflowComplete::failed(validated_workflow_id.clone(), e.to_string()),
                    );
                    state.clear_cancellation(&validated_workflow_id).await;
                    return Err(format!("Execution failed: {}", e));
                }
            }
        }
        // Cancellation branch - triggers when user clicks Cancel button
        _ = cancellation_token.cancelled() => {
            warn!(workflow_id = %validated_workflow_id, "Workflow cancelled by user during execution");
            emit_chunk(
                &window,
                StreamChunk::error(validated_workflow_id.clone(), "Cancelled by user".to_string()),
            );
            emit_complete(
                &window,
                WorkflowComplete::cancelled(validated_workflow_id.clone()),
            );
            state.clear_cancellation(&validated_workflow_id).await;
            return Err("Workflow cancelled by user".to_string());
        }
    };

    // Execution completed successfully - process the report
    let duration = start_time.elapsed().as_millis() as u64;

    // Emit tool end
    emit_chunk(
        &window,
        StreamChunk::tool_end(
            validated_workflow_id.clone(),
            validated_agent_id.clone(),
            duration,
        ),
    );

    // If this is the first message, save the system prompt for future conversations
    // This enables context reuse without reconstruction
    if let Some(ref system_prompt) = report.system_prompt {
        let system_message_id = Uuid::new_v4().to_string();
        let system_content = system_prompt.clone();

        // Save system prompt as a system message (will be loaded in future conversations)
        let insert_query = format!(
            "CREATE message:`{}` CONTENT {{ \
                workflow_id: '{}', \
                role: 'system', \
                content: {}, \
                tokens: 0, \
                tokens_input: 0, \
                tokens_output: 0, \
                timestamp: time::now() \
            }}",
            system_message_id,
            validated_workflow_id,
            serde_json::to_string(&system_content).unwrap_or_else(|_| "\"\"".to_string())
        );

        if let Err(e) = state.db.execute(&insert_query).await {
            warn!(error = %e, "Failed to persist system prompt as message");
        } else {
            info!(
                system_message_id = %system_message_id,
                system_prompt_len = system_content.len(),
                "Saved system prompt for workflow context reuse"
            );
        }
    }

    // Emit and persist reasoning step about execution completion
    let completion_reasoning = format!(
        "Execution completed in {}ms. Processing {} tool call(s).",
        duration,
        report.metrics.tool_executions.len()
    );
    emit_chunk(
        &window,
        StreamChunk::reasoning(validated_workflow_id.clone(), completion_reasoning.clone()),
    );

    // Persist the completion thinking step
    let completion_step = ThinkingStepCreate {
        workflow_id: validated_workflow_id.clone(),
        message_id: message_id.clone(),
        agent_id: validated_agent_id.clone(),
        step_number: thinking_step_number,
        content: completion_reasoning,
        duration_ms: Some(duration),
        tokens: None,
    };
    let completion_step_id = Uuid::new_v4().to_string();
    if let Err(e) = state
        .db
        .create("thinking_step", &completion_step_id, completion_step)
        .await
    {
        warn!(error = %e, "Failed to persist completion thinking step");
    }
    thinking_step_number += 1;

    // Clear the placeholder and stream the actual response content
    // First, emit a newline to visually separate from placeholder
    emit_chunk(
        &window,
        StreamChunk::token(validated_workflow_id.clone(), "\n".to_string()),
    );

    // Stream the response content in chunks
    let content = &report.content;
    let chunk_size = 50; // Characters per chunk for simulated streaming
    let mut cancelled = false;

    // OPT-WF-6: Single allocation outside loop instead of per-iteration
    let chars: Vec<char> = content.chars().collect();
    for (i, chunk) in chars.chunks(chunk_size).enumerate() {
        // OPT-WF-7: Use sync is_cancelled() instead of async state.is_cancelled()
        // CancellationToken::is_cancelled() is synchronous (no Mutex lock per iteration)
        if cancellation_token.is_cancelled() {
            warn!(workflow_id = %validated_workflow_id, "Streaming cancelled by user during response display");
            cancelled = true;
            emit_chunk(
                &window,
                StreamChunk::error(
                    validated_workflow_id.clone(),
                    "Cancelled by user".to_string(),
                ),
            );
            emit_complete(
                &window,
                WorkflowComplete::cancelled(validated_workflow_id.clone()),
            );
            // Clear the cancellation flag
            state.clear_cancellation(&validated_workflow_id).await;
            break;
        }

        let chunk_text: String = chunk.iter().collect();
        emit_chunk(
            &window,
            StreamChunk::token(validated_workflow_id.clone(), chunk_text),
        );

        // Small delay between chunks to simulate streaming
        if i < chars.len() / chunk_size {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    }

    if cancelled {
        return Err("Workflow cancelled by user".to_string());
    }

    // Get agent config for accurate provider/model info (OPT-7: avoid cloning by using &str temporarily)
    let (provider, model) = match state.registry.get(&validated_agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => {
            // Fallback if agent not found (shouldn't happen after successful execution)
            ("Unknown".to_string(), validated_agent_id.clone())
        }
    };
    // Note: Further optimization would require lifetime annotations in WorkflowMetrics, which
    // would be a breaking change. Current clone is acceptable as it's post-execution.

    // Load model to get pricing info for cost calculation
    // Note: model is the api_name (e.g. "mistral-large-latest"), not the UUID
    // We need to search by api_name + provider to find the correct model
    let (input_price, output_price, model_id) = {
        // Convert provider string to lowercase for matching (DB stores lowercase)
        let provider_lower = provider.to_lowercase();
        let model_query = format!(
            "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
             max_output_tokens, temperature_default, is_builtin, is_reasoning, \
             (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
             (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
             created_at, updated_at \
             FROM llm_model WHERE api_name = '{}' AND provider = '{}'",
            model, provider_lower
        );

        match state.db.db.query(&model_query).await {
            Ok(mut response) => {
                let models: Result<Vec<LLMModel>, _> = response.take(0);
                match models {
                    Ok(mut m) if !m.is_empty() => {
                        let loaded_model = m.remove(0);
                        info!(
                            model_api_name = %model,
                            model_id = %loaded_model.id,
                            input_price = loaded_model.input_price_per_mtok,
                            output_price = loaded_model.output_price_per_mtok,
                            "Loaded model for pricing"
                        );
                        (
                            loaded_model.input_price_per_mtok,
                            loaded_model.output_price_per_mtok,
                            loaded_model.id,
                        )
                    }
                    _ => {
                        warn!(model_api_name = %model, provider = %provider, "Model not found for pricing, using defaults");
                        (0.0, 0.0, model.clone())
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load model for pricing, using defaults");
                (0.0, 0.0, model.clone())
            }
        }
    };

    // Calculate cost using model pricing
    let cost_usd = calculate_cost(
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        input_price,
        output_price,
    );

    info!(
        tokens_input = report.metrics.tokens_input,
        tokens_output = report.metrics.tokens_output,
        input_price = input_price,
        output_price = output_price,
        cost_usd = cost_usd,
        "Calculated token cost"
    );

    // Update workflow with cumulative tokens, cost, model_id, and current context size
    // OPT-WF-2: Use ?? (null coalescing) instead of IF/THEN/ELSE to eliminate param duplication
    // Use explicit float formatting to avoid scientific notation (e.g., 1.6e-5)
    // current_context_tokens = tokens_input (actual context size at last API call)
    let update_query = format!(
        "UPDATE workflow:`{}` SET \
            total_tokens_input = (total_tokens_input ?? 0) + {}, \
            total_tokens_output = (total_tokens_output ?? 0) + {}, \
            total_cost_usd = (total_cost_usd ?? 0.0) + {:.10}, \
            model_id = '{}', \
            current_context_tokens = {}, \
            updated_at = time::now()",
        validated_workflow_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        cost_usd,
        model_id,                    // Use real model UUID, not api_name
        report.metrics.tokens_input  // Current context size (last API call)
    );

    // Log the query for debugging
    info!(query = %update_query, "Executing workflow token update");

    if let Err(e) = state.db.db.query(&update_query).await {
        error!(error = %e, query = %update_query, "Failed to update workflow cumulative tokens");
    } else {
        info!(
            workflow_id = %validated_workflow_id,
            tokens_input = report.metrics.tokens_input,
            tokens_output = report.metrics.tokens_output,
            current_context = report.metrics.tokens_input,
            cost_usd = cost_usd,
            model_id = %model_id,
            "Updated workflow cumulative tokens and context"
        );
    }

    // Convert tool executions to IPC-friendly format (OPT-7: clones necessary for IPC serialization)
    let tool_executions: Vec<WorkflowToolExecution> = report
        .metrics
        .tool_executions
        .iter()
        .map(|te| WorkflowToolExecution {
            tool_type: te.tool_type.clone(),
            tool_name: te.tool_name.clone(),
            server_name: te.server_name.clone(),
            input_params: te.input_params.clone(),
            output_result: te.output_result.clone(),
            success: te.success,
            error_message: te.error_message.clone(),
            duration_ms: te.duration_ms,
            iteration: te.iteration,
        })
        .collect();
    // Note: Clones here are necessary as WorkflowToolExecution needs owned data for Tauri IPC

    // Persist tool executions to database (message_id was generated earlier)
    for (idx, te) in tool_executions.iter().enumerate() {
        let execution_id = Uuid::new_v4().to_string();
        let execution = ToolExecutionCreate {
            workflow_id: validated_workflow_id.clone(),
            message_id: message_id.clone(),
            agent_id: validated_agent_id.clone(),
            tool_type: te.tool_type.clone(),
            tool_name: te.tool_name.clone(),
            server_name: te.server_name.clone(),
            input_params: te.input_params.clone(),
            output_result: te.output_result.clone(),
            success: te.success,
            error_message: te.error_message.clone(),
            duration_ms: te.duration_ms,
            iteration: te.iteration,
        };

        if let Err(e) = state
            .db
            .create("tool_execution", &execution_id, execution)
            .await
        {
            warn!(
                error = %e,
                tool_name = %te.tool_name,
                index = idx,
                "Failed to persist tool execution"
            );
        }
    }

    // Persist intermediate reasoning steps from agent execution
    for rs in &report.metrics.reasoning_steps {
        let step_id = Uuid::new_v4().to_string();
        let step = ThinkingStepCreate {
            workflow_id: validated_workflow_id.clone(),
            message_id: message_id.clone(),
            agent_id: validated_agent_id.clone(),
            step_number: thinking_step_number,
            content: rs.content.clone(),
            duration_ms: Some(rs.duration_ms),
            tokens: None,
        };
        if let Err(e) = state.db.create("thinking_step", &step_id, step).await {
            warn!(error = %e, step_number = thinking_step_number, "Failed to persist intermediate reasoning step");
        }
        thinking_step_number += 1;
    }

    info!(
        tool_executions_count = tool_executions.len(),
        thinking_steps_count = thinking_step_number,
        "Persisted tool executions and thinking steps to database"
    );

    // Build result with calculated cost
    let result = WorkflowResult {
        report: report.content,
        response: report.response,
        metrics: WorkflowMetrics {
            duration_ms: report.metrics.duration_ms,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cost_usd,
            provider,
            model,
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
        tool_executions,
    };

    // Emit completion
    emit_complete(
        &window,
        WorkflowComplete::success(validated_workflow_id.clone()),
    );

    info!(
        duration_ms = result.metrics.duration_ms,
        tokens_input = result.metrics.tokens_input,
        tokens_output = result.metrics.tokens_output,
        tool_executions_count = result.tool_executions.len(),
        "Streaming workflow execution completed"
    );

    // Cleanup: remove cancellation token from map on successful completion
    state.clear_cancellation(&validated_workflow_id).await;

    Ok(result)
}

/// Helper function to emit a stream chunk event.
fn emit_chunk(window: &Window, chunk: StreamChunk) {
    if let Err(e) = window.emit(events::WORKFLOW_STREAM, &chunk) {
        warn!(error = %e, "Failed to emit stream chunk");
    }
}

/// Helper function to emit a completion event.
fn emit_complete(window: &Window, complete: WorkflowComplete) {
    if let Err(e) = window.emit(events::WORKFLOW_COMPLETE, &complete) {
        warn!(error = %e, "Failed to emit completion event");
    }
}

/// Helper function to emit an error and completion.
fn emit_error(window: &Window, workflow_id: &str, error: &str) {
    emit_chunk(
        window,
        StreamChunk::error(workflow_id.to_string(), error.to_string()),
    );
    emit_complete(
        window,
        WorkflowComplete::failed(workflow_id.to_string(), error.to_string()),
    );
}

/// Cancels a streaming workflow execution immediately.
///
/// Triggers the cancellation token associated with the workflow, causing the
/// execute_workflow_streaming function to abort via tokio::select!.
/// This provides immediate cancellation, even during LLM execution.
///
/// # Arguments
/// * `workflow_id` - The workflow ID to cancel
/// * `state` - Application state containing the cancellation tokens
#[tauri::command]
#[instrument(name = "cancel_workflow_streaming", skip(state), fields(workflow_id = %workflow_id))]
pub async fn cancel_workflow_streaming(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Cancelling streaming workflow");

    // Validate workflow ID
    let validated_id = Validator::validate_uuid(&workflow_id).map_err(|e| {
        warn!(error = %e, "Invalid workflow_id");
        format!("Invalid workflow_id: {}", e)
    })?;

    // Request cancellation
    state.request_cancellation(&validated_id).await;
    info!(workflow_id = %validated_id, "Workflow cancellation requested");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::streaming::CompletionStatus;

    #[test]
    fn test_stream_chunk_creation() {
        let chunk = StreamChunk::token("wf_001".to_string(), "Hello".to_string());
        assert_eq!(chunk.workflow_id, "wf_001");
        assert!(chunk.content.is_some());

        let chunk = StreamChunk::tool_start("wf_001".to_string(), "search".to_string());
        assert!(chunk.tool.is_some());
        assert!(chunk.content.is_none());
    }

    #[test]
    fn test_workflow_complete_creation() {
        let complete = WorkflowComplete::success("wf_001".to_string());
        assert_eq!(complete.status, CompletionStatus::Completed);
        assert!(complete.error.is_none());

        let complete = WorkflowComplete::failed("wf_001".to_string(), "Error".to_string());
        assert_eq!(complete.status, CompletionStatus::Error);
        assert!(complete.error.is_some());
    }

    #[test]
    fn test_event_names() {
        assert_eq!(events::WORKFLOW_STREAM, "workflow_stream");
        assert_eq!(events::WORKFLOW_COMPLETE, "workflow_complete");
    }
}
