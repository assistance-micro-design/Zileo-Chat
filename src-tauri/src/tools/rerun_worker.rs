// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! RerunWorkerTool — re-runs the worker behind the current Kanban card.
//!
//! Self-gating: the target card is resolved via
//! `review_chat_workflow_id = <chat workflow id>` (captured at construction);
//! the worker workflow is `card.workflow_id` and the worker agent is
//! `card.target_agent_id`.
//!
//! The re-run executes the worker agent via `tool_loop::execute_with_tools`
//! with a freshly built `AgentToolContext`. The context is required for two
//! reasons:
//!   1. **Live streaming** — the captured `AppHandle` is propagated so the
//!      tool loop's `emit_progress` / `emit_reasoning` fire `WORKFLOW_STREAM`
//!      chunks on the worker workflow id. A page agent (or any UI) viewing
//!      that workflow can therefore stream the run in real time. The chunks
//!      are routed by the standard `backgroundWorkflowsStore` machinery,
//!      which auto-registers the workflow on first chunk (front-end change
//!      paired with this fix).
//!   2. **Sub-agent attribution** — the pre-allocated `assistant_message_id`
//!      is propagated as `current_message_id` so Spawn/Delegate/Parallel
//!      tools persist `sub_agent_execution.parent_message_id` at CREATE time;
//!      without it `load_workflow_blocks_core` silently drops orphan sub-agents
//!      on reload.
//!
//! Because the standard streaming flow persists worker messages from the
//! frontend, this backend-initiated path persists them itself: the instruction
//! as a `user` message, the produced report as an `assistant` message (so
//! `load_workflow_report` returns the refreshed report), the tool executions,
//! the reasoning steps, and the workflow's cumulative metrics + sub-agent
//! rollup. Mirrors `commands/streaming/persistence_step::finalize_completion`
//! minus the live response_block emission (the chunk loop has already emitted
//! everything live).
//!
//! Resolving `AppState` from the captured `AppHandle` is required because the
//! re-run needs the provider manager, tool factory and MCP manager (a tool
//! normally only carries `db`). The capture pattern mirrors TodoTool /
//! SpawnAgentTool; the `.state::<AppState>()` call mirrors main.rs:742.

use crate::agents::core::agent::{Report, ReportStatus, Task};
use crate::agents::execution::tool_loop::{execute_with_tools, ToolLoopContext};
use crate::commands::kanban_card::{get_kanban_card_core, resolve_card_id_by_review_chat};
use crate::commands::message::{save_message_core, SaveMessageParams};
use crate::commands::streaming::helpers::{aggregate_sub_agent_metrics, load_conversation_history};
use crate::commands::streaming::pricing::{
    load_model_pricing_info, update_workflow_cumulative_metrics, CumulativeMetricsUpdate,
};
use crate::db::DBClient;
use crate::models::function_calling::ToolChoiceMode;
use crate::security::Validator;
use crate::tools::context::AgentToolContext;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::utils::safe_truncate;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use crate::AppState;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tauri::{AppHandle, Manager};
use tracing::{debug, info, warn};

/// Cap on the report excerpt returned to the chatting agent.
const RERUN_REPORT_MAX_CHARS: usize = 6000;

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "RerunWorkerTool".to_string(),
    name: "RerunWorker".to_string(),
    summary: "Re-run the worker behind the current Kanban card with an extra instruction"
        .to_string(),
    description: ToolDescriptionBuilder::new(
        "Re-runs the worker workflow behind the current Kanban card, appending an instruction \
         so the worker regenerates or enriches its report.",
    )
    .use_when(&[
        "The report needs to be regenerated or enriched and you can phrase the fix as an instruction",
        "You want the worker to redo its task with extra guidance before validating the card",
    ])
    .do_not_use(&[
        "You are not in a card review chat (the tool cannot resolve a card and will error)",
        "The card never ran a worker (no workflow to re-run)",
        "You only want to move the card (use MoveCardTool) or edit a prompt (use PromptManager)",
    ])
    .operations(&[(
        "rerun",
        "Append `instruction` to the worker workflow and re-run it; returns the new report",
    )])
    .note(
        "The re-run is synchronous and can take a while (it runs the full worker agent). \
         The worker workflow's report is updated in place.",
    )
    .examples(&[
        json!({"instruction": "Add a section comparing this week to last week."}),
        json!({"instruction": "The tone is too technical — rewrite the summary for a general audience."}),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "instruction": {"type": "string", "description": "Extra instruction appended as a new user turn to the worker workflow."}
        },
        "required": ["instruction"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
}
});

