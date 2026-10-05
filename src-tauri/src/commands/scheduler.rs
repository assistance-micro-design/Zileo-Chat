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

// (No imports: scheduling constants below need none.)

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

/// Grace period (seconds) before a `doing` card whose linked workflow is no
/// longer alive is recovered at boot.
///
/// Distinct from [`ORPHAN_DOING_GRACE_SECS`], which targets cards with NO
/// `workflow_id`. This one targets cards that DID get a `workflow_id` stamped by
/// the frontend (`set_kanban_card_workflow_id`, which refreshes `updated_at`).
/// At boot no frontend run can be live yet, so any such card older than this
/// window belonged to a previous session: its worker either finished (and the
/// card missed its review transition) or died mid-run. The window's only job is
/// to spare a card the CURRENT boot's frontend is wiring up right now — its
/// `updated_at` stays fresh, so a real in-flight run is never touched. Sized at
/// 2× the tick like the orphan grace.
pub const STUCK_DOING_GRACE_SECS: i64 = 2 * SCHEDULER_TICK_SECS as i64;

// Submodules (split of the former monolithic `scheduler.rs`):
// - `runner` - background task spawner (tick loop)
// - `recovery` - orphan / stuck / stale card recovery
// - `queue` - due-schedule processing and card spawning
// - `concurrency` - promotion budget, claim, selection
// - `service` - card lifecycle (workflow linkage, completion)

pub mod concurrency;
pub mod queue;
pub mod recovery;
pub mod runner;
pub mod service;
#[cfg(test)]
mod tests;

pub use concurrency::*;
#[allow(unused_imports)]
pub use queue::*;
pub use recovery::*;
pub use runner::*;
pub use service::*;
