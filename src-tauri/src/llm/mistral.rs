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

//! Mistral AI provider implementation using rig-core
//!
//! Supports both standard chat models and reasoning models (Magistral).
//! Reasoning models return a different response format with thinking blocks
//! that requires custom HTTP handling.

use super::http::{self, ParsedContent};
use super::provider::{
    CompletionParams, LLMError, LLMResponse, ProviderType, ToolCompletionParams,
};
use super::sse::ProviderWireFormat;
use super::tool_format::{send_tool_completion, ToolChatRequest};
use crate::models::agent::ReasoningEffort;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// API request body for Mistral chat completions
#[derive(Debug, Serialize)]
struct MistralChatRequest {
    model: String,
    messages: Vec<MistralMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_effort: Option<String>,
}

/// Message in Mistral API format
#[derive(Debug, Serialize, Deserialize)]
struct MistralMessage {
    role: String,
    content: String,
}

/// API response from Mistral (handles both standard and reasoning models)
#[derive(Debug, Deserialize)]
struct MistralChatResponse {
    choices: Vec<MistralChoice>,
    usage: Option<MistralUsage>,
}

/// Choice in API response
#[derive(Debug, Deserialize)]
struct MistralChoice {
    message: MistralResponseMessage,
    finish_reason: Option<String>,
}

/// Response message - content can be string or array of content blocks
#[derive(Debug, Deserialize)]
struct MistralResponseMessage {
    #[serde(deserialize_with = "http::deserialize_content")]
    content: ParsedContent,
}

/// Usage statistics from API response
#[derive(Debug, Deserialize)]
struct MistralUsage {
    prompt_tokens: usize,
    completion_tokens: usize,
}

/// Mistral AI provider implementation.
///
/// All chat completions go through direct HTTP calls to expose real
/// `usage.prompt_tokens` / `usage.completion_tokens` (the rig client masks
/// usage behind its `Prompt::prompt()` API which returns only a `String`).
pub struct MistralProvider {
    /// API key (stored under RwLock for interior mutability across reconfigure)
    api_key: Arc<RwLock<Option<String>>>,
    /// Shared HTTP client for direct API calls (connection pooling)
    http_client: Arc<reqwest::Client>,
}

/// Mistral API base URL
const MISTRAL_API_URL: &str = "https://api.mistral.ai/v1/chat/completions";

impl MistralProvider {
    /// Creates a new unconfigured Mistral provider with a shared HTTP client.
    ///
    /// The HTTP client is used for direct API calls (chat, reasoning, tool calls)
    /// and provides connection pooling for better performance.
    pub fn new(http_client: Arc<reqwest::Client>) -> Self {
        Self {
            api_key: Arc::new(RwLock::new(None)),
            http_client,
        }
    }

    /// Configures the provider with an API key.
    pub async fn configure(&self, api_key: &str) -> Result<(), LLMError> {
        if api_key.is_empty() {
            return Err(LLMError::MissingApiKey("Mistral".to_string()));
        }
        *self.api_key.write().await = Some(api_key.to_string());
        info!("Mistral provider configured");
        Ok(())
    }

    /// Makes a direct HTTP call to Mistral API.
    /// Used for reasoning models that return a different response format.
    /// Sends `reasoning_effort` to control adjustable reasoning (e.g. mistral-small).
    async fn custom_complete(
        &self,
        prompt: &str,
        system_prompt: Option<&str>,
        model: &str,
        temperature: f64,
        max_tokens: usize,
        reasoning_effort: Option<&ReasoningEffort>,
    ) -> Result<LLMResponse, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        let system_text = system_prompt.unwrap_or("You are a helpful assistant.");

        // Build messages array
        let messages = vec![
            MistralMessage {
                role: "system".to_string(),
                content: system_text.to_string(),
            },
            MistralMessage {
                role: "user".to_string(),
                content: prompt.to_string(),
            },
        ];

        let request_body = MistralChatRequest {
            model: model.to_string(),
            messages,
            temperature: Some(temperature),
            max_tokens: Some(max_tokens),
            reasoning_effort: reasoning_effort
                .map(|e: &ReasoningEffort| e.to_mistral_str().to_string()),
        };

        debug!(
            model = model,
            temperature = temperature,
            max_tokens = max_tokens,
            reasoning_effort = ?reasoning_effort,
            "Making direct HTTP request to Mistral API (reasoning model)"
        );

        let (status, body) = http::send_and_read_body(
            self.http_client
                .post(MISTRAL_API_URL)
                .header("Authorization", format!("Bearer {}", api_key))
                .header("Content-Type", "application/json")
                .json(&request_body)
                .send()
                .await,
        )
        .await?;

