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

//! Persistence of MCP network connectivity settings (`settings:mcp_network`).
//!
//! Stored as a single JSON blob inside `settings:mcp_network.config`, mirroring
//! the `settings:stt` convention. The persisted blob seeds the process-global
//! snapshot in [`crate::mcp::network_settings`] which `MCPHttpHandle::connect()`
//! reads (it has no DB handle).

use crate::db::DBClient;
use crate::mcp::network_settings::{set_network_settings, McpNetworkSettings};
use crate::security::serialize_for_query;
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;
use tracing::{error, info, instrument, warn};

const MCP_NETWORK_RECORD_QUERY: &str = "SELECT config FROM settings:`settings:mcp_network`";

/// Partial update payload — only provided fields are applied.
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateMcpNetworkSettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow_private_network: Option<bool>,
}

/// Applies a partial update onto current settings. Pure function so the
/// merge logic can be tested without a database.
fn apply_update(current: &mut McpNetworkSettings, update: UpdateMcpNetworkSettingsRequest) {
    if let Some(allow) = update.allow_private_network {
        current.allow_private_network = allow;
    }
}

/// Fetches the persisted MCP network settings, falling back to defaults when
/// the row is missing or unparseable (fail-secure: defaults are strict).
pub async fn load_mcp_network_settings(db: &DBClient) -> Result<McpNetworkSettings, String> {
    let results: Vec<serde_json::Value> =
        db.query_json(MCP_NETWORK_RECORD_QUERY).await.map_err(|e| {
            error!(error = %e, "Failed to query MCP network settings");
            format!("Failed to query MCP network settings: {}", e)
        })?;

    if let Some(first) = results.first() {
        if let Some(config) = first.get("config") {
            if !config.is_null() {
                match serde_json::from_value::<McpNetworkSettings>(config.clone()) {
                    Ok(settings) => return Ok(settings),
                    Err(e) => {
                        warn!(error = %e, "Failed to parse stored MCP network settings, using defaults");
                    }
                }
            }
        }
    }

    Ok(McpNetworkSettings::default())
}

async fn persist_mcp_network_settings(
    db: &DBClient,
    settings: &McpNetworkSettings,
) -> Result<(), String> {
    let json_config = serialize_for_query(settings, "mcp network settings")?;
    let upsert = format!(
        "UPSERT settings:`settings:mcp_network` CONTENT {{ id: 'settings:mcp_network', config: {} }}",
        json_config
    );
    db.execute(&upsert).await.map_err(|e| {
        error!(error = %e, "Failed to save MCP network settings");
        format!("Failed to save MCP network settings: {}", e)
    })?;
    Ok(())
}

/// Returns the persisted MCP network settings, or strict defaults when none
/// are stored.
#[tauri::command]
#[instrument(name = "get_mcp_network_settings", skip(state))]
pub async fn get_mcp_network_settings(
    state: State<'_, AppState>,
) -> Result<McpNetworkSettings, String> {
    info!("Loading MCP network settings");
    load_mcp_network_settings(&state.db).await
}

/// Applies a partial update, persists it, and (only on a successful persist)
/// refreshes the process-global snapshot so the next connect honours it.
///
/// Fail-secure ordering: the DB write happens BEFORE the in-memory snapshot is
/// updated. If the persist fails, the global is left untouched (the downgrade
/// is not activated) and the error is surfaced.
#[tauri::command]
#[instrument(name = "update_mcp_network_settings", skip(state, request))]
pub async fn update_mcp_network_settings(
    request: UpdateMcpNetworkSettingsRequest,
    state: State<'_, AppState>,
) -> Result<McpNetworkSettings, String> {
    info!("Updating MCP network settings");
    let mut current = load_mcp_network_settings(&state.db).await?;
    apply_update(&mut current, request);

    // Persist FIRST. Only mirror into the process-global snapshot once the
    // durable write succeeded — a failed persist must not activate the relaxed
    // policy in memory.
    persist_mcp_network_settings(&state.db, &current).await?;
    set_network_settings(current);

    if current.allow_private_network {
        warn!("MCP private-network (LAN) access enabled by user");
    } else {
        info!("MCP private-network (LAN) access disabled");
    }

    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_update_is_noop() {
        let mut s = McpNetworkSettings::default();
        apply_update(&mut s, UpdateMcpNetworkSettingsRequest::default());
        assert!(!s.allow_private_network);
    }

    #[test]
    fn toggle_allow_private_network_on() {
        let mut s = McpNetworkSettings::default();
        apply_update(
            &mut s,
            UpdateMcpNetworkSettingsRequest {
                allow_private_network: Some(true),
            },
        );
        assert!(s.allow_private_network);
    }

    #[test]
    fn toggle_allow_private_network_off() {
        let mut s = McpNetworkSettings {
            allow_private_network: true,
        };
        apply_update(
            &mut s,
            UpdateMcpNetworkSettingsRequest {
                allow_private_network: Some(false),
            },
        );
        assert!(!s.allow_private_network);
    }

    #[test]
    fn update_request_deserializes_camel_case() {
        let req: UpdateMcpNetworkSettingsRequest =
            serde_json::from_str(r#"{"allowPrivateNetwork":true}"#).unwrap();
        assert_eq!(req.allow_private_network, Some(true));

        // Absent field -> None (leave as-is).
        let req: UpdateMcpNetworkSettingsRequest = serde_json::from_str("{}").unwrap();
        assert!(req.allow_private_network.is_none());
    }
}
