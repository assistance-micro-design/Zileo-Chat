// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Compose-card flow: turn a free-text description into a fully-formed
//! `KanbanCardCreate` payload by asking the Kanban-kind agent to fill it.
//!
//! This is an ephemeral, single-turn LLM completion — not a full workflow.
//! The Kanban agent's `llm` config drives provider/model selection.
//! The returned payload is NOT persisted here; the caller is expected to
//! review the proposal and (optionally) call `create_kanban_card_core`.

use crate::commands::agent::hydrate_llm_from_model;
use crate::db::DBClient;
use crate::llm::{CompletionParams, ProviderManager, ProviderType};
use crate::models::agent::ReasoningEffort;
use crate::models::{KanbanCardCreate, LLMConfig};
use crate::security::validate_uuid_field;
use crate::AppState;
use serde_json::Value;
use std::str::FromStr;
use std::sync::Arc;
use tauri::State;
use tracing::{debug, info, instrument, warn};

/// Cap on the description input length (sanity, prompt budget protection).
const MAX_DESCRIPTION_LEN: usize = 4_000;

/// Composes a kanban card from a short user description.
///
/// `kanban_agent_id` identifies the meta-agent doing the composition; its
/// `llm` field selects the provider/model. The agent's system_prompt is
/// reused and appended with strict JSON-format instructions.
pub async fn compose_card_from_description_core(
    db: &Arc<DBClient>,
    llm_manager: &Arc<ProviderManager>,
    kanban_agent_id: &str,
    description: &str,
) -> Result<KanbanCardCreate, String> {
    let kanban_agent_id = validate_uuid_field(kanban_agent_id, "kanban_agent_id")?;
    let trimmed_desc = description.trim();
    if trimmed_desc.is_empty() {
        return Err("description cannot be empty".to_string());
    }
    if trimmed_desc.len() > MAX_DESCRIPTION_LEN {
        return Err(format!("description exceeds {} chars", MAX_DESCRIPTION_LEN));
    }

    // 1. Load the Kanban agent (we need its llm config + system_prompt).
    let agent = load_kanban_agent(db, &kanban_agent_id).await?;

    // 2. Gather context: candidate agents + prompts the LLM can pick from.
    let agents = list_target_agents(db, &kanban_agent_id).await?;
    let prompts = list_prompt_summaries(db).await?;

    // 3. Build the prompt.
    let system_prompt = build_system_prompt(&agent.system_prompt, &agents, &prompts);
    let user_prompt = build_user_prompt(trimmed_desc, &kanban_agent_id);

    // 4. Single-turn completion — use the agent's full LLM config
    //    (temperature, max_tokens, is_reasoning, context_window) so the
    //    compose flow honours the same Settings as a regular chat turn.
    let provider = provider_type_from_string(&agent.llm.provider)?;
    let reasoning_effort = if agent.llm.is_reasoning {
        agent.reasoning_effort.clone()
    } else {
        None
    };
    let params = CompletionParams {
        prompt: user_prompt,
        system_prompt: Some(system_prompt),
        model: Some(agent.llm.model.clone()),
        temperature: agent.llm.temperature,
        max_tokens: agent.llm.max_tokens,
        reasoning_effort,
        context_window: agent.llm.context_window,
    };
    let response = llm_manager
        .complete_with_provider(provider, params)
        .await
        .map_err(|e| format!("LLM completion failed: {}", e))?;
    debug!(
        provider = %agent.llm.provider,
        model = %agent.llm.model,
        temperature = agent.llm.temperature,
        max_tokens = agent.llm.max_tokens,
        is_reasoning = agent.llm.is_reasoning,
        tokens_in = response.tokens_input,
        tokens_out = response.tokens_output,
        "Compose-card LLM completion done"
    );

    // 5. Parse the JSON payload — be forgiving about fenced blocks.
    let payload = extract_json_payload(&response.content).ok_or_else(|| {
        format!(
            "LLM response did not contain a JSON object: {}",
            response.content
        )
    })?;
    let card = parse_compose_response(&payload, &kanban_agent_id)?;
    info!(
        agent_id = %kanban_agent_id,
        target_agent_id = %card.target_agent_id,
        "Compose-card produced KanbanCardCreate"
    );
    Ok(card)
}