        if !status.is_success() {
            return Err(http::parse_api_error("Mistral", status, &body));
        }

        let response = parse_mistral_chat_response(&body, model)?;

        if response.thinking_content.is_some() {
            info!(
                tokens_input = response.tokens_input,
                tokens_output = response.tokens_output,
                response_len = response.content.len(),
                thinking_len = response.thinking_content.as_ref().map_or(0, |t| t.len()),
                "Mistral reasoning model completion with thinking"
            );
        } else {
            info!(
                tokens_input = response.tokens_input,
                tokens_output = response.tokens_output,
                response_len = response.content.len(),
                "Mistral chat completion successful"
            );
        }

        Ok(response)
    }

    /// Makes a direct HTTP call to Mistral API with function calling support.
    ///
    /// This method sends tools definitions and handles tool_calls in responses.
    ///
    /// # Arguments
    /// * `messages` - Conversation history as JSON messages
    /// * `tools` - Tool definitions in OpenAI format
    /// * `tool_choice` - How the model should use tools ("auto", "any", "none")
    /// * `model` - Model to use
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    #[instrument(
        name = "mistral_complete_with_tools",
        skip(self, params),
        fields(provider = "mistral", model = %params.model, tools_count = params.tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        params: &ToolCompletionParams,
    ) -> Result<serde_json::Value, LLMError> {
        let api_key = self
            .api_key
            .read()
            .await
            .clone()
            .ok_or_else(|| LLMError::NotConfigured("Mistral".to_string()))?;

        let body = build_mistral_tool_request(params);
        send_tool_completion(
            &self.http_client,
            "Mistral",
            MISTRAL_API_URL,
            &api_key,
            &body,
            ProviderWireFormat::Mistral,
        )
        .await
    }
}

/// Parses a Mistral `/v1/chat/completions` JSON body into an `LLMResponse`.
///
/// Mistral does not expose cache stats or provider-side cost, so the
/// corresponding `LLMResponse` fields are always `None` here. `thinking_tokens`
/// is estimated from `thinking_content.len()` because Mistral folds reasoning
/// tokens into `usage.completion_tokens` (no separate `reasoning_tokens` field
/// — confirmed against the Mistral chat completions docs as of 2026-05).
///
/// Extracted as a standalone function so the parsing logic can be unit-tested
/// against fixture JSON without spinning up a mock HTTP server.
fn parse_mistral_chat_response(body: &str, model: &str) -> Result<LLMResponse, LLMError> {
    let chat_response: MistralChatResponse = http::parse_json_response("Mistral", body)?;

    let choice = chat_response
        .choices
        .into_iter()
        .next()
        .ok_or_else(|| LLMError::RequestFailed("No choices in response".to_string()))?;

    let parsed = choice.message.content;
    let content = parsed.text;
    let thinking_content = parsed.thinking;
    let finish_reason = choice.finish_reason;

    let (tokens_input, tokens_output) = chat_response
        .usage
        .map(|u| (u.prompt_tokens, u.completion_tokens))
        .unwrap_or((0, 0));

    Ok(LLMResponse {
        content,
        tokens_input,
        tokens_output,
        model: model.to_string(),
        provider: ProviderType::Mistral,
        finish_reason,
        thinking_tokens: thinking_content
            .as_ref()
            .map(|t| crate::llm::utils::estimate_tokens(t)),
        thinking_content,
        cached_tokens: None,
        cache_write_tokens: None,
        provider_cost_usd: None,
    })
}

/// Builds the Mistral-specific tool chat request body.
///
/// Uses the streaming wire (SSE) variant of
/// [`ToolChatRequest::from_params_streaming`] to defeat Cloudflare's
/// origin-idle timeout on slow thinking models, then overrides
/// `reasoning_effort` to use [`ReasoningEffort::to_mistral_str`], which
/// only emits the values accepted by the Mistral API (`"high"` or
/// omitted). OpenAI-compat providers (OpenRouter, vLLM, ...) keep the
/// default `low`/`medium`/`high` mapping.
fn build_mistral_tool_request(params: &ToolCompletionParams) -> ToolChatRequest {
    let mut body = ToolChatRequest::from_params_streaming(params, params.messages.clone());
    body.reasoning_effort = params
        .reasoning_effort
        .as_ref()
        .map(|e| e.to_mistral_str().to_string());
    // The OpenRouter-style `reasoning` object is rejected by Mistral cloud
    // (their schema only accepts `reasoning_effort` at the top level for
    // tool chat completions). Strip it so the request stays valid.
    body.reasoning = None;
    body
}