/// Re-runs the worker behind the current Kanban card.
pub struct RerunWorkerTool {
    db: Arc<DBClient>,
    chat_workflow_id: Option<String>,
    /// Captured at construction (like TodoTool). Resolved to `AppState` at
    /// execution time so the detached re-run can reach the provider manager,
    /// tool factory and MCP manager. `None` self-gates the tool.
    app_handle: Option<AppHandle>,
}

impl RerunWorkerTool {
    pub fn new(
        db: Arc<DBClient>,
        chat_workflow_id: Option<String>,
        app_handle: Option<AppHandle>,
    ) -> Self {
        Self {
            db,
            chat_workflow_id,
            app_handle,
        }
    }

    /// Reads the UI language stamped on the worker workflow (best-effort,
    /// defaults to "en").
    async fn load_locale(&self, workflow_id: &str) -> String {
        let q = "SELECT locale FROM workflow WHERE id = $wid LIMIT 1";
        self.db
            .query_json_with_params(q, vec![("wid".to_string(), json!(workflow_id))])
            .await
            .ok()
            .and_then(|rows| {
                rows.into_iter()
                    .next()
                    .and_then(|r| r["locale"].as_str().map(String::from))
            })
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_else(|| "en".to_string())
    }
}

#[async_trait]
impl Tool for RerunWorkerTool {
    fn id(&self) -> &str {
        "RerunWorkerTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let instruction = input["instruction"].as_str().unwrap_or("");
        let instruction = Validator::validate_message(instruction)
            .map_err(|e| ToolError::ValidationFailed(format!("instruction: {:?}", e)))?;

        // Resolve the card and its worker workflow / agent.
        let card_id = resolve_card_id_by_review_chat(&self.db, self.chat_workflow_id.as_deref())
            .await
            .map_err(ToolError::ExecutionFailed)?;
        let card = get_kanban_card_core(&self.db, &card_id)
            .await
            .map_err(ToolError::ExecutionFailed)?;
        let worker_wf = card.workflow_id.clone().ok_or_else(|| {
            ToolError::ExecutionFailed("This card has no worker workflow to re-run".to_string())
        })?;
        let target_agent_id = card.target_agent_id.clone();

        // Resolve AppState from the captured handle (managers needed for the run).
        let app_handle = self.app_handle.clone().ok_or_else(|| {
            ToolError::ExecutionFailed(
                "RerunWorkerTool has no app handle — cannot reach the agent runtime".to_string(),
            )
        })?;
        let state = app_handle.state::<AppState>();
        let app_state: &AppState = &state;

        // Load the worker agent config from the registry.
        let agent = state.registry.get(&target_agent_id).await.ok_or_else(|| {
            ToolError::ExecutionFailed(format!(
                "Worker agent {} is not registered — cannot re-run",
                target_agent_id
            ))
        })?;
        let config = agent.config().clone();

        // Persist the instruction as a new user turn on the worker workflow so
        // the replayed history (and any future analyze) sees it.
        save_message_core(
            &state.db,
            SaveMessageParams {
                workflow_id: worker_wf.clone(),
                role: "user".to_string(),
                content: instruction.clone(),
                tokens_input: None,
                tokens_output: None,
                model: None,
                provider: None,
                duration_ms: None,
                thinking_tokens: None,
                cost_usd: None,
                cached_tokens: None,
                cache_write_tokens: None,
                model_id_used: None,
                message_id: None,
                attachments: None,
            },
        )
        .await
        .map_err(ToolError::ExecutionFailed)?;

        // Build the replay context (includes the instruction just saved).
        let locale = self.load_locale(&worker_wf).await;
        let (mut history_context, _count) = load_conversation_history(&state, &worker_wf, &locale)
            .await
            .map_err(ToolError::ExecutionFailed)?;

        // Pre-allocate the assistant message id and thread it for tool-execution
        // correlation, mirroring the streaming flow.
        let assistant_message_id = uuid::Uuid::new_v4().to_string();
        if let Some(obj) = history_context.as_object_mut() {
            obj.insert("message_id".to_string(), json!(assistant_message_id));
        }

        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            description: instruction.clone(),
            context: history_context,
        };

