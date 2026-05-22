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
//
// `#[allow(dead_code)]` is transitional: this tool ships in the Phase 2
// commit but is first wired in by Phase 3's refactor of
// `commands/compose_card.rs`. The annotation MUST be removed in that
// commit. Unit tests already exercise the tool end-to-end.
#![allow(dead_code)]

use crate::commands::kanban_card::{
    validate_description, validate_title, validate_variables_json, validate_xor_prompt,
};
use crate::models::KanbanCardCreate;
use crate::security::validate_uuid_field;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
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
                "description": "UUID of a reusable prompt (mutually exclusive with inline_prompt)"
            },
            "inline_prompt": {
                "type": "string",
                "description": "Free-form prompt text (mutually exclusive with prompt_id)"
            },
            "variables": {
                "type": "object",
                "description": "Variable substitutions for the prompt. Use {} when none."
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
}

impl SubmitComposedCardTool {
    pub fn new(capture: Arc<Mutex<Option<KanbanCardCreate>>>, kanban_agent_id: String) -> Self {
        Self {
            capture,
            kanban_agent_id,
        }
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

        let card = KanbanCardCreate {
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

    #[tokio::test]
    async fn captures_payload_with_prompt_id() {
        let capture = new_capture();
        let kanban_id = kanban_agent_uuid();
        let target_id = target_agent_uuid();
        let prompt_id = prompt_uuid();
        let tool = SubmitComposedCardTool::new(capture.clone(), kanban_id.clone());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid());
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
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture, kanban_agent_uuid());
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
        // Some models fill optional string fields with "" rather than omitting them;
        // the tool must treat that as the XOR-compatible "absent" case.
        let capture = new_capture();
        let tool = SubmitComposedCardTool::new(capture.clone(), kanban_agent_uuid());
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
}
