// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! LLM configuration and execution commands

use crate::llm::ProviderType;
use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{info, instrument};

/// LLM provider status response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderStatus {
    /// Provider type
    pub provider: String,
    /// Whether the provider is configured
    pub configured: bool,
    /// Current default model
    pub default_model: String,
    /// Available models
    pub available_models: Vec<String>,
}

/// LLM configuration response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LLMConfigResponse {
    /// Active provider
    pub active_provider: String,
    /// Mistral configuration status
    pub mistral: ProviderStatus,
    /// Ollama configuration status
    pub ollama: ProviderStatus,
    /// Ollama server URL
    pub ollama_url: String,
}

/// Gets the current LLM configuration
#[tauri::command]
#[instrument(name = "get_llm_config", skip(state))]
pub async fn get_llm_config(state: State<'_, AppState>) -> Result<LLMConfigResponse, String> {
    let config = state.llm_manager.get_config().await;

    let mistral_status = ProviderStatus {
        provider: "Mistral".to_string(),
        configured: state
            .llm_manager
            .is_provider_configured(ProviderType::Mistral),
        default_model: config.mistral_model.clone(),
        available_models: state
            .llm_manager
            .get_available_models(ProviderType::Mistral),
    };

    let ollama_status = ProviderStatus {
        provider: "Ollama".to_string(),
        configured: state
            .llm_manager
            .is_provider_configured(ProviderType::Ollama),
        default_model: config.ollama_model.clone(),
        available_models: state.llm_manager.get_available_models(ProviderType::Ollama),
    };

    Ok(LLMConfigResponse {
        active_provider: config.active_provider.to_string(),
        mistral: mistral_status,
        ollama: ollama_status,
        ollama_url: config.ollama_url,
    })
}

/// Configures the Mistral provider with an API key
#[tauri::command]
#[instrument(name = "configure_mistral", skip(state, api_key))]
pub async fn configure_mistral(api_key: String, state: State<'_, AppState>) -> Result<(), String> {
    // Validate API key format
    if api_key.is_empty() {
        return Err("API key cannot be empty".to_string());
    }

    state
        .llm_manager
        .configure_mistral(&api_key)
        .await
        .map_err(|e| format!("Failed to configure Mistral: {}", e))?;

    info!("Mistral provider configured successfully");
    Ok(())
}

/// Configures the Ollama provider
#[tauri::command]
#[instrument(name = "configure_ollama", skip(state))]
pub async fn configure_ollama(
    url: Option<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state
        .llm_manager
        .configure_ollama(url.as_deref())
        .await
        .map_err(|e| format!("Failed to configure Ollama: {}", e))?;

    info!(url = ?url, "Ollama provider configured successfully");
    Ok(())
}

/// Sets the active LLM provider
#[tauri::command]
#[instrument(name = "set_active_provider", skip(state))]
pub async fn set_active_provider(
    provider: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let provider_type: ProviderType = provider
        .parse()
        .map_err(|_| format!("Invalid provider: {}. Use 'Mistral' or 'Ollama'", provider))?;

    state
        .llm_manager
        .set_active_provider(provider_type)
        .await
        .map_err(|e| format!("Failed to set active provider: {}", e))?;

    info!(?provider_type, "Active provider changed");
    Ok(())
}

/// Sets the default model for a provider
#[tauri::command]
#[instrument(name = "set_default_model", skip(state))]
pub async fn set_default_model(
    provider: String,
    model: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let provider_type: ProviderType = provider
        .parse()
        .map_err(|_| format!("Invalid provider: {}", provider))?;

    // Validate model is in available list
    let available = state.llm_manager.get_available_models(provider_type);
    if !available.contains(&model) {
        return Err(format!(
            "Model '{}' not in available models for {}: {:?}",
            model, provider, available
        ));
    }

    state
        .llm_manager
        .set_default_model(provider_type, &model)
        .await;

    info!(?provider_type, model, "Default model updated");
    Ok(())
}

/// Gets available models for a provider
#[tauri::command]
pub async fn get_available_models(
    provider: String,
    state: State<'_, AppState>,
) -> Result<Vec<String>, String> {
    let provider_type: ProviderType = provider
        .parse()
        .map_err(|_| format!("Invalid provider: {}", provider))?;

    Ok(state.llm_manager.get_available_models(provider_type))
}

/// Tests the Ollama connection
#[tauri::command]
#[instrument(name = "test_ollama_connection", skip(state))]
pub async fn test_ollama_connection(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .llm_manager
        .ollama()
        .test_connection()
        .await
        .map_err(|e| format!("Connection test failed: {}", e))
}

/// Executes a simple LLM completion (for testing)
#[tauri::command]
#[instrument(name = "test_llm_completion", skip(state, prompt))]
pub async fn test_llm_completion(
    prompt: String,
    provider: Option<String>,
    model: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Validate prompt
    if prompt.is_empty() {
        return Err("Prompt cannot be empty".to_string());
    }

    // If provider specified, use it; otherwise use active
    let response = if let Some(p) = provider {
        let provider_type: ProviderType =
            p.parse().map_err(|_| format!("Invalid provider: {}", p))?;
        state
            .llm_manager
            .complete_with_provider(
                provider_type,
                &prompt,
                Some("You are a helpful assistant."),
                model.as_deref(),
                0.7,
                1000,
            )
            .await
    } else {
        state
            .llm_manager
            .complete(
                &prompt,
                Some("You are a helpful assistant."),
                model.as_deref(),
                0.7,
                1000,
            )
            .await
    };

    response
        .map(|r| r.content)
        .map_err(|e| format!("LLM completion failed: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_status_serialization() {
        let status = ProviderStatus {
            provider: "Mistral".to_string(),
            configured: true,
            default_model: "mistral-large-latest".to_string(),
            available_models: vec!["mistral-large-latest".to_string()],
        };

        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ProviderStatus = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.provider, "Mistral");
        assert!(deserialized.configured);
    }

    #[test]
    fn test_llm_config_response_serialization() {
        let config = LLMConfigResponse {
            active_provider: "Ollama".to_string(),
            mistral: ProviderStatus {
                provider: "Mistral".to_string(),
                configured: false,
                default_model: "mistral-large-latest".to_string(),
                available_models: vec![],
            },
            ollama: ProviderStatus {
                provider: "Ollama".to_string(),
                configured: true,
                default_model: "llama3.2".to_string(),
                available_models: vec!["llama3.2".to_string()],
            },
            ollama_url: "http://localhost:11434".to_string(),
        };

        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains("active_provider"));
        assert!(json.contains("ollama_url"));
    }
}
