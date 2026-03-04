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
    constants::workflow as wf_const,
    db::queries::workflow as wf_queries,
    llm::pricing::calculate_cost_with_cache,
    models::{
        llm_models::LLMModel, streaming::events, Message, Prompt, StreamChunk, ThinkingStepCreate,
        Workflow, WorkflowComplete, WorkflowMetrics, WorkflowResult, WorkflowToolExecution,
    },
    security::{validate_uuid_field, Validator},
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
    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

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

    // Use centralized query constant with bind param
    let query = format!("{} WHERE meta::id(id) = $wf_id", wf_queries::SELECT_BASIC);

    let json_results = state
        .db
        .query_json_with_params(
            &query,
            vec![(
                "wf_id".to_string(),
                serde_json::json!(validated_workflow_id),
            )],
        )
        .await
        .map_err(|e| {
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

    // Persist the initial thinking step
    let initial_step = ThinkingStepCreate {
        workflow_id: validated_workflow_id.clone(),
        message_id: message_id.clone(),
        agent_id: validated_agent_id.clone(),
        step_number: thinking_step_number,
        content: initial_reasoning,
        duration_ms: None,
        tokens: None,
        sequence: 0,
        source: "agent_flow".to_string(),
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

    // Load conversation history and build context for the LLM
    let (history_context, _history_count) =
        load_conversation_history(&state, &validated_workflow_id, &locale).await;

    // Create task with conversation history
    let task_id = Uuid::new_v4().to_string();
    info!(task_id = %task_id, "Creating task for streaming workflow");

    // Resolve skill references: {{skill:name}} -> LLM instruction to read via ReadSkillTool
    let resolved_message = Prompt::interpolate_skills(&validated_message);

    let task = Task {
        id: task_id.clone(),
        description: resolved_message,
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
    // The cancellation token is also propagated to sub-agents so they abort when the user cancels
    let execution_future = state.orchestrator.execute_with_mcp(
        &validated_agent_id,
        task,
        Some(state.mcp_manager.clone()),
        Some(cancellation_token.clone()),
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

        // Save system prompt as a system message (uses db.create with bind params
        // instead of format!() to prevent SurrealQL injection - ERR_SEC_001)
        let system_msg = crate::models::MessageCreate {
            workflow_id: validated_workflow_id.clone(),
            role: "system".to_string(),
            content: system_content.clone(),
            tokens: 0,
            tokens_input: Some(0),
            tokens_output: Some(0),
            model: None,
            provider: None,
            cost_usd: None,
            duration_ms: None,
        };

        match state
            .db
            .create("message", &system_message_id, system_msg)
            .await
        {
            Err(e) => {
                warn!(error = %e, "Failed to persist system prompt as message");
            }
            Ok(_) => {
                info!(
                    system_message_id = %system_message_id,
                    system_prompt_len = system_content.len(),
                    "Saved system prompt for workflow context reuse"
                );
            }
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
        sequence: 0,
        source: "agent_flow".to_string(),
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

    emit_chunk(
        &window,
        StreamChunk::response_block(
            validated_workflow_id.clone(),
            report.response.clone(),
            report.metrics.tokens_input,
            report.metrics.tokens_output,
            report.metrics.cached_tokens,
            report.metrics.cache_write_tokens,
        ),
    );

    // Load agent config, model pricing, and calculate cost
    let pricing = load_model_pricing_info(
        &state,
        &validated_agent_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        report.metrics.cached_tokens,
        report.metrics.cache_write_tokens,
    )
    .await;

    // Update workflow with cumulative tokens, cost, model_id, and current context size
    update_workflow_cumulative_metrics(
        &state,
        &validated_workflow_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        report.metrics.cached_tokens,
        report.metrics.cache_write_tokens,
        pricing.cost_usd,
        &pricing.model_id,
        report.metrics.context_tokens,
    )
    .await;

    aggregate_sub_agent_tokens(&state, &validated_workflow_id).await;

    // Convert tool executions to IPC-friendly format (clones necessary for IPC serialization)
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

    // Persist tool executions via shared persistence module
    crate::db::persist_tool_executions(
        &state.db,
        &report.metrics.tool_executions,
        &validated_workflow_id,
        &message_id,
        &validated_agent_id,
    )
    .await;

    // Persist intermediate reasoning steps via shared persistence module
    thinking_step_number = crate::db::persist_reasoning_steps(
        &state.db,
        &report.metrics.reasoning_steps,
        &validated_workflow_id,
        &message_id,
        &validated_agent_id,
        thinking_step_number,
    )
    .await;

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
            cost_usd: pricing.cost_usd,
            provider: pricing.provider,
            model: pricing.model,
            cached_tokens: report.metrics.cached_tokens,
            cache_write_tokens: report.metrics.cache_write_tokens,
            iteration_metrics: report.metrics.iteration_metrics.clone(),
        },
        tools_used: report.metrics.tools_used.clone(),
        mcp_calls: report.metrics.mcp_calls.clone(),
        tool_executions,
        message_id: message_id.clone(),
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

/// Loads conversation history and builds the context payload for the LLM.
///
/// Returns the history context JSON and the number of loaded messages.
async fn load_conversation_history(
    state: &AppState,
    workflow_id: &str,
    locale: &str,
) -> (serde_json::Value, usize) {
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
        WHERE workflow_id = $wf_id
        ORDER BY timestamp ASC
        LIMIT {}"#,
        wf_const::MESSAGE_HISTORY_LIMIT
    );

    let history_json = state
        .db
        .query_json_with_params(
            &history_query,
            vec![("wf_id".to_string(), serde_json::json!(workflow_id))],
        )
        .await
        .unwrap_or_default();
    let conversation_history: Vec<Message> = history_json
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    let has_system_message = conversation_history
        .iter()
        .any(|msg| matches!(msg.role, crate::models::MessageRole::System));

    let history_count = conversation_history.len();

    let history_context = if has_system_message && !conversation_history.is_empty() {
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
            "workflow_id": workflow_id,
            "locale": locale
        })
    } else {
        serde_json::json!({
            "is_primary_agent": true,
            "workflow_id": workflow_id,
            "locale": locale
        })
    };

    info!(
        history_count = history_count,
        has_system_message = has_system_message,
        is_continuation = has_system_message && !conversation_history.is_empty(),
        "Loaded conversation history for context"
    );

    (history_context, history_count)
}

/// Pricing information for a model, loaded from agent config and database.
struct ModelPricingInfo {
    provider: String,
    model: String,
    model_id: String,
    cost_usd: f64,
}

/// Loads agent configuration and model pricing info, then calculates cost.
///
/// Supports cached token pricing: when `cached_tokens` or `cache_write_tokens`
/// is provided, the cost splits input tokens between regular, cache-read, and cache-write rates.
async fn load_model_pricing_info(
    state: &AppState,
    agent_id: &str,
    tokens_input: usize,
    tokens_output: usize,
    cached_tokens: Option<usize>,
    cache_write_tokens: Option<usize>,
) -> ModelPricingInfo {
    let (provider, model) = match state.registry.get(agent_id).await {
        Some(agent) => {
            let config = agent.config();
            (config.llm.provider.clone(), config.llm.model.clone())
        }
        None => ("Unknown".to_string(), agent_id.to_string()),
    };

    let (input_price, output_price, cache_read_price, cache_write_price, model_id) = {
        let provider_lower = provider.to_lowercase();
        let model_query = "SELECT meta::id(id) AS id, provider, name, api_name, context_window, \
             max_output_tokens, temperature_default, is_builtin, is_reasoning, \
             (input_price_per_mtok ?? 0.0) AS input_price_per_mtok, \
             (output_price_per_mtok ?? 0.0) AS output_price_per_mtok, \
             (cache_read_price_per_mtok ?? 0.0) AS cache_read_price_per_mtok, \
             (cache_write_price_per_mtok ?? 0.0) AS cache_write_price_per_mtok, \
             created_at, updated_at \
             FROM llm_model WHERE api_name = $model_name AND provider = $provider_name";

        match state
            .db
            .db
            .query(model_query)
            .bind(("model_name", model.clone()))
            .bind(("provider_name", provider_lower.clone()))
            .await
        {
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
                            cache_read_price = loaded_model.cache_read_price_per_mtok,
                            cache_write_price = loaded_model.cache_write_price_per_mtok,
                            "Loaded model for pricing"
                        );
                        (
                            loaded_model.input_price_per_mtok,
                            loaded_model.output_price_per_mtok,
                            loaded_model.cache_read_price_per_mtok,
                            loaded_model.cache_write_price_per_mtok,
                            loaded_model.id,
                        )
                    }
                    _ => {
                        warn!(model_api_name = %model, provider = %provider, "Model not found for pricing, using defaults");
                        (0.0, 0.0, 0.0, 0.0, model.clone())
                    }
                }
            }
            Err(e) => {
                warn!(error = %e, "Failed to load model for pricing, using defaults");
                (0.0, 0.0, 0.0, 0.0, model.clone())
            }
        }
    };

    let cost_usd = calculate_cost_with_cache(
        tokens_input,
        tokens_output,
        cached_tokens,
        cache_write_tokens,
        input_price,
        output_price,
        cache_read_price,
        cache_write_price,
    );

    info!(
        tokens_input = tokens_input,
        tokens_output = tokens_output,
        cached_tokens = ?cached_tokens,
        cache_write_tokens = ?cache_write_tokens,
        input_price = input_price,
        output_price = output_price,
        cache_read_price = cache_read_price,
        cache_write_price = cache_write_price,
        cost_usd = cost_usd,
        "Calculated token cost"
    );

    ModelPricingInfo {
        provider,
        model,
        model_id,
        cost_usd,
    }
}

