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

//! Persistence of speech-to-text settings (`settings:stt`).
//!
//! Stored as a single JSON blob inside `settings:stt.config`, mirroring
//! the `settings:validation` / `settings:embedding_config` convention.

use crate::db::DBClient;
use crate::models::stt::{
    validate_voxtral_model_id, STTSettings, UpdateSTTSettingsRequest, MAX_CONTEXT_BIAS_ENTRIES,
    MAX_CONTEXT_BIAS_ENTRY_LEN, SUPPORTED_LANGUAGES,
};
use crate::security::serialize_for_query;
use crate::state::AppState;
use chrono::Utc;
use tauri::State;
use tracing::{error, info, instrument, warn};

const STT_RECORD_QUERY: &str = "SELECT config FROM settings:`settings:stt`";

/// Fetches the persisted STT settings, falling back to defaults when the
/// row is missing or unparseable.
async fn load_stt_settings(db: &DBClient) -> Result<STTSettings, String> {
    let results: Vec<serde_json::Value> = db.query_json(STT_RECORD_QUERY).await.map_err(|e| {
        error!(error = %e, "Failed to query STT settings");
        format!("Failed to query STT settings: {}", e)
    })?;

    if let Some(first) = results.first() {
        if let Some(config) = first.get("config") {
            if !config.is_null() {
                match serde_json::from_value::<STTSettings>(config.clone()) {
                    Ok(settings) => return Ok(settings),
                    Err(e) => {
                        warn!(error = %e, "Failed to parse stored STT settings, using defaults");
                    }
                }
            }
        }
    }

    Ok(STTSettings::default())
}

async fn persist_stt_settings(db: &DBClient, settings: &STTSettings) -> Result<(), String> {
    let json_config = serialize_for_query(settings, "stt settings")?;
    let upsert = format!(
        "UPSERT settings:`settings:stt` CONTENT {{ id: 'settings:stt', config: {} }}",
        json_config
    );
    db.execute(&upsert).await.map_err(|e| {
        error!(error = %e, "Failed to save STT settings");
        format!("Failed to save STT settings: {}", e)
    })?;
    Ok(())
}

/// Applies a partial update onto current settings. Pure function so the
/// validation paths can be tested without a database.
fn apply_update(current: &mut STTSettings, update: UpdateSTTSettingsRequest) -> Result<(), String> {
    if let Some(enabled) = update.enabled {
        current.enabled = enabled;
    }

    if let Some(model_id) = update.model_id {
        validate_voxtral_model_id(&model_id)?;
        current.model_id = model_id.trim().to_string();
    }

    if let Some(bias) = update.context_bias {
        if bias.len() > MAX_CONTEXT_BIAS_ENTRIES {
            return Err(format!(
                "context_bias exceeds {} entries",
                MAX_CONTEXT_BIAS_ENTRIES
            ));
        }
        let mut cleaned = Vec::with_capacity(bias.len());
        for entry in bias {
            let trimmed = entry.trim();
            if trimmed.is_empty() {
                continue;
            }
            if trimmed.len() > MAX_CONTEXT_BIAS_ENTRY_LEN {
                return Err(format!(
                    "context_bias entry exceeds {} characters",
                    MAX_CONTEXT_BIAS_ENTRY_LEN
                ));
            }
            if trimmed.chars().any(|c| c.is_control()) {
                return Err("context_bias entries must not contain control characters".to_string());
            }
            cleaned.push(trimmed.to_string());
        }
        current.context_bias = cleaned;
    }

    if let Some(language_outer) = update.language {
        match language_outer {
            None => current.language = None,
            Some(lang) => {
                let normalized = lang.trim().to_lowercase();
                if !SUPPORTED_LANGUAGES.iter().any(|s| *s == normalized) {
                    return Err(format!(
                        "Unsupported language '{}'. Use one of: {}",
                        lang,
                        SUPPORTED_LANGUAGES.join(", ")
                    ));
                }
                current.language = Some(normalized);
            }
        }
    }

    current.updated_at = Utc::now();
    Ok(())
}

