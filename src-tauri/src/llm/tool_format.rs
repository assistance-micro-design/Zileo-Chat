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
use crate::llm::sse::{collect_sse_to_json, ProviderWireFormat};
use crate::llm::ToolCompletionParams;
use crate::models::agent::ReasoningEffort;
use serde::Serialize;
use std::sync::Arc;
use tracing::{info, warn};

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
    /// Wire-level streaming flag. When `Some(true)`, the request asks the
    /// provider to emit Server-Sent Events; the body is then reconstructed
    /// to look identical to the non-stream JSON response. This exists
    /// solely to defeat Cloudflare's ~100s origin-idle timeout on slow
    /// thinking models (no UI streaming, no adapter changes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// `stream_options.include_usage: true` asks OpenAI-compatible
    /// providers to emit a final empty-`choices` chunk that carries
    /// `usage`, so token counts survive the streaming path.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_options: Option<StreamOptions>,
    /// OpenRouter-style reasoning toggle (`reasoning: { effort: "high" }`).
    ///
    /// OpenAI's standard `reasoning_effort: "high"` parameter is silently
    /// ignored by some provider gateways (notably OpenRouter and RouterLab)
    /// — they only enable reasoning when the dedicated `reasoning` object
    /// is present. We send BOTH side-by-side so each gateway picks up
    /// whichever it understands. Mistral cloud overrides this through
    /// [`crate::llm::mistral::build_mistral_tool_request`] which clears
    /// the field (Mistral's API rejects unknown top-level keys for some
    /// model families).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningParam>,
}

/// `stream_options` payload for OpenAI-compatible chat completions.
#[derive(Debug, Serialize)]
pub(crate) struct StreamOptions {
    pub include_usage: bool,
}

/// `reasoning` payload (OpenRouter / RouterLab dialect).
///
/// Mirrored from
/// <https://openrouter.ai/docs/guides/best-practices/reasoning-tokens>.
///
/// Both fields are sent side-by-side because the gateways diverge on what
/// they actually honour:
/// - OpenRouter natif lit `effort` et le mappe selon le backend (OpenAI,
///   Anthropic 4.6+, Gemini, ...).
/// - RouterLab (proxy Anthropic Messages API) ignore `effort` et n'honore
///   que `max_tokens` (mappé sur `thinking.budget_tokens` côté Anthropic).
/// - Anthropic legacy via OpenRouter accepte `max_tokens` (budget tokens).
///
/// `skip_serializing_if` garantit qu'aucun champ vide ne pollue le payload
/// pour les providers stricts.
#[derive(Debug, Serialize)]
pub(crate) struct ReasoningParam {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

/// Maps a [`ReasoningEffort`] level to a thinking budget in tokens.
///
/// Used by gateways that ignore `reasoning.effort` and only honour the
/// Anthropic-style `reasoning.max_tokens` field (notably RouterLab, which
/// proxies via Anthropic Messages API). Values respect:
/// - >= 1024 (Anthropic minimum, below which `thinking.budget_tokens` errors)
/// - <= 8192 (DeepSeek-R1 quality cliff, cohérent with the OpenRouter
///   Anthropic legacy mapping for `effort: "high"`)
/// - < default `max_tokens` global (16384 chez Zileo), pour laisser de la
///   place a la reponse finale apres le budget de raisonnement
pub(crate) fn effort_to_max_tokens(effort: &ReasoningEffort) -> u32 {
    match effort {
        ReasoningEffort::Low => 2048,
        ReasoningEffort::Medium => 4096,
        ReasoningEffort::High => 8192,
    }
}

impl ToolChatRequest {
    /// Build a non-streaming request body from completion params, optionally
    /// rewriting the `messages` array (used by OpenAI-compat to apply prompt
    /// caching).
    ///
    /// Used for any call path that does not need wire-level streaming
    /// (embeddings, connectivity tests, ...). The two production tool-loop
    /// providers (Mistral, OpenAI-compat) call
    /// [`from_params_streaming`](Self::from_params_streaming) instead.
    pub(crate) fn from_params(
        params: &ToolCompletionParams,
        messages: Vec<serde_json::Value>,
    ) -> Self {
        let effort_str = params
            .reasoning_effort
            .as_ref()
            .map(|e| e.as_str().to_string());
        // Mirror reasoning_effort into the OpenRouter-style object so
        // provider gateways that only honour the `reasoning` shape
        // (RouterLab, OpenRouter, ...) still enable thinking. The
        // standard `reasoning_effort` field above stays for OpenAI /
        // Mistral dialects. Both `effort` and `max_tokens` are emitted
        // because gateways diverge: RouterLab proxies via Anthropic and
        // ignores `effort` entirely, only honouring `max_tokens`.
        // Mistral overrides BOTH explicitly via
        // `build_mistral_tool_request` to avoid sending the unknown
        // key.
        let reasoning_param = params
            .reasoning_effort
            .as_ref()
            .map(|effort| ReasoningParam {
                effort: Some(effort.as_str().to_string()),
                max_tokens: Some(effort_to_max_tokens(effort)),
            });
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
            reasoning_effort: effort_str,
            stream: None,
            stream_options: None,
            reasoning: reasoning_param,
        }
    }

