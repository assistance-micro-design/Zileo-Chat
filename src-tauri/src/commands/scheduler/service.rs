// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban scheduler — a background tokio task that ticks every 60s and:
//!
//! 1. Processes due `kanban_schedule` rows by spawning a fresh `kanban_card`
//!    in the `todo` column / `ready` status, then re-arms `next_run_at`.
//! 2. Promotes pending cards (column=todo, status=ready) to the `doing`
//!    column and emits a `kanban:card_ready` event so the frontend can pick
//!    them up and start the workflow.
//!
//! Card lifecycle service: workflow linkage and completion.
use crate::db::DBClient;
use serde_json::json;
use std::sync::Arc;

/// Convenience helper used by the workflow-complete listener: when a workflow
/// finishes (success or failure), the card linked to it transitions to the
/// `review` column with the matching status, so the user can verify the
/// report or read the error summary.
/// Look up the kanban_card.id linked to a given workflow_id. Returns
/// `Ok(None)` when the workflow was not spawned by a Kanban card (i.e.
/// no card has its workflow_id set to this value).
pub async fn card_id_for_workflow(
    db: &Arc<DBClient>,
    workflow_id: &str,
) -> Result<Option<String>, String> {
    let q = "SELECT meta::id(id) AS id FROM kanban_card WHERE workflow_id = $wid LIMIT 1";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(q, vec![("wid".to_string(), json!(workflow_id))])
        .await
        .map_err(|e| format!("Failed to look up kanban_card by workflow_id: {}", e))?;
    Ok(rows
        .into_iter()
        .next()
        .and_then(|r| r["id"].as_str().map(String::from)))
}

pub async fn mark_card_done_core(
    db: &Arc<DBClient>,
    workflow_id: &str,
    success: bool,
    error_summary: Option<&str>,
) -> Result<(), String> {
    let status = if success { "done" } else { "failed" };
    let err_sql = match error_summary {
        Some(s) if !s.is_empty() => {
            let json = serde_json::to_string(&s)
                .map_err(|e| format!("Failed to serialize error_summary: {}", e))?;
            format!("error_summary = {}", json)
        }
        _ => "error_summary = NONE".to_string(),
    };
    let q = format!(
        "UPDATE kanban_card SET status = '{}', `column` = 'review', {}, \
         updated_at = time::now() WHERE workflow_id = $wid",
        status, err_sql
    );
    db.execute_with_params(&q, vec![("wid".to_string(), json!(workflow_id))])
        .await
        .map_err(|e| format!("Failed to mark card done: {}", e))?;
    Ok(())
}