impl MistralProvider {
    /// Returns true if the provider has a configured API key.
    pub fn is_configured(&self) -> bool {
        // Use try_read to avoid blocking - returns false if lock unavailable
        self.api_key
            .try_read()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Generates a completion for the given prompt.
    ///
    /// All Mistral chat completions go through `custom_complete` (direct HTTP)
    /// so that real `usage.prompt_tokens` / `usage.completion_tokens` reach
    /// `LLMResponse`. The previous rig path returned only the assistant text
    /// (Mistral's rig `Prompt::prompt()` impl exposes a `String`), which forced
    /// us to estimate token counts heuristically and diverge from the API
    /// invoice.
    pub async fn complete(&self, params: CompletionParams) -> Result<LLMResponse, LLMError> {
        let model_name = params.model.as_deref().unwrap_or("mistral-large-latest");

        debug!(
            model = model_name,
            effort = ?params.reasoning_effort,
            temperature = params.temperature,
            max_tokens = params.max_tokens,
            "Starting Mistral completion (direct HTTP)"
        );

        self.custom_complete(
            &params.prompt,
            params.system_prompt.as_deref(),
            model_name,
            params.temperature,
            params.max_tokens,
            params.reasoning_effort.as_ref(),
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a MistralProvider with a test HTTP client.
    fn test_mistral_provider() -> MistralProvider {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .read_timeout(std::time::Duration::from_secs(
                    crate::constants::llm_http::DEFAULT_READ_TIMEOUT_SECS,
                ))
                .build()
                .expect("test HTTP client"),
        );
        MistralProvider::new(http_client)
    }

    #[tokio::test]
    async fn test_mistral_provider_configure() {
        let provider = test_mistral_provider();

        // Initially not configured
        assert!(!provider.is_configured());

        // Configure with a fake key (won't make real API calls in test)
        let result = provider.configure("test-api-key").await;
        assert!(result.is_ok());

        // Now should be configured
        assert!(provider.is_configured());
    }

    #[tokio::test]
    async fn test_mistral_provider_complete_not_configured() {
        let provider = test_mistral_provider();

        let result = provider
            .complete(CompletionParams {
                prompt: "Hello".to_string(),
                system_prompt: None,
                model: None,
                temperature: 0.7,
                max_tokens: 1000,
                reasoning_effort: None,
                context_window: None,
            })
            .await;

        assert!(result.is_err());
        match result {
            Err(LLMError::NotConfigured(_)) => {}
            _ => panic!("Expected NotConfigured error"),
        }
    }

    #[test]
    fn test_mistral_response_uses_shared_content_deserializer() {
        // Integration test: MistralResponseMessage uses http::deserialize_content
        let json = r#"{"content": [
            {"type": "thinking", "thinking": "Step 1"},
            {"type": "text", "text": "Answer"}
        ]}"#;
        let msg: MistralResponseMessage = serde_json::from_str(json).unwrap();
        assert_eq!(msg.content.text, "Answer");
        assert_eq!(msg.content.thinking, Some("Step 1".to_string()));
    }

