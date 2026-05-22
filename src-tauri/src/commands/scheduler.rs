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

/// Cards stuck in the `done` column past this many days are auto-purged on
/// each scheduler tick. Cards that are themselves a recurrence template
/// (referenced by an enabled `kanban_schedule`) are never purged regardless
/// of age — they are the user's blueprint and must persist.
pub const DONE_CARD_TTL_DAYS: i64 = 3;

/// Maximum number of missed occurrences we spawn per schedule per tick.
/// Prevents an explosion of cards if the app was closed for weeks.
pub const MAX_CATCHUP_PER_SCHEDULE: usize = 7;

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
            match purge_stale_done_cards_core(&db).await {
                Ok(ids) if !ids.is_empty() => {
                    info!(
                        purged = ids.len(),
                        "Kanban scheduler: purged stale done cards"
                    );
                    let _ = app_handle.emit("kanban:cards_purged", json!({ "card_ids": ids }));
                }
                Ok(_) => debug!("Kanban scheduler: nothing to purge"),
                Err(e) => warn!(error = %e, "Kanban scheduler: purge error"),
            }
        }
    })
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
        "SELECT meta::id(id) AS id FROM kanban_card \
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
        "SELECT meta::id(id) AS id, card_template_id, days_of_week, hour, minute, \
         next_run_at, skip_if_pending FROM kanban_schedule \
         WHERE enabled = true AND next_run_at <= <datetime> '{}'",
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
        let skip_if_pending = row["skip_if_pending"].as_bool().unwrap_or(false);

        if schedule_id.is_empty() || template_id.is_empty() {
            continue;
        }

        // F: skip-if-pending guard. If a previous instance is still in flight
        // (todo or doing) for this template, do not spawn a new one — just
        // re-arm next_run_at to the next future occurrence so we don't poll
        // the same template every tick.
        if skip_if_pending && template_has_pending_instance(db, &template_id).await? {
            info!(
                schedule_id = %schedule_id,
                template_id = %template_id,
                "skip_if_pending=true and an instance is still pending — skipping spawn"
            );
            rearm_schedule(db, &schedule_id, &days, hour, minute, now, false).await;
            continue;
        }

        // A: catchup loop. Walk from the schedule's stored `next_run_at`
        // forward through every missed occurrence up to `now`, spawning one
        // card per missed slot. Cap at MAX_CATCHUP_PER_SCHEDULE so an app
        // closed for weeks doesn't dump 50+ cards at boot.
        let mut cursor = row["next_run_at"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or(now);

        let mut local_spawned = 0usize;
        for _ in 0..MAX_CATCHUP_PER_SCHEDULE {
            match spawn_card_from_template(db, &template_id).await {
                Ok(_) => {
                    local_spawned += 1;
                }
                Err(e) => {
                    let orphan = e.starts_with("Template card not found");
                    if orphan {
                        // B: auto-disable orphan schedule so it stops polluting
                        // the logs every minute. The user can re-enable it via
                        // the UI after re-creating a template card.
                        let _ = db
                            .execute(&format!(
                                "UPDATE kanban_schedule:`{}` SET enabled = false",
                                schedule_id
                            ))
                            .await;
                        info!(
                            schedule_id = %schedule_id,
                            template_id = %template_id,
                            "Template card missing — schedule auto-disabled"
                        );
                    } else {
                        warn!(
                            schedule_id = %schedule_id,
                            template_id = %template_id,
                            error = %e,
                            "Failed to spawn card from template; will retry next tick"
                        );
                    }
                    break;
                }
            }
            // Advance the cursor: next occurrence strictly after the current
            // cursor. If still in the past, loop again to catch up.
            cursor = compute_next_run_at(&days, hour, minute, cursor);
            if cursor > now {
                break;
            }
        }
        spawned += local_spawned;

        // Re-arm next_run_at to whatever the cursor ended on (or the next
        // future occurrence relative to now if the cap was hit).
        let next = if cursor > now {
            cursor
        } else {
            compute_next_run_at(&days, hour, minute, now)
        };
        let next_str = next.to_rfc3339();
        let upd = if local_spawned > 0 {
            format!(
                "UPDATE kanban_schedule:`{}` SET next_run_at = <datetime> '{}', \
                 last_run_at = <datetime> '{}'",
                schedule_id, next_str, now_str
            )
        } else {
            format!(
                "UPDATE kanban_schedule:`{}` SET next_run_at = <datetime> '{}'",
                schedule_id, next_str
            )
        };
        if let Err(e) = db.execute(&upd).await {
            warn!(schedule_id = %schedule_id, error = %e, "Failed to re-arm schedule");
        }
    }
    Ok(spawned)
}

