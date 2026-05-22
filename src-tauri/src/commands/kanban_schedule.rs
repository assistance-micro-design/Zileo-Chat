// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban schedule CRUD commands.

use crate::db::DBClient;
use crate::models::kanban_schedule::validate_schedule_create;
use crate::models::{KanbanSchedule, KanbanScheduleCreate, KanbanScheduleUpdate};
use crate::security::validate_uuid_field;
use crate::AppState;
#[cfg(test)]
use chrono::Timelike;
use chrono::{DateTime, Datelike, Duration, Local, TimeZone, Utc};
use serde_json::json;
use tauri::State;
use tracing::{info, instrument};

const KANBAN_SCHEDULE_FIELDS: &str =
    "meta::id(id) AS id, card_template_id, days_of_week, hour, minute, next_run_at, \
    last_run_at, enabled, skip_if_pending, created_at";

/// Computes the next datetime matching a recurrence after `now`. The
/// `hour`/`minute` parameters are interpreted as wall-clock values in the
/// **local timezone** (where the user lives), the same way the time picker
/// captures them in the UI. The returned `DateTime<Utc>` is the corresponding
/// UTC instant suitable for storage and comparison against UTC `now`.
///
/// Returns `now` itself if `days_of_week` is empty (defensive: schedule is idle).
pub fn compute_next_run_at(
    days_of_week: &[u8],
    hour: u8,
    minute: u8,
    now: DateTime<Utc>,
) -> DateTime<Utc> {
    compute_next_run_at_in_tz(days_of_week, hour, minute, now, &Local)
}

/// Parameterised variant — the `tz` argument decides which timezone the
/// `hour`/`minute` wall-clock values are anchored to. Production always
/// passes `&Local`; tests pass `&Utc` so they remain deterministic regardless
/// of the host machine's timezone.
pub fn compute_next_run_at_in_tz<Tz>(
    days_of_week: &[u8],
    hour: u8,
    minute: u8,
    now: DateTime<Utc>,
    tz: &Tz,
) -> DateTime<Utc>
where
    Tz: TimeZone,
{
    if days_of_week.is_empty() {
        return now;
    }
    let now_local = now.with_timezone(tz);
    for delta in 0..=14_i64 {
        let candidate_date = now_local.date_naive() + Duration::days(delta);
        // chrono weekday: Monday=0..Sunday=6 via num_days_from_monday().
        let day_num = candidate_date.weekday().num_days_from_monday() as u8;
        if !days_of_week.contains(&day_num) {
            continue;
        }
        // Wall-clock match in the user's timezone, then convert to UTC.
        // DST gaps/overlaps (.single() = None) are skipped — the next loop
        // iteration picks the following day, which is acceptable for the
        // rare ambiguity window.
        let candidate_local = tz
            .with_ymd_and_hms(
                candidate_date.year(),
                candidate_date.month(),
                candidate_date.day(),
                hour as u32,
                minute as u32,
                0,
            )
            .single();
        if let Some(c_local) = candidate_local {
            let c_utc = c_local.with_timezone(&Utc);
            if c_utc > now {
                return c_utc;
            }
        }
    }
    now
}

