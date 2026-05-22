// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! `SubmitComposedCardTool` — final step of the Kanban compose flow.
//!
//! Captures a `KanbanCardCreate` payload into a shared `Arc<Mutex<_>>` slot
//! so the parent `compose_card.rs` flow can pick it up after `tool_loop`
//! returns. The tool does NOT persist the card — the caller decides whether
//! to forward the proposal to `create_kanban_card_core` or surface it to the
//! user for review.
//!
//! The tool is **private**: never registered in `tools/registry.rs` and
//! never instantiable via the factory. It is constructed directly by
//! `compose_card.rs` and injected via the `extra_tools` parameter of
//! `tool_loop::execute_with_tools`.
//!
//! `kanban_agent_id` is wired in via the struct so the LLM cannot spoof a
//! different author for the card.

use crate::commands::kanban_card::{
    validate_description, validate_title, validate_variables_json, validate_xor_prompt,
};
use crate::db::DBClient;
use crate::models::KanbanCardCreate;
use crate::security::validate_uuid_field;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use tracing::{info, warn};

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "SubmitComposedCardTool".to_string(),
    name: "SubmitComposedCard".to_string(),
    summary: "Finalize a kanban card proposal. Call exactly once when ready.".to_string(),
    description: ToolDescriptionBuilder::new(
        "Finalize a kanban card proposal. Call exactly once when ready to submit your card.",
    )
    .use_when(&[
        "You have decided on a title, description, target agent and either a prompt_id or an inline_prompt",
        "You are ready to commit your composition and end your turn",
    ])
    .do_not_use(&[
        "Before you have discovered the available target agents and prompts via ListAgentsTool and PromptManagerTool",
        "More than once per compose session - if you must correct, call again with the full corrected payload (the latest call wins)",
        "To persist or modify an existing card - this tool only captures a proposal for user review",
    ])
    .operations(&[(
        "submit",
        "Submit the final card payload. Required: title, target_agent_id, exactly one of (prompt_id, inline_prompt). Optional: description, variables, target_folder_id.",
    )])
    .examples(&[
        json!({
            "title": "Weekly PR digest",
            "description": "Summarize merged PRs this week",
            "target_agent_id": "11111111-2222-3333-4444-555555555555",
            "prompt_id": "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "variables": {"week": "21"}
        }),
        json!({
            "title": "Refactor login flow",
            "description": "Drop legacy session middleware",
            "target_agent_id": "11111111-2222-3333-4444-555555555555",
            "inline_prompt": "Refactor the login handler to remove the deprecated middleware.",
            "variables": {}
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "Card title (1-200 chars)",
                "minLength": 1,
                "maxLength": 200
            },
            "description": {
                "type": "string",
                "description": "Optional card description (max 5000 chars)",
                "maxLength": 5000
            },
            "target_agent_id": {
                "type": "string",
                "description": "UUID of the permanent agent that will execute the card"
            },
            "prompt_id": {
                "type": "string",
                "description": "UUID of a reusable prompt from the library (mutually exclusive with inline_prompt). Discover candidates and their declared variables via PromptManager.list_prompts then PromptManager.get_prompt(id). The returned `variables` array tells you exactly which keys to fill in the `variables` map below."
            },
            "inline_prompt": {
                "type": "string",
                "description": "Free-form prompt text (mutually exclusive with prompt_id). Use {{var_name}} placeholders if you reference variables; mirror those names as keys in the `variables` map below."
            },
            "variables": {
                "type": "object",
                "description": "Variable substitutions applied at workflow execution time. When `prompt_id` is set you MUST provide every variable name declared by that prompt (call PromptManager.get_prompt to list them) — missing keys are rejected. When `inline_prompt` is set, provide one key per `{{name}}` placeholder you wrote. Use {} only when neither path declares variables."
            },
            "target_folder_id": {
                "type": "string",
                "description": "Optional UUID of the workflow folder to file the card under"
            }
        },
        "required": ["title", "target_agent_id"]
    }),
    output_schema: json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "message": {"type": "string"}
        }
    }),
    requires_confirmation: false,
}
});

