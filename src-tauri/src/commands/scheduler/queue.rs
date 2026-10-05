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
//! Due-schedule processing: spawn cards from templates, re-arm.
use super::MAX_CATCHUP_PER_SCHEDULE;
use crate::commands::kanban_schedule::compute_next_run_at;
use crate::db::DBClient;
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use tracing::{info, warn};

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
pub(crate) async fn template_has_pending_instance(
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
    // Exclude `proposed` cards: a generated card awaiting validation is
    // stored with `column='todo'` but is NOT a real pending instance — counting
    // it would make `skip_if_pending` skip the recurrence spawn while the user
    // has not yet validated anything, silently dropping the scheduled run.
    let count_q = "SELECT count() AS c FROM kanban_card \
        WHERE `column` IN ['todo', 'doing'] \
          AND status != 'proposed' \
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
