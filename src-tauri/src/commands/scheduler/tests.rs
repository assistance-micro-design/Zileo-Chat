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
//! Tests for the Kanban scheduler.
use super::*;
use crate::constants::workflow::DEFAULT_MAX_CONCURRENT_WORKFLOWS;
use crate::db::DBClient;
use chrono::Utc;
use std::sync::Arc;

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

/// A `proposed` clone (generated, awaiting validation, stored
/// `column='todo'`) must NOT be counted as a pending instance, otherwise
/// `skip_if_pending` would skip the recurrence spawn while nothing real is in
/// flight. A genuine `ready` clone, by contrast, MUST still count.
#[tokio::test]
async fn pending_instance_ignores_proposed_clone() {
    let (state, _g) = setup_test_state().await;
    let agent_id = uuid::Uuid::new_v4().to_string();
    let template_id = uuid::Uuid::new_v4().to_string();
    seed_card(&state.db, &template_id, &agent_id, "daily", "todo", "ready").await;

    // A proposed clone of the same triplet must be ignored.
    let proposed = uuid::Uuid::new_v4().to_string();
    seed_card(&state.db, &proposed, &agent_id, "daily", "todo", "proposed").await;
    assert!(
        !template_has_pending_instance(&state.db, &template_id)
            .await
            .unwrap(),
        "a proposed clone must not be counted as a pending instance"
    );

    // A real ready clone DOES count (guards against over-filtering).
    let ready = uuid::Uuid::new_v4().to_string();
    seed_card(&state.db, &ready, &agent_id, "daily", "todo", "ready").await;
    assert!(
        template_has_pending_instance(&state.db, &template_id)
            .await
            .unwrap(),
        "a real ready clone must be counted as a pending instance"
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

/// The cap exposed to the frontend MUST be the very constant the scheduler
/// uses for its promotion slot budget — that single-source coupling is the
/// whole point of the command (the board's "X / N actifs" badge and the
/// worker cap can never drift from the backend authority). Guards against a
/// literal being re-hardcoded on either side.
#[test]
fn get_max_concurrent_exposes_scheduler_budget() {
    assert_eq!(
        get_max_concurrent_workflows(),
        DEFAULT_MAX_CONCURRENT_WORKFLOWS,
        "the exposed cap must equal the scheduler's promotion budget"
    );
    assert!(
        get_max_concurrent_workflows() > 0,
        "a zero cap would deadlock promotion"
    );
}

/// K1-bis boot recovery. Four `doing` cards carrying a `workflow_id`:
///   - old + no message            -> requeued (never started)
///   - old + only a USER message   -> requeued (died mid-run, no result)
///   - old + an ASSISTANT message  -> finalized to `review` (run completed,
///                                    missed its transition)
///   - fresh updated_at            -> spared (the current boot is wiring it up)
#[tokio::test]
async fn test_recover_stuck_doing_cards() {
    let (state, _g) = setup_test_state().await;
    let agent_id = uuid::Uuid::new_v4().to_string();

    let no_msg = uuid::Uuid::new_v4().to_string();
    let no_msg_wf = uuid::Uuid::new_v4().to_string();
    let user_only = uuid::Uuid::new_v4().to_string();
    let user_only_wf = uuid::Uuid::new_v4().to_string();
    let completed = uuid::Uuid::new_v4().to_string();
    let completed_wf = uuid::Uuid::new_v4().to_string();
    let recent = uuid::Uuid::new_v4().to_string();
    let recent_wf = uuid::Uuid::new_v4().to_string();

    let seed = |cid: &str, wf: &str, ts: &str| {
        format!(
            "CREATE kanban_card:`{cid}` CONTENT {{
                    id: '{cid}', title: 't', description: '',
                    kanban_agent_id: '{agent_id}', target_agent_id: '{agent_id}',
                    prompt_id: NONE, inline_prompt: 'p', variables: '{{}}',
                    target_folder_id: NONE, status: 'doing', `column`: 'doing',
                    `column_order`: 0, workflow_id: '{wf}', error_summary: NONE,
                    created_at: {ts}, updated_at: {ts}
                }}"
        )
    };
    let old = "time::now() - 200s";
    state
        .db
        .execute(&seed(&no_msg, &no_msg_wf, old))
        .await
        .unwrap();
    state
        .db
        .execute(&seed(&user_only, &user_only_wf, old))
        .await
        .unwrap();
    state
        .db
        .execute(&seed(&completed, &completed_wf, old))
        .await
        .unwrap();
    state
        .db
        .execute(&seed(&recent, &recent_wf, "time::now()"))
        .await
        .unwrap();

    // A message of the given role for a workflow.
    let seed_msg = |wf: &str, role: &str| {
        let mid = uuid::Uuid::new_v4().to_string();
        format!(
            "CREATE message:`{mid}` SET id = '{mid}', workflow_id = '{wf}', \
                 role = '{role}', content = 'x', tokens = 0, \
                 created_at = time::now(), updated_at = time::now()"
        )
    };
    // user_only: only a user message (worker started, never finished).
    state
        .db
        .execute(&seed_msg(&user_only_wf, "user"))
        .await
        .unwrap();
    // completed: a user AND an assistant message (run produced its result).
    state
        .db
        .execute(&seed_msg(&completed_wf, "user"))
        .await
        .unwrap();
    state
        .db
        .execute(&seed_msg(&completed_wf, "assistant"))
        .await
        .unwrap();

    let recovery = recover_stuck_doing_cards_core(&state.db, STUCK_DOING_GRACE_SECS)
        .await
        .unwrap();

    assert_eq!(
        recovery.finalized,
        vec![completed.clone()],
        "only the card with an assistant message is finalized to review"
    );
    let mut requeued = recovery.requeued.clone();
    requeued.sort();
    let mut expected = vec![no_msg.clone(), user_only.clone()];
    expected.sort();
    assert_eq!(
        requeued, expected,
        "both message-less / user-only cards requeue"
    );

    let check = |cid: String| {
        let db = state.db.clone();
        async move {
            let rows = db
                .query_json(&format!(
                    "SELECT status, `column`, workflow_id FROM kanban_card:`{}`",
                    cid
                ))
                .await
                .unwrap();
            (
                rows[0]["status"].as_str().unwrap_or("").to_string(),
                rows[0]["column"].as_str().unwrap_or("").to_string(),
                rows[0]["workflow_id"].as_str().is_some(),
            )
        }
    };
    // Requeued cards: ready/todo, workflow_id cleared.
    for cid in [no_msg, user_only] {
        let (s, c, wf_present) = check(cid).await;
        assert_eq!((s.as_str(), c.as_str()), ("ready", "todo"));
        assert!(!wf_present, "workflow_id must be cleared on requeue");
    }
    // Finalized card: done/review, workflow_id preserved.
    let (s, c, wf_present) = check(completed).await;
    assert_eq!((s.as_str(), c.as_str()), ("done", "review"));
    assert!(wf_present, "workflow_id stays on a finalized card");
    // Fresh card: untouched.
    assert_eq!(check(recent).await.0, "doing", "recent card spared");
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
