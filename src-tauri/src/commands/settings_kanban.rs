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
use crate::models::agent::deserialize_explicit_option;
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, instrument, warn};

const KANBAN_RECORD_QUERY: &str = "SELECT config FROM settings:`settings:kanban`";

/// Persisted Kanban settings.
///
/// `Copy` is intentionally not derived: the optional supervisor ids are
/// `String`-backed, so the struct is `Clone` only.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KanbanSettings {
    /// Hard wall-clock ceiling (seconds) for a single detached compose run.
    /// Clamped to `[COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS]`.
    pub compose_timeout_secs: u64,
    /// Global supervisor agent for the compose flow. `None` (or absent) falls
    /// back to the agent passed by the card creator (legacy behaviour).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_agent_id: Option<String>,
    /// Global supervisor agent for the analyze flow. `None` (or absent) falls
    /// back to the card's own `kanban_agent_id`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub analyze_agent_id: Option<String>,
}

impl Default for KanbanSettings {
    fn default() -> Self {
        Self {
            compose_timeout_secs: COMPOSE_TIMEOUT_DEFAULT_SECS,
            compose_agent_id: None,
            analyze_agent_id: None,
        }
    }
}

/// Partial update payload — only provided fields are applied.
///
/// The agent ids use the tri-state PATCH semantic (absent = leave as-is, `null`
/// = clear back to fallback, value = set). `deserialize_explicit_option`
/// preserves the absent-vs-null distinction that serde would otherwise collapse.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateKanbanSettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compose_timeout_secs: Option<u64>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub compose_agent_id: Option<Option<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_explicit_option"
    )]
    pub analyze_agent_id: Option<Option<String>>,
}