/// Tool capturing a `KanbanCardCreate` proposal for the compose flow.
///
/// Concurrent safety: the `capture` slot is guarded by a `tokio::sync::Mutex`
/// (no poisoning). The caller (`compose_card.rs`) owns the `Arc` and reads
/// the slot after `tool_loop` returns.
pub struct SubmitComposedCardTool {
    capture: Arc<Mutex<Option<KanbanCardCreate>>>,
    kanban_agent_id: String,
    db: Arc<DBClient>,
}

impl SubmitComposedCardTool {
    pub fn new(
        capture: Arc<Mutex<Option<KanbanCardCreate>>>,
        kanban_agent_id: String,
        db: Arc<DBClient>,
    ) -> Self {
        Self {
            capture,
            kanban_agent_id,
            db,
        }
    }

    /// Loads the variable names declared by a persisted prompt so the Submit
    /// tool can cross-check the agent-provided `variables` map. Returns an
    /// empty set if the prompt has no variables column or is missing — the
    /// caller treats that as "no check possible".
    async fn fetch_prompt_variable_names(&self, prompt_id: &str) -> ToolResult<BTreeSet<String>> {
        let query = format!("SELECT variables FROM prompt:`{prompt_id}`");
        let rows = self.db.query_json(&query).await.map_err(|e| {
            ToolError::DatabaseError(format!("Failed to load prompt for cross-check: {e}"))
        })?;
        let row = rows.into_iter().next().ok_or_else(|| {
            ToolError::ValidationFailed(format!("prompt_id {prompt_id} not found"))
        })?;
        let names = row
            .get("variables")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()).map(String::from))
                    .collect::<BTreeSet<String>>()
            })
            .unwrap_or_default();
        Ok(names)
    }

    /// Normalises the `variables` input (object preferred, stringified JSON accepted)
    /// into a JSON-string suitable for the SCHEMAFULL `variables` field.
    fn variables_to_string(input: &Value) -> ToolResult<String> {
        if input.is_null() || input.is_string() && input.as_str().unwrap_or("").is_empty() {
            return Ok("{}".to_string());
        }
        if let Some(s) = input.as_str() {
            let parsed: Value = serde_json::from_str(s).map_err(|e| {
                ToolError::InvalidInput(format!("variables string is not valid JSON: {}", e))
            })?;
            if !parsed.is_object() {
                return Err(ToolError::InvalidInput(
                    "variables must be a JSON object".to_string(),
                ));
            }
            return Ok(s.to_string());
        }
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "variables must be an object or a stringified JSON object".to_string(),
            ));
        }
        serde_json::to_string(input)
            .map_err(|e| ToolError::ExecutionFailed(format!("variables serialize: {}", e)))
    }
}

