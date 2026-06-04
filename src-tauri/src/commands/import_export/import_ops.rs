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

//! Per-entity import operations (schema v1.3).
//!
//! Each function handles importing a specific entity type with conflict resolution.
//! Import order: custom_providers -> models -> mcp_servers -> skills -> agents -> prompts

use crate::db::client::DBClient;
use crate::db::sanitize_for_surrealdb;
use crate::models::import_export::*;
use crate::models::prompt::Prompt;
use std::collections::HashMap;

use super::helpers::{
    persist_imported_entity, resolve_import_entity, resolve_import_model, ImportAction,
    ImportTracking,
};

/// Imports agent entities with conflict resolution.
/// Includes all v1.1 fields: folders, require_file_confirmation, llm.is_reasoning, llm.context_window.
pub async fn import_agents(
    db: &DBClient,
    agents: &[AgentExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for agent in agents {
        match resolve_import_entity(db, "agent", "agent", &agent.name, selected, resolutions).await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.agents += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                // Fail-closed: the MCP tool allowlist is
                // an authorization boundary. It is NEVER hydrated from a
                // third-party import file — the imported value is dropped and the
                // allowlist is forced to empty. The user re-arms it manually
                // after verifying the servers (a high-severity warning is emitted
                // at validation time). The `server_id` keys would not survive the
                // round-trip anyway, so importing them would be both unsafe and
                // meaningless.
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "lifecycle": agent.lifecycle,
                    "llm": {
                        "provider": agent.llm.provider,
                        "model": agent.llm.model,
                        "temperature": agent.llm.temperature,
                        "max_tokens": agent.llm.max_tokens,
                        "is_reasoning": agent.llm.is_reasoning,
                        "context_window": agent.llm.context_window,
                    },
                    "tools": agent.tools,
                    "mcp_servers": agent.mcp_servers,
                    "skills": agent.skills,
                    "system_prompt": agent.system_prompt,
                    "max_tool_iterations": agent.max_tool_iterations,
                    "reasoning_effort": agent.reasoning_effort,
                    "folders": agent.folders,
                    "require_file_confirmation": agent.require_file_confirmation,
                    "kind": agent.kind,
                    "auto_analyze_reports": agent.auto_analyze_reports,
                    "mcp_tool_allowlist": [],
                }));
                match persist_imported_entity(db, "agent", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.agents += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "agent".to_string(),
                        entity_id: agent.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Maps an exported MCP server to an [`MCPServerConfig`] and runs the full
/// validation gate before persistence.
///
/// Covers: id/name/args/env validation + the Docker spawn guard (via
/// `validate_mcp_server_config`), the SSRF screen in **import** mode
/// (`allow_loopback = false`, so a loopback/private/metadata HTTP target is
/// rejected up front), and the extra-headers invariants. Returns the validated
/// config (merged env included) or a human-readable error.
fn validate_imported_mcp_server(
    server: &MCPServerExportData,
    id: &str,
    name: &str,
    env: HashMap<String, String>,
) -> Result<crate::models::mcp::MCPServerConfig, String> {
    use crate::commands::mcp::validation::{validate_extra_headers, validate_mcp_server_config};
    use crate::mcp::ssrf::{screen_request_url, ScreenPolicy};
    use crate::models::mcp::{MCPAuthType, MCPDeploymentMethod, MCPServerConfig};

    let command: MCPDeploymentMethod =
        serde_json::from_value(serde_json::Value::String(server.command.clone()))
            .map_err(|_| format!("unknown MCP deployment method '{}'", server.command))?;

    let config = MCPServerConfig {
        id: id.to_string(),
        name: name.to_string(),
        enabled: server.enabled,
        command,
        args: server.args.clone(),
        env,
        description: server.description.clone(),
        auth_type: server.auth_type,
        auth_metadata: server.auth_metadata.clone(),
        extra_headers: server.extra_headers.clone(),
    };

    // id/name/args/env + Docker spawn guard.
    let validated = validate_mcp_server_config(&config)?;

    // SSRF screen for HTTP servers, IMPORT mode (loopback + private blocked).
    // A locally-hosted MCP server (localhost / LAN) works at runtime but is
    // deliberately refused on import: an import file can come from an untrusted
    // third party, and re-pointing a server at the local machine / LAN is an
    // SSRF vector. We surface an explicit, actionable message instead of the
    // raw screening error so the user knows the server must be re-created by
    // hand in Settings > MCP.
    if validated.command == MCPDeploymentMethod::Http {
        let url = validated
            .args
            .first()
            .ok_or_else(|| "HTTP MCP server requires a URL in args[0]".to_string())?;
        screen_request_url(url, ScreenPolicy::IMPORT).map_err(|e| {
            format!(
                "HTTP MCP server '{}' points at a local or private address ({}) and cannot be \
                 imported for security reasons. Re-create it manually in Settings > MCP after \
                 import.",
                name, e
            )
        })?;
    }

    // Extra-header invariants (charset + Authorization conflict).
    let auth_active = validated.auth_type.unwrap_or(MCPAuthType::None) != MCPAuthType::None;
    if let Some(headers) = validated.extra_headers.as_ref() {
        validate_extra_headers(headers, auth_active)?;
    }

    Ok(validated)
}

/// Imports MCP server entities with conflict resolution and env additions.
pub async fn import_mcp_servers(
    db: &DBClient,
    servers: &[MCPServerExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    mcp_additions: &HashMap<String, MCPAdditions>,
    t: &mut ImportTracking<'_>,
) {
    for server in servers {
        match resolve_import_entity(db, "mcp_server", "mcp", &server.name, selected, resolutions)
            .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.mcp_servers += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let mut env = server.env.clone();
                if let Some(additions) = mcp_additions.get(&server.name) {
                    for (key, value) in &additions.add_env {
                        env.insert(key.clone(), value.clone());
                    }
                }

                // Validate the (mapped) server before persisting; an
                // invalid entry is reported and skipped, never persisted, and
                // does not abort the rest of the batch.
                let validated = match validate_imported_mcp_server(server, &id, &name, env) {
                    Ok(c) => c,
                    Err(e) => {
                        t.errors.push(ImportError {
                            entity_type: "mcp".to_string(),
                            entity_id: server.name.clone(),
                            error: e,
                        });
                        continue;
                    }
                };

                let env_str =
                    serde_json::to_string(&validated.env).unwrap_or_else(|_| "{}".to_string());

                // v1.2 — HTTP auth metadata (secrets are NOT in the export
                // payload). `auth_type::None` is normalised to no row entry,
                // matching `MCPServerCreate::from_config` semantics.
                let auth_type = validated.auth_type.and_then(|t| match t {
                    crate::models::mcp::MCPAuthType::None => None,
                    other => serde_json::to_value(other)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string)),
                });
                let auth_metadata = validated
                    .auth_metadata
                    .as_ref()
                    .and_then(|m| serde_json::to_string(m).ok());
                let extra_headers = validated
                    .extra_headers
                    .as_ref()
                    .filter(|h| !h.is_empty())
                    .and_then(|h| serde_json::to_string(h).ok());

                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": validated.name,
                    "enabled": validated.enabled,
                    "command": validated.command.to_string(),
                    "args": validated.args,
                    "env": env_str,
                    "description": validated.description,
                    "auth_type": auth_type,
                    "auth_metadata": auth_metadata,
                    "extra_headers": extra_headers,
                }));
                match persist_imported_entity(db, "mcp_server", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.mcp_servers += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "mcp".to_string(),
                        entity_id: server.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports model entities with conflict resolution.
pub async fn import_models(
    db: &DBClient,
    models: &[LLMModelExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for model in models {
        // Models are unique on (provider, api_name), NOT name.
        // Resolve the conflict (and Overwrite target) by that real key so an
        // Overwrite updates the colliding row instead of failing on the index.
        match resolve_import_model(
            db,
            &model.provider,
            &model.api_name,
            &model.name,
            selected,
            resolutions,
        )
        .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.models += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "provider": model.provider,
                    "name": name,
                    "api_name": model.api_name,
                    "context_window": model.context_window,
                    "max_output_tokens": model.max_output_tokens,
                    "temperature_default": model.temperature_default,
                    "is_builtin": model.is_builtin,
                    "is_reasoning": model.is_reasoning,
                    "supports_vision": model.supports_vision,
                    "supports_forced_tool_choice": model.supports_forced_tool_choice,
                    "input_price_per_mtok": model.input_price_per_mtok,
                    "output_price_per_mtok": model.output_price_per_mtok,
                    "cache_read_price_per_mtok": model.cache_read_price_per_mtok,
                    "cache_write_price_per_mtok": model.cache_write_price_per_mtok,
                }));
                match persist_imported_entity(db, "llm_model", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.models += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "model".to_string(),
                        entity_id: model.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports prompt entities with conflict resolution.
pub async fn import_prompts(
    db: &DBClient,
    prompts: &[PromptExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for prompt in prompts {
        match resolve_import_entity(db, "prompt", "prompt", &prompt.name, selected, resolutions)
            .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.prompts += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let variables = Prompt::detect_variables(&prompt.content);
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "description": prompt.description,
                    "category": prompt.category,
                    "content": prompt.content,
                    "variables": variables,
                }));
                match persist_imported_entity(db, "prompt", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.prompts += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "prompt".to_string(),
                        entity_id: prompt.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports skill entities with conflict resolution (v1.1).
pub async fn import_skills(
    db: &DBClient,
    skills: &[SkillExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for skill in skills {
        match resolve_import_entity(db, "skill", "skill", &skill.name, selected, resolutions).await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.skills += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "description": skill.description,
                    "category": skill.category,
                    "content": skill.content,
                    "enabled": skill.enabled,
                    "kind": skill.kind,
                }));
                match persist_imported_entity(db, "skill", &id, data, is_overwrite).await {
                    Ok(()) => t.imported.skills += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "skill".to_string(),
                        entity_id: skill.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

/// Imports custom provider entities with conflict resolution (v1.1).
/// Custom providers use name as primary key (not UUID).
pub async fn import_custom_providers(
    db: &DBClient,
    providers: &[CustomProviderExportData],
    selected: &[String],
    resolutions: &HashMap<String, ConflictResolution>,
    t: &mut ImportTracking<'_>,
) {
    for provider in providers {
        match resolve_import_entity(
            db,
            "custom_provider",
            "custom_provider",
            &provider.name,
            selected,
            resolutions,
        )
        .await
        {
            ImportAction::NotSelected => continue,
            ImportAction::Skipped => {
                t.skipped.custom_providers += 1;
                continue;
            }
            ImportAction::Import {
                id,
                name,
                is_overwrite,
            } => {
                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "display_name": provider.display_name,
                    "base_url": provider.base_url,
                    "enabled": provider.enabled,
                    "supports_cache_control": provider.supports_cache_control,
                    "supports_reasoning_param": provider.supports_reasoning_param,
                }));
                match persist_imported_entity(db, "custom_provider", &id, data, is_overwrite).await
                {
                    Ok(()) => t.imported.custom_providers += 1,
                    Err(e) => t.errors.push(ImportError {
                        entity_type: "custom_provider".to_string(),
                        entity_id: provider.name.clone(),
                        error: e,
                    }),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::mcp::MCPDeploymentMethod;

    fn export(command: &str, args: &[&str]) -> MCPServerExportData {
        MCPServerExportData {
            name: "srv".to_string(),
            enabled: true,
            command: command.to_string(),
            args: args.iter().map(|s| s.to_string()).collect(),
            env: HashMap::new(),
            description: None,
            auth_type: None,
            auth_metadata: None,
            extra_headers: None,
            created_at: None,
            updated_at: None,
        }
    }

    fn validate(
        server: &MCPServerExportData,
    ) -> Result<crate::models::mcp::MCPServerConfig, String> {
        validate_imported_mcp_server(server, "mcp-import-1", "Imported", server.env.clone())
    }

    // ---- SSRF screen in import mode (loopback blocked) ----

    #[test]
    fn import_http_localhost_rejected() {
        let err = validate(&export("http", &["http://localhost:8080/"]))
            .expect_err("localhost HTTP MCP server must be rejected on import");
        // F1: the message must be actionable, not the raw SSRF screen error.
        assert!(err.contains("local or private address"), "got: {err}");
        assert!(err.contains("Settings > MCP"), "got: {err}");
    }

    #[test]
    fn import_http_private_rejected() {
        let err = validate(&export("http", &["http://10.0.0.1/"]))
            .expect_err("private-LAN HTTP MCP server must be rejected on import");
        assert!(err.contains("local or private address"), "got: {err}");
    }

    #[test]
    fn import_http_metadata_rejected() {
        assert!(validate(&export("http", &["http://169.254.169.254/"])).is_err());
    }

    #[test]
    fn import_http_public_ok() {
        assert!(validate(&export("http", &["https://api.example.com/"])).is_ok());
    }

    // ---- Docker spawn guard applies early at import ----

    #[test]
    fn import_docker_malicious_mount_rejected() {
        assert!(validate(&export("docker", &["run", "-i", "-v", "/:/host", "img"])).is_err());
    }

    #[test]
    fn import_docker_safe_ok() {
        assert!(validate(&export("docker", &["run", "-i", "image:tag"])).is_ok());
    }

    // ---- mapping + misc ----

    #[test]
    fn import_unknown_command_rejected() {
        assert!(validate(&export("weird", &["x"])).is_err());
    }

    #[test]
    fn import_mapping_uses_action_id_and_name() {
        let cfg = validate(&export("docker", &["run", "-i", "image:tag"])).expect("ok");
        assert_eq!(cfg.id, "mcp-import-1");
        assert_eq!(cfg.name, "Imported");
        assert_eq!(cfg.command, MCPDeploymentMethod::Docker);
    }

    #[test]
    fn import_env_shell_chars_round_trip_ok() {
        // R-QUA-3: shell metachars in env must survive import.
        let mut server = export("docker", &["run", "-i", "image:tag"]);
        server
            .env
            .insert("TOKEN".to_string(), "$(secret)&more".to_string());
        let cfg =
            validate_imported_mcp_server(&server, "mcp-import-2", "Imported", server.env.clone())
                .expect("shell metachars in env must be accepted");
        assert_eq!(
            cfg.env.get("TOKEN").map(String::as_str),
            Some("$(secret)&more")
        );
    }

    #[test]
    fn import_env_control_char_rejected() {
        let mut server = export("docker", &["run", "-i", "image:tag"]);
        server.env.insert("BAD".to_string(), "a\nb".to_string());
        assert!(validate_imported_mcp_server(
            &server,
            "mcp-import-3",
            "Imported",
            server.env.clone()
        )
        .is_err());
    }
}
