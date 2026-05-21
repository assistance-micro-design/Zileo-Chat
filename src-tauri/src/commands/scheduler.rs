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
//! Slot accounting uses the count of in-flight cards (status=doing) and the
//! constant `DEFAULT_MAX_CONCURRENT_WORKFLOWS`. The frontend is expected to
//! be open so it can consume the event; this is a deliberate trade-off
//! documented in the spec.

use crate::commands::kanban_schedule::compute_next_run_at;
use crate::constants::workflow::DEFAULT_MAX_CONCURRENT_WORKFLOWS;
use crate::db::DBClient;
use chrono::Utc;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Tick period for the scheduler loop.
pub const SCHEDULER_TICK_SECS: u64 = 60;

/// Spawns the kanban scheduler task. The returned handle is parked in
/// [`crate::state::AppState`] so the runtime owns it and shutdown can
/// `abort()` it deterministically.
///
/// `shutdown` is polled on every tick before any work; the loop exits cleanly
/// when it flips to `true`. We use `Acquire` so writes from the shutdown side
/// are visible without a heavier `SeqCst` ordering.
pub fn spawn_kanban_scheduler_task(
    db: Arc<DBClient>,
    app_handle: AppHandle,
    shutdown: Arc<AtomicBool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(SCHEDULER_TICK_SECS));
        // The first .tick() returns immediately; that's fine — it gives us
        // a chance to honour any cards already due at startup.
        loop {
            tick.tick().await;
            if shutdown.load(Ordering::Acquire) {
                info!("Kanban scheduler: shutdown requested, exiting loop");
                break;
            }
            match process_due_schedules_core(&db).await {
                Ok(n) if n > 0 => info!(spawned = n, "Kanban scheduler: spawned due cards"),
                Ok(_) => debug!("Kanban scheduler: no schedules due"),
                Err(e) => warn!(error = %e, "Kanban scheduler: schedules error"),
            }
            match start_next_pending_card_core(&db, &app_handle).await {
                Ok(n) if n > 0 => info!(started = n, "Kanban scheduler: promoted cards"),
                Ok(_) => debug!("Kanban scheduler: no slots / no cards to promote"),
                Err(e) => warn!(error = %e, "Kanban scheduler: queue error"),
            }
        }
    })
}

/// Reads every enabled schedule whose `next_run_at` is in the past and
/// materialises a new card per template. Returns the number of cards created.
///
/// A schedule has a `card_template_id` that points to an existing
/// `kanban_card` whose fields are cloned as the seed for the new card. The
/// new card is created in `column=todo` / `status=ready` so it joins the
/// promotion queue immediately.
pub async fn process_due_schedules_core(db: &Arc<DBClient>) -> Result<usize, String> {
    let now = Utc::now();
    let now_str = now.to_rfc3339();
    let q = format!(
        "SELECT meta::id(id) AS id, card_template_id, days_of_week, hour, minute \
         FROM kanban_schedule WHERE enabled = true AND next_run_at <= <datetime> '{}'",
        now_str
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to load due schedules: {}", e))?;

    let mut spawned = 0usize;
    for row in rows {
        let schedule_id = row["id"].as_str().unwrap_or("").to_string();
        let template_id = row["card_template_id"].as_str().unwrap_or("").to_string();
        let days: Vec<u8> = row["days_of_week"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_u64().map(|x| x as u8))
                    .collect()
            })
            .unwrap_or_default();
        let hour = row["hour"].as_u64().unwrap_or(0) as u8;
        let minute = row["minute"].as_u64().unwrap_or(0) as u8;

        if schedule_id.is_empty() || template_id.is_empty() {
            continue;
        }

        // Clone the template card into a fresh `ready` card.
        if let Err(e) = spawn_card_from_template(db, &template_id).await {
            warn!(
                schedule_id = %schedule_id,
                template_id = %template_id,
                error = %e,
                "Failed to spawn card from template; skipping this tick"
            );
            // Don't rearm — let it retry next minute. (Avoids losing tasks
            // silently when the template was deleted.)
            continue;
        }
        spawned += 1;

        // Re-arm next_run_at + stamp last_run_at.
        let next = compute_next_run_at(&days, hour, minute, now).to_rfc3339();
        let upd = format!(
            "UPDATE kanban_schedule:`{}` SET next_run_at = <datetime> '{}', \
             last_run_at = <datetime> '{}'",
            schedule_id, next, now_str
        );
        if let Err(e) = db.execute(&upd).await {
            warn!(schedule_id = %schedule_id, error = %e, "Failed to re-arm schedule");
        }
    }
    Ok(spawned)
}