fn provider_type_from_string(s: &str) -> Result<ProviderType, String> {
    ProviderType::from_str(s).map_err(|e| format!("Invalid provider '{}': {}", s, e))
}

// ---------------------------------------------------------------------------
// Loaders
// ---------------------------------------------------------------------------

struct ComposerAgent {
    system_prompt: String,
    llm: LLMConfig,
    reasoning_effort: Option<ReasoningEffort>,
}

async fn load_kanban_agent(db: &Arc<DBClient>, id: &str) -> Result<ComposerAgent, String> {
    let q = format!(
        "SELECT system_prompt, kind, llm, reasoning_effort FROM agent:`{}`",
        id
    );
    let rows = db
        .query_json(&q)
        .await
        .map_err(|e| format!("Failed to load Kanban agent: {}", e))?;
    let row = rows
        .into_iter()
        .next()
        .ok_or_else(|| format!("Kanban agent not found: {}", id))?;
    let kind = row["kind"].as_str();
    if kind != Some("kanban") {
        return Err(format!(
            "Agent {} is not a Kanban-kind agent (kind={:?})",
            id, kind
        ));
    }
    let system_prompt = row["system_prompt"].as_str().unwrap_or("").to_string();

    let llm_value = row
        .get("llm")
        .cloned()
        .ok_or_else(|| "Kanban agent has no llm config".to_string())?;
    let mut llm: LLMConfig = serde_json::from_value(llm_value)
        .map_err(|e| format!("Failed to deserialize agent llm config: {}", e))?;
    if llm.model.trim().is_empty() {
        return Err("Kanban agent has no LLM model configured".to_string());
    }
    // Re-sync is_reasoning / context_window / temperature / max_tokens from
    // the llm_model row so a stale agent snapshot can't shadow Settings edits.
    hydrate_llm_from_model(db, &mut llm).await?;

    let reasoning_effort: Option<ReasoningEffort> = row
        .get("reasoning_effort")
        .filter(|v| !v.is_null())
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| format!("Failed to deserialize reasoning_effort: {}", e))?;

    Ok(ComposerAgent {
        system_prompt,
        llm,
        reasoning_effort,
    })
}

struct AgentBrief {
    id: String,
    name: String,
    description: String,
}

async fn list_target_agents(
    db: &Arc<DBClient>,
    kanban_agent_id: &str,
) -> Result<Vec<AgentBrief>, String> {
    let q = "SELECT meta::id(id) AS id, name, description FROM agent ORDER BY name ASC";
    let rows = db
        .query_json(q)
        .await
        .map_err(|e| format!("Failed to list agents: {}", e))?;
    Ok(rows
        .into_iter()
        .filter_map(|r| {
            let id = r["id"].as_str()?.to_string();
            if id == kanban_agent_id {
                return None;
            }
            Some(AgentBrief {
                id,
                name: r["name"].as_str().unwrap_or("").to_string(),
                description: r["description"].as_str().unwrap_or("").to_string(),
            })
        })
        .collect())
}

struct PromptBrief {
    id: String,
    name: String,
    description: String,
    variables: Vec<String>,
}