        // Build the AgentToolContext so:
        //   * the tool loop's emit_progress fires `WORKFLOW_STREAM` chunks on
        //     `worker_wf` (live streaming when the UI views it),
        //   * sub-agent tools (Spawn/Delegate/Parallel) persist `parent_message_id`
        //     at CREATE time (otherwise orphans are silently dropped by
        //     load_workflow_blocks_core).
        // No cancellation token: the chat workflow's token is for the supervisor,
        // not the detached re-run (which must complete to refresh the card).
        let agent_ctx = AgentToolContext::from_app_state(
            app_state,
            Some(state.mcp_manager.clone()),
            Some(app_handle.clone()),
        )
        .with_current_message_id(assistant_message_id.clone());

        debug!(card_id = %card_id, worker_wf = %worker_wf, agent = %target_agent_id, "RerunWorkerTool executing worker re-run with live streaming");
        let report = execute_with_tools(
            ToolLoopContext {
                config: &config,
                provider_manager: &state.llm_manager,
                tool_factory: Some(&state.tool_factory),
                agent_context: Some(&agent_ctx),
            },
            task,
            Some(state.mcp_manager.clone()),
            None,
            vec![],
            // Conversational re-run: let the model finish naturally.
            ToolChoiceMode::Auto,
        )
        .await
        .map_err(|e| ToolError::ExecutionFailed(format!("Worker re-run failed: {}", e)))?;

        let succeeded = matches!(report.status, ReportStatus::Success);

        // Persist the produced report as the new assistant message (so
        // load_workflow_report returns the refreshed report).
        save_message_core(
            &state.db,
            SaveMessageParams {
                workflow_id: worker_wf.clone(),
                role: "assistant".to_string(),
                content: report.response.clone(),
                tokens_input: Some(report.metrics.tokens_input as u64),
                tokens_output: Some(report.metrics.tokens_output as u64),
                model: Some(config.llm.model.clone()),
                provider: Some(config.llm.provider.clone()),
                duration_ms: Some(report.metrics.duration_ms),
                thinking_tokens: report.metrics.thinking_tokens.map(|t| t as u64),
                cost_usd: None,
                cached_tokens: report.metrics.cached_tokens.map(|t| t as u64),
                cache_write_tokens: report.metrics.cache_write_tokens.map(|t| t as u64),
                model_id_used: None,
                message_id: Some(assistant_message_id.clone()),
                attachments: None,
            },
        )
        .await
        .map_err(ToolError::ExecutionFailed)?;

        // Persist every artifact the standard streaming path persists
        // (tool_executions + thinking_step + cumulative metrics + sub-agent
        // rollup), so the worker workflow's blocks reload identically to a
        // normal run. Without this the page agent would show an assistant
        // bubble but no reasoning blocks and stale token counters.
        persist_rerun_artifacts_core(
            app_state,
            &worker_wf,
            &assistant_message_id,
            &target_agent_id,
            &report,
        )
        .await;

