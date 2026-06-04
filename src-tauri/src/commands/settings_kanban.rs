// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Persistence of Kanban settings (`settings:kanban`).
//!
//! Stored as a single JSON blob inside `settings:kanban.config`, mirroring the
//! `settings:stt` convention. For now the only setting is the configurable
//! wall-clock ceiling for a detached card compose run (`compose_timeout_secs`).

use crate::constants::compose::{
    COMPOSE_TIMEOUT_DEFAULT_SECS, COMPOSE_TIMEOUT_MAX_SECS, COMPOSE_TIMEOUT_MIN_SECS,
};
use crate::db::DBClient;
use crate::security::serialize_for_query;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, instrument, warn};

const KANBAN_RECORD_QUERY: &str = "SELECT config FROM settings:`settings:kanban`";

/// Persisted Kanban settings.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanSettings {
    /// Hard wall-clock ceiling (seconds) for a single detached compose run.
    /// Clamped to `[COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS]`.
    pub compose_timeout_secs: u64,
}

impl Default for KanbanSettings {
    fn default() -> Self {
        Self {
            compose_timeout_secs: COMPOSE_TIMEOUT_DEFAULT_SECS,
        }
    }
}

/// Partial update payload — only provided fields are applied.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKanbanSettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_timeout_secs: Option<u64>,
}

/// Applies a partial update onto current settings, clamping the compose timeout
/// to its bounds. Pure function so the clamp logic can be tested without a
/// database.
fn apply_update(current: &mut KanbanSettings, update: UpdateKanbanSettingsRequest) {
    if let Some(secs) = update.compose_timeout_secs {
        current.compose_timeout_secs =
            secs.clamp(COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS);
    }
}

/// Resolves the effective compose timeout (seconds) from a loaded settings
/// result.
///
/// - `Ok(settings)` → its `compose_timeout_secs`, clamped to the bounds as a
///   defense-in-depth measure.
/// - `Err(_)` → the (raised) default, so a transient settings-load failure
///   never silently shortens the compose ceiling.
///
/// Pure so the compose integration path is unit-testable without a DB.
pub fn effective_compose_timeout(loaded: Result<KanbanSettings, String>) -> u64 {
    match loaded {
        Ok(settings) => settings
            .compose_timeout_secs
            .clamp(COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS),
        Err(_) => COMPOSE_TIMEOUT_DEFAULT_SECS,
    }
}

/// Fetches the persisted Kanban settings, falling back to defaults when the row
/// is missing or unparseable. The returned compose timeout is clamped to its
/// bounds as a defense-in-depth measure (a row hand-edited out of range is
/// repaired on read).
pub async fn load_kanban_settings(db: &DBClient) -> Result<KanbanSettings, String> {
    let results: Vec<serde_json::Value> =
        db.query_json(KANBAN_RECORD_QUERY).await.map_err(|e| {
            error!(error = %e, "Failed to query Kanban settings");
            format!("Failed to query Kanban settings: {}", e)
        })?;

    let mut settings = KanbanSettings::default();
    if let Some(first) = results.first() {
        if let Some(config) = first.get("config") {
            if !config.is_null() {
                match serde_json::from_value::<KanbanSettings>(config.clone()) {
                    Ok(parsed) => settings = parsed,
                    Err(e) => {
                        warn!(error = %e, "Failed to parse stored Kanban settings, using defaults");
                    }
                }
            }
        }
    }

    settings.compose_timeout_secs = settings
        .compose_timeout_secs
        .clamp(COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS);
    Ok(settings)
}

async fn persist_kanban_settings(db: &DBClient, settings: &KanbanSettings) -> Result<(), String> {
    let json_config = serialize_for_query(settings, "kanban settings")?;
    let upsert = format!(
        "UPSERT settings:`settings:kanban` CONTENT {{ id: 'settings:kanban', config: {} }}",
        json_config
    );
    db.execute(&upsert).await.map_err(|e| {
        error!(error = %e, "Failed to save Kanban settings");
        format!("Failed to save Kanban settings: {}", e)
    })?;
    Ok(())
}

/// Returns the persisted Kanban settings, or defaults when none are stored.
#[tauri::command]
#[instrument(name = "get_kanban_settings", skip(state))]
pub async fn get_kanban_settings(state: State<'_, AppState>) -> Result<KanbanSettings, String> {
    info!("Loading Kanban settings");
    load_kanban_settings(&state.db).await
}

/// Applies a partial update and persists the (clamped) result.
#[tauri::command]
#[instrument(name = "update_kanban_settings", skip(state, request))]
pub async fn update_kanban_settings(
    request: UpdateKanbanSettingsRequest,
    state: State<'_, AppState>,
) -> Result<KanbanSettings, String> {
    info!("Updating Kanban settings");
    let mut current = load_kanban_settings(&state.db).await?;
    apply_update(&mut current, request);
    persist_kanban_settings(&state.db, &current).await?;
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_uses_compose_timeout_default() {
        assert_eq!(
            KanbanSettings::default().compose_timeout_secs,
            COMPOSE_TIMEOUT_DEFAULT_SECS
        );
    }

    #[test]
    fn empty_update_is_noop() {
        let mut s = KanbanSettings::default();
        apply_update(&mut s, UpdateKanbanSettingsRequest::default());
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_DEFAULT_SECS);
    }

    #[test]
    fn valid_value_is_applied() {
        let mut s = KanbanSettings::default();
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(900),
            },
        );
        assert_eq!(s.compose_timeout_secs, 900);
    }

    #[test]
    fn below_min_is_clamped_up() {
        let mut s = KanbanSettings::default();
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(1),
            },
        );
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_MIN_SECS);
    }

    #[test]
    fn above_max_is_clamped_down() {
        let mut s = KanbanSettings::default();
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(100_000),
            },
        );
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_MAX_SECS);
    }

    #[test]
    fn boundary_values_are_accepted() {
        let mut s = KanbanSettings::default();
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(COMPOSE_TIMEOUT_MIN_SECS),
            },
        );
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_MIN_SECS);
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(COMPOSE_TIMEOUT_MAX_SECS),
            },
        );
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_MAX_SECS);
    }

    #[test]
    fn update_request_deserializes_camel_case() {
        let req: UpdateKanbanSettingsRequest =
            serde_json::from_str(r#"{"composeTimeoutSecs":720}"#).unwrap();
        assert_eq!(req.compose_timeout_secs, Some(720));

        // Absent field -> None (leave as-is).
        let req: UpdateKanbanSettingsRequest = serde_json::from_str("{}").unwrap();
        assert!(req.compose_timeout_secs.is_none());
    }

    #[test]
    fn settings_serialize_camel_case() {
        let json = serde_json::to_string(&KanbanSettings {
            compose_timeout_secs: 600,
        })
        .unwrap();
        assert!(json.contains("composeTimeoutSecs"));
    }

    // -------- effective_compose_timeout (compose integration path) --------

    #[test]
    fn effective_timeout_ok_in_range_is_kept() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 900,
            })),
            900
        );
    }

    #[test]
    fn effective_timeout_ok_above_max_is_clamped() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 5000,
            })),
            COMPOSE_TIMEOUT_MAX_SECS
        );
    }

    #[test]
    fn effective_timeout_ok_below_min_is_clamped() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 0,
            })),
            COMPOSE_TIMEOUT_MIN_SECS
        );
    }

    #[test]
    fn effective_timeout_err_falls_back_to_default() {
        assert_eq!(
            effective_compose_timeout(Err("db unavailable".to_string())),
            COMPOSE_TIMEOUT_DEFAULT_SECS
        );
    }
}
