// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod commands;
mod db;
mod llm;
mod models;
mod security;
mod state;

use state::AppState;
use std::sync::Arc;
use tracing_subscriber::{
    fmt::{self, format::FmtSpan},
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

/// Initializes the tracing subscriber with structured logging.
///
/// In debug mode, uses pretty console output.
/// In release mode, uses JSON format for machine parsing.
/// Controlled via RUST_LOG environment variable (default: info).
fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("zileo_chat=info,warn"));

    // Use JSON format in release, pretty format in debug
    #[cfg(not(debug_assertions))]
    {
        let json_layer = fmt::layer()
            .json()
            .with_target(true)
            .with_thread_ids(true)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(json_layer)
            .init();
    }

    #[cfg(debug_assertions)]
    {
        let fmt_layer = fmt::layer()
            .with_target(true)
            .with_thread_ids(false)
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .pretty();

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize structured logging
    init_tracing();

    // Get database path
    let app_data_dir = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());

    let db_path = format!("{}/.zileo/db", app_data_dir);
    std::fs::create_dir_all(format!("{}/.zileo", app_data_dir))?;

    // Initialize AppState
    let app_state = AppState::new(&db_path)
        .await
        .expect("Failed to initialize AppState");

    tracing::info!("Application state initialized");

    // Register default simple agent (demo, no LLM)
    {
        use agents::SimpleAgent;
        use models::{AgentConfig, LLMConfig, Lifecycle};

        let config = AgentConfig {
            id: "simple_agent".to_string(),
            name: "Simple Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "simple".to_string(),
                temperature: 0.7,
                max_tokens: 2000,
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "You are a simple demo agent for the base implementation.".to_string(),
        };

        let agent = SimpleAgent::new(config);
        app_state
            .registry
            .register("simple_agent".to_string(), Arc::new(agent))
            .await;
    }

    tracing::info!("Simple agent registered");

    // Register LLM agent for Ollama (local)
    {
        use agents::LLMAgent;
        use models::{AgentConfig, LLMConfig, Lifecycle};

        let ollama_config = AgentConfig {
            id: "ollama_agent".to_string(),
            name: "Ollama Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Ollama".to_string(),
                model: "llama3.2".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "You are a helpful AI assistant powered by Ollama. Provide clear, accurate, and helpful responses.".to_string(),
        };

        let ollama_agent = LLMAgent::new(ollama_config, app_state.llm_manager.clone());
        app_state
            .registry
            .register("ollama_agent".to_string(), Arc::new(ollama_agent))
            .await;
    }

    // Register LLM agent for Mistral (cloud)
    {
        use agents::LLMAgent;
        use models::{AgentConfig, LLMConfig, Lifecycle};

        let mistral_config = AgentConfig {
            id: "mistral_agent".to_string(),
            name: "Mistral Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Mistral".to_string(),
                model: "mistral-large-latest".to_string(),
                temperature: 0.7,
                max_tokens: 4096,
            },
            tools: vec![],
            mcp_servers: vec![],
            system_prompt: "You are a helpful AI assistant powered by Mistral AI. Provide clear, accurate, and helpful responses.".to_string(),
        };

        let mistral_agent = LLMAgent::new(mistral_config, app_state.llm_manager.clone());
        app_state
            .registry
            .register("mistral_agent".to_string(), Arc::new(mistral_agent))
            .await;
    }

    tracing::info!("LLM agents registered (ollama_agent, mistral_agent)");

    // Initialize secure keystore
    let keystore = commands::SecureKeyStore::default();
    tracing::info!("Secure keystore initialized");

    // Run Tauri application
    tauri::Builder::default()
        .manage(app_state)
        .manage(keystore)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            // Workflow commands
            commands::workflow::create_workflow,
            commands::workflow::execute_workflow,
            commands::workflow::load_workflows,
            commands::workflow::delete_workflow,
            // Agent commands
            commands::agent::list_agents,
            commands::agent::get_agent_config,
            // Security commands
            commands::security::save_api_key,
            commands::security::get_api_key,
            commands::security::delete_api_key,
            commands::security::has_api_key,
            commands::security::list_api_key_providers,
            // LLM commands
            commands::llm::get_llm_config,
            commands::llm::configure_mistral,
            commands::llm::configure_ollama,
            commands::llm::set_active_provider,
            commands::llm::set_default_model,
            commands::llm::get_available_models,
            commands::llm::test_ollama_connection,
            commands::llm::test_llm_completion,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
