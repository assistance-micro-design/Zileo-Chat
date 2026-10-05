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
//! Orphan / stuck / stale card recovery and cleanup.
use super::service::mark_card_done_core;
use super::DONE_CARD_TTL_DAYS;
use crate::db::DBClient;
use serde_json::json;
use std::sync::Arc;

/// Safety net for orphaned `doing` cards (K1): a card promoted to `doing`
/// whose `kanban:card_ready` event was lost (no /kanban page mounted to consume
/// it) stays `doing` with `workflow_id = NONE` forever and, since slots are
/// counted by `status='doing'`, permanently consumes the budget — eventually
/// deadlocking promotion entirely.
///
/// After `grace_secs` (measured via `updated_at`, stamped at promotion), such
/// cards are reset to their PRE-promotion state `status='ready', column='todo'`
/// so the slot frees and the scheduler re-promotes them. Resetting `status`
/// alone would NOT re-promote — the promotion query requires
/// `status='ready' AND column='todo'` — hence both fields.
///
/// Returns the reclaimed card ids. The UPDATE re-evaluates the same predicate
/// so a card that received its `workflow_id` between the SELECT and the UPDATE
/// is spared (ERR_SURREAL race-safety, mirrors the purge).
pub async fn reclaim_orphaned_doing_cards_core(
    db: &Arc<DBClient>,
    grace_secs: i64,
) -> Result<Vec<String>, String> {
    let pick_q = format!(
        "SELECT meta::id(id) AS id FROM kanban_card \
         WHERE `column` = 'doing' AND workflow_id IS NONE \
           AND updated_at < time::now() - {}s",
        grace_secs
    );
    let rows = db
        .query_json(&pick_q)
        .await
        .map_err(|e| format!("Failed to pick orphaned doing cards: {}", e))?;
    let ids: Vec<String> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(String::from))
        .collect();
    if ids.is_empty() {
        return Ok(ids);
    }

    let ids_json = serde_json::to_string(&ids)
        .map_err(|e| format!("Failed to serialize orphan ids: {}", e))?;
    let upd = format!(
        "UPDATE kanban_card SET status = 'ready', `column` = 'todo', updated_at = time::now() \
         WHERE meta::id(id) IN {} AND `column` = 'doing' AND workflow_id IS NONE",
        ids_json
    );
    db.execute(&upd)
        .await
        .map_err(|e| format!("Failed to reclaim orphaned doing cards: {}", e))?;
    Ok(ids)
}

/// Outcome of [`recover_stuck_doing_cards_core`], split by the action taken.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct StuckDoingRecovery {
    /// Cards whose workflow had produced a final assistant message (the run
    /// completed but the card missed its `review` transition) — moved to
    /// `review` via [`mark_card_done_core`].
    pub finalized: Vec<String>,
    /// Cards whose workflow never produced a final message (never started, or
    /// died mid-run) — reset to `ready`/`todo` for a fresh promotion.
    pub requeued: Vec<String>,
}

impl StuckDoingRecovery {
    /// True when nothing was recovered (both buckets empty).
    pub fn is_empty(&self) -> bool {
        self.finalized.is_empty() && self.requeued.is_empty()
    }
}

/// Recovers `doing` cards whose linked workflow is no longer alive (K1-bis).
///
/// Intended for boot-time use: at boot no frontend run is live yet, so every
/// `doing` card carrying a `workflow_id` older than `grace_secs` is a leftover
/// from a previous session that neither orphan reconciler can reach
/// (`runCardWorkflow` short-circuits once `workflow_id` is set; both reconcilers
/// target only `workflow_id = NONE`). Left untouched, such a card holds a
/// `doing` slot forever (slots are counted by `status='doing'`).
///
/// A persisted ASSISTANT message is the reliable run-completed signal:
/// `WorkflowExecutorService` saves the assistant message only AFTER the
/// streaming call returns. (The `workflow.status` field is unusable here — the
/// streaming path never transitions it out of `idle`.) So per card:
///   - **has** an assistant message → the run finished but the card missed its
///     `review` transition (e.g. shutdown raced the `workflow_complete`
///     listener) → replay [`mark_card_done_core`] to move it to `review`
///     (the boot auto-analyze catch-up then picks it up). Re-running it would
///     waste a completed (paid) LLM run.
///   - **no** assistant message → the run never completed (never started, or
///     crashed/was-killed mid-run) → reset to `ready`/`todo` so the scheduler
///     re-runs it from scratch (a partial dead run leaves no usable result).
///
/// Safety (never touches a live run): a run started by the CURRENT boot has a
/// fresh `updated_at` (stamped by `set_kanban_card_workflow_id`) and is excluded
/// by `grace_secs`. The requeue UPDATE re-checks `column='doing'` for
/// race-safety, mirroring [`reclaim_orphaned_doing_cards_core`].
pub async fn recover_stuck_doing_cards_core(
    db: &Arc<DBClient>,
    grace_secs: i64,
) -> Result<StuckDoingRecovery, String> {
    // 1. Candidate `doing` cards that DO carry a workflow_id, past the grace
    //    window. (Cards with NO workflow_id are the orphan reclaimer's job.)
    let pick_q = format!(
        "SELECT meta::id(id) AS id, workflow_id FROM kanban_card \
         WHERE `column` = 'doing' AND workflow_id IS NOT NONE \
           AND updated_at < time::now() - {}s",
        grace_secs
    );
    let rows = db
        .query_json(&pick_q)
        .await
        .map_err(|e| format!("Failed to pick stuck doing cards: {}", e))?;

    // 2. Partition by the run-completed signal (a persisted assistant message).
    let mut recovery = StuckDoingRecovery::default();
    let mut requeue: Vec<String> = Vec::new();
    for row in &rows {
        let Some(card_id) = row["id"].as_str() else {
            continue;
        };
        let Some(wf_id) = row["workflow_id"].as_str() else {
            continue;
        };
        let count_rows: Vec<serde_json::Value> = db
            .query_with_params(
                "SELECT count() AS c FROM message \
                 WHERE workflow_id = $wid AND role = 'assistant' GROUP ALL",
                vec![("wid".to_string(), json!(wf_id))],
            )
            .await
            .map_err(|e| format!("Failed to count assistant messages for workflow: {}", e))?;
        let has_assistant = count_rows
            .into_iter()
            .next()
            .and_then(|r| r["c"].as_u64())
            .unwrap_or(0)
            > 0;
        if has_assistant {
            // Finished run that missed its review transition → finalize it.
            mark_card_done_core(db, wf_id, true, None).await?;
            recovery.finalized.push(card_id.to_string());
        } else {
            requeue.push(card_id.to_string());
        }
    }

    // 3. Reset the never-completed cards to their pre-promotion state. Re-check
    //    `column='doing'` so a card that started executing between the SELECT
    //    and now is spared.
    if !requeue.is_empty() {
        let ids_json = serde_json::to_string(&requeue)
            .map_err(|e| format!("Failed to serialize requeue ids: {}", e))?;
        let upd = format!(
            "UPDATE kanban_card SET status = 'ready', `column` = 'todo', \
             workflow_id = NONE, updated_at = time::now() \
             WHERE meta::id(id) IN {} AND `column` = 'doing'",
            ids_json
        );
        db.execute(&upd)
            .await
            .map_err(|e| format!("Failed to reset stuck doing cards: {}", e))?;
        recovery.requeued = requeue;
    }
    Ok(recovery)
}