        if !succeeded {
            warn!(card_id = %card_id, "Worker re-run finished with a failure report");
        }
        info!(card_id = %card_id, worker_wf = %worker_wf, succeeded, "Worker re-run finished");

        Ok(json!({
            "success": succeeded,
            "card_id": card_id,
            "workflow_id": worker_wf,
            "report": safe_truncate(&report.response, RERUN_REPORT_MAX_CHARS, true),
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        match input["instruction"].as_str() {
            Some(s) if !s.trim().is_empty() => Ok(()),
            _ => Err(ToolError::InvalidInput(
                "instruction is required and must be a non-empty string".to_string(),
            )),
        }
    }
}

/// Persists every artifact produced by the re-run, mirroring the standard
/// streaming path's `finalize_completion` (minus the live `response_block`
/// emission, which has already happened during the tool loop).
///
/// Performed in a single helper for two reasons:
///   * keeps the four side-effects atomic from the caller's perspective
///     (`execute` doesn't have to remember to chain them in the right order),
///   * exposes a `_core` testable in isolation: a fake `Report` exercises the
///     full DB-write fan-out without spinning up an LLM.
///
/// Order matches `persistence_step.rs`:
///   1. cumulative metrics (workflow row totals)
///   2. sub-agent metrics rollup
///   3. tool_execution rows
///   4. thinking_step rows
pub(crate) async fn persist_rerun_artifacts_core(
    state: &AppState,
    workflow_id: &str,
    message_id: &str,
    agent_id: &str,
    report: &Report,
) {
    let pricing = load_model_pricing_info(
        state,
        agent_id,
        report.metrics.tokens_input,
        report.metrics.tokens_output,
        report.metrics.cached_tokens,
        report.metrics.cache_write_tokens,
        report.metrics.provider_cost_usd,
    )
    .await;

    update_workflow_cumulative_metrics(
        state,
        &CumulativeMetricsUpdate {
            workflow_id,
            tokens_input: report.metrics.tokens_input,
            tokens_output: report.metrics.tokens_output,
            cached_tokens: report.metrics.cached_tokens,
            cache_write_tokens: report.metrics.cache_write_tokens,
            cost_usd: pricing.cost_usd,
            model_id: &pricing.model_id,
            context_tokens: report.metrics.context_tokens,
        },
    )
    .await;

    aggregate_sub_agent_metrics(state, workflow_id).await;

    crate::db::persist_tool_executions(
        &state.db,
        &report.metrics.tool_executions,
        workflow_id,
        message_id,
        agent_id,
    )
    .await;

    // start_step_number = 1: the re-run owns the message_id, so its thinking
    // steps live in a fresh numbering namespace scoped by message_id.
    crate::db::persist_reasoning_steps(
        &state.db,
        &report.metrics.reasoning_steps,
        workflow_id,
        message_id,
        agent_id,
        1,
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    #[tokio::test]
    async fn self_gates_without_workflow_context() {
        let (state, _g) = setup_test_state().await;
        let tool = RerunWorkerTool::new(state.db.clone(), None, None);
        let err = tool
            .execute(json!({"instruction": "do it"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn rejects_empty_instruction() {
        let (state, _g) = setup_test_state().await;
        let tool = RerunWorkerTool::new(
            state.db.clone(),
            Some(uuid::Uuid::new_v4().to_string()),
            None,
        );
        let err = tool
            .execute(json!({"instruction": "  "}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    /// Regression: previously `RerunWorkerTool` only persisted
    /// `tool_execution` rows, dropping `thinking_step` blocks on the floor.
    /// `load_workflow_blocks_core` would then return an assistant message
    /// with no reasoning blocks at all (and orphan sub-agents silently
    /// filtered out). This test exercises the artifact persistence in
    /// isolation: a synthetic `Report` carrying one tool execution + two
    /// reasoning steps must produce 1 + 2 rows respectively.
    #[tokio::test]
    async fn persists_thinking_steps_and_tool_executions() {
        use crate::agents::core::agent::{
            ReasoningSource, ReasoningStepData, Report, ReportMetrics, ReportStatus,
            ToolExecutionData,
        };
        use crate::test_utils::seed_test_workflow;

        let (state, _g) = setup_test_state().await;
        let workflow_id = seed_test_workflow(&state.db).await;
        let message_id = uuid::Uuid::new_v4().to_string();
        let agent_id = uuid::Uuid::new_v4().to_string();

        let report = Report {
            status: ReportStatus::Success,
            content: "report".to_string(),
            response: "response".to_string(),
            metrics: ReportMetrics {
                duration_ms: 1234,
                tokens_input: 100,
                tokens_output: 50,
                context_tokens: 100,
                cached_tokens: None,
                cache_write_tokens: None,
                thinking_tokens: None,
                provider_cost_usd: None,
                tools_used: vec!["FakeTool".to_string()],
                mcp_calls: vec![],
                tool_executions: vec![ToolExecutionData {
                    tool_type: "local".to_string(),
                    tool_name: "FakeTool".to_string(),
                    server_name: None,
                    input_params: json!({}),
                    output_result: json!({}),
                    success: true,
                    error_message: None,
                    duration_ms: 10,
                    iteration: 1,
                    sequence: 2,
                }],
                reasoning_steps: vec![
                    ReasoningStepData {
                        content: "first step".to_string(),
                        duration_ms: 5,
                        sequence: 1,
                        source: ReasoningSource::AgentFlow,
                    },
                    ReasoningStepData {
                        content: "second step".to_string(),
                        duration_ms: 7,
                        sequence: 3,
                        source: ReasoningSource::ModelThinking,
                    },
                ],
                iteration_metrics: vec![],
            },
        };

        persist_rerun_artifacts_core(&state, &workflow_id, &message_id, &agent_id, &report).await;

        let thinking_rows = state
            .db
            .query_json_with_params(
                "SELECT meta::id(id) AS id, content FROM thinking_step \
                 WHERE message_id = $mid",
                vec![("mid".to_string(), json!(message_id.clone()))],
            )
            .await
            .expect("thinking_step query failed");
        assert_eq!(
            thinking_rows.len(),
            2,
            "expected 2 thinking_step rows, got: {thinking_rows:?}"
        );

        let tool_rows = state
            .db
            .query_json_with_params(
                "SELECT meta::id(id) AS id, tool_name FROM tool_execution \
                 WHERE message_id = $mid",
                vec![("mid".to_string(), json!(message_id.clone()))],
            )
            .await
            .expect("tool_execution query failed");
        assert_eq!(
            tool_rows.len(),
            1,
            "expected 1 tool_execution row, got: {tool_rows:?}"
        );
    }

    /// Regression lock for c52622a: a re-run whose worker spawned sub-agents
    /// must roll their metrics up onto the worker workflow row (and bump the
    /// cumulative token totals) — the symptom of the original bug was orphan
    /// sub-agents and stale counters at reload because the detached re-run path
    /// never called `aggregate_sub_agent_metrics` / `update_workflow_cumulative_metrics`.
    ///
    /// Sub-agent executions are persisted at CREATE time by the Spawn/Delegate/
    /// Parallel tools (here seeded directly), so the rollup reads them from the
    /// DB rather than from the `Report`. We assert both the sub-agent rollup and
    /// the cumulative metrics derived from the synthetic `Report`.
    #[tokio::test]
    async fn persists_sub_agent_rollup_and_cumulative_metrics() {
        use crate::agents::core::agent::{Report, ReportMetrics, ReportStatus};
        use crate::test_utils::{seed_sub_agent_execution, seed_test_workflow};

        let (state, _g) = setup_test_state().await;
        let workflow_id = seed_test_workflow(&state.db).await;
        let message_id = uuid::Uuid::new_v4().to_string();
        let agent_id = uuid::Uuid::new_v4().to_string();

        // Two completed sub-agents on the worker workflow + one still running
        // (the latter must be excluded from the rollup).
        seed_sub_agent_execution(
            &state.db,
            &workflow_id,
            "sub-1",
            "completed",
            1_000,
            500,
            Some(0.05),
        )
        .await;
        seed_sub_agent_execution(
            &state.db,
            &workflow_id,
            "sub-2",
            "completed",
            2_000,
            800,
            Some(0.07),
        )
        .await;
        seed_sub_agent_execution(
            &state.db,
            &workflow_id,
            "sub-run",
            "running",
            9_999,
            9_999,
            Some(99.0),
        )
        .await;

        let report = Report {
            status: ReportStatus::Success,
            content: "report".to_string(),
            response: "response".to_string(),
            metrics: ReportMetrics {
                duration_ms: 1234,
                tokens_input: 100,
                tokens_output: 50,
                context_tokens: 100,
                cached_tokens: None,
                cache_write_tokens: None,
                thinking_tokens: None,
                provider_cost_usd: None,
                tools_used: vec![],
                mcp_calls: vec![],
                tool_executions: vec![],
                reasoning_steps: vec![],
                iteration_metrics: vec![],
            },
        };

        persist_rerun_artifacts_core(&state, &workflow_id, &message_id, &agent_id, &report).await;

        let rows = state
            .db
            .query_json(&format!(
                "SELECT \
                    (sub_agent_tokens_input ?? 0) AS sub_in, \
                    (sub_agent_tokens_output ?? 0) AS sub_out, \
                    (sub_agent_cost_usd ?? 0.0) AS sub_cost, \
                    (total_tokens_input ?? 0) AS total_in, \
                    (total_tokens_output ?? 0) AS total_out \
                 FROM workflow:`{}`",
                workflow_id
            ))
            .await
            .expect("workflow row query failed");
        let row = rows.into_iter().next().expect("workflow row missing");

        // Sub-agent rollup: only the two completed executions are summed.
        assert_eq!(
            row["sub_in"].as_i64(),
            Some(3_000),
            "completed sub-agent inputs summed"
        );
        assert_eq!(
            row["sub_out"].as_i64(),
            Some(1_300),
            "completed sub-agent outputs summed"
        );
        let sub_cost = row["sub_cost"].as_f64().unwrap_or(0.0);
        assert!(
            (sub_cost - 0.12).abs() < 0.000001,
            "completed sub-agent costs summed (0.05 + 0.07), got {sub_cost}"
        );

        // Cumulative metrics from the synthetic report (seed_test_workflow
        // leaves totals at NONE → coalesced to 0 + report values).
        assert_eq!(
            row["total_in"].as_i64(),
            Some(100),
            "cumulative input bumped from report"
        );
        assert_eq!(
            row["total_out"].as_i64(),
            Some(50),
            "cumulative output bumped from report"
        );
    }

    #[tokio::test]
    async fn errors_when_card_has_no_worker_workflow() {
        let (state, _g) = setup_test_state().await;
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        let agent = uuid::Uuid::new_v4().to_string();
        // Card linked to the chat but with workflow_id = NONE.
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent}', target_agent_id: '{agent}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'review', `column`: 'review',
                `column_order`: 0, workflow_id: NONE,
                review_chat_workflow_id: '{chat_wf}', error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();

        // No app handle is set in tests, but the worker-workflow check fires
        // before the app-handle resolution, so this exercises that guard.
        let tool = RerunWorkerTool::new(state.db.clone(), Some(chat_wf), None);
        let err = tool
            .execute(json!({"instruction": "redo"}))
            .await
            .unwrap_err();
        match err {
            ToolError::ExecutionFailed(msg) => {
                assert!(
                    msg.contains("no worker workflow"),
                    "expected worker-workflow guard, got: {msg}"
                );
            }
            other => panic!("expected ExecutionFailed, got {other:?}"),
        }
    }
}