/// Normalizes a supervisor id: a trimmed, non-empty string is kept; an empty or
/// whitespace-only string is treated as "clear" (`None`).
fn normalize_agent_id(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Applies a partial update onto current settings, clamping the compose timeout
/// to its bounds and applying the tri-state semantic to the supervisor ids
/// (absent = leave, `null`/empty = clear, value = set). Pure function so the
/// logic can be tested without a database.
fn apply_update(current: &mut KanbanSettings, update: UpdateKanbanSettingsRequest) {
    if let Some(secs) = update.compose_timeout_secs {
        current.compose_timeout_secs =
            secs.clamp(COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS);
    }
    if let Some(maybe_id) = update.compose_agent_id {
        current.compose_agent_id = maybe_id.and_then(normalize_agent_id);
    }
    if let Some(maybe_id) = update.analyze_agent_id {
        current.analyze_agent_id = maybe_id.and_then(normalize_agent_id);
    }
}

/// Returns whether the agent still exists AND is Kanban-kind. Shared by the
/// compose re-validation and the role-resolution helper so the
/// "configured agent disappeared / changed kind" check lives in one place.
pub(crate) async fn kanban_agent_exists(db: &DBClient, agent_id: &str) -> Result<bool, String> {
    let validated = validate_uuid_field(agent_id, "kanban_agent_id")?;
    let q = format!(
        "SELECT meta::id(id) AS id FROM agent:`{}` WHERE kind = 'kanban'",
        validated
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to re-check Kanban agent: {}", e))?;
    Ok(!rows.is_empty())
}

/// Resolves the effective agent id for a Kanban role (compose / analyze) with a
/// graceful fallback.
///
/// The `configured` id wins only when it is set (non-empty after trim) AND the
/// agent still exists AND is still Kanban-kind. Otherwise — absent, blank,
/// deleted, or demoted to a non-kanban kind — the `fallback` is used (the agent
/// passed by the creator for compose, or the card's own `kanban_agent_id` for
/// analyze). A lookup error also yields the fallback so a transient DB hiccup
/// never breaks the flow.
pub(crate) async fn resolve_role_agent_id(
    db: &DBClient,
    configured: Option<&str>,
    fallback: &str,
) -> String {
    if let Some(id) = configured.map(str::trim).filter(|s| !s.is_empty()) {
        if kanban_agent_exists(db, id).await.unwrap_or(false) {
            return id.to_string();
        }
    }
    fallback.to_string()
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

/// Validates a supervisor id that is being SET (tri-state `Some(Some(value))`
/// with a non-blank value): the UUID must be well-formed and the agent must
/// exist AND be Kanban-kind. Defense-in-depth — the runtime resolution still
/// falls back gracefully if the agent disappears later. Absent / cleared / blank
/// ids are skipped (no row to check).
async fn validate_role_agent_update(
    db: &DBClient,
    field: &str,
    value: &Option<Option<String>>,
) -> Result<(), String> {
    let Some(Some(raw)) = value else {
        return Ok(());
    };
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(());
    }
    validate_uuid_field(trimmed, field)?;
    if !kanban_agent_exists(db, trimmed).await? {
        return Err(format!(
            "{} must reference an existing Kanban-kind agent",
            field
        ));
    }
    Ok(())
}

/// Applies a partial update and persists the (clamped) result.
#[tauri::command]
#[instrument(name = "update_kanban_settings", skip(state, request))]
pub async fn update_kanban_settings(
    request: UpdateKanbanSettingsRequest,
    state: State<'_, AppState>,
) -> Result<KanbanSettings, String> {
    info!("Updating Kanban settings");
    // Validate any SET ids before mutating/persisting (fail-closed): a value
    // that does not resolve to an existing Kanban agent is rejected here rather
    // than silently stored and fallen-back-from at runtime.
    validate_role_agent_update(&state.db, "composeAgentId", &request.compose_agent_id).await?;
    validate_role_agent_update(&state.db, "analyzeAgentId", &request.analyze_agent_id).await?;

    let mut current = load_kanban_settings(&state.db).await?;
    apply_update(&mut current, request);
    persist_kanban_settings(&state.db, &current).await?;
    Ok(current)
}

/// Loads the bare `system_prompt` of a Kanban-kind agent for the read-only
/// prompt preview. Deliberately light: it does NOT require a configured LLM
/// model (unlike `load_kanban_agent_config`), so an agent that has not picked a
/// model yet can still be previewed. Validates that the agent exists and is
/// Kanban-kind.
async fn load_agent_system_prompt(db: &DBClient, agent_id: &str) -> Result<String, String> {
    let validated = validate_uuid_field(agent_id, "agent_id")?;
    let q = format!("SELECT system_prompt, kind FROM agent:`{}`", validated);
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to load agent for preview: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Agent not found: {}", validated))?;
    if row["kind"].as_str() != Some("kanban") {
        return Err(format!("Agent {} is not a Kanban-kind agent", validated));
    }
    Ok(row["system_prompt"].as_str().unwrap_or("").to_string())
}

/// Returns the effective system prompt the supervisor agent would run with for a
/// given role, so the settings UI can show a read-only preview without
/// duplicating the prompt-building text on the frontend.
///
/// `mode` is `"compose"` or `"analyze"`; any other value is rejected. The result
/// is the agent's own `system_prompt` prefixed with the production role block
/// (`build_compose_system_prompt` / `build_analyze_system_prompt`).
#[tauri::command]
#[instrument(name = "preview_kanban_role_prompt", skip(state))]
pub async fn preview_kanban_role_prompt(
    agent_id: String,
    mode: String,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let system_prompt = load_agent_system_prompt(&state.db, &agent_id).await?;
    match mode.as_str() {
        "compose" => Ok(crate::commands::compose_card::build_compose_system_prompt(
            &system_prompt,
        )),
        "analyze" => {
            Ok(crate::commands::kanban_analyzer::build_analyze_system_prompt(&system_prompt))
        }
        other => Err(format!("Unknown preview mode: {}", other)),
    }
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
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
                ..Default::default()
            },
        );
        assert_eq!(s.compose_timeout_secs, COMPOSE_TIMEOUT_MIN_SECS);
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_timeout_secs: Some(COMPOSE_TIMEOUT_MAX_SECS),
                ..Default::default()
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
            ..Default::default()
        })
        .unwrap();
        assert!(json.contains("composeTimeoutSecs"));
    }

    #[test]
    fn settings_skip_serializing_absent_agent_ids() {
        // Default (both ids None) must NOT emit the agent id keys.
        let json = serde_json::to_string(&KanbanSettings::default()).unwrap();
        assert!(!json.contains("composeAgentId"));
        assert!(!json.contains("analyzeAgentId"));
    }

    #[test]
    fn settings_serialize_agent_ids_when_set() {
        let json = serde_json::to_string(&KanbanSettings {
            compose_timeout_secs: 600,
            compose_agent_id: Some("a".to_string()),
            analyze_agent_id: Some("b".to_string()),
        })
        .unwrap();
        assert!(json.contains("composeAgentId"));
        assert!(json.contains("analyzeAgentId"));
    }

    // -------- tri-state agent id update (apply_update) --------

    #[test]
    fn absent_agent_id_leaves_current() {
        let mut s = KanbanSettings {
            compose_agent_id: Some("keep".to_string()),
            ..Default::default()
        };
        // Outer None == field absent from the JSON payload.
        apply_update(&mut s, UpdateKanbanSettingsRequest::default());
        assert_eq!(s.compose_agent_id.as_deref(), Some("keep"));
    }

    #[test]
    fn null_agent_id_clears_current() {
        let mut s = KanbanSettings {
            analyze_agent_id: Some("old".to_string()),
            ..Default::default()
        };
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                analyze_agent_id: Some(None),
                ..Default::default()
            },
        );
        assert!(s.analyze_agent_id.is_none());
    }

    #[test]
    fn value_agent_id_sets_current() {
        let mut s = KanbanSettings::default();
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_agent_id: Some(Some("new-id".to_string())),
                ..Default::default()
            },
        );
        assert_eq!(s.compose_agent_id.as_deref(), Some("new-id"));
    }

    #[test]
    fn blank_agent_id_is_normalized_to_clear() {
        let mut s = KanbanSettings {
            compose_agent_id: Some("old".to_string()),
            ..Default::default()
        };
        apply_update(
            &mut s,
            UpdateKanbanSettingsRequest {
                compose_agent_id: Some(Some("   ".to_string())),
                ..Default::default()
            },
        );
        assert!(
            s.compose_agent_id.is_none(),
            "a whitespace-only id must clear the setting"
        );
    }

    #[test]
    fn update_request_distinguishes_absent_null_value() {
        // Absent -> outer None.
        let req: UpdateKanbanSettingsRequest = serde_json::from_str("{}").unwrap();
        assert!(req.compose_agent_id.is_none());

        // Explicit null -> Some(None) (clear).
        let req: UpdateKanbanSettingsRequest =
            serde_json::from_str(r#"{"composeAgentId":null}"#).unwrap();
        assert_eq!(req.compose_agent_id, Some(None));

        // Value -> Some(Some(value)) (set).
        let req: UpdateKanbanSettingsRequest =
            serde_json::from_str(r#"{"analyzeAgentId":"abc"}"#).unwrap();
        assert_eq!(req.analyze_agent_id, Some(Some("abc".to_string())));
    }

    // -------- effective_compose_timeout (compose integration path) --------

    #[test]
    fn effective_timeout_ok_in_range_is_kept() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 900,
                ..Default::default()
            })),
            900
        );
    }

    #[test]
    fn effective_timeout_ok_above_max_is_clamped() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 5000,
                ..Default::default()
            })),
            COMPOSE_TIMEOUT_MAX_SECS
        );
    }

    #[test]
    fn effective_timeout_ok_below_min_is_clamped() {
        assert_eq!(
            effective_compose_timeout(Ok(KanbanSettings {
                compose_timeout_secs: 0,
                ..Default::default()
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

    // -------- DB-aware resolution + validation + preview --------

    use crate::test_utils::setup_test_state;

    /// Seeds a minimal SCHEMAFULL-valid agent with the given `kind`
    /// (`'kanban'`, `'standard'`, or `NONE`) and `system_prompt`.
    async fn seed_agent(db: &DBClient, id: &str, kind_sql: &str, system_prompt: &str) {
        let sp = serde_json::to_string(system_prompt).unwrap();
        let q = format!(
            "CREATE agent:`{id}` SET \
                id = '{id}', name = 'agent-{id}', lifecycle = 'permanent', \
                llm = {{ provider: 'mistral', model: 'mistral-medium', \
                         temperature: 0.7, max_tokens: 4000 }}, \
                tools = [], mcp_servers = [], system_prompt = {sp}, \
                max_tool_iterations = 50, reasoning_effort = NONE, kind = {kind_sql}, \
                created_at = time::now(), updated_at = time::now()"
        );
        db.execute(&q).await.unwrap();
    }

    #[tokio::test]
    async fn resolve_uses_configured_when_existing_kanban() {
        let (state, _g) = setup_test_state().await;
        let configured = uuid::Uuid::new_v4().to_string();
        let fallback = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &configured, "'kanban'", "P").await;

        let effective = resolve_role_agent_id(&state.db, Some(&configured), &fallback).await;
        assert_eq!(effective, configured);
    }

    #[tokio::test]
    async fn resolve_falls_back_when_absent() {
        let (state, _g) = setup_test_state().await;
        let fallback = uuid::Uuid::new_v4().to_string();

        assert_eq!(
            resolve_role_agent_id(&state.db, None, &fallback).await,
            fallback
        );
        // Blank configured id also falls back.
        assert_eq!(
            resolve_role_agent_id(&state.db, Some("   "), &fallback).await,
            fallback
        );
    }

    #[tokio::test]
    async fn resolve_falls_back_when_configured_deleted() {
        let (state, _g) = setup_test_state().await;
        // A well-formed UUID that was never inserted (deleted agent).
        let configured = uuid::Uuid::new_v4().to_string();
        let fallback = uuid::Uuid::new_v4().to_string();

        assert_eq!(
            resolve_role_agent_id(&state.db, Some(&configured), &fallback).await,
            fallback
        );
    }

    #[tokio::test]
    async fn resolve_falls_back_when_configured_not_kanban() {
        let (state, _g) = setup_test_state().await;
        let configured = uuid::Uuid::new_v4().to_string();
        let fallback = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &configured, "'standard'", "P").await;

        assert_eq!(
            resolve_role_agent_id(&state.db, Some(&configured), &fallback).await,
            fallback
        );
    }

    #[tokio::test]
    async fn update_rejects_non_kanban_agent_id() {
        let (state, _g) = setup_test_state().await;
        let agent = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &agent, "'standard'", "P").await;

        let err =
            validate_role_agent_update(&state.db, "composeAgentId", &Some(Some(agent.clone())))
                .await
                .expect_err("a non-kanban agent must be rejected");
        assert!(err.contains("composeAgentId"), "got: {err}");
    }

    #[tokio::test]
    async fn update_accepts_kanban_agent_and_persists() {
        let (state, _g) = setup_test_state().await;
        let agent = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &agent, "'kanban'", "P").await;

        validate_role_agent_update(&state.db, "analyzeAgentId", &Some(Some(agent.clone())))
            .await
            .expect("a kanban agent must validate");

        // Cleared / absent ids skip validation entirely (no error).
        validate_role_agent_update(&state.db, "analyzeAgentId", &Some(None))
            .await
            .expect("clear is always valid");
        validate_role_agent_update(&state.db, "analyzeAgentId", &None)
            .await
            .expect("absent is always valid");
    }

    #[tokio::test]
    async fn preview_compose_and_analyze_contain_agent_prompt_and_block() {
        let (state, _g) = setup_test_state().await;
        let agent = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &agent, "'kanban'", "ROLE_MARKER").await;

        let compose = load_agent_system_prompt(&state.db, &agent).await.unwrap();
        let compose_prompt = crate::commands::compose_card::build_compose_system_prompt(&compose);
        assert!(compose_prompt.contains("ROLE_MARKER"));
        assert!(compose_prompt.contains("# Compose-card mode"));

        let analyze_prompt =
            crate::commands::kanban_analyzer::build_analyze_system_prompt(&compose);
        assert!(analyze_prompt.contains("ROLE_MARKER"));
        assert!(analyze_prompt.contains("# Report analysis mode"));
    }

    #[tokio::test]
    async fn preview_loader_rejects_non_kanban_agent() {
        let (state, _g) = setup_test_state().await;
        let agent = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &agent, "'standard'", "P").await;

        assert!(
            load_agent_system_prompt(&state.db, &agent).await.is_err(),
            "previewing a non-kanban agent must error"
        );
    }
}