/// Updates workflow cumulative token counts, cost, model, and context size.
#[allow(clippy::too_many_arguments)]
async fn update_workflow_cumulative_metrics(
    state: &AppState,
    workflow_id: &str,
    tokens_input: usize,
    tokens_output: usize,
    cached_tokens: Option<usize>,
    cache_write_tokens: Option<usize>,
    cost_usd: f64,
    model_id: &str,
    context_tokens: usize,
) {
    let cached = cached_tokens.unwrap_or(0);
    let cache_write = cache_write_tokens.unwrap_or(0);
    let update_query = format!(
        "UPDATE workflow:`{}` SET \
            total_tokens_input = (total_tokens_input ?? 0) + $tokens_in, \
            total_tokens_output = (total_tokens_output ?? 0) + $tokens_out, \
            total_cached_tokens = (total_cached_tokens ?? 0) + $cached, \
            total_cache_write_tokens = (total_cache_write_tokens ?? 0) + $cache_write, \
            total_cost_usd = (total_cost_usd ?? 0.0) + $cost, \
            model_id = $model_id, \
            current_context_tokens = $context_tokens, \
            updated_at = time::now()",
        workflow_id
    );

    info!(
        tokens_in = tokens_input,
        tokens_out = tokens_output,
        cached = cached,
        cache_write = cache_write,
        cost = cost_usd,
        model_id = %model_id,
        "Executing workflow token update"
    );

    if let Err(e) = state
        .db
        .db
        .query(&update_query)
        .bind(("tokens_in", tokens_input))
        .bind(("tokens_out", tokens_output))
        .bind(("cached", cached))
        .bind(("cache_write", cache_write))
        .bind(("cost", cost_usd))
        .bind(("model_id", model_id.to_string()))
        .bind(("context_tokens", context_tokens))
        .await
    {
        error!(error = %e, "Failed to update workflow cumulative tokens");
    } else {
        info!(
            workflow_id = %workflow_id,
            tokens_input = tokens_input,
            tokens_output = tokens_output,
            cached_tokens = cached,
            cache_write_tokens = cache_write,
            current_context = context_tokens,
            cost_usd = cost_usd,
            model_id = %model_id,
            "Updated workflow cumulative tokens and context"
        );
    }
}

