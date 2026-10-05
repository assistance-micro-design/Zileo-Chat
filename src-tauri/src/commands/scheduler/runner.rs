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
//! Background task spawner: the 60s tick loop.
use super::concurrency::start_next_pending_card_core;
use super::queue::process_due_schedules_core;
use super::recovery::{purge_stale_done_cards_core, reclaim_orphaned_doing_cards_core};
use super::{ORPHAN_DOING_GRACE_SECS, SCHEDULER_TICK_SECS};
use crate::db::DBClient;
use serde_json::json;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

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
