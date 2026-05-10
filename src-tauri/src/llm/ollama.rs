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

//! Ollama local provider implementation via direct HTTP

use super::http;
use super::provider::{
    CompletionParams, LLMError, LLMResponse, ProviderType, ToolCompletionParams,
};
use crate::models::agent::ReasoningEffort;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, instrument};

/// Default Ollama server URL
pub const DEFAULT_OLLAMA_URL: &str = "http://localhost:11434";

/// Builds the `options` JSON object for Ollama API requests.
///
/// Includes `num_ctx` only when `context_window` is provided, otherwise
/// lets Ollama use its default context size.
fn build_options(
    temperature: f64,
    max_tokens: usize,
    context_window: Option<usize>,
) -> serde_json::Value {
    let mut options = serde_json::json!({
        "temperature": temperature,
        "num_predict": max_tokens
    });
    if let Some(ctx) = context_window {
        options["num_ctx"] = serde_json::json!(ctx);
    }
    options
}

/// Ollama local provider implementation
pub struct OllamaProvider {
    /// Server URL
    server_url: Arc<RwLock<String>>,
    /// Configured flag
    configured: Arc<RwLock<bool>>,
    /// Shared HTTP client for API calls (connection pooling)
    http_client: Arc<reqwest::Client>,
}

impl OllamaProvider {
    /// Creates a new Ollama provider with default settings and a shared HTTP client.
    ///
    /// The HTTP client provides connection pooling for better performance.
    pub fn new(http_client: Arc<reqwest::Client>) -> Self {
        Self {
            server_url: Arc::new(RwLock::new(DEFAULT_OLLAMA_URL.to_string())),
            configured: Arc::new(RwLock::new(false)),
            http_client,
        }
    }

    /// Configures the provider with the given server URL.
    pub async fn configure(&self, url: Option<&str>) -> Result<(), LLMError> {
        let server_url = url.unwrap_or(DEFAULT_OLLAMA_URL);
        *self.server_url.write().await = server_url.to_string();
        *self.configured.write().await = true;

        info!(url = server_url, "Ollama provider configured");
        Ok(())
    }

    /// Tests connection to the Ollama server
    pub async fn test_connection(&self) -> Result<bool, LLMError> {
        let url = self.server_url.read().await.clone();
        let test_url = format!("{}/api/version", url);

        let response = self
            .http_client
            .get(&test_url)
            .send()
            .await
            .map_err(|e| LLMError::ConnectionError(e.to_string()))?;

        Ok(response.status().is_success())
    }