#[async_trait]
impl Tool for SubmitComposedCardTool {
    fn id(&self) -> &str {
        "SubmitComposedCardTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let title_raw = input["title"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("title is required".to_string()))?;
        let title = validate_title(title_raw).map_err(ToolError::ValidationFailed)?;

        let description_raw = input["description"].as_str().unwrap_or("");
        let description =
            validate_description(description_raw).map_err(ToolError::ValidationFailed)?;

        let target_agent_id_raw = input["target_agent_id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("target_agent_id is required".to_string()))?;
        let target_agent_id = validate_uuid_field(target_agent_id_raw, "target_agent_id")
            .map_err(ToolError::InvalidInput)?;

        // prompt_id / inline_prompt: treat empty strings as absence so models
        // that fill both slots with "" don't trip the XOR check.
        let prompt_id = input["prompt_id"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        let inline_prompt = input["inline_prompt"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());
        validate_xor_prompt(&prompt_id, &inline_prompt).map_err(ToolError::ValidationFailed)?;
        let prompt_id = match prompt_id {
            Some(id) => {
                Some(validate_uuid_field(&id, "prompt_id").map_err(ToolError::InvalidInput)?)
            }
            None => None,
        };

        let target_folder_id = match input["target_folder_id"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
        {
            Some(id) => {
                Some(validate_uuid_field(id, "target_folder_id").map_err(ToolError::InvalidInput)?)
            }
            None => None,
        };

        let variables = Self::variables_to_string(&input["variables"])?;
        let variables = validate_variables_json(&variables).map_err(ToolError::ValidationFailed)?;

        // Cross-check: when targeting an existing prompt, every variable the
        // prompt declares MUST be present in the agent-provided map.
        // Otherwise the worker would execute the card with literal `{{var}}`
        // placeholders in the resolved prompt. The Submit tool is idempotent
        // (latest call wins) so the agent can correct and resubmit when the
        // error surfaces in its next tool result.
        if let Some(ref pid) = prompt_id {
            let expected = self.fetch_prompt_variable_names(pid).await?;
            if !expected.is_empty() {
                let provided: BTreeSet<String> = serde_json::from_str::<Value>(&variables)
                    .ok()
                    .and_then(|v| {
                        v.as_object()
                            .map(|m| m.keys().cloned().collect::<BTreeSet<String>>())
                    })
                    .unwrap_or_default();
                let missing: Vec<&String> = expected.difference(&provided).collect();
                if !missing.is_empty() {
                    let names = missing
                        .iter()
                        .map(|s| s.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    return Err(ToolError::ValidationFailed(format!(
                        "prompt_id {pid} declares variables that are not provided: {names}. \
                         Call PromptManager.get_prompt({pid}) to discover them, then \
                         resubmit with a complete variables object."
                    )));
                }
            }
        }

        let card = KanbanCardCreate {
            id: None,
            title,
            description,
            kanban_agent_id: self.kanban_agent_id.clone(),
            target_agent_id,
            prompt_id,
            inline_prompt,
            variables,
            target_folder_id,
        };

        let mut slot = self.capture.lock().await;
        if slot.is_some() {
            warn!(
                kanban_agent_id = %self.kanban_agent_id,
                "SubmitComposedCardTool called more than once in the same session; \
                 keeping latest proposal and discarding the previous one"
            );
        }
        info!(
            kanban_agent_id = %self.kanban_agent_id,
            target_agent_id = %card.target_agent_id,
            "SubmitComposedCardTool captured card proposal"
        );
        *slot = Some(card);

        Ok(json!({
            "success": true,
            "message": "Card composed. End your response with a brief rationale (2-3 sentences) explaining your choice."
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "expected a JSON object payload".to_string(),
            ));
        }
        if !input["title"].is_string() {
            return Err(ToolError::InvalidInput(
                "title is required and must be a string".to_string(),
            ));
        }
        if !input["target_agent_id"].is_string() {
            return Err(ToolError::InvalidInput(
                "target_agent_id is required and must be a string".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    fn new_capture() -> Arc<Mutex<Option<KanbanCardCreate>>> {
        Arc::new(Mutex::new(None))
    }

    fn kanban_agent_uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn target_agent_uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn prompt_uuid() -> String {
        uuid::Uuid::new_v4().to_string()
    }

    /// Seeds a minimal `prompt` row with the requested variable names so the
    /// Submit tool's cross-check can resolve them.
    async fn seed_prompt(db: &Arc<DBClient>, id: &str, vars: &[&str]) {
        let vars_sql = if vars.is_empty() {
            "[]".to_string()
        } else {
            let items = vars
                .iter()
                .map(|n| format!("{{ name: '{n}', description: NONE, default_value: NONE }}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{items}]")
        };
        let q = format!(
            "CREATE prompt:`{id}` SET \
                id = '{id}', \
                name = 'test-prompt', \
                description = '', \
                category = 'custom', \
                content = 'test', \
                variables = {vars_sql}, \
                created_at = time::now(), \
                updated_at = time::now()"
        );
        db.execute(&q).await.unwrap();
    }

    #[tokio::test]
    async fn captures_payload_with_prompt_id() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let kanban_id = kanban_agent_uuid();
        let target_id = target_agent_uuid();
        let prompt_id = prompt_uuid();
        seed_prompt(&state.db, &prompt_id, &["week"]).await;
        let tool =
            SubmitComposedCardTool::new(capture.clone(), kanban_id.clone(), state.db.clone());
        let result = tool
            .execute(json!({
                "title": "Weekly digest",
                "description": "Summarize PRs",
                "target_agent_id": target_id,
                "prompt_id": prompt_id,
                "variables": {"week": "21"}
            }))
            .await
            .unwrap();
        assert_eq!(result["success"], json!(true));

        let slot = capture.lock().await;
        let card = slot.as_ref().expect("payload captured");
        assert_eq!(card.title, "Weekly digest");
        assert_eq!(card.kanban_agent_id, kanban_id);
        assert_eq!(card.target_agent_id, target_id);
        assert_eq!(card.prompt_id.as_deref(), Some(prompt_id.as_str()));
        assert!(card.inline_prompt.is_none());
        assert!(card.variables.contains("week"));
    }

    #[tokio::test]
    async fn captures_payload_with_inline_prompt_and_empty_variables() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool =
            SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid(), state.db.clone());
        tool.execute(json!({
            "title": "Refactor login",
            "target_agent_id": target_agent_uuid(),
            "inline_prompt": "do the thing",
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        let card = slot.as_ref().unwrap();
        assert_eq!(card.inline_prompt.as_deref(), Some("do the thing"));
        assert!(card.prompt_id.is_none());
        assert_eq!(card.variables, "{}");
    }

    #[tokio::test]
    async fn second_call_overwrites_first() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool =
            SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid(), state.db.clone());
        tool.execute(json!({
            "title": "First",
            "target_agent_id": target_agent_uuid(),
            "inline_prompt": "x",
        }))
        .await
        .unwrap();
        tool.execute(json!({
            "title": "Second",
            "target_agent_id": target_agent_uuid(),
            "inline_prompt": "y",
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        assert_eq!(slot.as_ref().unwrap().title, "Second");
    }

    #[tokio::test]
    async fn rejects_both_prompt_id_and_inline_prompt() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "title": "x",
                "target_agent_id": target_agent_uuid(),
                "prompt_id": prompt_uuid(),
                "inline_prompt": "y",
            }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::ValidationFailed(_)),
            "got: {:?}",
            err
        );
        assert!(err.to_string().contains("mutually exclusive"));
    }

    #[tokio::test]
    async fn rejects_neither_prompt_id_nor_inline_prompt() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "title": "x",
                "target_agent_id": target_agent_uuid(),
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn rejects_invalid_target_agent_uuid() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "title": "x",
                "target_agent_id": "not-a-uuid",
                "inline_prompt": "y",
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        assert!(err.to_string().contains("target_agent_id"));
    }

    #[tokio::test]
    async fn rejects_missing_title() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "target_agent_id": target_agent_uuid(),
                "inline_prompt": "y",
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn accepts_empty_prompt_id_string_as_absent() {
        let (state, _g) = setup_test_state().await;
        // Some models fill optional string fields with "" rather than omitting them;
        // the tool must treat that as the XOR-compatible "absent" case.
        let capture = new_capture();
        let tool =
            SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid(), state.db.clone());
        tool.execute(json!({
            "title": "x",
            "target_agent_id": target_agent_uuid(),
            "prompt_id": "",
            "inline_prompt": "y",
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        assert!(slot.as_ref().unwrap().prompt_id.is_none());
    }

    #[tokio::test]
    async fn rejects_missing_variable_keys_for_prompt_id() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let prompt_id = prompt_uuid();
        seed_prompt(&state.db, &prompt_id, &["topic", "language"]).await;
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "title": "x",
                "target_agent_id": target_agent_uuid(),
                "prompt_id": prompt_id,
                "variables": { "topic": "Mistral AI" }
            }))
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::ValidationFailed(_)),
            "got: {:?}",
            err
        );
        assert!(err.to_string().contains("language"));
    }

    #[tokio::test]
    async fn rejects_prompt_id_not_found() {
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        let err = tool
            .execute(json!({
                "title": "x",
                "target_agent_id": target_agent_uuid(),
                "prompt_id": prompt_uuid(),
                "variables": {}
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn accepts_extra_variable_keys_beyond_prompt() {
        // Extra keys are allowed (forward-compat): only MISSING required ones fail.
        let (state, _g) = setup_test_state().await;
        let capture = new_capture();
        let prompt_id = prompt_uuid();
        seed_prompt(&state.db, &prompt_id, &["topic"]).await;
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid(), state.db.clone());
        tool.execute(json!({
            "title": "x",
            "target_agent_id": target_agent_uuid(),
            "prompt_id": prompt_id,
            "variables": { "topic": "X", "unused": "Y" }
        }))
        .await
        .unwrap();
    }
}