/// Reads the template card, then inserts a fresh card with column=todo and
/// status=ready (so it skips the manual "ready" gesture).
async fn spawn_card_from_template(db: &Arc<DBClient>, template_id: &str) -> Result<String, String> {
    let sel = format!(
        "SELECT title, description, kanban_agent_id, target_agent_id, prompt_id, \
         inline_prompt, variables, target_folder_id FROM kanban_card:`{}`",
        template_id
    );
    let rows = db
        .query_json(&sel)
        .await
        .map_err(|e| format!("Failed to load template card: {}", e))?;
    let tmpl = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Template card not found: {}", template_id))?;

    let id = uuid::Uuid::new_v4().to_string();
    let prompt_id_sql = tmpl["prompt_id"]
        .as_str()
        .map(|s| format!("'{}'", s))
        .unwrap_or_else(|| "NONE".to_string());
    let inline_prompt_sql = if tmpl["inline_prompt"].as_str().is_some() {
        "$inline".to_string()
    } else {
        "NONE".to_string()
    };
    let folder_sql = tmpl["target_folder_id"]
        .as_str()
        .map(|s| format!("'{}'", s))
        .unwrap_or_else(|| "NONE".to_string());

    let q = format!(
        "CREATE kanban_card:`{id}` CONTENT {{
            id: '{id}',
            title: $title,
            description: $description,
            kanban_agent_id: $kanban,
            target_agent_id: $target,
            prompt_id: {prompt_id_sql},
            inline_prompt: {inline_prompt_sql},
            variables: $vars,
            target_folder_id: {folder_sql},
            status: 'ready',
            `column`: 'todo',
            `column_order`: 0,
            workflow_id: NONE,
            error_summary: NONE,
            created_at: time::now(),
            updated_at: time::now()
        }}"
    );
    let mut params: Vec<(String, serde_json::Value)> = vec![
        ("title".to_string(), tmpl["title"].clone()),
        ("description".to_string(), tmpl["description"].clone()),
        ("kanban".to_string(), tmpl["kanban_agent_id"].clone()),
        ("target".to_string(), tmpl["target_agent_id"].clone()),
        ("vars".to_string(), tmpl["variables"].clone()),
    ];
    if let Some(s) = tmpl["inline_prompt"].as_str() {
        params.push(("inline".to_string(), json!(s)));
    }
    db.execute_with_params(&q, params)
        .await
        .map_err(|e| format!("Failed to create scheduled card: {}", e))?;
    Ok(id)
}

