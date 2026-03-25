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

//! Connection test commands for LLM providers.

use std::time::Instant;
use tauri::State;
use tracing::{info, instrument, warn};

use super::validate_provider_string;
use crate::commands::security::SecureKeyStore;
use crate::llm::ProviderType;
use crate::models::llm_models::ConnectionTestResult;
use crate::state::AppState;

/// Converts a `test_connection()` Result<bool, E> into a ConnectionTestResult.
fn connection_test_outcome(
    result: Result<bool, impl std::fmt::Display>,
    provider_type: ProviderType,
    start: Instant,
    label: &str,
) -> ConnectionTestResult {
    match result {
        Ok(true) => {
            let latency = start.elapsed().as_millis() as u64;
            info!(provider = %label, latency_ms = latency, "Connection successful");
            ConnectionTestResult::success(provider_type, latency, None)
        }
        Ok(false) => {
            warn!(provider = %label, "Connection returned false");
            ConnectionTestResult::failure(provider_type, "Connection test returned false".into())
        }
        Err(e) => {
            warn!(provider = %label, error = %e, "Connection failed");
            ConnectionTestResult::failure(provider_type, format!("Connection failed: {}", e))
        }
    }
}

/// Tests Mistral API connectivity by listing models.
async fn test_mistral_api(
    api_key: &str,
    provider_type: ProviderType,
    start: Instant,
) -> ConnectionTestResult {
    let client = match reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            return ConnectionTestResult::failure(
                provider_type,
                format!("Failed to create HTTP client: {}", e),
            )
        }
    };

    let response = client
        .get("https://api.mistral.ai/v1/models")
        .header("Authorization", format!("Bearer {}", api_key))
        .send()
        .await;

    let latency = start.elapsed().as_millis() as u64;

    match response {
        Ok(resp) if resp.status().is_success() => {
            info!(latency_ms = latency, "Mistral connection successful");
            ConnectionTestResult::success(provider_type, latency, None)
        }
        Ok(resp) => {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(status = %status, body = %body, "Mistral API error");
            ConnectionTestResult::failure(
                provider_type,
                format!("API error ({}): {}", status, body),
            )
        }
        Err(e) => {
            warn!(error = %e, "Mistral connection failed");
            ConnectionTestResult::failure(provider_type, format!("Connection failed: {}", e))
        }
    }
}

/// Tests connection to an LLM provider.
///
/// Delegates to provider-specific test logic and returns a unified result.
#[tauri::command]
#[instrument(name = "test_provider_connection", skip(state, keystore), fields(provider = %provider))]
pub async fn test_provider_connection(
    provider: String,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<ConnectionTestResult, String> {
    let provider_type = validate_provider_string(&provider)?;

    info!("Testing provider connection");

    let start = Instant::now();

    match provider_type {
        ProviderType::Ollama => {
            let result = state.llm_manager.ollama().test_connection().await;
            Ok(connection_test_outcome(
                result,
                provider_type,
                start,
                "ollama",
            ))
        }
        ProviderType::Mistral => {
            let api_key = match keystore.get_key("Mistral") {
                Some(key) => key,
                None => {
                    return Ok(ConnectionTestResult::failure(
                        provider_type,
                        "API key not configured".into(),
                    ));
                }
            };
            Ok(test_mistral_api(&api_key, provider_type, start).await)
        }
        ProviderType::Custom(ref name) => {
            let name = name.clone();
            match state.llm_manager.get_custom_provider(&name).await {
                Some(cp) => {
                    let result = cp.test_connection().await;
                    Ok(connection_test_outcome(result, provider_type, start, &name))
                }
                None => Ok(ConnectionTestResult::failure(
                    provider_type,
                    format!("Custom provider '{}' not found", name),
                )),
            }
        }
    }
}
