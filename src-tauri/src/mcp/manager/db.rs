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

//! MCP Manager database operations
//!
//! Server configuration persistence, tool call logging.

use super::MCPManager;
use crate::db::sanitize_for_surrealdb;
use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::{MCPCallLogCreate, MCPServerConfig, MCPServerCreate};
use crate::security::Validator;
use tracing::{debug, info, warn};

impl MCPManager {
    /// Saves a server configuration to the database
    pub(crate) async fn save_server_config(&self, config: &MCPServerConfig) -> MCPResult<()> {
        let create_data = MCPServerCreate::from_config(config);

        debug!(
            server_id = %config.id,
            env_count = config.env.len(),
            env_keys = ?config.env.keys().collect::<Vec<_>>(),
            "Saving MCP server config to database"
        );

        self.db
            .create("mcp_server", &config.id, create_data)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "save server config".to_string(),
                message: e.to_string(),
            })?;

        info!(
            server_id = %config.id,
            env_count = config.env.len(),
            "Server configuration saved to database"
        );

        Ok(())
    }

    /// Updates a server configuration in the database
    pub async fn update_server_config(&self, config: &MCPServerConfig) -> MCPResult<()> {
        // Validate the server id before interpolating it into the UPDATE query.
        // Strict UUID v4 rejects backticks, null bytes and crafted ids.
        let server_id =
            Validator::validate_uuid(&config.id).map_err(|e| MCPError::InvalidConfig {
                field: "id".to_string(),
                reason: e.to_string(),
            })?;

        // Serialize each field to JSON for the query
        let name_json = serde_json::to_string(&config.name)?;
        let command_json = serde_json::to_string(&config.command)?;
        let args_json = serde_json::to_string(&config.args)?;
        // env is stored as a JSON string (to bypass SurrealDB SCHEMAFULL filtering)
        let env_str = serde_json::to_string(&config.env)?; // {"KEY":"value"}
        let env_json = serde_json::to_string(&env_str)?; // "{\"KEY\":\"value\"}"
        let description_json = match &config.description {
            Some(desc) => serde_json::to_string(desc)?,
            None => "NONE".to_string(),
        };

        // v1.2 auth fields - persisted as JSON strings (ERR_SURREAL_001).
        // `MCPAuthType::None` is stored as NONE so the DB stays clean for
        // servers without authentication (no auth_type / no auth_metadata).
        let auth_type_json = match config.auth_type {
            Some(t) if t != crate::models::mcp::MCPAuthType::None => {
                let s = serde_json::to_value(t)?
                    .as_str()
                    .map(String::from)
                    .ok_or_else(|| MCPError::DatabaseError {
                        context: "update server config".to_string(),
                        message: "auth_type serialization produced non-string".to_string(),
                    })?;
                serde_json::to_string(&s)?
            }
            _ => "NONE".to_string(),
        };

        let auth_metadata_json = match config
            .auth_metadata
            .as_ref()
            .filter(|m| m.header_name.is_some() || m.username.is_some())
        {
            Some(m) => {
                let inner = serde_json::to_string(m)?;
                serde_json::to_string(&inner)?
            }
            None => "NONE".to_string(),
        };

        let extra_headers_json = match config.extra_headers.as_ref().filter(|h| !h.is_empty()) {
            Some(h) => {
                let inner = serde_json::to_string(h)?;
                serde_json::to_string(&inner)?
            }
            None => "NONE".to_string(),
        };

        let query = format!(
            "UPDATE mcp_server:`{}` SET \
                name = {}, \
                enabled = {}, \
                command = {}, \
                args = {}, \
                env = {}, \
                description = {}, \
                auth_type = {}, \
                auth_metadata = {}, \
                extra_headers = {}, \
                updated_at = time::now()",
            server_id,
            name_json,
            config.enabled,
            command_json,
            args_json,
            env_json,
            description_json,
            auth_type_json,
            auth_metadata_json,
            extra_headers_json
        );

        self.db
            .execute(&query)
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "update server config".to_string(),
                message: e.to_string(),
            })?;

        // Also update in-memory client if it exists (lookup by NAME, not ID)
        {
            let name = {
                let id_lookup = self.id_to_name.read().await;
                id_lookup.get(&config.id).cloned()
            };

            if let Some(name) = name {
                let mut clients = self.clients.write().await;
                if let Some(client) = clients.get_mut(&name) {
                    client.update_config(config.clone());
                    debug!(
                        server_id = %config.id,
                        server_name = %name,
                        "Server configuration updated in memory"
                    );
                }
            }
        }

        debug!(
            server_id = %config.id,
            "Server configuration updated in database"
        );

        Ok(())
    }

    /// Deletes a server configuration from the database
    pub async fn delete_server_config(&self, id: &str) -> MCPResult<()> {
        // First stop the server if running (by ID)
        let _ = self.stop_server(id).await;

        // Use raw query instead of SDK delete method (which has issues with record IDs)
        let query = format!("DELETE mcp_server:`{}`", id);

        let _: Vec<serde_json::Value> =
            self.db
                .query_json(&query)
                .await
                .map_err(|e| MCPError::DatabaseError {
                    context: "delete server config".to_string(),
                    message: e.to_string(),
                })?;

        debug!(server_id = %id, "Server configuration deleted from database");

        Ok(())
    }

    /// Gets all saved server configurations from the database
    pub(crate) async fn get_saved_configs(&self) -> MCPResult<Vec<MCPServerConfig>> {
        use crate::mcp::helpers::{
            parse_auth_metadata_json, parse_auth_type, parse_deployment_method, parse_env_json,
            parse_extra_headers_json,
        };

        let query = "SELECT meta::id(id) AS id, name, enabled, command, args, env, description, \
                     auth_type, auth_metadata, extra_headers FROM mcp_server";

        let result: Vec<serde_json::Value> =
            self.db
                .query_json(query)
                .await
                .map_err(|e| MCPError::DatabaseError {
                    context: "get saved configs".to_string(),
                    message: e.to_string(),
                })?;

        info!(
            result_count = result.len(),
            "Retrieved MCP server configs from database"
        );

        let configs: Vec<MCPServerConfig> = result
            .into_iter()
            .filter_map(|v| {
                let server_id = v.get("id").and_then(|i| i.as_str()).unwrap_or("unknown");

                // Use helper for deployment method parsing
                let command = match parse_deployment_method(v.get("command")) {
                    Some(method) => method,
                    None => {
                        warn!(
                            server_id = %server_id,
                            command_value = ?v.get("command"),
                            "Unknown deployment method, skipping server"
                        );
                        return None;
                    }
                };

                // Use helper for env parsing (with logging)
                let env_value = v.get("env");
                let env = parse_env_json(env_value);
                if !env.is_empty() {
                    debug!(
                        server_id = %server_id,
                        env_count = env.len(),
                        "Loaded env variables from database"
                    );
                }

                // v1.2 auth fields - all optional, parsed from JSON strings
                let auth_type = parse_auth_type(v.get("auth_type"));
                let auth_metadata = parse_auth_metadata_json(v.get("auth_metadata"));
                let extra_headers = parse_extra_headers_json(v.get("extra_headers"));

                Some(MCPServerConfig {
                    id: v.get("id")?.as_str()?.to_string(),
                    name: v.get("name")?.as_str()?.to_string(),
                    enabled: v.get("enabled")?.as_bool()?,
                    command,
                    args: v
                        .get("args")?
                        .as_array()?
                        .iter()
                        .filter_map(|a| a.as_str().map(String::from))
                        .collect(),
                    env,
                    description: v
                        .get("description")
                        .and_then(|d| d.as_str().map(String::from)),
                    auth_type,
                    auth_metadata,
                    extra_headers,
                })
            })
            .collect();

        Ok(configs)
    }

    /// Gets a single server configuration from the database
    pub async fn get_server_config(&self, id: &str) -> MCPResult<Option<MCPServerConfig>> {
        let configs = self.get_saved_configs().await?;
        Ok(configs.into_iter().find(|c| c.id == id))
    }

    /// Logs a tool call to the database
    ///
    /// Uses execute_with_params instead of create() to avoid SurrealDB SDK 2.x
    /// deserialization issues with union types (array | object) in the result field.
    ///
    /// NOTE: MCP responses may contain null characters (\0) which cause SurrealDB
    /// to panic. We sanitize the data before insertion to prevent this.
    pub(crate) async fn log_call(&self, log: MCPCallLogCreate) -> MCPResult<()> {
        let json_data = serde_json::to_value(&log).map_err(|e| MCPError::DatabaseError {
            context: "log call serialization".to_string(),
            message: e.to_string(),
        })?;

        // Sanitize to remove null characters that cause SurrealDB panics
        let json_data = sanitize_for_surrealdb(json_data);

        let query = format!("CREATE mcp_call_log:`{}` CONTENT $data", log.id);
        self.db
            .execute_with_params(&query, vec![("data".to_string(), json_data)])
            .await
            .map_err(|e| MCPError::DatabaseError {
                context: "log call".to_string(),
                message: e.to_string(),
            })?;

        Ok(())
    }
}