    /// Makes a direct HTTP call to Ollama API with function calling support.
    ///
    /// This method sends tools definitions and handles tool_calls in responses.
    /// Uses Ollama's OpenAI-compatible API endpoint for tools.
    ///
    /// # Arguments
    /// * `messages` - Conversation history as JSON messages
    /// * `tools` - Tool definitions in OpenAI format
    /// * `model` - Model to use (must support tools: qwen2.5, llama3.1+, mistral)
    /// * `temperature` - Sampling temperature
    /// * `max_tokens` - Maximum tokens to generate
    ///
    /// # Returns
    /// Raw JSON response from the API (caller should use adapter to parse)
    ///
    /// # Note
    /// Not all Ollama models support tools. Recommended models:
    /// - qwen2.5 (best tool support)
    /// - llama3.1, llama3.2
    /// - mistral, mistral-nemo
    #[instrument(
        name = "ollama_complete_with_tools",
        skip(self, params),
        fields(provider = "ollama", model = %params.model, tools_count = params.tools.len())
    )]
    pub async fn complete_with_tools(
        &self,
        params: &ToolCompletionParams,
    ) -> Result<serde_json::Value, LLMError> {
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        let options = build_options(params.temperature, params.max_tokens, params.context_window);

        // Build request body with tools
        let mut body = serde_json::json!({
            "model": params.model,
            "messages": params.messages,
            "stream": false,
            "options": options
        });

        // Add tools if provided
        if !params.tools.is_empty() {
            body["tools"] = serde_json::json!(params.tools);
        }

        debug!(
            model = %params.model,
            temperature = params.temperature,
            max_tokens = params.max_tokens,
            tools_count = params.tools.len(),
            "Making Ollama API request with tools"
        );

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}: {}",
                    server_url, e
                ))
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read Ollama response: {}", e))
        })?;

        if !status.is_success() {
            return Err(http::parse_api_error("Ollama", status, &response_text));
        }

        let json_response: serde_json::Value = http::parse_json_response("Ollama", &response_text)?;

        // Log basic info
        let has_tool_calls = json_response
            .pointer("/message/tool_calls")
            .and_then(|v| v.as_array())
            .map(|arr| !arr.is_empty())
            .unwrap_or(false);

        info!(
            has_tool_calls = has_tool_calls,
            done = json_response
                .get("done")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            "Ollama tool completion successful"
        );

        Ok(json_response)
    }

    /// Makes a direct HTTP call to Ollama API with thinking support.
    ///
    /// Sends the `think` parameter to enable reasoning in supported models.
    /// Extracts `message.thinking` from the response as thinking content.
    async fn thinking_complete(
        &self,
        prompt: &str,
        system_text: &str,
        model: &str,
        effort: &ReasoningEffort,
        params: &CompletionParams,
    ) -> Result<LLMResponse, LLMError> {
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        let options = build_options(params.temperature, params.max_tokens, params.context_window);

        let body = serde_json::json!({
            "model": model,
            "messages": [
                { "role": "system", "content": system_text },
                { "role": "user", "content": prompt }
            ],
            "stream": false,
            "think": effort.as_str(),
            "options": options
        });

        debug!(model = model, "Making Ollama API request with thinking");

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}: {}",
                    server_url, e
                ))
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read Ollama response: {}", e))
        })?;

        if !status.is_success() {
            return Err(http::parse_api_error("Ollama", status, &response_text));
        }

        let json: serde_json::Value = http::parse_json_response("Ollama", &response_text)?;

        let content = json
            .pointer("/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let thinking_content = json
            .pointer("/message/thinking")
            .and_then(|v| v.as_str())
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.to_string());

        let tokens_input = json
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let tokens_output = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        info!(
            tokens_input = tokens_input,
            tokens_output = tokens_output,
            response_len = content.len(),
            has_thinking = thinking_content.is_some(),
            "Ollama thinking completion successful"
        );

        // Ollama folds reasoning into `eval_count` (no separate thinking
        // count). When `message.thinking` is present, we estimate from text
        // length so downstream display has a non-zero figure. Real billing is
        // not affected since Ollama is local and free.
        let thinking_tokens = thinking_content
            .as_ref()
            .map(|t| crate::llm::utils::estimate_tokens(t));

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model.to_string(),
            provider: ProviderType::Ollama,
            finish_reason: Some("stop".to_string()),
            thinking_tokens,
            thinking_content,
            // Ollama is local: no cache stats, no provider-side cost.
            cached_tokens: None,
            cache_write_tokens: None,
            provider_cost_usd: None,
        })
    }
}

impl OllamaProvider {
    /// Returns true if the provider has been configured via `configure()`.
    pub fn is_configured(&self) -> bool {
        // Use try_read to avoid blocking - returns false if lock unavailable
        self.configured
            .try_read()
            .map(|guard| *guard)
            .unwrap_or(false)
    }

