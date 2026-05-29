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

/// Grace period (seconds) before an orphaned `doing` card is reclaimed.
///
/// A card promoted to `doing` whose `kanban:card_ready` event was lost (the
/// /kanban page was not mounted to consume it) stays `doing` with
/// `workflow_id = NONE` forever. Since slots are counted by `status='doing'`,
/// such cards permanently consume the budget and eventually deadlock it. Two
/// ticks: a healthy card receives its `workflow_id` within seconds of
/// promotion, so 2× the tick is a safe floor that never reclaims a card still
/// being wired up by the frontend.
pub const ORPHAN_DOING_GRACE_SECS: i64 = 2 * SCHEDULER_TICK_SECS as i64;

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
            // Reclaim orphaned `doing` cards BEFORE counting slots so any freed
            // slot is available to the promotion step in the same tick (K1).
            match reclaim_orphaned_doing_cards_core(&db, ORPHAN_DOING_GRACE_SECS).await {
                Ok(ids) if !ids.is_empty() => {
                    info!(
                        reclaimed = ids.len(),
                        "Kanban scheduler: reclaimed orphaned doing cards"
                    )
                }
                Ok(_) => debug!("Kanban scheduler: no orphaned doing cards"),
                Err(e) => warn!(error = %e, "Kanban scheduler: orphan reclaim error"),
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

        // K7 guard: an empty `days_of_week` makes compute_next_run_at return
        // `now`, so the schedule would fire every tick forever (DoS). Newer
        // payloads are rejected at validation, but a legacy/forged row could
        // still carry empty days — auto-disable it so it stops polluting every
        // tick (the user can re-enable after fixing the days via the UI).
        if days.is_empty() {
            let _ = db
                .execute(&format!(
                    "UPDATE kanban_schedule:`{}` SET enabled = false",
                    schedule_id
                ))
                .await;
            warn!(
                schedule_id = %schedule_id,
                "Schedule has empty days_of_week — auto-disabled (would fire every tick)"
            );
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

        // If we hit the cap while still behind `now`, older missed occurrences
        // were intentionally skipped — surface it (no silent drop).
        if local_spawned == MAX_CATCHUP_PER_SCHEDULE && cursor <= now {
            warn!(
                schedule_id = %schedule_id,
                template_id = %template_id,
                cap = MAX_CATCHUP_PER_SCHEDULE,
                "Catch-up cap reached — older missed occurrences skipped this tick"
            );
        }

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
async fn try_claim_pending_card_core(db: &Arc<DBClient>, card_id: &str) -> Result<bool, String> {
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

/// Slot-budget + candidate selection for promotion, WITHOUT the per-card claim
/// or the `kanban:card_ready` emit.
///
/// Extracted from `start_next_pending_card_core` so the slot accounting (an
/// empty result when in-flight `doing` cards saturate
/// `DEFAULT_MAX_CONCURRENT_WORKFLOWS`), the schedule-template exclusion and the
/// `column_order` ordering can be asserted without a Tauri `AppHandle` (the
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

/// Counts how many cards are currently `doing` (in-flight) and promotes the
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

    /// Seeds a `kanban_card`. `kanban_agent_id` == `target_agent_id` == the
    /// passed `agent_id` so the `skip_if_pending` triplet match is predictable.
    async fn seed_card(
        db: &Arc<DBClient>,
        card_id: &str,
        agent_id: &str,
        title: &str,
        column: &str,
        status: &str,
    ) {
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: '{title}', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: '{status}', `column`: '{column}',
                `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        db.execute(&q).await.unwrap();
    }

    /// Seeds an enabled schedule for `template_id`. `due=true` sets
    /// `next_run_at` in the past (now - 1h); all 7 weekdays so the re-arm
    /// always lands a future occurrence after one catch-up step.
    async fn seed_schedule(
        db: &Arc<DBClient>,
        schedule_id: &str,
        template_id: &str,
        due: bool,
        skip_if_pending: bool,
    ) {
        let next = if due {
            "time::now() - 1h"
        } else {
            "time::now() + 7d"
        };
        let q = format!(
            "CREATE kanban_schedule:`{schedule_id}` CONTENT {{
                id: '{schedule_id}', card_template_id: '{template_id}',
                days_of_week: [0, 1, 2, 3, 4, 5, 6], hour: 9, minute: 0,
                next_run_at: {next},
                last_run_at: NONE, enabled: true, skip_if_pending: {skip_if_pending},
                created_at: time::now()
            }}"
        );
        db.execute(&q).await.unwrap();
    }

    /// `select_cards_to_promote_core` excludes cards referenced as a schedule
    /// template (the user's blueprint) and surfaces only regular ready/todo
    /// cards. Exercises the real helper (replaces the old query-duplicating
    /// test that only mirrored the WHERE clause).
    #[tokio::test]
    async fn select_to_promote_excludes_schedule_templates() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let template_id = uuid::Uuid::new_v4().to_string();
        let regular_id = uuid::Uuid::new_v4().to_string();
        seed_card(
            &state.db,
            &template_id,
            &agent_id,
            "template",
            "todo",
            "ready",
        )
        .await;
        seed_card(
            &state.db,
            &regular_id,
            &agent_id,
            "regular",
            "todo",
            "ready",
        )
        .await;
        let sid = uuid::Uuid::new_v4().to_string();
        seed_schedule(&state.db, &sid, &template_id, false, false).await;

        let cards = select_cards_to_promote_core(&state.db).await.unwrap();
        let ids: Vec<String> = cards
            .iter()
            .filter_map(|c| c["id"].as_str().map(String::from))
            .collect();
        assert!(
            !ids.contains(&template_id),
            "template card must be excluded from promotion, got {ids:?}"
        );
        assert!(
            ids.contains(&regular_id),
            "regular ready card must remain eligible, got {ids:?}"
        );
    }

    /// Slot accounting: when in-flight `doing` cards saturate
    /// `DEFAULT_MAX_CONCURRENT_WORKFLOWS`, the helper offers nothing for
    /// promotion (free == 0) even with a ready card waiting.
    #[tokio::test]
    async fn select_to_promote_returns_empty_when_slots_full() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        for _ in 0..DEFAULT_MAX_CONCURRENT_WORKFLOWS {
            let cid = uuid::Uuid::new_v4().to_string();
            seed_card(&state.db, &cid, &agent_id, "inflight", "doing", "doing").await;
        }
        let ready = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &ready, &agent_id, "waiting", "todo", "ready").await;

        let cards = select_cards_to_promote_core(&state.db).await.unwrap();
        assert!(
            cards.is_empty(),
            "no promotion while all {} slots are full, got {cards:?}",
            DEFAULT_MAX_CONCURRENT_WORKFLOWS
        );
    }

    /// A due schedule spawns at least one fresh todo/ready clone (bounded by the
    /// catch-up cap) and re-arms `next_run_at` into the future while staying
    /// enabled.
    #[tokio::test]
    async fn process_due_spawns_card_and_rearms() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let template_id = uuid::Uuid::new_v4().to_string();
        seed_card(
            &state.db,
            &template_id,
            &agent_id,
            "Recurring",
            "done",
            "done",
        )
        .await;
        let sid = uuid::Uuid::new_v4().to_string();
        seed_schedule(&state.db, &sid, &template_id, true, false).await;

        let spawned = process_due_schedules_core(&state.db).await.unwrap();
        assert!(spawned >= 1, "a due schedule must spawn at least one card");
        assert!(
            spawned <= MAX_CATCHUP_PER_SCHEDULE,
            "spawns must never exceed the catch-up cap"
        );

        let clones = state
            .db
            .query_json(
                "SELECT meta::id(id) AS id FROM kanban_card \
                 WHERE title = 'Recurring' AND `column` = 'todo' AND status = 'ready'",
            )
            .await
            .unwrap();
        assert_eq!(
            clones.len(),
            spawned,
            "one fresh todo/ready clone per spawn"
        );

        let sched = state
            .db
            .query_json(&format!(
                "SELECT enabled, next_run_at FROM kanban_schedule:`{}`",
                sid
            ))
            .await
            .unwrap();
        assert_eq!(
            sched[0]["enabled"], true,
            "a healthy schedule stays enabled"
        );
        let next = sched[0]["next_run_at"]
            .as_str()
            .expect("next_run_at must be present");
        let next_dt = chrono::DateTime::parse_from_rfc3339(next)
            .expect("next_run_at must be rfc3339")
            .with_timezone(&Utc);
        assert!(
            next_dt > Utc::now(),
            "next_run_at must be re-armed into the future, got {next}"
        );
    }

    /// `skip_if_pending=true`: when a previous instance of the template is still
    /// pending (matching title + agents in `todo`), the due schedule must NOT
    /// spawn a duplicate.
    #[tokio::test]
    async fn process_due_skips_spawn_when_instance_pending() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let template_id = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &template_id, &agent_id, "Daily", "done", "done").await;
        // A pending instance with the same triplet (title + agents) in todo.
        let pending = uuid::Uuid::new_v4().to_string();
        seed_card(&state.db, &pending, &agent_id, "Daily", "todo", "ready").await;
        let sid = uuid::Uuid::new_v4().to_string();
        seed_schedule(&state.db, &sid, &template_id, true, true).await;

        let spawned = process_due_schedules_core(&state.db).await.unwrap();
        assert_eq!(
            spawned, 0,
            "skip_if_pending must suppress spawn while an instance is pending"
        );

        let todo = state
            .db
            .query_json(
                "SELECT meta::id(id) AS id FROM kanban_card \
                 WHERE title = 'Daily' AND `column` = 'todo'",
            )
            .await
            .unwrap();
        assert_eq!(
            todo.len(),
            1,
            "no new clone must be spawned, only the pre-existing instance"
        );
    }

    /// A due schedule whose template card no longer exists must auto-disable
    /// itself (so it stops polluting every tick) and spawn nothing.
    #[tokio::test]
    async fn process_due_auto_disables_schedule_with_missing_template() {
        let (state, _g) = setup_test_state().await;
        let missing_template = uuid::Uuid::new_v4().to_string(); // no card created
        let sid = uuid::Uuid::new_v4().to_string();
        seed_schedule(&state.db, &sid, &missing_template, true, false).await;

        let spawned = process_due_schedules_core(&state.db).await.unwrap();
        assert_eq!(spawned, 0, "an orphan schedule spawns nothing");

        let sched = state
            .db
            .query_json(&format!("SELECT enabled FROM kanban_schedule:`{}`", sid))
            .await
            .unwrap();
        assert_eq!(
            sched[0]["enabled"], false,
            "orphan schedule (missing template) must auto-disable"
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

    /// K1: an orphaned `doing` card (workflow_id NONE, older than the grace
    /// period) must be reset to `status='ready', column='todo'` so the slot
    /// frees and the scheduler re-promotes it. A recent `doing` card (within
    /// grace) and a `doing` card that already has a workflow_id are spared.
    #[tokio::test]
    async fn test_reclaim_orphaned_doing_cards() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();

        let orphan = uuid::Uuid::new_v4().to_string(); // doing, no wf, old -> reclaimed
        let recent = uuid::Uuid::new_v4().to_string(); // doing, no wf, fresh -> spared
        let linked = uuid::Uuid::new_v4().to_string(); // doing, has wf, old -> spared
        let wf = uuid::Uuid::new_v4().to_string();

        let seed = |cid: &str, wf_sql: &str, ts: &str| {
            format!(
                "CREATE kanban_card:`{cid}` CONTENT {{
                    id: '{cid}', title: 't', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'doing', `column`: 'doing',
                    `column_order`: 0, workflow_id: {wf_sql}, error_summary: NONE,
                    created_at: {ts}, updated_at: {ts}
                }}"
            )
        };
        state
            .db
            .execute(&seed(&orphan, "NONE", "time::now() - 200s"))
            .await
            .unwrap();
        state
            .db
            .execute(&seed(&recent, "NONE", "time::now()"))
            .await
            .unwrap();
        state
            .db
            .execute(&seed(&linked, &format!("'{}'", wf), "time::now() - 200s"))
            .await
            .unwrap();

        let reclaimed = reclaim_orphaned_doing_cards_core(&state.db, ORPHAN_DOING_GRACE_SECS)
            .await
            .unwrap();
        assert_eq!(
            reclaimed,
            vec![orphan.clone()],
            "only the old orphan is reclaimed"
        );

        let check = |cid: String| {
            let db = state.db.clone();
            async move {
                let rows = db
                    .query_json(&format!(
                        "SELECT status, `column` FROM kanban_card:`{}`",
                        cid
                    ))
                    .await
                    .unwrap();
                (
                    rows[0]["status"].as_str().unwrap_or("").to_string(),
                    rows[0]["column"].as_str().unwrap_or("").to_string(),
                )
            }
        };
        assert_eq!(check(orphan).await, ("ready".into(), "todo".into()));
        assert_eq!(check(recent).await, ("doing".into(), "doing".into()));
        assert_eq!(check(linked).await, ("doing".into(), "doing".into()));
    }

    /// K7: a legacy/forged enabled schedule with empty `days_of_week` must be
    /// auto-disabled by the scheduler instead of spawning a card every tick.
    #[tokio::test]
    async fn test_process_due_auto_disables_empty_days_schedule() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let template_id = uuid::Uuid::new_v4().to_string();
        let sid = uuid::Uuid::new_v4().to_string();

        // Template card for the schedule to clone (so spawn would otherwise work).
        let card = format!(
            "CREATE kanban_card:`{template_id}` CONTENT {{
                id: '{template_id}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'ready', `column`: 'todo',
                `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&card).await.unwrap();

        // Forged enabled schedule with empty days, due now.
        let sched = format!(
            "CREATE kanban_schedule:`{sid}` CONTENT {{
                id: '{sid}', card_template_id: '{template_id}',
                days_of_week: [], hour: 9, minute: 0,
                next_run_at: time::now() - 1h,
                last_run_at: NONE, enabled: true, skip_if_pending: false,
                created_at: time::now()
            }}"
        );
        state.db.execute(&sched).await.unwrap();

        let spawned = process_due_schedules_core(&state.db).await.unwrap();
        assert_eq!(spawned, 0, "empty-days schedule must not spawn any card");

        let rows = state
            .db
            .query_json(&format!("SELECT enabled FROM kanban_schedule:`{}`", sid))
            .await
            .unwrap();
        assert_eq!(
            rows[0]["enabled"], false,
            "empty-days schedule must be auto-disabled"
        );
    }

    /// K2: the claim must be atomic. A first claim on a `ready` card wins
    /// (flips to doing, returns true); a second concurrent claim on the same
    /// card loses (returns false, no second flip) — this is what prevents the
    /// three concurrent promoters from each emitting `card_ready` and starting
    /// two workflows for one card.
    #[tokio::test]
    async fn test_try_claim_pending_card_is_atomic() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'ready', `column`: 'todo',
                `column_order`: 0, workflow_id: NONE, error_summary: NONE,
                created_at: time::now(), updated_at: time::now()
            }}"
        );
        state.db.execute(&q).await.unwrap();

        // First claim wins.
        let first = try_claim_pending_card_core(&state.db, &card_id)
            .await
            .expect("claim query runs");
        assert!(first, "first claim on a ready card must win");

        // Card is now doing.
        let after = state
            .db
            .query_json(&format!(
                "SELECT status, `column` FROM kanban_card:`{}`",
                card_id
            ))
            .await
            .unwrap();
        assert_eq!(after[0]["status"], "doing");
        assert_eq!(after[0]["column"], "doing");

        // Second claim loses (no longer ready) — no double promotion.
        let second = try_claim_pending_card_core(&state.db, &card_id)
            .await
            .expect("claim query runs");
        assert!(
            !second,
            "second claim must lose the race (card already doing)"
        );
    }

    /// I5: when the scheduler auto-purges a stale `done` card, its confined
    /// review chat workflow (hidden_from_list) must be cascaded too — otherwise
    /// the hidden chat + its messages leak in the DB forever.
    #[tokio::test]
    async fn test_purge_cascades_review_chat_workflow() {
        let (state, _g) = setup_test_state().await;
        let agent_id = uuid::Uuid::new_v4().to_string();
        let card_id = uuid::Uuid::new_v4().to_string();
        let chat_wf = uuid::Uuid::new_v4().to_string();

        // Hidden review chat workflow + a message attached to it.
        let create_wf = format!(
            "CREATE workflow:`{chat_wf}` SET id = '{chat_wf}', name = 'chat', agent_id = '{agent_id}', \
             status = 'idle', hidden_from_list = true, pinned = false, \
             created_at = time::now(), updated_at = time::now()"
        );
        state.db.execute(&create_wf).await.unwrap();
        let mid = uuid::Uuid::new_v4().to_string();
        let create_msg = format!(
            "CREATE message:`{mid}` SET id = '{mid}', workflow_id = '{chat_wf}', role = 'assistant', \
             content = 'seed', tokens = 0, created_at = time::now(), updated_at = time::now()"
        );
        state.db.execute(&create_msg).await.unwrap();

        // Stale done card (older than the TTL) linked to the chat workflow.
        let create_card = format!(
            "CREATE kanban_card:`{card_id}` CONTENT {{
                id: '{card_id}', title: 't', description: '',
                kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                target_folder_id: NONE, status: 'done', `column`: 'done',
                `column_order`: 0, workflow_id: NONE,
                review_chat_workflow_id: '{chat_wf}', error_summary: NONE,
                created_at: time::now() - 4d, updated_at: time::now() - 4d
            }}"
        );
        state.db.execute(&create_card).await.unwrap();

        let purged = purge_stale_done_cards_core(&state.db).await.unwrap();
        assert_eq!(purged.len(), 1, "the stale card must be purged");

        // Chat workflow + its messages must be cascaded.
        let wf_rows = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM workflow:`{}`",
                chat_wf
            ))
            .await
            .unwrap();
        assert!(
            wf_rows.is_empty(),
            "hidden chat workflow must be cascaded on purge"
        );
        let msg_rows = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM message WHERE workflow_id = '{}'",
                chat_wf
            ))
            .await
            .unwrap();
        assert!(
            msg_rows.is_empty(),
            "chat messages must be cascaded on purge"
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
