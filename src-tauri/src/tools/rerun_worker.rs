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
//! The re-run executes the worker agent **detached** (no streaming window,
//! `agent_context: None`) via `tool_loop::execute_with_tools`, replaying the
//! worker workflow's conversation history plus the new instruction. Because the
//! standard streaming flow persists worker messages from the frontend, this
//! detached path persists them itself: the instruction as a `user` message and
//! the produced report as an `assistant` message (so `load_workflow_report`
//! returns the refreshed report), plus the tool executions.
//!
//! Resolving `AppState` from the captured `AppHandle` is required because the
//! re-run needs the provider manager, tool factory and MCP manager (a tool
//! normally only carries `db`). The capture pattern mirrors TodoTool /
//! SpawnAgentTool; the `.state::<AppState>()` call mirrors main.rs:742.

use crate::agents::core::agent::{ReportStatus, Task};
use crate::agents::execution::tool_loop::{execute_with_tools, ToolLoopContext};
use crate::commands::kanban_card::{get_kanban_card_core, resolve_card_id_by_review_chat};
use crate::commands::message::{save_message_core, SaveMessageParams};
use crate::commands::streaming::helpers::load_conversation_history;
use crate::db::DBClient;
use crate::models::function_calling::ToolChoiceMode;
use crate::security::Validator;
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

        debug!(card_id = %card_id, worker_wf = %worker_wf, agent = %target_agent_id, "RerunWorkerTool executing detached worker run");
        let report = execute_with_tools(
            ToolLoopContext {
                config: &config,
                provider_manager: &state.llm_manager,
                tool_factory: Some(&state.tool_factory),
                agent_context: None,
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
        // load_workflow_report returns the refreshed report) + tool executions.
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

        crate::db::persist_tool_executions(
            &state.db,
            &report.metrics.tool_executions,
            &worker_wf,
            &assistant_message_id,
            &target_agent_id,
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