    /// Generates a completion for the given prompt.
    pub async fn complete(&self, params: CompletionParams) -> Result<LLMResponse, LLMError> {
        let model_name = params.model.as_deref().unwrap_or("llama3.2");
        let system_text = params
            .system_prompt
            .as_deref()
            .unwrap_or("You are a helpful assistant.");

        // When reasoning_effort is set, use thinking path with `think` parameter
        if let Some(ref effort) = params.reasoning_effort {
            debug!(
                model = model_name,
                effort = ?effort,
                "Using direct HTTP call for Ollama thinking model"
            );
            return self
                .thinking_complete(&params.prompt, system_text, model_name, effort, &params)
                .await;
        }

        // Direct HTTP call to get real token counts from API
        let server_url = self.server_url.read().await.clone();
        let url = format!("{}/api/chat", server_url);

        let options = build_options(params.temperature, params.max_tokens, params.context_window);

        let body = serde_json::json!({
            "model": model_name,
            "messages": [
                { "role": "system", "content": system_text },
                { "role": "user", "content": &params.prompt }
            ],
            "stream": false,
            "options": options
        });

        debug!(
            model = model_name,
            temperature = params.temperature,
            max_tokens = params.max_tokens,
            "Starting Ollama completion"
        );

        let response = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                LLMError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}: {}",
                    server_url, e
                ))
            })?;

        let status = response.status();
        let response_text = response.text().await.map_err(|e| {
            LLMError::RequestFailed(format!("Failed to read Ollama response: {}", e))
        })?;

        if !status.is_success() {
            // Check for model not found in error response
            if response_text.contains("not found") || response_text.contains("model") {
                return Err(LLMError::ModelNotFound(format!(
                    "Model '{}' not found. Try: ollama pull {}",
                    model_name, model_name
                )));
            }
            return Err(http::parse_api_error("Ollama", status, &response_text));
        }

        let json: serde_json::Value = http::parse_json_response("Ollama", &response_text)?;

        let content = json
            .pointer("/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let tokens_input = json
            .get("prompt_eval_count")
            .and_then(|v| v.as_u64())
            .unwrap_or(0) as usize;

        let tokens_output = json.get("eval_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;

        info!(
            tokens_input = tokens_input,
            tokens_output = tokens_output,
            response_len = content.len(),
            "Ollama completion successful"
        );

        Ok(LLMResponse {
            content,
            tokens_input,
            tokens_output,
            model: model_name.to_string(),
            provider: ProviderType::Ollama,
            finish_reason: Some("stop".to_string()),
            thinking_content: None,
            thinking_tokens: None,
            cached_tokens: None,
            cache_write_tokens: None,
            provider_cost_usd: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates an OllamaProvider with a test HTTP client.
    fn test_ollama_provider() -> OllamaProvider {
        let http_client = Arc::new(
            reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("test HTTP client"),
        );
        OllamaProvider::new(http_client)
    }

    #[tokio::test]
    async fn test_ollama_provider_configure() {
        let provider = test_ollama_provider();

        // Initially not configured
        assert!(!provider.is_configured());

        // Configure
        let result = provider.configure(None).await;
        assert!(result.is_ok());

        // Now should be configured
        assert!(provider.is_configured());
    }

    #[tokio::test]
    async fn test_ollama_provider_complete_error_handling() {
        // Direct HTTP call will fail: ConnectionError if Ollama is not running,
        // or ModelNotFound if running but model doesn't exist
        let provider = test_ollama_provider();

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
            Err(LLMError::ConnectionError(_) | LLMError::ModelNotFound(_)) => {}
            Err(other) => panic!(
                "Expected ConnectionError or ModelNotFound, got: {:?}",
                other
            ),
            Ok(_) => panic!("Expected error"),
        }
    }

    #[test]
    fn test_thinking_complete_effort_mapping() {
        // Verify that effort.as_str() returns the correct string values
        // that will be sent as the "think" parameter to Ollama API
        assert_eq!(ReasoningEffort::Low.as_str(), "low");
        assert_eq!(ReasoningEffort::Medium.as_str(), "medium");
        assert_eq!(ReasoningEffort::High.as_str(), "high");

        // Verify JSON body construction uses effort string, not boolean
        let effort = ReasoningEffort::Low;
        let body = serde_json::json!({
            "think": effort.as_str(),
        });
        assert_eq!(body["think"], "low");

        let effort = ReasoningEffort::High;
        let body = serde_json::json!({
            "think": effort.as_str(),
        });
        assert_eq!(body["think"], "high");
    }

    #[test]
    fn test_build_options_includes_num_ctx_when_provided() {
        let options = build_options(0.7, 4096, Some(32768));
        assert_eq!(options["num_ctx"], 32768);
        assert_eq!(options["temperature"], 0.7);
        assert_eq!(options["num_predict"], 4096);
    }

    #[test]
    fn test_build_options_omits_num_ctx_when_none() {
        let options = build_options(0.7, 4096, None);
        assert!(options.get("num_ctx").is_none());
        assert_eq!(options["temperature"], 0.7);
        assert_eq!(options["num_predict"], 4096);
    }
}
