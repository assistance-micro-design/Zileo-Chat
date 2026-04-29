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

//! Shared request shape and POST helper for OpenAI-style tool chat completions.
//!
//! Factors the request body and the auth + send + parse pipeline shared by
//! `MistralProvider::complete_with_tools` and
//! `OpenAiCompatibleProvider::complete_with_tools`. Ollama uses a different
//! payload shape (no `tool_choice`, no `tools` array under that name) and
//! still owns its dedicated path.
//!
//! The provider-side response interpretation (parse_tool_calls,
//! extract_content, extract_thinking, ...) lives in the
//! [`ProviderToolAdapter`](super::tool_adapter::ProviderToolAdapter) trait,
//! which already implements the rest of the contract envisioned by the
//! spec for `ProviderToolFormat`.

use crate::llm::http;
use crate::llm::provider::LLMError;
use crate::llm::ToolCompletionParams;
use serde::Serialize;
use std::sync::Arc;
use tracing::{debug, info};

/// Body of an OpenAI-style chat-completions request that supports tools.
///
/// Used unchanged by Mistral and any OpenAI-compatible provider (OpenRouter,
/// Together, vLLM, etc.). All optional fields use `skip_serializing_if` so
/// providers that ignore them (e.g. `tools` for a tool-less call) get a
/// minimal payload.
#[derive(Debug, Serialize)]
pub(crate) struct ToolChatRequest {
    pub model: String,
    pub messages: Vec<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<String>,
}

impl ToolChatRequest {
    /// Build a request body from completion params, optionally rewriting the
    /// `messages` array (used by OpenAI-compat to apply prompt caching).
    pub(crate) fn from_params(
        params: &ToolCompletionParams,
        messages: Vec<serde_json::Value>,
    ) -> Self {
        Self {
            model: params.model.clone(),
            messages,
            temperature: Some(params.temperature),
            max_tokens: Some(params.max_tokens),
            tools: if params.tools.is_empty() {
                None
            } else {
                Some(params.tools.clone())
            },
            tool_choice: params.tool_choice.clone(),
            reasoning_effort: params
                .reasoning_effort
                .as_ref()
                .map(|e| e.as_str().to_string()),
        }
    }
}

/// POST a tool chat-completion request, return the parsed JSON response.
///
/// Common steps:
/// 1. POST `body` to `url` with `Authorization: Bearer <api_key>`
/// 2. Read the response body (HTTP-level errors → [`LLMError::RequestFailed`])
/// 3. Reject non-2xx by mapping the error JSON via [`http::parse_api_error`]
/// 4. Parse the success body as JSON via [`http::parse_json_response`]
/// 5. Log token usage if the response carries a `usage` object
///
/// Cancellation is handled at the upper layer (the future returned by the
/// HTTP client is dropped on cancel, which tears down the in-flight TCP
/// stream).
pub(crate) async fn send_tool_completion(
    http_client: &Arc<reqwest::Client>,
    provider_name: &str,
    url: &str,
    api_key: &str,
    body: &ToolChatRequest,
) -> Result<serde_json::Value, LLMError> {
    debug!(
        provider = provider_name,
        model = %body.model,
        temperature = ?body.temperature,
        max_tokens = ?body.max_tokens,
        reasoning_effort = ?body.reasoning_effort,
        tools_count = body.tools.as_ref().map(|t| t.len()).unwrap_or(0),
        "Sending tool completion request"
    );

    let (status, response_body) = http::send_and_read_body(
        http_client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(body)
            .send()
            .await,
    )
    .await?;

    if !status.is_success() {
        return Err(http::parse_api_error(provider_name, status, &response_body));
    }

    let json_response: serde_json::Value =
        http::parse_json_response(provider_name, &response_body)?;

    if let Some(usage) = json_response.get("usage") {
        let prompt_tokens = usage
            .get("prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let completion_tokens = usage
            .get("completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        info!(
            provider = provider_name,
            tokens_input = prompt_tokens,
            tokens_output = completion_tokens,
            "Tool completion successful"
        );
    }

    Ok(json_response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::agent::ReasoningEffort;
    use serde_json::json;

    fn sample_params() -> ToolCompletionParams {
        ToolCompletionParams {
            messages: vec![json!({"role": "user", "content": "hi"})],
            tools: vec![],
            tool_choice: None,
            model: "gpt-4".into(),
            temperature: 0.7,
            max_tokens: 1024,
            context_window: None,
            reasoning_effort: None,
        }
    }

    #[test]
    fn from_params_omits_empty_tools() {
        let body = ToolChatRequest::from_params(&sample_params(), vec![]);
        assert!(body.tools.is_none());
        let serialized = serde_json::to_value(&body).unwrap();
        assert!(serialized.get("tools").is_none());
    }

    #[test]
    fn from_params_keeps_tools_when_present() {
        let mut params = sample_params();
        params.tools = vec![json!({"type": "function", "function": {"name": "x"}})];
        let body = ToolChatRequest::from_params(&params, vec![]);
        assert!(body.tools.is_some());
        assert_eq!(body.tools.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn from_params_serializes_reasoning_effort_string() {
        let mut params = sample_params();
        params.reasoning_effort = Some(ReasoningEffort::High);
        let body = ToolChatRequest::from_params(&params, vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert_eq!(serialized["reasoning_effort"], "high");
    }

    #[test]
    fn from_params_preserves_low_medium_for_openai_compat() {
        // Regression guard: OpenAI-compat providers (OpenRouter, vLLM, ...) accept
        // low/medium/high. The Mistral-specific override lives in
        // `llm::mistral::build_mistral_tool_request`; from_params itself must keep
        // forwarding the value verbatim (via ReasoningEffort::as_str).
        for (effort, expected) in [
            (ReasoningEffort::Low, "low"),
            (ReasoningEffort::Medium, "medium"),
            (ReasoningEffort::High, "high"),
        ] {
            let mut params = sample_params();
            params.reasoning_effort = Some(effort.clone());
            let body = ToolChatRequest::from_params(&params, vec![]);
            let serialized = serde_json::to_value(&body).unwrap();
            assert_eq!(
                serialized["reasoning_effort"], expected,
                "OpenAI-compat must preserve {:?} as {:?}",
                effort, expected
            );
        }
    }

    #[test]
    fn from_params_skips_reasoning_effort_when_none() {
        let body = ToolChatRequest::from_params(&sample_params(), vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert!(serialized.get("reasoning_effort").is_none());
    }

    #[test]
    fn from_params_uses_provided_messages_not_params_messages() {
        let custom = vec![json!({"role": "system", "content": "rewritten"})];
        let body = ToolChatRequest::from_params(&sample_params(), custom.clone());
        assert_eq!(body.messages, custom);
    }
}
