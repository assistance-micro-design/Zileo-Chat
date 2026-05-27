// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! ScheduleCardTool — attaches a recurrence to the current Kanban card.
//!
//! Self-gating: the target card is resolved via
//! `review_chat_workflow_id = <chat workflow id>`. Wraps the existing
//! `create_kanban_schedule_core` (validation included). Attaching a schedule
//! turns the current card into a recurring TEMPLATE: the scheduler then spawns
//! fresh `todo/ready` clones at each occurrence, and the template card itself
//! is excluded from auto-promotion and the auto-purge of `done` cards. This is
//! the intended behaviour for a recurrence, and the tool description states it.

use crate::commands::kanban_card::resolve_card_id_by_review_chat;
use crate::commands::kanban_schedule::create_kanban_schedule_core;
use crate::db::DBClient;
use crate::models::KanbanScheduleCreate;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
    id: "ScheduleCardTool".to_string(),
    name: "ScheduleCard".to_string(),
    summary: "Attach a weekly recurrence to the current Kanban card (it becomes a template)"
        .to_string(),
    description: ToolDescriptionBuilder::new(
        "Attaches a weekly recurrence to the Kanban card backing the current review chat.",
    )
    .use_when(&[
        "The user wants this card's work to repeat on a weekly schedule",
        "You want the scheduler to spawn fresh copies of this card at set days/times",
    ])
    .do_not_use(&[
        "You are not in a card review chat (the tool cannot resolve a card and will error)",
        "You only want to rerun the worker once (use RerunWorkerTool)",
    ])
    .operations(&[(
        "schedule",
        "Create a recurrence with days_of_week (0=Mon..6=Sun), hour (0-23), minute (0-59)",
    )])
    .note(
        "IMPORTANT: attaching a schedule turns THIS card into a recurring template. \
         The scheduler spawns fresh todo/ready clones at each occurrence, and this \
         template card is no longer auto-archived from Done. The returned next_run_at \
         confirms the next occurrence.",
    )
    .examples(&[
        json!({"days_of_week": [0, 2, 4], "hour": 9, "minute": 0}),
        json!({"days_of_week": [6], "hour": 18, "minute": 30, "skip_if_pending": true}),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "days_of_week": {"type": "array", "items": {"type": "integer", "minimum": 0, "maximum": 6}, "description": "Days to fire on: 0=Mon .. 6=Sun. Non-empty."},
            "hour": {"type": "integer", "minimum": 0, "maximum": 23, "description": "Local wall-clock hour (0-23)."},
            "minute": {"type": "integer", "minimum": 0, "maximum": 59, "description": "Local wall-clock minute (0-59)."},
            "skip_if_pending": {"type": "boolean", "description": "Skip the occurrence if a previous instance is still pending (default false)."}
        },
        "required": ["days_of_week", "hour", "minute"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

/// Attaches a recurrence to the Kanban card backing the current review chat.
pub struct ScheduleCardTool {
    db: Arc<DBClient>,
    chat_workflow_id: Option<String>,
}

impl ScheduleCardTool {
    pub fn new(db: Arc<DBClient>, chat_workflow_id: Option<String>) -> Self {
        Self {
            db,
            chat_workflow_id,
        }
    }

    /// Reads a JSON integer in `[0, 255]` into a `u8`, rejecting out-of-range
    /// or non-integer values. SurrealDB ASSERTs ultimately bound the values,
    /// but failing here gives the model a clearer error.
    fn read_u8(value: &Value, field: &str) -> ToolResult<u8> {
        value
            .as_u64()
            .filter(|n| *n <= u8::MAX as u64)
            .map(|n| n as u8)
            .ok_or_else(|| ToolError::InvalidInput(format!("{} must be an integer 0-255", field)))
    }
}

#[async_trait]
impl Tool for ScheduleCardTool {
    fn id(&self) -> &str {
        "ScheduleCardTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let card_id = resolve_card_id_by_review_chat(&self.db, self.chat_workflow_id.as_deref())
            .await
            .map_err(ToolError::ExecutionFailed)?;

        let days_of_week: Vec<u8> = input["days_of_week"]
            .as_array()
            .ok_or_else(|| ToolError::InvalidInput("days_of_week must be an array".to_string()))?
            .iter()
            .map(|v| Self::read_u8(v, "days_of_week entry"))
            .collect::<ToolResult<Vec<u8>>>()?;
        let hour = Self::read_u8(&input["hour"], "hour")?;
        let minute = Self::read_u8(&input["minute"], "minute")?;
        let skip_if_pending = input["skip_if_pending"].as_bool().unwrap_or(false);

        debug!(card_id = %card_id, ?days_of_week, hour, minute, "ScheduleCardTool execute");
        // create_kanban_schedule_core runs validate_schedule_create (days
        // non-empty, hour 0-23, minute 0-59) before persisting.
        let schedule = create_kanban_schedule_core(
            &self.db,
            KanbanScheduleCreate {
                card_template_id: card_id.clone(),
                days_of_week,
                hour,
                minute,
                skip_if_pending,
            },
        )
        .await
        .map_err(ToolError::ExecutionFailed)?;

        info!(card_id = %card_id, schedule_id = %schedule.id, "Recurrence attached to card via chat");
        Ok(json!({
            "success": true,
            "schedule_id": schedule.id,
            "card_template_id": schedule.card_template_id,
            "next_run_at": schedule.next_run_at.to_rfc3339(),
            "note": "This card is now a recurring template; the scheduler will spawn fresh copies and it will no longer be auto-archived.",
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        // A recurrence with no day is meaningless; `validate_schedule_create`
        // only bounds the ranges, so reject the empty case here.
        match input["days_of_week"].as_array() {
            Some(days) if !days.is_empty() => {}
            _ => {
                return Err(ToolError::InvalidInput(
                    "days_of_week must be a non-empty array (0=Mon..6=Sun)".to_string(),
                ));
            }
        }
        if !input["hour"].is_number() || !input["minute"].is_number() {
            return Err(ToolError::InvalidInput(
                "hour and minute (integers) are required".to_string(),
            ));
        }
        Ok(())
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
    async fn self_gates_without_card() {
        let (state, _g) = setup_test_state().await;
        let tool = ScheduleCardTool::new(state.db.clone(), Some(uuid::Uuid::new_v4().to_string()));
        let err = tool
            .execute(json!({"days_of_week": [0], "hour": 9, "minute": 0}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn rejects_invalid_input() {
        let (state, _g) = setup_test_state().await;
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_review_card(&state.db, &card_id, &chat_wf).await;
        let tool = ScheduleCardTool::new(state.db.clone(), Some(chat_wf));

        // Empty days rejected by the tool's own input validation.
        let err = tool
            .execute(json!({"days_of_week": [], "hour": 9, "minute": 0}))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));

        // hour > 23 rejected by validate_schedule_create (via create core).
        let err2 = tool
            .execute(json!({"days_of_week": [0], "hour": 25, "minute": 0}))
            .await
            .unwrap_err();
        assert!(matches!(err2, ToolError::ExecutionFailed(_)));
    }

    #[tokio::test]
    async fn schedule_targets_current_card_and_makes_it_template() {
        let (state, _g) = setup_test_state().await;
        let chat_wf = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        seed_review_card(&state.db, &card_id, &chat_wf).await;
        let tool = ScheduleCardTool::new(state.db.clone(), Some(chat_wf));

        let res = tool
            .execute(json!({"days_of_week": [0, 2, 4], "hour": 9, "minute": 30}))
            .await
            .unwrap();
        assert_eq!(res["success"], true);
        assert_eq!(res["card_template_id"], card_id);
        assert!(res["next_run_at"].is_string());

        // The schedule must reference the current card.
        let rows = state
            .db
            .query_json(&format!(
                "SELECT card_template_id FROM kanban_schedule WHERE card_template_id = '{}'",
                card_id
            ))
            .await
            .unwrap();
        assert_eq!(rows.len(), 1, "schedule must target the current card");
    }
}