async fn list_prompt_summaries(db: &Arc<DBClient>) -> Result<Vec<PromptBrief>, String> {
    let q = "SELECT meta::id(id) AS id, name, description, variables, updated_at FROM prompt \
             ORDER BY updated_at DESC LIMIT 50";
    let rows = db
        .query_json(q)
        .await
        .map_err(|e| format!("Failed to list prompts: {}", e))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let vars: Vec<String> = r["variables"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v["name"].as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            PromptBrief {
                id: r["id"].as_str().unwrap_or("").to_string(),
                name: r["name"].as_str().unwrap_or("").to_string(),
                description: r["description"].as_str().unwrap_or("").to_string(),
                variables: vars,
            }
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Prompt construction
// ---------------------------------------------------------------------------

fn build_system_prompt(
    agent_system_prompt: &str,
    agents: &[AgentBrief],
    prompts: &[PromptBrief],
) -> String {
    let mut s = String::new();
    if !agent_system_prompt.trim().is_empty() {
        s.push_str(agent_system_prompt.trim());
        s.push_str("\n\n");
    }
    s.push_str("# Compose-card mode\n\n");
    s.push_str(
        "Your task: read the user demand and produce a JSON payload describing a kanban card. \
         The card will be executed by the target agent you pick.\n\n",
    );
    s.push_str("## Available target agents\n\n");
    if agents.is_empty() {
        s.push_str("(none — you must use inline_prompt and pick the user's preferred agent)\n");
    } else {
        for a in agents {
            s.push_str(&format!("- id=`{}` name=\"{}\"", a.id, a.name));
            if !a.description.is_empty() {
                s.push_str(&format!(" — {}", a.description));
            }
            s.push('\n');
        }
    }
    s.push_str("\n## Available prompts\n\n");
    if prompts.is_empty() {
        s.push_str("(none — you must use inline_prompt)\n");
    } else {
        for p in prompts {
            s.push_str(&format!("- id=`{}` name=\"{}\"", p.id, p.name));
            if !p.description.is_empty() {
                s.push_str(&format!(" — {}", p.description));
            }
            if !p.variables.is_empty() {
                s.push_str(&format!(" — variables: {}", p.variables.join(", ")));
            }
            s.push('\n');
        }
    }
    s.push_str("\n## Output contract (STRICT)\n\n");
    s.push_str(
        "Reply with ONE JSON object only, no prose, no markdown fences. Required fields:\n\
         - `title` (string, 1..=200 chars)\n\
         - `description` (string, 0..=5000 chars)\n\
         - `target_agent_id` (string, uuid of the picked agent)\n\
         - EITHER `prompt_id` (string, uuid of a listed prompt) OR `inline_prompt` (string, the prompt text). Never both.\n\
         - `variables` (object: key->string). Provide every variable required by the picked `prompt_id`. \
         If you use `inline_prompt`, set `variables` to `{}`.\n\
         Optional: `target_folder_id` (string, uuid).\n",
    );
    s
}

fn build_user_prompt(description: &str, _kanban_agent_id: &str) -> String {
    format!(
        "User demand:\n\n{}\n\n\
         Now compose the kanban card. Output JSON only.",
        description
    )
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Best-effort extraction of the first balanced JSON object from arbitrary
/// LLM output. Handles fenced markdown blocks and leading prose.
fn extract_json_payload(content: &str) -> Option<Value> {
    // Try whole content first.
    if let Ok(v) = serde_json::from_str::<Value>(content.trim()) {
        return Some(v);
    }
    // Strip ```json ... ``` or ``` ... ``` fences.
    let trimmed = content.trim();
    if let Some(stripped) = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
    {
        if let Some(end) = stripped.rfind("```") {
            let inner = stripped[..end].trim();
            if let Ok(v) = serde_json::from_str::<Value>(inner) {
                return Some(v);
            }
        }
    }
    // Last-resort: scan for `{ ... }` balanced.
    let bytes = content.as_bytes();
    let start = bytes.iter().position(|b| *b == b'{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
            continue;
        }
        match b {
            b'"' => in_str = true,
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let slice = &content[start..=i];
                    if let Ok(v) = serde_json::from_str::<Value>(slice) {
                        return Some(v);
                    }
                    break;
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_compose_response(
    payload: &Value,
    kanban_agent_id: &str,
) -> Result<KanbanCardCreate, String> {
    let title = payload["title"]
        .as_str()
        .ok_or_else(|| "Missing title".to_string())?
        .trim()
        .to_string();
    if title.is_empty() || title.len() > 200 {
        return Err("title must be 1..=200 chars".to_string());
    }
    let description = payload["description"]
        .as_str()
        .unwrap_or("")
        .trim()
        .to_string();
    if description.len() > 5000 {
        return Err("description exceeds 5000 chars".to_string());
    }
    let target_agent_id = payload["target_agent_id"]
        .as_str()
        .ok_or_else(|| "Missing target_agent_id".to_string())?
        .trim()
        .to_string();
    validate_uuid_field(&target_agent_id, "target_agent_id")?;

    let prompt_id = payload["prompt_id"].as_str().map(|s| s.trim().to_string());
    let inline_prompt = payload["inline_prompt"]
        .as_str()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());
    match (&prompt_id, &inline_prompt) {
        (Some(_), Some(_)) => {
            return Err("prompt_id and inline_prompt are mutually exclusive".to_string())
        }
        (None, None) => return Err("either prompt_id or inline_prompt is required".to_string()),
        _ => {}
    }
    if let Some(ref pid) = prompt_id {
        validate_uuid_field(pid, "prompt_id")?;
    }

    let variables = match payload.get("variables") {
        Some(v) if v.is_object() => {
            serde_json::to_string(v).map_err(|e| format!("Failed to serialize variables: {}", e))?
        }
        Some(_) => return Err("variables must be an object".to_string()),
        None => "{}".to_string(),
    };

    let target_folder_id = match payload["target_folder_id"].as_str() {
        Some(s) if !s.trim().is_empty() => {
            let v = s.trim().to_string();
            validate_uuid_field(&v, "target_folder_id")?;
            Some(v)
        }
        _ => None,
    };

    Ok(KanbanCardCreate {
        title,
        description,
        kanban_agent_id: kanban_agent_id.to_string(),
        target_agent_id,
        prompt_id,
        inline_prompt,
        variables,
        target_folder_id,
    })
}

// ---------------------------------------------------------------------------
// Tauri wrapper
// ---------------------------------------------------------------------------

#[tauri::command]
#[instrument(name = "compose_card_from_description", skip(state, description))]
pub async fn compose_card_from_description(
    kanban_agent_id: String,
    description: String,
    state: State<'_, AppState>,
) -> Result<KanbanCardCreate, String> {
    let result = compose_card_from_description_core(
        &state.db,
        &state.llm_manager,
        &kanban_agent_id,
        &description,
    )
    .await;
    if let Err(ref e) = result {
        warn!(error = %e, "compose_card_from_description failed");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent_id() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    #[test]
    fn test_extract_json_payload_plain() {
        let raw = r#"{"title":"x","description":"y"}"#;
        let v = extract_json_payload(raw).unwrap();
        assert_eq!(v["title"], "x");
    }

    #[test]
    fn test_extract_json_payload_fenced() {
        let raw = "```json\n{\"title\":\"x\"}\n```";
        let v = extract_json_payload(raw).unwrap();
        assert_eq!(v["title"], "x");
    }

    #[test]
    fn test_extract_json_payload_prose_preamble() {
        let raw =
            "Sure! Here is the card:\n\n{\"title\":\"x\",\"description\":\"y\"}\n\nLet me know.";
        let v = extract_json_payload(raw).unwrap();
        assert_eq!(v["title"], "x");
    }

    #[test]
    fn test_extract_json_payload_returns_none_on_garbage() {
        assert!(extract_json_payload("no json here").is_none());
    }

    #[test]
    fn test_parse_compose_response_minimal_inline() {
        let kid = agent_id();
        let tid = agent_id();
        let payload = serde_json::json!({
            "title": "Weekly report",
            "description": "",
            "target_agent_id": tid,
            "inline_prompt": "Summarize the week",
        });
        let card = parse_compose_response(&payload, &kid).unwrap();
        assert_eq!(card.title, "Weekly report");
        assert_eq!(card.target_agent_id, tid);
        assert_eq!(card.inline_prompt.as_deref(), Some("Summarize the week"));
        assert!(card.prompt_id.is_none());
        assert_eq!(card.variables, "{}");
        assert_eq!(card.kanban_agent_id, kid);
    }

    #[test]
    fn test_parse_compose_response_with_prompt_and_vars() {
        let kid = agent_id();
        let tid = agent_id();
        let pid = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "title": "Digest",
            "description": "Weekly summary",
            "target_agent_id": tid,
            "prompt_id": pid,
            "variables": {"week": "21"}
        });
        let card = parse_compose_response(&payload, &kid).unwrap();
        assert_eq!(card.prompt_id.as_deref(), Some(pid.as_str()));
        assert_eq!(card.variables, "{\"week\":\"21\"}");
    }

    #[test]
    fn test_parse_compose_response_rejects_xor_both() {
        let kid = agent_id();
        let tid = agent_id();
        let pid = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "title": "x",
            "description": "",
            "target_agent_id": tid,
            "prompt_id": pid,
            "inline_prompt": "foo"
        });
        let err = parse_compose_response(&payload, &kid).unwrap_err();
        assert!(err.contains("mutually exclusive"));
    }

    #[test]
    fn test_parse_compose_response_rejects_xor_none() {
        let kid = agent_id();
        let tid = agent_id();
        let payload = serde_json::json!({
            "title": "x",
            "description": "",
            "target_agent_id": tid,
        });
        let err = parse_compose_response(&payload, &kid).unwrap_err();
        assert!(err.contains("required"));
    }

    #[test]
    fn test_parse_compose_response_rejects_bad_uuid() {
        let kid = agent_id();
        let payload = serde_json::json!({
            "title": "x",
            "description": "",
            "target_agent_id": "not-a-uuid",
            "inline_prompt": "p",
        });
        assert!(parse_compose_response(&payload, &kid).is_err());
    }

    #[test]
    fn test_parse_compose_response_rejects_empty_title() {
        let kid = agent_id();
        let tid = agent_id();
        let payload = serde_json::json!({
            "title": "   ",
            "description": "",
            "target_agent_id": tid,
            "inline_prompt": "p",
        });
        assert!(parse_compose_response(&payload, &kid).is_err());
    }

    #[test]
    fn test_parse_compose_response_with_folder() {
        let kid = agent_id();
        let tid = agent_id();
        let fid = uuid::Uuid::new_v4().to_string();
        let payload = serde_json::json!({
            "title": "x",
            "description": "",
            "target_agent_id": tid,
            "inline_prompt": "p",
            "target_folder_id": fid,
        });
        let card = parse_compose_response(&payload, &kid).unwrap();
        assert_eq!(card.target_folder_id.as_deref(), Some(fid.as_str()));
    }

    #[test]
    fn test_build_system_prompt_lists_agents_and_prompts() {
        let aid = agent_id();
        let pid = uuid::Uuid::new_v4().to_string();
        let prompt = build_system_prompt(
            "You are a Kanban agent.",
            &[AgentBrief {
                id: aid.clone(),
                name: "Writer".to_string(),
                description: "Drafts long-form content".to_string(),
            }],
            &[PromptBrief {
                id: pid.clone(),
                name: "Summary".to_string(),
                description: "Compact summary".to_string(),
                variables: vec!["topic".to_string()],
            }],
        );
        assert!(prompt.contains(&aid));
        assert!(prompt.contains("Writer"));
        assert!(prompt.contains(&pid));
        assert!(prompt.contains("topic"));
        assert!(prompt.contains("STRICT"));
    }
}
