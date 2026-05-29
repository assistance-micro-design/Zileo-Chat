// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! MoveCardTool — lets the Kanban agent move the card it is chatting about.
//!
//! Self-gating: the target card is resolved via
//! `review_chat_workflow_id = <chat workflow id>` (captured at construction).
//! Outside a card review chat the tool returns a clear error (no-op). Only the
//! two user-meaningful transitions are exposed:
//! - `validate`  -> column `done` (status flips to `done` via move_kanban_card_core)
//! - `send_back` -> column `todo` (re-queue): status resets to `ready` and the
//!   scheduler re-promotes the card to `doing` via the existing promotion path
//!   (option b, 2026-05-29). `doing` is never a manual target.
//!
//! Both wrap `move_kanban_card_core`, so the transition guard
//! (`is_transition_allowed`) and the status reset stay authoritative.

use crate::commands::kanban_card::{move_kanban_card_core, resolve_card_id_by_review_chat};
use crate::db::DBClient;
use crate::models::KanbanColumn;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "MoveCardTool".to_string(),
    name: "MoveCard".to_string(),
    summary: "Move the current Kanban card: validate it to Done, or send it back to the queue"
        .to_string(),
    description: ToolDescriptionBuilder::new(
        "Moves the Kanban card backing the current review chat to Done (validate) or back to \
         the Todo queue (send_back) for a fresh run.",
    )
    .use_when(&[
        "The report is good and you want to validate the card (move it to Done)",
        "The report needs rework and you want to re-queue the card so it runs again from scratch",
    ])
    .do_not_use(&[
        "You are not in a card review chat (the tool cannot resolve a card and will error)",
        "You only want to re-generate the report without re-queuing the card (use RerunWorkerTool)",
    ])
    .operations(&[
        ("validate", "Move the card to Done (status becomes done)"),
        (
            "send_back",
            "Re-queue the card to Todo (status becomes ready); the scheduler re-promotes it to Doing automatically",
        ),
    ])
    .examples(&[
        json!({"action": "validate"}),
        json!({"action": "send_back"}),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "action": {"type": "string", "enum": ["validate", "send_back"], "description": "validate -> Done; send_back -> re-queue to Todo for a fresh run."}
        },
        "required": ["action"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
}
});

/// Moves the Kanban card backing the current review chat.
pub struct MoveCardTool {
    db: Arc<DBClient>,
    /// The chat workflow id this tool instance is scoped to. `None` when the
    /// tool was created outside a workflow context (self-gates to an error).
    chat_workflow_id: Option<String>,
}

impl MoveCardTool {
    pub fn new(db: Arc<DBClient>, chat_workflow_id: Option<String>) -> Self {
        Self {
            db,
            chat_workflow_id,
        }
    }

    /// Resolves the card whose review chat is this workflow. Errors clearly
    /// when the tool runs outside a card review chat.
    async fn resolve_card_id(&self) -> ToolResult<String> {
        resolve_card_id_by_review_chat(&self.db, self.chat_workflow_id.as_deref())
            .await
            .map_err(ToolError::ExecutionFailed)
    }
}

#[async_trait]
impl Tool for MoveCardTool {
    fn id(&self) -> &str {
        "MoveCardTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let action = input["action"].as_str().unwrap_or("");
        let card_id = self.resolve_card_id().await?;

        let target = match action {
            "validate" => KanbanColumn::Done,
            // send_back always re-queues to Todo (option b): status resets to
            // 'ready' in move_kanban_card_core and the scheduler re-promotes the
            // card to Doing. Doing is never a manual target.
            "send_back" => KanbanColumn::Todo,
            other => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown action '{}'. Use 'validate' or 'send_back'.",
                    other
                )));
            }
        };

        debug!(card_id = %card_id, action = %action, column = ?target, "MoveCardTool execute");
        let card = move_kanban_card_core(&self.db, &card_id, target, 0)
            .await
            .map_err(ToolError::ExecutionFailed)?;
        info!(card_id = %card.id, column = ?card.column, "Kanban card moved via chat");
        Ok(json!({
            "success": true,
            "card_id": card.id,
            "column": card.column.as_str(),
            "status": format!("{:?}", card.status).to_lowercase(),
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        match input["action"].as_str() {
            Some("validate") | Some("send_back") => Ok(()),
            _ => Err(ToolError::InvalidInput(
                "action is required and must be 'validate' or 'send_back'".to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    async fn seed_review_card(db: &Arc<DBClient>, card_id: &str, chat_wf: &str) {
        let agent = uuid::Uuid::new_v4().to_string();
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
        db.execute(&q).await.unwrap();
    }

    #[tokio::test]
    async fn self_gates_without_workflow_context() {
        let (state, _g) = setup_test_state().await;
        let tool = MoveCardTool::new(state.db.clone(), None);
        let err = tool
            .execute(json!({"action": "validate"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn self_gates_when_no_card_linked() {
        let (state, _g) = setup_test_state().await;
        let tool = MoveCardTool::new(state.db.clone(), Some(uuid::Uuid::new_v4().to_string()));
        let err = tool
            .execute(json!({"action": "validate"}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn validate_moves_card_to_done() {
        let (state, _g) = setup_test_state().await;
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_review_card(&state.db, &card_id, &chat_wf).await;

        let tool = MoveCardTool::new(state.db.clone(), Some(chat_wf));
        let res = tool.execute(json!({"action": "validate"})).await.unwrap();
        assert_eq!(res["success"], true);
        assert_eq!(res["column"], "done");
        assert_eq!(res["status"], "done");
    }

    #[tokio::test]
    async fn send_back_requeues_card_to_todo_as_ready() {
        let (state, _g) = setup_test_state().await;
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        // seed_review_card seeds status='review'; bump to 'done' to mirror a
        // real review card (mark_card_done_core) so the reset is meaningful.
        seed_review_card(&state.db, &card_id, &chat_wf).await;
        state
            .db
            .execute(&format!(
                "UPDATE kanban_card:`{card_id}` SET status = 'done'"
            ))
            .await
            .unwrap();

        let tool = MoveCardTool::new(state.db.clone(), Some(chat_wf));
        // send_back re-queues to Todo (option b): no target_column accepted.
        let res = tool.execute(json!({"action": "send_back"})).await.unwrap();
        assert_eq!(res["column"], "todo");
        // status must reset to 'ready' so the scheduler can re-promote it.
        assert_eq!(res["status"], "ready");
    }
}
