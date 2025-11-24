// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod commands;
mod db;
mod models;
mod state;

use state::AppState;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

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

    // Register default simple agent
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

    // Run Tauri application
    tauri::Builder::default()
        .manage(app_state)
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            commands::workflow::create_workflow,
            commands::workflow::execute_workflow,
            commands::workflow::load_workflows,
            commands::workflow::delete_workflow,
            commands::agent::list_agents,
            commands::agent::get_agent_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