/// Returns the persisted STT settings, or defaults when none are stored.
#[tauri::command]
#[instrument(name = "get_stt_settings", skip(state))]
pub async fn get_stt_settings(state: State<'_, AppState>) -> Result<STTSettings, String> {
    info!("Loading STT settings");
    load_stt_settings(&state.db).await
}

/// Applies a partial update and persists the result.
#[tauri::command]
#[instrument(name = "update_stt_settings", skip(state, config))]
pub async fn update_stt_settings(
    config: UpdateSTTSettingsRequest,
    state: State<'_, AppState>,
) -> Result<STTSettings, String> {
    info!("Updating STT settings");
    let mut current = load_stt_settings(&state.db).await?;
    apply_update(&mut current, config)?;
    persist_stt_settings(&state.db, &current).await?;
    Ok(current)
}

/// Restores defaults and persists them.
#[tauri::command]
#[instrument(name = "reset_stt_settings", skip(state))]
pub async fn reset_stt_settings(state: State<'_, AppState>) -> Result<STTSettings, String> {
    info!("Resetting STT settings to defaults");
    let settings = STTSettings::default();
    persist_stt_settings(&state.db, &settings).await?;
    Ok(settings)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_update() -> UpdateSTTSettingsRequest {
        UpdateSTTSettingsRequest::default()
    }

    #[test]
    fn empty_update_only_bumps_timestamp() {
        let mut s = STTSettings::default();
        let before = s.updated_at;
        std::thread::sleep(std::time::Duration::from_millis(2));
        apply_update(&mut s, empty_update()).unwrap();
        assert!(s.updated_at > before);
        assert!(!s.enabled);
    }

    #[test]
    fn toggle_enabled_flag() {
        let mut s = STTSettings::default();
        apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                enabled: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.enabled);
    }

    #[test]
    fn valid_model_id_is_accepted() {
        let mut s = STTSettings::default();
        apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                model_id: Some("voxtral-small-2507".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(s.model_id, "voxtral-small-2507");
    }

    #[test]
    fn invalid_model_id_is_rejected() {
        let mut s = STTSettings::default();
        let err = apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                model_id: Some("whisper-1".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_lowercase().contains("voxtral"));
    }

    #[test]
    fn realtime_model_id_is_rejected() {
        let mut s = STTSettings::default();
        let err = apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                model_id: Some("voxtral-mini-realtime-2602".to_string()),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_lowercase().contains("realtime"));
    }

    #[test]
    fn context_bias_is_trimmed_and_filtered() {
        let mut s = STTSettings::default();
        apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                context_bias: Some(vec![
                    "  Zileo  ".to_string(),
                    "".to_string(),
                    "SurrealDB".to_string(),
                ]),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            s.context_bias,
            vec!["Zileo".to_string(), "SurrealDB".to_string()]
        );
    }

    #[test]
    fn context_bias_rejects_too_many_entries() {
        let mut s = STTSettings::default();
        let bias: Vec<String> = (0..=MAX_CONTEXT_BIAS_ENTRIES)
            .map(|i| format!("hint{}", i))
            .collect();
        let err = apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                context_bias: Some(bias),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("context_bias"));
    }

    #[test]
    fn context_bias_rejects_control_chars() {
        let mut s = STTSettings::default();
        let err = apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                context_bias: Some(vec!["bad\nentry".to_string()]),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.to_lowercase().contains("control"));
    }

    #[test]
    fn language_set_then_cleared_via_tri_state() {
        let mut s = STTSettings::default();
        // Set
        apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                language: Some(Some("FR".to_string())),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(s.language.as_deref(), Some("fr"));
        // Clear
        apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                language: Some(None),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(s.language.is_none());
    }

    #[test]
    fn unsupported_language_is_rejected() {
        let mut s = STTSettings::default();
        let err = apply_update(
            &mut s,
            UpdateSTTSettingsRequest {
                language: Some(Some("xx".to_string())),
                ..Default::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("language") || err.to_lowercase().contains("unsupported"));
    }
}