/// Aggregates sub-agent tokens into separate workflow fields.
///
/// Queries all completed sub_agent_execution records for this workflow
/// and stores their token totals in sub_agent_tokens_input/output.
/// These are kept separate from total_tokens_input/output (main agent only)
/// so the frontend can display both independently and compute combined totals.
async fn aggregate_sub_agent_tokens(state: &AppState, workflow_id: &str) {
    let sum_query = "SELECT math::sum(tokens_input) AS total_in, \
                            math::sum(tokens_output) AS total_out \
                     FROM sub_agent_execution \
                     WHERE workflow_id = $wf_id AND status = 'completed' \
                     GROUP ALL";

    match state
        .db
        .db
        .query(sum_query)
        .bind(("wf_id", workflow_id.to_string()))
        .await
    {
        Ok(mut response) => {
            let result: Option<serde_json::Value> = response.take(0).unwrap_or(None);
            if let Some(row) = result {
                let tokens_in = row.get("total_in").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let tokens_out =
                    row.get("total_out").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

                if tokens_in > 0 || tokens_out > 0 {
                    let update_query = format!(
                        "UPDATE workflow:`{}` SET \
                            sub_agent_tokens_input = $tokens_in, \
                            sub_agent_tokens_output = $tokens_out",
                        workflow_id
                    );

                    if let Err(e) = state
                        .db
                        .db
                        .query(&update_query)
                        .bind(("tokens_in", tokens_in))
                        .bind(("tokens_out", tokens_out))
                        .await
                    {
                        error!(error = %e, "Failed to store sub-agent tokens");
                    } else {
                        info!(
                            workflow_id = %workflow_id,
                            sub_agent_tokens_in = tokens_in,
                            sub_agent_tokens_out = tokens_out,
                            "Stored sub-agent tokens in separate fields"
                        );
                    }
                }
            }
        }
        Err(e) => {
            error!(error = %e, "Failed to query sub-agent tokens for aggregation");
        }
    }
}

// persist_tool_executions_batch and persist_reasoning_steps_batch
// moved to db::persistence module for reuse by sub-agent executor

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

    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

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
        let chunk = StreamChunk::reasoning("wf_001".to_string(), "Analyzing...".to_string());
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