    /// Same as [`from_params`](Self::from_params) but flips the wire to SSE.
    ///
    /// Sets `stream: true` and `stream_options.include_usage: true`. The
    /// response is reassembled by [`crate::llm::sse::collect_sse_to_json`]
    /// into a JSON value structurally identical to the non-stream body, so
    /// downstream adapters are unaffected.
    pub(crate) fn from_params_streaming(
        params: &ToolCompletionParams,
        messages: Vec<serde_json::Value>,
    ) -> Self {
        let mut body = Self::from_params(params, messages);
        body.stream = Some(true);
        body.stream_options = Some(StreamOptions {
            include_usage: true,
        });
        body
    }
}

/// POST a tool chat-completion request, return the parsed JSON response.
///
/// Common steps:
/// 1. POST `body` to `url` with `Authorization: Bearer <api_key>`
/// 2. Reject non-2xx by mapping the error JSON via [`http::parse_api_error`]
///    (the body is read with `.text()`; SSE is never parsed on errors)
/// 3. On success, choose between two body-collection paths:
///    - **Streaming** (`body.stream == Some(true)` AND response advertises
///      `text/event-stream`): hand the response body to
///      [`collect_sse_to_json`], which reassembles the deltas into the same
///      JSON shape the provider would have returned non-streaming.
///    - **Non-streaming**: classic `.text()` + `parse_json_response`.
/// 4. Log token usage if the resulting JSON carries a `usage` object.
///
/// Cancellation is handled at the upper layer
/// (`complete_with_tools_cancellable`): the future returned by this fn is
/// dropped on cancel, which closes the in-flight TCP stream and tears down
/// the SSE reader.
pub(crate) async fn send_tool_completion(
    http_client: &Arc<reqwest::Client>,
    provider_name: &str,
    url: &str,
    api_key: &str,
    body: &ToolChatRequest,
    wire_format: ProviderWireFormat,
) -> Result<serde_json::Value, LLMError> {
    info!(
        provider = provider_name,
        model = %body.model,
        temperature = ?body.temperature,
        max_tokens = ?body.max_tokens,
        reasoning_effort = ?body.reasoning_effort,
        tools_count = body.tools.as_ref().map(|t| t.len()).unwrap_or(0),
        stream = body.stream.unwrap_or(false),
        "Sending tool completion request"
    );

    let response = http_client
        .post(url)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(body)
        .send()
        .await
        .map_err(|e| LLMError::RequestFailed(format!("HTTP request failed: {}", e)))?;

    let status = response.status();

    // 4xx/5xx: drain the body as text and surface a structured error. SSE is
    // never parsed on the error path — proxies almost always serve error
    // bodies as JSON or plain text, even when streaming was requested.
    if !status.is_success() {
        let body_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read error response body: {}", e))
        })?;
        return Err(http::parse_api_error(provider_name, status, &body_text));
    }

    // 2xx: choose between streaming reassembly and the classic JSON path.
    let want_stream = body.stream == Some(true);
    let is_sse = http::is_sse_response(response.headers());

    let json_response: serde_json::Value = if want_stream && is_sse {
        collect_sse_to_json(response, wire_format).await?
    } else {
        if want_stream && !is_sse {
            warn!(
                provider = provider_name,
                content_type = ?response.headers().get(reqwest::header::CONTENT_TYPE),
                "Streaming requested but response is not SSE; falling back to text body"
            );
        }
        let body_text = response
            .text()
            .await
            .map_err(|e| LLMError::RequestFailed(format!("Failed to read response body: {}", e)))?;
        http::parse_json_response(provider_name, &body_text)?
    };

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

    #[test]
    fn from_params_omits_stream_fields_by_default() {
        let body = ToolChatRequest::from_params(&sample_params(), vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert!(serialized.get("stream").is_none());
        assert!(serialized.get("stream_options").is_none());
    }

    #[test]
    fn from_params_streaming_sets_stream_and_include_usage() {
        let body = ToolChatRequest::from_params_streaming(&sample_params(), vec![]);
        assert_eq!(body.stream, Some(true));
        assert!(body.stream_options.is_some());
        assert!(body.stream_options.as_ref().unwrap().include_usage);
    }

    #[test]
    fn from_params_streaming_serializes_correctly() {
        let body = ToolChatRequest::from_params_streaming(&sample_params(), vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert_eq!(serialized["stream"], true);
        assert_eq!(serialized["stream_options"]["include_usage"], true);
    }

    /// Regression: providers like RouterLab and OpenRouter only honour the
    /// OpenRouter-style `reasoning: { effort: "high" }` object. Sending only
    /// the standard `reasoning_effort` left them with thinking silently
    /// disabled. The tool format must mirror BOTH so each gateway picks up
    /// whichever it understands.
    #[test]
    fn from_params_emits_openrouter_reasoning_object() {
        let mut params = sample_params();
        params.reasoning_effort = Some(ReasoningEffort::High);
        let body = ToolChatRequest::from_params(&params, vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert_eq!(serialized["reasoning_effort"], "high");
        assert_eq!(serialized["reasoning"]["effort"], "high");
    }

    /// Without `reasoning_effort` the OpenRouter-style object must be
    /// omitted entirely (no empty `reasoning: {}` shape that would confuse
    /// gateways).
    #[test]
    fn from_params_omits_reasoning_object_when_effort_absent() {
        let body = ToolChatRequest::from_params(&sample_params(), vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert!(serialized.get("reasoning").is_none());
        assert!(serialized.get("reasoning_effort").is_none());
    }

    /// Regression: RouterLab proxies via Anthropic Messages API and
    /// silently ignores `reasoning.effort`. The only field it honours is
    /// `reasoning.max_tokens` (mapped to `thinking.budget_tokens` côté
    /// Anthropic). Verified empirically with a curl matrix on
    /// `kimi-k2.6` — without `max_tokens` the model returns no
    /// `reasoning_content`.
    #[test]
    fn from_params_emits_reasoning_max_tokens_for_routerlab() {
        let mut params = sample_params();
        params.reasoning_effort = Some(ReasoningEffort::High);
        let body = ToolChatRequest::from_params(&params, vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert_eq!(serialized["reasoning"]["max_tokens"], 8192);
    }

    /// `effort` and `max_tokens` must coexist in the same `reasoning`
    /// object so each gateway picks up whichever it understands. Sending
    /// only one would silently disable thinking on the gateways that
    /// honour the other.
    #[test]
    fn from_params_emits_both_effort_and_max_tokens() {
        let mut params = sample_params();
        params.reasoning_effort = Some(ReasoningEffort::Medium);
        let body = ToolChatRequest::from_params(&params, vec![]);
        let serialized = serde_json::to_value(&body).unwrap();
        assert_eq!(serialized["reasoning"]["effort"], "medium");
        assert_eq!(serialized["reasoning"]["max_tokens"], 4096);
    }

    /// Mapping enforced for every variant — picked from the doc matrix:
    /// `>= 1024` (Anthropic min) and `<= 8192` (DeepSeek-R1 quality cliff,
    /// cohérent with OpenRouter Anthropic legacy mapping for "high").
    #[test]
    fn effort_to_max_tokens_maps_each_variant() {
        assert_eq!(effort_to_max_tokens(&ReasoningEffort::Low), 2048);
        assert_eq!(effort_to_max_tokens(&ReasoningEffort::Medium), 4096);
        assert_eq!(effort_to_max_tokens(&ReasoningEffort::High), 8192);
    }

    /// Anthropic rejects `thinking.budget_tokens >= max_tokens` with a
    /// 400 error. Default `max_tokens` for tool completions in Zileo is
    /// 16384 (cf. `ToolCompletionParams::max_tokens` defaults), so every
    /// budget must stay strictly below that ceiling.
    #[test]
    fn effort_to_max_tokens_stays_below_default_max_tokens() {
        const DEFAULT_MAX_TOKENS: u32 = 16384;
        for effort in [
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
        ] {
            let budget = effort_to_max_tokens(&effort);
            assert!(
                budget < DEFAULT_MAX_TOKENS,
                "{:?} budget {} must be < {}",
                effort,
                budget,
                DEFAULT_MAX_TOKENS
            );
            assert!(
                budget >= 1024,
                "{:?} budget {} must be >= 1024 (Anthropic min)",
                effort,
                budget
            );
        }
    }

    #[test]
    fn from_params_streaming_preserves_tool_fields() {
        // Streaming flips wire flags but does not alter tool wiring or
        // reasoning_effort: regression guard for the bascule.
        let mut params = sample_params();
        params.tools = vec![json!({"type": "function", "function": {"name": "x"}})];
        params.reasoning_effort = Some(ReasoningEffort::High);
        let body = ToolChatRequest::from_params_streaming(&params, vec![]);
        assert_eq!(body.stream, Some(true));
        assert!(body.tools.is_some());
        assert_eq!(body.reasoning_effort, Some("high".to_string()));
    }
}
