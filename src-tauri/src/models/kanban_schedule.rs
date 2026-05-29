// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Kanban schedule: recurrence rule for a kanban_card template.

use super::agent::deserialize_explicit_option;
use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Recurrence rule for instantiating a kanban_card on a regular cadence.
///
/// `days_of_week`: 0 = Monday, 6 = Sunday (ISO).
/// Empty `days_of_week` means the schedule is effectively idle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanSchedule {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub card_template_id: String,
    pub days_of_week: Vec<u8>,
    pub hour: u8,
    pub minute: u8,
    pub next_run_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_run_at: Option<DateTime<Utc>>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// When true, skip spawning a new card if a previous instance (column in
    /// todo/doing) is still pending. Prevents backlog when execution outlasts
    /// the recurrence cadence.
    #[serde(default)]
    pub skip_if_pending: bool,
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanScheduleCreate {
    pub card_template_id: String,
    pub days_of_week: Vec<u8>,
    pub hour: u8,
    pub minute: u8,
    #[serde(default)]
    pub skip_if_pending: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct KanbanScheduleUpdate {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days_of_week: Option<Vec<u8>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hour: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub minute: Option<u8>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub enabled: Option<Option<bool>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_if_pending: Option<bool>,
}

/// Validates a schedule create payload. Returns error if any field is out of range.
///
/// Reused by both create and update (the update path builds a
/// `KanbanScheduleCreate` from the merged fields) and by the ScheduleCard tool,
/// so the guards here cover every entry point.
pub fn validate_schedule_create(create: &KanbanScheduleCreate) -> Result<(), String> {
    // An empty `days_of_week` makes compute_next_run_at return `now`, so the
    // schedule fires every tick and spawns up to MAX_CATCHUP_PER_SCHEDULE cards
    // per minute, indefinitely (K7 DoS). A recurrence needs at least one day.
    if create.days_of_week.is_empty() {
        return Err("days_of_week must be non-empty".to_string());
    }
    if create.hour > 23 {
        return Err(format!("hour must be 0..=23, got {}", create.hour));
    }
    if create.minute > 59 {
        return Err(format!("minute must be 0..=59, got {}", create.minute));
    }
    for d in &create.days_of_week {
        if *d > 6 {
            return Err(format!("days_of_week must be 0..=6, got {}", d));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_schedule_ok() {
        let c = KanbanScheduleCreate {
            card_template_id: "c1".to_string(),
            days_of_week: vec![0, 2, 4],
            hour: 14,
            minute: 30,
            skip_if_pending: false,
        };
        assert!(validate_schedule_create(&c).is_ok());
    }

    #[test]
    fn test_validate_schedule_rejects_empty_days() {
        // K7: an empty days_of_week is a DoS vector (fires every tick) and must
        // be rejected at the validation boundary (covers create + update + tool).
        let c = KanbanScheduleCreate {
            card_template_id: "c1".to_string(),
            days_of_week: vec![],
            hour: 9,
            minute: 0,
            skip_if_pending: false,
        };
        let err = validate_schedule_create(&c).unwrap_err();
        assert!(err.contains("days_of_week"), "got: {err}");
    }

    #[test]
    fn test_validate_schedule_bad_hour() {
        let c = KanbanScheduleCreate {
            card_template_id: "c1".to_string(),
            days_of_week: vec![0],
            hour: 24,
            minute: 0,
            skip_if_pending: false,
        };
        assert!(validate_schedule_create(&c).is_err());
    }

    #[test]
    fn test_validate_schedule_bad_minute() {
        let c = KanbanScheduleCreate {
            card_template_id: "c1".to_string(),
            days_of_week: vec![0],
            hour: 1,
            minute: 60,
            skip_if_pending: false,
        };
        assert!(validate_schedule_create(&c).is_err());
    }

    #[test]
    fn test_validate_schedule_bad_day() {
        let c = KanbanScheduleCreate {
            card_template_id: "c1".to_string(),
            days_of_week: vec![7],
            hour: 1,
            minute: 0,
            skip_if_pending: false,
        };
        assert!(validate_schedule_create(&c).is_err());
    }
}