pub async fn create_kanban_schedule_core(
    db: &DBClient,
    data: KanbanScheduleCreate,
) -> Result<KanbanSchedule, String> {
    validate_schedule_create(&data)?;
    let card_template_id = validate_uuid_field(&data.card_template_id, "card_template_id")?;
    let next_run_at = compute_next_run_at(&data.days_of_week, data.hour, data.minute, Utc::now());

    let id = uuid::Uuid::new_v4().to_string();
    let next_run_str = next_run_at.to_rfc3339();
    let query = format!(
        "CREATE kanban_schedule:`{id}` CONTENT {{
            id: '{id}',
            card_template_id: '{card_template_id}',
            days_of_week: $days,
            hour: $hour,
            minute: $minute,
            next_run_at: <datetime> '{next_run_str}',
            last_run_at: NONE,
            enabled: true,
            skip_if_pending: $skip,
            created_at: time::now()
        }}"
    );
    db.execute_with_params(
        &query,
        vec![
            ("days".to_string(), json!(data.days_of_week)),
            ("hour".to_string(), json!(data.hour)),
            ("minute".to_string(), json!(data.minute)),
            ("skip".to_string(), json!(data.skip_if_pending)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to create kanban_schedule: {}", e))?;
    get_kanban_schedule_core(db, &id).await
}

pub async fn get_kanban_schedule_core(db: &DBClient, id: &str) -> Result<KanbanSchedule, String> {
    let validated = validate_uuid_field(id, "schedule_id")?;
    let query = format!("SELECT {KANBAN_SCHEDULE_FIELDS} FROM kanban_schedule:`{validated}`");
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to load kanban_schedule: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| "Schedule not found".to_string())?;
    serde_json::from_value(row).map_err(|e| format!("Failed to deserialize schedule: {}", e))
}

pub async fn list_kanban_schedules_core(db: &DBClient) -> Result<Vec<KanbanSchedule>, String> {
    let query =
        format!("SELECT {KANBAN_SCHEDULE_FIELDS} FROM kanban_schedule ORDER BY next_run_at ASC");
    let rows = db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to list schedules: {}", e))?;
    rows.into_iter()
        .map(|v| serde_json::from_value(v).map_err(|e| format!("Failed to deserialize: {}", e)))
        .collect()
}

pub async fn update_kanban_schedule_core(
    db: &DBClient,
    id: &str,
    update: KanbanScheduleUpdate,
) -> Result<KanbanSchedule, String> {
    let validated = validate_uuid_field(id, "schedule_id")?;
    let existing = get_kanban_schedule_core(db, &validated).await?;

    let new_days = update
        .days_of_week
        .unwrap_or_else(|| existing.days_of_week.clone());
    let new_hour = update.hour.unwrap_or(existing.hour);
    let new_minute = update.minute.unwrap_or(existing.minute);
    let new_enabled = match update.enabled {
        Some(Some(v)) => v,
        Some(None) => existing.enabled, // explicit null = keep (no "disable" via clear)
        None => existing.enabled,
    };
    // Re-validate via the model helper
    validate_schedule_create(&KanbanScheduleCreate {
        card_template_id: existing.card_template_id.clone(),
        days_of_week: new_days.clone(),
        hour: new_hour,
        minute: new_minute,
        skip_if_pending: existing.skip_if_pending,
    })?;

    let new_skip = update.skip_if_pending.unwrap_or(existing.skip_if_pending);
    let next_run_at = compute_next_run_at(&new_days, new_hour, new_minute, Utc::now());
    let next_run_str = next_run_at.to_rfc3339();

    let query = format!(
        "UPDATE kanban_schedule:`{validated}` SET days_of_week = $days, hour = $hour, minute = $minute, enabled = $enabled, skip_if_pending = $skip, next_run_at = <datetime> '{next_run_str}'"
    );
    db.execute_with_params(
        &query,
        vec![
            ("days".to_string(), json!(new_days)),
            ("hour".to_string(), json!(new_hour)),
            ("minute".to_string(), json!(new_minute)),
            ("enabled".to_string(), json!(new_enabled)),
            ("skip".to_string(), json!(new_skip)),
        ],
    )
    .await
    .map_err(|e| format!("Failed to update schedule: {}", e))?;
    get_kanban_schedule_core(db, &validated).await
}

pub async fn delete_kanban_schedule_core(db: &DBClient, id: &str) -> Result<(), String> {
    let validated = validate_uuid_field(id, "schedule_id")?;
    let query = format!("DELETE kanban_schedule:`{}`", validated);
    db.execute(&query)
        .await
        .map_err(|e| format!("Failed to delete schedule: {}", e))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Tauri wrappers
// ---------------------------------------------------------------------------

#[tauri::command]
#[instrument(name = "create_kanban_schedule", skip(state, config))]
pub async fn create_kanban_schedule(
    config: KanbanScheduleCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating kanban schedule");
    let schedule = create_kanban_schedule_core(&state.db, config).await?;
    Ok(schedule.id)
}

#[tauri::command]
#[instrument(name = "get_kanban_schedule", skip(state), fields(id = %id))]
pub async fn get_kanban_schedule(
    id: String,
    state: State<'_, AppState>,
) -> Result<KanbanSchedule, String> {
    get_kanban_schedule_core(&state.db, &id).await
}

#[tauri::command]
#[instrument(name = "list_kanban_schedules", skip(state))]
pub async fn list_kanban_schedules(
    state: State<'_, AppState>,
) -> Result<Vec<KanbanSchedule>, String> {
    list_kanban_schedules_core(&state.db).await
}

#[tauri::command]
#[instrument(name = "update_kanban_schedule", skip(state, config), fields(id = %id))]
pub async fn update_kanban_schedule(
    id: String,
    config: KanbanScheduleUpdate,
    state: State<'_, AppState>,
) -> Result<KanbanSchedule, String> {
    update_kanban_schedule_core(&state.db, &id, config).await
}

#[tauri::command]
#[instrument(name = "delete_kanban_schedule", skip(state), fields(id = %id))]
pub async fn delete_kanban_schedule(id: String, state: State<'_, AppState>) -> Result<(), String> {
    delete_kanban_schedule_core(&state.db, &id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_next_run_empty_days() {
        let now = Utc::now();
        let next = compute_next_run_at_in_tz(&[], 10, 0, now, &Utc);
        assert_eq!(next, now);
    }

    #[test]
    fn test_compute_next_run_picks_future_same_day() {
        // Sunday 2026-05-24 09:00 UTC, target Sunday 10:00 -> same day 10:00
        let now = Utc.with_ymd_and_hms(2026, 5, 24, 9, 0, 0).unwrap();
        // Sunday = 6 (num_days_from_monday)
        let next = compute_next_run_at_in_tz(&[6], 10, 0, now, &Utc);
        assert_eq!(next.hour(), 10);
        assert_eq!(next.day(), 24);
    }

    #[test]
    fn test_compute_next_run_rolls_to_next_week() {
        // Sunday 11:00, target Sunday 10:00 -> next Sunday
        let now = Utc.with_ymd_and_hms(2026, 5, 24, 11, 0, 0).unwrap();
        let next = compute_next_run_at_in_tz(&[6], 10, 0, now, &Utc);
        assert_eq!(next.day(), 31); // next Sunday
        assert_eq!(next.hour(), 10);
    }

    #[test]
    fn test_compute_next_run_picks_earliest_day_in_set() {
        // Friday 2026-05-22 13:00 UTC, target Mon (0) + Wed (2) at 09:00
        // Next match = Monday 2026-05-25 09:00
        let now = Utc.with_ymd_and_hms(2026, 5, 22, 13, 0, 0).unwrap();
        let next = compute_next_run_at_in_tz(&[0, 2], 9, 0, now, &Utc);
        assert_eq!(next.day(), 25);
        assert_eq!(next.weekday().num_days_from_monday(), 0);
    }

    /// Regression for the timezone bug: a user in CEST (UTC+2) saving "14:46"
    /// expects the schedule to fire at 14:46 in their local clock, not 14:46
    /// UTC (which is 16:46 CEST — two hours late).
    #[test]
    fn test_compute_next_run_respects_local_timezone() {
        use chrono::FixedOffset;
        // Fixed +02:00 offset simulates summer-time Paris.
        let cest = FixedOffset::east_opt(2 * 3600).unwrap();
        // Friday 2026-05-22 12:00 UTC == 14:00 CEST. User schedule = "Fri 14:46".
        // The next fire should be 14:46 CEST = 12:46 UTC same day.
        let now = Utc.with_ymd_and_hms(2026, 5, 22, 12, 0, 0).unwrap();
        let next = compute_next_run_at_in_tz(&[4], 14, 46, now, &cest);
        // Friday = 4 (num_days_from_monday)
        let expected = Utc.with_ymd_and_hms(2026, 5, 22, 12, 46, 0).unwrap();
        assert_eq!(next, expected);
    }
}
