// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod commands;
mod db;
mod llm;
mod mcp;
mod models;
mod security;
mod state;
mod tools;

use state::AppState;
use tauri::Manager;
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

    // Note: Agents are loaded in setup hook after app_handle is set
    // This ensures AgentToolContext has access to app_handle for validation events

    // Load MCP servers from database
    if let Err(e) = app_state.mcp_manager.load_from_db().await {
        tracing::warn!(error = %e, "Failed to load MCP servers from database");
    } else {
        let count = app_state.mcp_manager.connected_count().await;
        tracing::info!(count = count, "MCP servers loaded from database");
    }

    // Seed builtin LLM models if needed
    {
        use models::llm_models::get_all_builtin_models;

        let builtin_models = get_all_builtin_models();
        let mut inserted = 0;

        for model in &builtin_models {
            let check_query = format!(
                "SELECT count() FROM llm_model WHERE id = '{}' GROUP ALL",
                model.id
            );
            let count_result: Vec<serde_json::Value> = app_state
                .db
                .db
                .query(&check_query)
                .await
                .map(|mut r| r.take(0).unwrap_or_default())
                .unwrap_or_default();

            let exists = count_result
                .first()
                .and_then(|v| v.get("count").and_then(|c| c.as_i64()))
                .unwrap_or(0)
                > 0;

            if !exists {
                let insert_query = format!(
                    "CREATE llm_model:`{}` CONTENT {{ \
                        id: '{}', \
                        provider: '{}', \
                        name: '{}', \
                        api_name: '{}', \
                        context_window: {}, \
                        max_output_tokens: {}, \
                        temperature_default: {}, \
                        is_builtin: true, \
                        created_at: time::now(), \
                        updated_at: time::now() \
                    }}",
                    model.id,
                    model.id,
                    model.provider,
                    model.name.replace('\'', "''"),
                    model.api_name.replace('\'', "''"),
                    model.context_window,
                    model.max_output_tokens,
                    model.temperature_default
                );

                if app_state.db.execute(&insert_query).await.is_ok() {
                    inserted += 1;
                }
            }
        }

        if inserted > 0 {
            tracing::info!(
                total = builtin_models.len(),
                inserted = inserted,
                "Builtin LLM models seeded"
            );
        } else {
            tracing::debug!("All builtin LLM models already exist");
        }
    }

    // Initialize secure keystore
    let keystore = commands::SecureKeyStore::default();
    tracing::info!("Secure keystore initialized");

    // Initialize LLM providers from saved configuration
    app_state.initialize_providers_from_config(&keystore).await;

    // Initialize embedding service from saved configuration (if any)
    app_state.initialize_embedding_from_config(&keystore).await;

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
            commands::workflow::load_workflow_full_state,
            // Agent commands (CRUD)
            commands::agent::list_agents,
            commands::agent::get_agent_config,
            commands::agent::create_agent,
            commands::agent::update_agent,
            commands::agent::delete_agent,
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
            commands::llm::test_mistral_connection,
            commands::llm::test_llm_completion,
            // Model CRUD commands
            commands::models::list_models,
            commands::models::get_model,
            commands::models::create_model,
            commands::models::update_model,
            commands::models::delete_model,
            commands::models::get_provider_settings,
            commands::models::update_provider_settings,
            commands::models::test_provider_connection,
            commands::models::seed_builtin_models,
            // Validation commands (Phase 5)
            commands::validation::create_validation_request,
            commands::validation::list_pending_validations,
            commands::validation::list_workflow_validations,
            commands::validation::approve_validation,
            commands::validation::reject_validation,
            commands::validation::delete_validation,
            // Memory commands (Phase 5)
            commands::memory::add_memory,
            commands::memory::list_memories,
            commands::memory::get_memory,
            commands::memory::delete_memory,
            commands::memory::search_memories,
            commands::memory::clear_memories_by_type,
            // Streaming commands (Phase 5)
            commands::streaming::execute_workflow_streaming,
            commands::streaming::cancel_workflow_streaming,
            // Message commands (Phase 6 - Message Persistence)
            commands::message::save_message,
            commands::message::load_workflow_messages,
            commands::message::load_workflow_messages_paginated,
            commands::message::delete_message,
            commands::message::clear_workflow_messages,
            // Tool execution commands (Phase 3 - Tool Execution Persistence)
            commands::tool_execution::save_tool_execution,
            commands::tool_execution::load_workflow_tool_executions,
            commands::tool_execution::load_message_tool_executions,
            commands::tool_execution::delete_tool_execution,
            commands::tool_execution::clear_workflow_tool_executions,
            // Thinking step commands (Phase 4 - Thinking Steps Persistence)
            commands::thinking::save_thinking_step,
            commands::thinking::load_workflow_thinking_steps,
            commands::thinking::load_message_thinking_steps,
            commands::thinking::delete_thinking_step,
            commands::thinking::clear_workflow_thinking_steps,
            // Task commands (Todo Tool)
            commands::task::create_task,
            commands::task::get_task,
            commands::task::list_workflow_tasks,
            commands::task::list_tasks_by_status,
            commands::task::update_task,
            commands::task::update_task_status,
            commands::task::complete_task,
            commands::task::delete_task,
            // MCP commands (Phase 3)
            commands::mcp::list_mcp_servers,
            commands::mcp::get_mcp_server,
            commands::mcp::create_mcp_server,
            commands::mcp::update_mcp_server,
            commands::mcp::delete_mcp_server,
            commands::mcp::test_mcp_server,
            commands::mcp::start_mcp_server,
            commands::mcp::stop_mcp_server,
            commands::mcp::list_mcp_tools,
            commands::mcp::call_mcp_tool,
            // Migration commands (Memory Tool Phase 2)
            commands::migration::migrate_memory_schema,
            commands::migration::get_memory_schema_status,
            // Embedding commands (Memory Tool Phase 5)
            commands::embedding::get_embedding_config,
            commands::embedding::save_embedding_config,
            commands::embedding::test_embedding,
            commands::embedding::get_memory_stats,
            commands::embedding::update_memory,
            commands::embedding::export_memories,
            commands::embedding::import_memories,
            commands::embedding::regenerate_embeddings,
        ])
        .setup(|app| {
            // Set the app handle in AppState for event emission (validation, etc.)
            // Uses std::sync::RwLock for synchronous access
            let state = app.state::<AppState>();
            let handle = app.handle().clone();
            if let Ok(mut guard) = state.inner().app_handle.write() {
                *guard = Some(handle);
                tracing::info!("App handle set in AppState for event emission");
            }

            // Load agents from database AFTER app_handle is set
            // This ensures AgentToolContext has access to app_handle for validation events
            // Clone the necessary data for the async task
            let db = state.inner().db.clone();
            let registry = state.inner().registry.clone();
            let orchestrator = state.inner().orchestrator.clone();
            let llm_manager = state.inner().llm_manager.clone();
            let tool_factory = state.inner().tool_factory.clone();
            let app_handle_clone = state.inner().app_handle.clone();
            let mcp_manager = state.inner().mcp_manager.clone();

            tauri::async_runtime::spawn(async move {
                // Suppress unused warning for orchestrator (used in context)
                let _ = &orchestrator;

                // Load agents from database
                let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, system_prompt FROM agent";
                let results: Vec<serde_json::Value> = match db.db.query(query).await {
                    Ok(mut r) => r.take(0).unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to query agents from database");
                        return;
                    }
                };

                let mut loaded = 0;
                for row in results {
                    let id = row["id"].as_str().unwrap_or("").to_string();
                    if id.is_empty() {
                        continue;
                    }

                    let name = row["name"].as_str().unwrap_or("Unknown").to_string();
                    let lifecycle_str = row["lifecycle"].as_str().unwrap_or("permanent");
                    let lifecycle = if lifecycle_str == "temporary" {
                        crate::models::Lifecycle::Temporary
                    } else {
                        crate::models::Lifecycle::Permanent
                    };

                    let llm_value = &row["llm"];
                    let llm = crate::models::LLMConfig {
                        provider: llm_value["provider"].as_str().unwrap_or("Mistral").to_string(),
                        model: llm_value["model"].as_str().unwrap_or("mistral-large-latest").to_string(),
                        temperature: llm_value["temperature"].as_f64().unwrap_or(0.7) as f32,
                        max_tokens: llm_value["max_tokens"].as_u64().unwrap_or(4096) as usize,
                    };

                    let tools: Vec<String> = row["tools"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    let mcp_servers_list: Vec<String> = row["mcp_servers"]
                        .as_array()
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    let system_prompt = row["system_prompt"].as_str().unwrap_or("You are a helpful assistant.").to_string();

                    let max_tool_iterations = row["max_tool_iterations"]
                        .as_u64()
                        .map(|v| v as usize)
                        .unwrap_or(50)
                        .clamp(1, 200);

                    let config = crate::models::AgentConfig {
                        id: id.clone(),
                        name,
                        lifecycle,
                        llm,
                        tools,
                        mcp_servers: mcp_servers_list,
                        system_prompt,
                        max_tool_iterations,
                    };

                    // Create agent context with app_handle
                    let app_handle = app_handle_clone.read().ok().and_then(|guard| guard.clone());
                    let context = crate::tools::AgentToolContext::new(
                        registry.clone(),
                        orchestrator.clone(),
                        llm_manager.clone(),
                        Some(mcp_manager.clone()),
                        tool_factory.clone(),
                        app_handle,
                    );

                    let llm_agent = crate::agents::LLMAgent::with_context(
                        config,
                        llm_manager.clone(),
                        tool_factory.clone(),
                        context,
                    );
                    registry.register(id, std::sync::Arc::new(llm_agent)).await;
                    loaded += 1;
                }

                tracing::info!(count = loaded, "Agents loaded from database");
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");

    Ok(())
}
