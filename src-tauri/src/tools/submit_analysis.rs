// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! `SubmitAnalysisTool` — final step of the Kanban analyze flow.
//!
//! Captures an `AnalyzeReport` verdict into a shared `Arc<Mutex<_>>` slot.
//! The parent `kanban_analyzer.rs` flow picks the verdict up after
//! `tool_loop` returns, applies it to the card (column / error_summary) and
//! emits the appropriate Tauri event.
//!
//! Private tool: not registered, not factory-instantiable. Injected via the
//! `extra_tools` parameter of `tool_loop::execute_with_tools`.

use crate::commands::kanban_analyzer::{AnalyzeReport, AnalyzeVerdict};
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tokio::sync::Mutex;
use tracing::{info, warn};

const REASONING_MIN_CHARS: usize = 5;
const REASONING_MAX_CHARS: usize = 4_000;
const PROMPT_EDIT_MAX_CHARS: usize = 8_000;

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "SubmitAnalysisTool".to_string(),
    name: "SubmitAnalysis".to_string(),
    summary: "Submit your verdict on a worker agent's report. Call exactly once.".to_string(),
    description: ToolDescriptionBuilder::new(
        "Submit your verdict on a worker agent's report. Call exactly once when ready.",
    )
    .use_when(&[
        "You have read the worker's final report and decided whether it fulfils the user's demand",
        "You are ready to commit your verdict and end your turn",
    ])
    .do_not_use(&[
        "Before reading the worker's final report carefully",
        "More than once per analysis session - if you must correct, call again with the full corrected payload (latest call wins)",
    ])
    .operations(&[(
        "submit",
        "Submit the verdict. Required: verdict (approve|reject|needs_improvement), reasoning. \
         Required iff verdict=needs_improvement: suggested_prompt_edit (FULL new prompt text, not a diff).",
    )])
    .examples(&[
        json!({
            "verdict": "approve",
            "reasoning": "Report addresses every requirement and references the correct files."
        }),
        json!({
            "verdict": "reject",
            "reasoning": "Worker hallucinated file names that don't exist. Cannot be salvaged."
        }),
        json!({
            "verdict": "needs_improvement",
            "reasoning": "Worker stopped at the first error instead of trying alternatives.",
            "suggested_prompt_edit": "<full replacement prompt text here>"
        }),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "verdict": {
                "type": "string",
                "enum": ["approve", "reject", "needs_improvement"],
                "description": "Verdict on the worker's report."
            },
            "reasoning": {
                "type": "string",
                "description": "Justification (5-4000 chars, in the user's language).",
                "minLength": 5,
                "maxLength": 4000
            },
            "suggested_prompt_edit": {
                "type": "string",
                "description": "FULL replacement prompt text. Required iff verdict=needs_improvement.",
                "maxLength": 8000
            }
        },
        "required": ["verdict", "reasoning"]
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

/// Tool capturing an `AnalyzeReport` verdict for the analyze flow.
pub struct SubmitAnalysisTool {
    capture: Arc<Mutex<Option<AnalyzeReport>>>,
}

impl SubmitAnalysisTool {
    pub fn new(capture: Arc<Mutex<Option<AnalyzeReport>>>) -> Self {
        Self { capture }
    }

    fn parse_verdict(s: &str) -> ToolResult<AnalyzeVerdict> {
        match s.trim().to_lowercase().as_str() {
            "approve" => Ok(AnalyzeVerdict::Approve),
            "reject" => Ok(AnalyzeVerdict::Reject),
            "needs_improvement" => Ok(AnalyzeVerdict::NeedsImprovement),
            other => Err(ToolError::InvalidInput(format!(
                "verdict must be one of approve|reject|needs_improvement, got '{}'",
                other
            ))),
        }
    }
}

#[async_trait]
impl Tool for SubmitAnalysisTool {
    fn id(&self) -> &str {
        "SubmitAnalysisTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let verdict_str = input["verdict"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("verdict is required".to_string()))?;
        let verdict = Self::parse_verdict(verdict_str)?;

        let reasoning = input["reasoning"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("reasoning is required".to_string()))?
            .trim()
            .to_string();
        if reasoning.chars().count() < REASONING_MIN_CHARS {
            return Err(ToolError::ValidationFailed(format!(
                "reasoning is too short (min {} chars)",
                REASONING_MIN_CHARS
            )));
        }
        if reasoning.chars().count() > REASONING_MAX_CHARS {
            return Err(ToolError::ValidationFailed(format!(
                "reasoning exceeds {} chars",
                REASONING_MAX_CHARS
            )));
        }

        let suggested_prompt_edit = input["suggested_prompt_edit"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());

        match (&verdict, &suggested_prompt_edit) {
            (AnalyzeVerdict::NeedsImprovement, None) => {
                return Err(ToolError::ValidationFailed(
                    "suggested_prompt_edit is required when verdict is 'needs_improvement'"
                        .to_string(),
                ));
            }
            (AnalyzeVerdict::Approve | AnalyzeVerdict::Reject, Some(_)) => {
                // Non-fatal: still capture but warn — the field is meaningful
                // only for needs_improvement.
                warn!(
                    verdict = ?verdict,
                    "SubmitAnalysisTool received suggested_prompt_edit for a non-needs_improvement verdict; ignoring"
                );
            }
            _ => {}
        }

        if let Some(ref edit) = suggested_prompt_edit {
            if edit.chars().count() > PROMPT_EDIT_MAX_CHARS {
                return Err(ToolError::ValidationFailed(format!(
                    "suggested_prompt_edit exceeds {} chars",
                    PROMPT_EDIT_MAX_CHARS
                )));
            }
        }

        // For non-needs_improvement verdicts, force the field to None so
        // downstream consumers can rely on the invariant.
        let final_prompt_edit = match verdict {
            AnalyzeVerdict::NeedsImprovement => suggested_prompt_edit,
            _ => None,
        };

        let report = AnalyzeReport {
            verdict: verdict.clone(),
            reasoning,
            suggested_prompt_edit: final_prompt_edit,
        };

        let mut slot = self.capture.lock().await;
        if slot.is_some() {
            warn!(
                "SubmitAnalysisTool called more than once in the same session; \
                 keeping latest verdict and discarding the previous one"
            );
        }
        info!(verdict = ?verdict, "SubmitAnalysisTool captured verdict");
        *slot = Some(report);

        Ok(json!({
            "success": true,
            "message": "Verdict submitted. You may end your turn now."
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "expected a JSON object payload".to_string(),
            ));
        }
        if !input["verdict"].is_string() {
            return Err(ToolError::InvalidInput(
                "verdict is required and must be a string".to_string(),
            ));
        }
        if !input["reasoning"].is_string() {
            return Err(ToolError::InvalidInput(
                "reasoning is required and must be a string".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_capture() -> Arc<Mutex<Option<AnalyzeReport>>> {
        Arc::new(Mutex::new(None))
    }

    #[tokio::test]
    async fn captures_approve_verdict() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture.clone());
        tool.execute(json!({
            "verdict": "approve",
            "reasoning": "Report addresses every requirement."
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        let report = slot.as_ref().unwrap();
        assert_eq!(report.verdict, AnalyzeVerdict::Approve);
        assert!(report.suggested_prompt_edit.is_none());
    }

    #[tokio::test]
    async fn needs_improvement_requires_suggested_prompt_edit() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture);
        let err = tool
            .execute(json!({
                "verdict": "needs_improvement",
                "reasoning": "Worker bailed too early."
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
        assert!(err.to_string().contains("suggested_prompt_edit"));
    }

    #[tokio::test]
    async fn needs_improvement_with_edit_captured() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture.clone());
        tool.execute(json!({
            "verdict": "needs_improvement",
            "reasoning": "Try alternatives next time.",
            "suggested_prompt_edit": "Retry on first error before giving up."
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        let report = slot.as_ref().unwrap();
        assert_eq!(report.verdict, AnalyzeVerdict::NeedsImprovement);
        assert_eq!(
            report.suggested_prompt_edit.as_deref(),
            Some("Retry on first error before giving up.")
        );
    }

    #[tokio::test]
    async fn approve_strips_suggested_prompt_edit() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture.clone());
        tool.execute(json!({
            "verdict": "approve",
            "reasoning": "All good.",
            "suggested_prompt_edit": "ignored"
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        assert!(slot.as_ref().unwrap().suggested_prompt_edit.is_none());
    }

    #[tokio::test]
    async fn rejects_unknown_verdict() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture);
        let err = tool
            .execute(json!({
                "verdict": "looks_good",
                "reasoning": "All good."
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        assert!(err.to_string().contains("approve"));
    }

    #[tokio::test]
    async fn rejects_too_short_reasoning() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture);
        let err = tool
            .execute(json!({
                "verdict": "approve",
                "reasoning": "ok"
            }))
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
        assert!(err.to_string().contains("too short"));
    }

    #[tokio::test]
    async fn second_call_overwrites_first() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture.clone());
        tool.execute(json!({
            "verdict": "approve",
            "reasoning": "First take."
        }))
        .await
        .unwrap();
        tool.execute(json!({
            "verdict": "reject",
            "reasoning": "Second take after closer reading."
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        assert_eq!(slot.as_ref().unwrap().verdict, AnalyzeVerdict::Reject);
    }

    #[tokio::test]
    async fn accepts_uppercase_verdict() {
        let capture = new_capture();
        let tool = SubmitAnalysisTool::new(capture.clone());
        tool.execute(json!({
            "verdict": "APPROVE",
            "reasoning": "case-insensitive parsing"
        }))
        .await
        .unwrap();
        let slot = capture.lock().await;
        assert_eq!(slot.as_ref().unwrap().verdict, AnalyzeVerdict::Approve);
    }
}
