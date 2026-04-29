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

//! Per-entity import operations (schema v1.1).
//!
//! Each function handles importing a specific entity type with conflict resolution.
//! Import order: custom_providers -> models -> mcp_servers -> skills -> agents -> prompts

use crate::db::client::DBClient;
use crate::db::sanitize_for_surrealdb;
use crate::models::import_export::*;
use crate::models::prompt::Prompt;
use std::collections::HashMap;

use super::helpers::{
    persist_imported_entity, resolve_import_entity, ImportAction, ImportTracking,
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
                let env_str = serde_json::to_string(&env).unwrap_or_else(|_| "{}".to_string());

                // v1.2 — HTTP auth metadata (secrets are NOT in the export
                // payload). `auth_type::None` is normalised to no row entry,
                // matching `MCPServerCreate::from_config` semantics.
                let auth_type = server.auth_type.and_then(|t| match t {
                    crate::models::mcp::MCPAuthType::None => None,
                    other => serde_json::to_value(other)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_string)),
                });
                let auth_metadata = server
                    .auth_metadata
                    .as_ref()
                    .and_then(|m| serde_json::to_string(m).ok());
                let extra_headers = server
                    .extra_headers
                    .as_ref()
                    .filter(|h| !h.is_empty())
                    .and_then(|h| serde_json::to_string(h).ok());

                let data = sanitize_for_surrealdb(serde_json::json!({
                    "name": name,
                    "enabled": server.enabled,
                    "command": server.command,
                    "args": server.args,
                    "env": env_str,
                    "description": server.description,
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
        match resolve_import_entity(db, "llm_model", "model", &model.name, selected, resolutions)
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
