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
//! Promotion budget, atomic claim, selection and card promotion.
use crate::constants::workflow::DEFAULT_MAX_CONCURRENT_WORKFLOWS;
use crate::db::DBClient;
use serde_json::json;
use std::sync::Arc;
use tauri::{AppHandle, Emitter};
use tracing::{debug, warn};

/// Tauri command exposing the backend's worker-promotion concurrency budget
/// ([`DEFAULT_MAX_CONCURRENT_WORKFLOWS`]) to the frontend.
///
/// The Kanban board reads this so its "X / N actifs" badge — and any frontend
/// slot accounting — reflects the SAME cap the scheduler uses to promote
/// `todo→doing`, instead of duplicating the literal. Synchronous and
/// dependency-free: it returns a compile-time constant, so it never fails.
#[tauri::command]
pub fn get_max_concurrent_workflows() -> usize {
    DEFAULT_MAX_CONCURRENT_WORKFLOWS
}

/// Atomically claims a pending card for execution by flipping it to `doing`
/// ONLY if it is still `status='ready'`. Returns `true` when this caller won
/// the claim, `false` when a concurrent promoter already flipped it.
///
/// `start_next_pending_card_core` is reached from three concurrent sites (the
/// scheduler tick, the `workflow_complete` listener, `create_kanban_card`), and
/// the SELECT→UPDATE was previously non-atomic with an UNCONDITIONAL flip, so
/// two promoters could each emit `card_ready` for the same card → two workflows
/// for one card. The `WHERE status='ready'` guard makes the flip the single
/// atomic gate: only the first UPDATE matches; the rest return zero rows and
/// MUST NOT emit `card_ready`.
pub(crate) async fn try_claim_pending_card_core(
    db: &Arc<DBClient>,
    card_id: &str,
) -> Result<bool, String> {
    // card_id comes from `meta::id(id)` of a prior SELECT (trusted clean UUID),
    // so format! is safe here (security rules: validated record ids).
    let q = format!(
        "UPDATE kanban_card:`{}` SET status = 'doing', `column` = 'doing', \
         updated_at = time::now() WHERE status = 'ready' RETURN meta::id(id) AS id",
        card_id
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to claim card for promotion: {}", e))?;
    Ok(!rows.is_empty())
}
/// emit is impossible to mock from a unit test). Returns the ready `todo` cards
/// (`id`, `title`, `target_agent_id`, …) the caller should try to claim, capped
/// at the number of free slots.
pub(crate) async fn select_cards_to_promote_core(
    db: &Arc<DBClient>,
) -> Result<Vec<serde_json::Value>, String> {
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
        return Ok(Vec::new());
    }

    // 2. Pull the next N ready cards in `todo` column ordered by column_order.
    //
    // `column_order` and `created_at` must be in the SELECT projection or
    // SurrealDB 2.6 rejects the query with "Missing order idiom" (the parser
    // resolves ORDER BY against the projected idioms, not the table schema).
    // Exclude cards that are templates for an enabled schedule: those cards
    // are the user's blueprint and must NOT auto-execute. Only the clones
    // spawned by `spawn_card_from_template` (fresh UUIDs) should be picked.
    let pick_q = format!(
        "SELECT meta::id(id) AS id, title, target_agent_id, \
         `column_order`, created_at \
         FROM kanban_card \
         WHERE status = 'ready' AND `column` = 'todo' \
           AND meta::id(id) NOT IN (SELECT VALUE card_template_id FROM kanban_schedule WHERE enabled = true) \
         ORDER BY `column_order` ASC, created_at ASC LIMIT {}",
        free
    );
    db.query_json(&pick_q)
        .await
        .map_err(|e| format!("Failed to pick ready cards: {}", e))
}
/// next `ready` cards from `todo` until the slot budget is used up. Emits
/// `kanban:card_ready` per promoted card.
pub async fn start_next_pending_card_core(
    db: &Arc<DBClient>,
    app_handle: &AppHandle,
) -> Result<usize, String> {
    let cards = select_cards_to_promote_core(db).await?;

    let mut promoted = 0usize;
    for card in cards {
        let card_id = card["id"].as_str().unwrap_or("").to_string();
        if card_id.is_empty() {
            continue;
        }
        // 3. Atomically claim the card (flip to doing only if still `ready`).
        //    workflow_id stays NONE — the frontend sets it once
        //    execute_workflow_streaming returns the wf id. If another promoter
        //    already claimed it, skip WITHOUT emitting card_ready (avoids a
        //    duplicate workflow — K2 double-promotion race).
        match try_claim_pending_card_core(db, &card_id).await {
            Ok(true) => {}
            Ok(false) => {
                debug!(card_id = %card_id, "Card already claimed by another promoter, skipping");
                continue;
            }
            Err(e) => {
                warn!(card_id = %card_id, error = %e, "Failed to claim card for promotion");
                continue;
            }
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