    #[test]
    fn test_reasoning_effort_serialized_in_request() {
        // reasoning_effort should appear in JSON when Some
        let request = MistralChatRequest {
            model: "mistral-small-latest".to_string(),
            messages: vec![MistralMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            reasoning_effort: Some("high".to_string()),
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(json["reasoning_effort"], "high");
    }

    #[test]
    fn test_reasoning_effort_none_not_serialized() {
        // reasoning_effort should be omitted from JSON when None
        let request = MistralChatRequest {
            model: "mistral-small-latest".to_string(),
            messages: vec![MistralMessage {
                role: "user".to_string(),
                content: "Hello".to_string(),
            }],
            temperature: Some(0.7),
            max_tokens: Some(1000),
            reasoning_effort: None,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert!(json.get("reasoning_effort").is_none());
    }

    fn sample_tool_params(effort: Option<ReasoningEffort>) -> ToolCompletionParams {
        ToolCompletionParams {
            messages: vec![serde_json::json!({"role": "user", "content": "hi"})],
            tools: vec![],
            tool_choice: None,
            model: "mistral-medium-3.5".to_string(),
            temperature: 0.7,
            max_tokens: 1024,
            context_window: None,
            reasoning_effort: effort,
        }
    }

    #[test]
    fn build_mistral_tool_request_maps_high_to_high() {
        let params = sample_tool_params(Some(ReasoningEffort::High));
        let body = build_mistral_tool_request(&params);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["reasoning_effort"], "high");
    }

    #[test]
    fn build_mistral_tool_request_maps_low_medium_to_high() {
        // Mistral does not expose intensity levels: any explicit reasoning level
        // means "reasoning enabled" and is sent as "high". Disabling reasoning
        // is done by passing None (no field), not by selecting a level.
        for effort in [ReasoningEffort::Low, ReasoningEffort::Medium] {
            let params = sample_tool_params(Some(effort.clone()));
            let body = build_mistral_tool_request(&params);
            let json = serde_json::to_value(&body).unwrap();
            assert_eq!(
                json["reasoning_effort"], "high",
                "{:?} should map to \"high\" for Mistral",
                effort
            );
        }
    }

    #[test]
    fn build_mistral_tool_request_omits_reasoning_when_none() {
        let params = sample_tool_params(None);
        let body = build_mistral_tool_request(&params);
        let json = serde_json::to_value(&body).unwrap();
        assert!(json.get("reasoning_effort").is_none());
        assert!(json.get("reasoning").is_none());
    }

    /// Regression: the OpenRouter-style `reasoning: { effort }` object is
    /// added by the shared `from_params` builder so OpenAI-compat gateways
    /// pick it up. Mistral cloud rejects unknown top-level keys — strip the
    /// object so the request stays valid even when reasoning_effort is set.
    #[test]
    fn build_mistral_tool_request_strips_openrouter_reasoning_object() {
        let params = sample_tool_params(Some(ReasoningEffort::High));
        let body = build_mistral_tool_request(&params);
        let json = serde_json::to_value(&body).unwrap();
        assert_eq!(json["reasoning_effort"], "high");
        assert!(
            json.get("reasoning").is_none(),
            "Mistral request must NOT carry the OpenRouter-style \
             `reasoning` object, got: {}",
            json
        );
    }

    #[test]
    fn build_mistral_tool_request_preserves_messages_and_model() {
        let params = sample_tool_params(Some(ReasoningEffort::Medium));
        let body = build_mistral_tool_request(&params);
        assert_eq!(body.model, "mistral-medium-3.5");
        assert_eq!(body.messages, params.messages);
    }

    // parse_mistral_chat_response — exact tokens from API.

    #[test]
    fn parse_response_uses_real_tokens_from_usage() {
        let body = r#"{
            "choices": [{
                "message": {"role":"assistant","content":"Bonjour"},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 42, "completion_tokens": 18, "total_tokens": 60}
        }"#;
        let resp =
            parse_mistral_chat_response(body, "mistral-large-latest").expect("response parses");
        assert_eq!(resp.tokens_input, 42);
        assert_eq!(resp.tokens_output, 18);
        assert_eq!(resp.content, "Bonjour");
        assert_eq!(resp.provider, ProviderType::Mistral);
        // Mistral never reports cache stats or provider cost.
        assert_eq!(resp.cached_tokens, None);
        assert_eq!(resp.cache_write_tokens, None);
        assert_eq!(resp.provider_cost_usd, None);
    }

    #[test]
    fn parse_response_handles_magistral_thinking_blocks() {
        // Magistral returns content as an array of blocks (thinking + text).
        let body = r#"{
            "choices": [{
                "message": {"role":"assistant","content":[
                    {"type":"thinking","thinking":[{"type":"text","text":"Step 1"}]},
                    {"type":"text","text":"42"}
                ]},
                "finish_reason": "stop"
            }],
            "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30}
        }"#;
        let resp =
            parse_mistral_chat_response(body, "magistral-medium-2509").expect("response parses");
        assert_eq!(resp.content, "42");
        assert_eq!(resp.thinking_content, Some("Step 1".to_string()));
        // Mistral folds reasoning_tokens into completion_tokens — thinking_tokens
        // is therefore an estimate from thinking_content (not zero).
        assert!(resp.thinking_tokens.is_some());
        assert_eq!(resp.tokens_output, 20);
    }

    #[test]
    fn parse_response_falls_back_to_zero_when_usage_missing() {
        let body = r#"{
            "choices": [{
                "message": {"role":"assistant","content":"Hi"},
                "finish_reason": "stop"
            }]
        }"#;
        let resp = parse_mistral_chat_response(body, "test").expect("response parses");
        assert_eq!(resp.tokens_input, 0);
        assert_eq!(resp.tokens_output, 0);
    }

    #[test]
    fn parse_response_errors_when_no_choices() {
        let body = r#"{"choices":[]}"#;
        let result = parse_mistral_chat_response(body, "test");
        assert!(matches!(result, Err(LLMError::RequestFailed(_))));
    }
}