/// Deletes cards stuck in `done` for more than `DONE_CARD_TTL_DAYS` days,
/// EXCEPT cards that are templates of an enabled recurrence schedule —
/// those are the user's blueprint. Linked `kanban_card_interaction` rows
/// are cascaded. `workflow` rows are intentionally preserved (workflows
/// remain consultable independently of their originating card).
///
/// Returns the list of purged card ids so callers can emit a UI refresh
/// event with the exact set of removed ids.
pub async fn purge_stale_done_cards_core(db: &Arc<DBClient>) -> Result<Vec<String>, String> {
    // 1. Resolve the victim set first. We need the ids both to cascade
    //    interactions and to ship them to the frontend listener.
    let pick_q = format!(
        "SELECT meta::id(id) AS id, review_chat_workflow_id FROM kanban_card \
         WHERE `column` = 'done' \
           AND updated_at < time::now() - {}d \
           AND meta::id(id) NOT IN \
               (SELECT VALUE card_template_id FROM kanban_schedule WHERE enabled = true)",
        DONE_CARD_TTL_DAYS
    );
    let rows = db
        .query_json(&pick_q)
        .await
        .map_err(|e| format!("Failed to pick stale done cards: {}", e))?;
    let victims: Vec<String> = rows
        .iter()
        .filter_map(|r| r["id"].as_str().map(String::from))
        .collect();
    if victims.is_empty() {
        return Ok(victims);
    }
    // Confined review chat workflows (hidden_from_list) linked to the victims.
    // Cascaded individually below so they don't leak in the DB (I5).
    let chat_workflows: Vec<String> = rows
        .iter()
        .filter_map(|r| r["review_chat_workflow_id"].as_str().map(String::from))
        .collect();

    // 2. Cascade interactions first (foreign-key-like cleanup; SurrealDB
    //    has no real FK so we do it explicitly).
    let ids_json = serde_json::to_string(&victims)
        .map_err(|e| format!("Failed to serialize purge ids: {}", e))?;
    let del_interactions = format!(
        "DELETE kanban_card_interaction WHERE card_id IN {}",
        ids_json
    );
    db.execute(&del_interactions)
        .await
        .map_err(|e| format!("Failed to cascade interactions: {}", e))?;

    // 2b. Cascade each victim's confined review chat workflow (workflow row +
    //     messages + execution blocks). Hidden from the sidebar, it would
    //     otherwise be unreachable and leak forever.
    for chat_wf in &chat_workflows {
        crate::commands::kanban_card::cascade_review_chat_workflow(db, chat_wf).await;
    }

    // 3. Delete the cards themselves. Re-evaluate the same predicate to
    //    stay race-safe: if the user just dropped a new schedule on one of
    //    them between step 1 and now, the NOT IN clause will save it.
    let del_cards = format!(
        "DELETE kanban_card \
         WHERE meta::id(id) IN {} \
           AND `column` = 'done' \
           AND meta::id(id) NOT IN \
               (SELECT VALUE card_template_id FROM kanban_schedule WHERE enabled = true)",
        ids_json
    );
    db.execute(&del_cards)
        .await
        .map_err(|e| format!("Failed to delete stale done cards: {}", e))?;

    Ok(victims)
}