/// Sets `next_run_at` to the next future occurrence (computed from `now`) and,
/// when `did_spawn` is true, stamps `last_run_at`. Best-effort: errors are
/// logged but never propagated.
async fn rearm_schedule(
    db: &Arc<DBClient>,
    schedule_id: &str,
    days: &[u8],
    hour: u8,
    minute: u8,
    now: chrono::DateTime<Utc>,
    did_spawn: bool,
) {
    let next = compute_next_run_at(days, hour, minute, now).to_rfc3339();
    let upd = if did_spawn {
        let now_str = now.to_rfc3339();
        format!(
            "UPDATE kanban_schedule:`{}` SET next_run_at = <datetime> '{}', \
             last_run_at = <datetime> '{}'",
            schedule_id, next, now_str
        )
    } else {
        format!(
            "UPDATE kanban_schedule:`{}` SET next_run_at = <datetime> '{}'",
            schedule_id, next
        )
    };
    if let Err(e) = db.execute(&upd).await {
        warn!(schedule_id = %schedule_id, error = %e, "Failed to re-arm schedule");
    }
}

/// Returns true if at least one card cloned from this template is still
/// pending (column todo) or in flight (column doing). Used by the
/// `skip_if_pending` guard.
///
/// The schema does not carry a foreign-key link from a spawned card back to
/// its template, so we rely on the invariant that `spawn_card_from_template`
/// clones title + kanban_agent_id + target_agent_id verbatim. Any card in
/// `todo`/`doing` whose triplet matches the template's (and is not the
/// template itself) is considered a pending instance.
async fn template_has_pending_instance(
    db: &Arc<DBClient>,
    template_id: &str,
) -> Result<bool, String> {
    let tmpl_q = format!(
        "SELECT title, kanban_agent_id, target_agent_id FROM kanban_card:`{}`",
        template_id
    );
    let tmpl_rows = db
        .query_json(&tmpl_q)
        .await
        .map_err(|e| format!("Failed to load template for pending check: {}", e))?;
    let Some(tmpl) = tmpl_rows.into_iter().next() else {
        return Ok(false);
    };
    let title = tmpl["title"].as_str().unwrap_or("");
    let ka = tmpl["kanban_agent_id"].as_str().unwrap_or("");
    let ta = tmpl["target_agent_id"].as_str().unwrap_or("");
    if title.is_empty() || ka.is_empty() || ta.is_empty() {
        return Ok(false);
    }
    let count_q = "SELECT count() AS c FROM kanban_card \
        WHERE `column` IN ['todo', 'doing'] \
          AND meta::id(id) != $tid \
          AND title = $title \
          AND kanban_agent_id = $ka \
          AND target_agent_id = $ta \
        GROUP ALL";
    let rows: Vec<serde_json::Value> = db
        .query_with_params(
            count_q,
            vec![
                ("tid".to_string(), json!(template_id)),
                ("title".to_string(), json!(title)),
                ("ka".to_string(), json!(ka)),
                ("ta".to_string(), json!(ta)),
            ],
        )
        .await
        .map_err(|e| format!("Failed to check pending instances: {}", e))?;
    let count = rows
        .into_iter()
        .next()
        .and_then(|r| r["c"].as_u64())
        .unwrap_or(0);
    Ok(count > 0)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    // start_next_pending_card_core is exercised via the spawn loop; here we
    // only cover mark_card_done_core which has no AppHandle dependency.
    // The promotion-slot logic relies on a real Tauri AppHandle for the
    // event emit, which cannot be mocked from a unit test.

    /// Regression: a card that is referenced as `card_template_id` by an
    /// enabled schedule must never be promoted to `doing`. The template is
    /// the user's blueprint; only fresh clones spawned by the schedule are
    /// eligible for execution. Mirrors the WHERE clause in
    /// `start_next_pending_card_core`.
    #[tokio::test]
    async fn test_promotion_query_excludes_schedule_templates() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let template_id = uuid::Uuid::new_v4().to_string();
        let regular_id = uuid::Uuid::new_v4().to_string();

        for (cid, title) in [(&template_id, "template"), (&regular_id, "regular")] {
            let q = format!(
                "CREATE kanban_card:`{cid}` CONTENT {{
                    id: '{cid}', title: '{title}', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'ready', `column`: 'todo',
                    `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                    created_at: time::now(), updated_at: time::now()
                }}"
            );
            state.db.execute(&q).await.unwrap();
        }

        // Attach an enabled schedule to the template card.
        let sid = uuid::Uuid::new_v4().to_string();
        let sched = format!(
            "CREATE kanban_schedule:`{sid}` CONTENT {{
                id: '{sid}', card_template_id: '{template_id}',
                days_of_week: [0], hour: 9, minute: 0,
                next_run_at: time::now() + 7d,
                last_run_at: NONE, enabled: true, skip_if_pending: false,
                created_at: time::now()
            }}"
        );
        state.db.execute(&sched).await.unwrap();

        // Run the same query the production code uses.
        let pick_q = "SELECT meta::id(id) AS id, title, `column_order`, created_at \
             FROM kanban_card \
             WHERE status = 'ready' AND `column` = 'todo' \
               AND meta::id(id) NOT IN (SELECT VALUE card_template_id FROM kanban_schedule WHERE enabled = true) \
             ORDER BY `column_order` ASC, created_at ASC";
        let rows = state.db.query_json(pick_q).await.unwrap();
        let ids: Vec<String> = rows
            .iter()
            .filter_map(|r| r["id"].as_str().map(String::from))
            .collect();
        assert!(
            !ids.contains(&template_id),
            "template card must be excluded from promotion"
        );
        assert!(
            ids.contains(&regular_id),
            "regular ready card must remain eligible"
        );
    }

    /// Purge of stale `done` cards: cards in `done` whose `updated_at` is
    /// older than DONE_CARD_TTL_DAYS must be deleted, UNLESS they are
    /// referenced as `card_template_id` by an enabled `kanban_schedule`
    /// (those are recurrence blueprints). Linked
    /// `kanban_card_interaction` rows are cascaded; `workflow` rows are
    /// preserved.
    #[tokio::test]
    async fn test_purge_stale_done_cards() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();

        // 1. Stale done, no schedule -> MUST be purged.
        let stale_id = uuid::Uuid::new_v4().to_string();
        // 2. Stale done, IS a schedule template -> MUST stay.
        let stale_tmpl_id = uuid::Uuid::new_v4().to_string();
        // 3. Recent done -> MUST stay.
        let recent_id = uuid::Uuid::new_v4().to_string();
        // 4. Stale but in `review` -> MUST stay (only `done` is purged).
        let stale_review_id = uuid::Uuid::new_v4().to_string();

        let stale_ts = "time::now() - 4d";
        let recent_ts = "time::now() - 1d";

        for (cid, col, ts) in [
            (&stale_id, "done", stale_ts),
            (&stale_tmpl_id, "done", stale_ts),
            (&recent_id, "done", recent_ts),
            (&stale_review_id, "review", stale_ts),
        ] {
            let q = format!(
                "CREATE kanban_card:`{cid}` CONTENT {{
                    id: '{cid}', title: 't', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'done', `column`: '{col}',
                    `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                    created_at: {ts}, updated_at: {ts}
                }}"
            );
            state.db.execute(&q).await.unwrap();
        }

        // Attach enabled schedule to stale_tmpl_id.
        let sid = uuid::Uuid::new_v4().to_string();
        let sched = format!(
            "CREATE kanban_schedule:`{sid}` CONTENT {{
                id: '{sid}', card_template_id: '{stale_tmpl_id}',
                days_of_week: [0], hour: 9, minute: 0,
                next_run_at: time::now() + 7d,
                last_run_at: NONE, enabled: true, skip_if_pending: false,
                created_at: time::now()
            }}"
        );
        state.db.execute(&sched).await.unwrap();

        // Attach an interaction row to the stale card (must be cascaded).
        let int_id = uuid::Uuid::new_v4().to_string();
        let interaction = format!(
            "CREATE kanban_card_interaction:`{int_id}` CONTENT {{
                id: '{int_id}', card_id: '{stale_id}', kind: 'compose',
                kanban_agent_id: '{agent_id}', provider: 'mistral',
                model_id_used: 'm', task_input: 'x',
                iterations: [], created_at: time::now()
            }}"
        );
        state.db.execute(&interaction).await.unwrap();

        let purged = purge_stale_done_cards_core(&state.db).await.unwrap();
        assert_eq!(purged.len(), 1, "exactly one card must be purged");
        assert_eq!(
            purged[0], stale_id,
            "the only purged card must be the stale non-template"
        );

        // Verify DB state.
        let remaining = state
            .db
            .query_json("SELECT meta::id(id) AS id FROM kanban_card")
            .await
            .unwrap();
        let remaining_ids: Vec<String> = remaining
            .iter()
            .filter_map(|r| r["id"].as_str().map(String::from))
            .collect();
        assert!(
            !remaining_ids.contains(&stale_id),
            "stale non-template card must be gone"
        );
        assert!(
            remaining_ids.contains(&stale_tmpl_id),
            "stale template card must stay"
        );
        assert!(
            remaining_ids.contains(&recent_id),
            "recent done card must stay"
        );
        assert!(
            remaining_ids.contains(&stale_review_id),
            "stale review card must stay"
        );

        // Interaction must be cascaded.
        let interactions = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM kanban_card_interaction WHERE card_id = '{}'",
                stale_id
            ))
            .await
            .unwrap();
        assert!(
            interactions.is_empty(),
            "interaction rows must be cascaded on purge"
        );
    }

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