/// Counts how many cards are currently `doing` (in-flight) and promotes the
/// next `ready` cards from `todo` until the slot budget is used up. Emits
/// `kanban:card_ready` per promoted card.
pub async fn start_next_pending_card_core(
    db: &Arc<DBClient>,
    app_handle: &AppHandle,
) -> Result<usize, String> {
    // 1. Slot budget.
    let in_flight_q = "SELECT count() AS c FROM kanban_card WHERE status = 'doing' GROUP ALL";
    let rows = db
        .query_json(in_flight_q)
        .await
        .map_err(|e| format!("Failed to count in-flight cards: {}", e))?;
    let in_flight = rows
        .into_iter()
        .next()
        .and_then(|r| r["c"].as_u64())
        .unwrap_or(0) as usize;
    let free = DEFAULT_MAX_CONCURRENT_WORKFLOWS.saturating_sub(in_flight);
    if free == 0 {
        return Ok(0);
    }

    // 2. Pull the next N ready cards in `todo` column ordered by column_order.
    //
    // `column_order` and `created_at` must be in the SELECT projection or
    // SurrealDB 2.6 rejects the query with "Missing order idiom" (the parser
    // resolves ORDER BY against the projected idioms, not the table schema).
    let pick_q = format!(
        "SELECT meta::id(id) AS id, title, target_agent_id, \
         `column_order`, created_at \
         FROM kanban_card \
         WHERE status = 'ready' AND `column` = 'todo' \
         ORDER BY `column_order` ASC, created_at ASC LIMIT {}",
        free
    );
    let cards = db
        .query_json(&pick_q)
        .await
        .map_err(|e| format!("Failed to pick ready cards: {}", e))?;

    let mut promoted = 0usize;
    for card in cards {
        let card_id = card["id"].as_str().unwrap_or("").to_string();
        if card_id.is_empty() {
            continue;
        }
        // 3. Flip to `doing` (status + column). workflow_id stays NONE — the
        //    frontend sets it once execute_workflow_streaming returns the wf id.
        let upd = format!(
            "UPDATE kanban_card:`{}` SET status = 'doing', `column` = 'doing', \
             updated_at = time::now()",
            card_id
        );
        if let Err(e) = db.execute(&upd).await {
            warn!(card_id = %card_id, error = %e, "Failed to promote card");
            continue;
        }
        // 4. Notify frontend.
        let _ = app_handle.emit(
            "kanban:card_ready",
            json!({
                "card_id": card_id,
                "title": card["title"].clone(),
                "target_agent_id": card["target_agent_id"].clone(),
            }),
        );
        promoted += 1;
    }
    Ok(promoted)
}

/// Convenience helper used by the workflow-complete listener: when a workflow
/// finishes (success or failure), the card linked to it transitions to the
/// `review` column with the matching status, so the user can verify the
/// report or read the error summary.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    // start_next_pending_card_core is exercised via the spawn loop; here we
    // only cover mark_card_done_core which has no AppHandle dependency.
    // The promotion-slot logic relies on a real Tauri AppHandle for the
    // event emit, which cannot be mocked from a unit test.

    #[tokio::test]
    async fn test_mark_card_done_sets_review_column() {
        let (state, _g) = setup_test_state().await;
        let card_id = uuid::Uuid::new_v4().to_string();
        let wid = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{id}` CONTENT {{
                id: '{id}', title: 'x', description: '',
                kanban_agent_id: '{a}', target_agent_id: '{b}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'doing', `column`: 'doing',
                `column_order`: 0, workflow_id: '{wid}', error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}",
            id = card_id,
            a = uuid::Uuid::new_v4(),
            b = uuid::Uuid::new_v4(),
            wid = wid
        );
        state.db.execute(&q).await.unwrap();

        mark_card_done_core(&state.db, &wid, true, None)
            .await
            .unwrap();
        let after = state
            .db
            .query_json(&format!(
                "SELECT status, `column`, error_summary FROM kanban_card:`{}`",
                card_id
            ))
            .await
            .unwrap();
        assert_eq!(after[0]["status"], "done");
        assert_eq!(after[0]["column"], "review");

        // Failure path stamps error_summary.
        let card2 = uuid::Uuid::new_v4().to_string();
        let wid2 = uuid::Uuid::new_v4().to_string();
        let q2 = format!(
            "CREATE kanban_card:`{id}` CONTENT {{
                id: '{id}', title: 'x', description: '',
                kanban_agent_id: '{a}', target_agent_id: '{b}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'doing', `column`: 'doing',
                `column_order`: 0, workflow_id: '{wid}', error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}",
            id = card2,
            a = uuid::Uuid::new_v4(),
            b = uuid::Uuid::new_v4(),
            wid = wid2
        );
        state.db.execute(&q2).await.unwrap();
        mark_card_done_core(&state.db, &wid2, false, Some("timeout"))
            .await
            .unwrap();
        let after2 = state
            .db
            .query_json(&format!(
                "SELECT status, error_summary FROM kanban_card:`{}`",
                card2
            ))
            .await
            .unwrap();
        assert_eq!(after2[0]["status"], "failed");
        assert_eq!(after2[0]["error_summary"], "timeout");
    }
}
