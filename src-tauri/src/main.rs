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

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agents;
mod commands;
mod constants;
mod db;
mod llm;
mod mcp;
mod models;
mod security;
mod state;
mod tools;

#[cfg(test)]
mod test_utils;

use state::AppState;
use tauri::menu::{
    AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{Emitter, Manager};
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

    // Note: Builtin model seeding is handled by the seed_builtin_models Tauri command
    // (invokable from frontend). get_all_builtin_models() returns an empty Vec.

    // Initialize secure keystore (synchronous, instant). Fail startup rather than
    // silently downgrading to unencrypted storage if secure keychain/AES setup fails.
    let keystore = commands::SecureKeyStore::new()
        .map_err(|e| anyhow::anyhow!("Failed to initialize secure keystore: {}", e))?;
    tracing::info!("Secure keystore initialized");

    // Run MCP loading, provider init, and embedding init in parallel.
    // MCP is fully independent. Providers and embedding both need keystore
    // (already initialized above) but are independent of each other.
    let (mcp_result, _, _) = tokio::join!(
        app_state.mcp_manager.load_from_db(),
        app_state.initialize_providers_from_config(&keystore),
        app_state.initialize_embedding_from_config(&keystore),
    );

    if let Err(e) = mcp_result {
        tracing::warn!(error = %e, "Failed to load MCP servers from database");
    } else {
        let count = app_state.mcp_manager.connected_count().await;
        tracing::info!(count = count, "MCP servers loaded from database");
    }

    // Run Tauri application
    let app = tauri::Builder::default()
        .manage(app_state)
        .manage(keystore)
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            // Workflow commands
            commands::workflow::create_workflow,
            commands::workflow::load_workflows,
            commands::workflow::rename_workflow,
            commands::workflow::delete_workflow,
            commands::workflow::delete_workflows_batch,
            commands::workflow::load_workflow_full_state,
            commands::workflow::move_workflow_to_folder,
            commands::workflow::move_workflows_to_folder,
            commands::workflow::toggle_workflow_pinned,
            // Workflow folder commands
            commands::workflow_folder::create_workflow_folder,
            commands::workflow_folder::list_workflow_folders,
            commands::workflow_folder::rename_workflow_folder,
            commands::workflow_folder::update_folder_color,
            commands::workflow_folder::delete_workflow_folder,
            commands::workflow_folder::reorder_workflow_folders,
            // Kanban card commands
            commands::kanban_card::create_kanban_card,
            commands::kanban_card::get_kanban_card,
            commands::kanban_card::list_kanban_cards,
            commands::kanban_card::update_kanban_card,
            commands::kanban_card::delete_kanban_card,
            commands::kanban_card::move_kanban_card,
            commands::kanban_card::duplicate_kanban_card_as_template,
            commands::kanban_card::set_kanban_card_workflow_id,
            commands::kanban_analyzer::analyze_card_report,
            commands::kanban_card_chat::open_card_review_chat,
            commands::kanban_interaction::load_card_interactions,
            // Kanban schedule commands
            commands::kanban_schedule::create_kanban_schedule,
            commands::kanban_schedule::get_kanban_schedule,
            commands::kanban_schedule::list_kanban_schedules,
            commands::kanban_schedule::update_kanban_schedule,
            commands::kanban_schedule::delete_kanban_schedule,
            commands::compose_card::compose_card_from_description,
            // Agent commands (CRUD)
            commands::agent::list_agents,
            commands::agent::get_agent_config,
            commands::agent::create_agent,
            commands::agent::update_agent,
            commands::agent::delete_agent,
            // Security commands
            commands::security::save_api_key,
            commands::security::delete_api_key,
            commands::security::has_api_key,
            commands::security::list_api_key_providers,
            // Custom provider commands
            commands::custom_provider::list_providers,
            commands::custom_provider::create_custom_provider,
            commands::custom_provider::update_custom_provider,
            commands::custom_provider::delete_custom_provider,
            // Model CRUD commands
            commands::llm_models::crud::list_models,
            commands::llm_models::crud::get_model,
            commands::llm_models::crud::get_model_by_api_name,
            commands::llm_models::crud::create_model,
            commands::llm_models::crud::update_model,
            commands::llm_models::crud::delete_model,
            commands::llm_models::provider_settings::get_provider_settings,
            commands::llm_models::provider_settings::update_provider_settings,
            commands::llm_models::connection::test_provider_connection,
            commands::llm_models::seed::seed_builtin_models,
            commands::validation::create_validation_request,
            commands::validation::list_pending_validations,
            commands::validation::list_workflow_validations,
            commands::validation::approve_validation,
            commands::validation::reject_validation,
            commands::validation::delete_validation,
            commands::validation::get_validation_settings,
            commands::validation::update_validation_settings,
            commands::validation::reset_validation_settings,
            // Tool discovery for validation settings
            commands::validation::list_available_tools,
            // Validation audit log
            commands::validation_audit::list_validation_audit,
            commands::validation_audit::get_validation_audit_stats,
            commands::validation_audit::purge_validation_audit_now,
            commands::validation_audit::export_validation_audit_csv,
            commands::memory::add_memory,
            commands::memory::list_memories,
            commands::memory::get_memory,
            commands::memory::delete_memory,
            commands::memory::search_memories,
            commands::memory::clear_memories_by_type,
            commands::memory::purge_expired_memories,
            commands::streaming::execution::execute_workflow_streaming,
            commands::streaming::execution::cancel_workflow_streaming,
            commands::message::save_message,
            commands::message::load_workflow_messages,
            commands::message::load_workflow_messages_paginated,
            commands::message::delete_message,
            commands::message::load_workflow_blocks,
            commands::message::get_workflow_last_assistant_message_metrics,
            commands::tool_execution::save_tool_execution,
            commands::tool_execution::get_tool_execution,
            commands::tool_execution::load_workflow_tool_executions,
            commands::tool_execution::load_message_tool_executions,
            commands::tool_execution::delete_tool_execution,
            commands::tool_execution::clear_workflow_tool_executions,
            commands::thinking::save_thinking_step,
            commands::thinking::load_workflow_thinking_steps,
            commands::thinking::load_message_thinking_steps,
            commands::thinking::delete_thinking_step,
            commands::thinking::clear_workflow_thinking_steps,
            // Sub-agent execution commands (Activity persistence)
            commands::sub_agent_execution::load_workflow_sub_agent_executions,
            commands::sub_agent_execution::clear_workflow_sub_agent_executions,
            // Task commands (Todo Tool)
            commands::task::create_task,
            commands::task::get_task,
            commands::task::list_workflow_tasks,
            commands::task::list_tasks_by_status,
            commands::task::update_task,
            commands::task::update_task_status,
            commands::task::complete_task,
            commands::task::delete_task,
            commands::mcp::crud::list_mcp_servers,
            commands::mcp::crud::get_mcp_server,
            commands::mcp::crud::create_mcp_server,
            commands::mcp::crud::update_mcp_server,
            commands::mcp::crud::delete_mcp_server,
            commands::mcp::lifecycle::test_mcp_server,
            commands::mcp::lifecycle::start_mcp_server,
            commands::mcp::lifecycle::stop_mcp_server,
            commands::mcp::lifecycle::list_mcp_legacy_http_auth,
            commands::mcp::tools::list_mcp_tools,
            commands::mcp::tools::call_mcp_tool,
            commands::mcp::tools::get_mcp_latency_metrics,
            commands::migration::get_memory_schema_status,
            commands::migration::migrate_mcp_http_schema,
            commands::migration::migrate_memory_v2_schema,
            commands::migration::migrate_reasoning_effort,
            commands::migration::migrate_sidebar_features,
            commands::migration::migrate_mcp_auth_v1,
            commands::migration::migrate_token_cost_accuracy_v1,
            commands::embedding::config::get_embedding_config,
            commands::embedding::config::save_embedding_config,
            commands::embedding::config::delete_embedding_config,
            commands::embedding::config::reinit_embedding_service,
            commands::embedding::config::test_embedding,
            commands::embedding::operations::update_memory,
            commands::embedding::operations::export_memories,
            commands::embedding::operations::import_memories,
            commands::embedding::operations::reindex_memory_chunks,
            commands::embedding::operations::cancel_reindex_job,
            commands::embedding::operations::get_reindex_job_status,
            commands::embedding::stats::get_memory_stats,
            commands::embedding::stats::get_memory_token_stats,
            // Prompt commands (Prompt Library)
            commands::prompt::list_prompts,
            commands::prompt::get_prompt,
            commands::prompt::create_prompt,
            commands::prompt::update_prompt,
            commands::prompt::delete_prompt,
            commands::prompt::search_prompts,
            // Prompt version commands
            commands::prompt_version::list_prompt_versions,
            commands::prompt_version::get_prompt_version,
            commands::prompt_version::restore_prompt_version,
            commands::prompt_version::delete_prompt_version,
            // Skill commands (Tool Skills)
            commands::skill::list_skills,
            commands::skill::get_skill,
            commands::skill::create_skill,
            commands::skill::update_skill,
            commands::skill::delete_skill,
            // Skill version commands
            commands::skill_version::list_skill_versions,
            commands::skill_version::get_skill_version,
            commands::skill_version::restore_skill_version,
            commands::skill_version::delete_skill_version,
            // FileManager commands
            commands::file_manager::validate_agent_folder,
            commands::file_manager::list_trash,
            commands::file_manager::restore_from_trash_cmd,
            commands::file_manager::read_image_for_attachment,
            // Import/Export commands
            commands::import_export::export::prepare_export_preview,
            commands::import_export::export::generate_export_file,
            commands::import_export::export::save_export_to_file,
            commands::import_export::import::validate_import,
            commands::import_export::import::execute_import,
            commands::user_question::submit_user_response,
            commands::user_question::get_pending_questions,
            commands::user_question::skip_question,
            // Speech-to-text (Voxtral) commands
            commands::stt::transcribe_audio,
            commands::settings_stt::get_stt_settings,
            commands::settings_stt::update_stt_settings,
            commands::settings_stt::reset_stt_settings,
        ])
        .setup(|app| {
            let legal_notice = MenuItemBuilder::with_id("legal-notice", "Mentions l\u{00e9}gales")
                .build(app)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let privacy_policy =
                MenuItemBuilder::with_id("privacy-policy", "Politique de confidentialit\u{00e9} & RGPD")
                    .build(app)
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let separator = PredefinedMenuItem::separator(app)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
            let about = PredefinedMenuItem::about(
                app,
                Some("À propos de Zileo Chat"),
                Some(AboutMetadata {
                    name: Some("Zileo Chat".into()),
                    version: Some(env!("CARGO_PKG_VERSION").into()),
                    copyright: Some("© 2025 Assistance Micro Design".into()),
                    ..Default::default()
                }),
            )
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            let help_menu = SubmenuBuilder::new(app, "Aide")
                .items(&[&legal_notice, &privacy_policy, &separator, &about])
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            let menu = MenuBuilder::new(app)
                .item(&help_menu)
                .build()
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            app.set_menu(menu)
                .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;

            // WebKitGTK refuses getUserMedia by default. Auto-allow ONLY
            // microphone (UserMediaPermissionRequest) so the dictation FAB
            // can capture audio. Any other permission class (geolocation,
            // notifications, clipboard-read, ...) is denied to keep the
            // surface tight even if a future feature triggers a new request.
            // No-op on macOS / Windows (handled by their native webviews).
            #[cfg(target_os = "linux")]
            {
                use webkit2gtk::{
                    gio::prelude::Cast, PermissionRequestExt, UserMediaPermissionRequest,
                    UserMediaPermissionRequestExt, WebViewExt,
                };
                if let Some(main_window) = app.get_webview_window("main") {
                    main_window
                        .with_webview(|webview| {
                            webview.inner().connect_permission_request(
                                |_webview, permission_request| {
                                    if let Some(media_request) = permission_request
                                        .downcast_ref::<UserMediaPermissionRequest>(
                                    ) {
                                        if media_request.is_for_audio_device() {
                                            permission_request.allow();
                                            return true;
                                        }
                                    }
                                    permission_request.deny();
                                    true
                                },
                            );
                        })
                        .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                    tracing::info!("WebKitGTK permission handler installed (mic-only)");
                } else {
                    tracing::warn!(
                        "Could not locate 'main' webview window; mic permission may fail"
                    );
                }
            }

            // Handle menu events - emit Tauri events for frontend to listen
            app.on_menu_event(move |app_handle, event| {
                let event_id = event.id().as_ref();
                match event_id {
                    "legal-notice" => {
                        if let Err(e) = app_handle.emit("open-legal-notice", ()) {
                            tracing::warn!(error = %e, "Failed to emit open-legal-notice event");
                        }
                    }
                    "privacy-policy" => {
                        if let Err(e) = app_handle.emit("open-privacy-policy", ()) {
                            tracing::warn!(error = %e, "Failed to emit open-privacy-policy event");
                        }
                    }
                    _ => {}
                }
            });

            tracing::info!("Native Help menu initialized with legal notices");

            // Set the app handle in AppState for event emission (validation, etc.)
            // Uses std::sync::RwLock for synchronous access
            let state = app.state::<AppState>();
            let handle = app.handle().clone();
            if let Ok(mut guard) = state.inner().app_handle.write() {
                *guard = Some(handle);
                tracing::info!("App handle set in AppState for event emission");
            }

            // Spawn the Kanban scheduler.
            // The shutdown flag is polled on every tick; cleanup in
            // RunEvent::ExitRequested flips it so the loop exits cleanly.
            {
                let db = state.inner().db.clone();
                let app_handle = app.handle().clone();
                let shutdown = state.inner().kanban_scheduler_shutdown.clone();
                let handle = commands::scheduler::spawn_kanban_scheduler_task(
                    db,
                    app_handle,
                    shutdown,
                );
                let slot = state.inner().kanban_scheduler_handle.clone();
                tauri::async_runtime::spawn(async move {
                    *slot.lock().await = Some(handle);
                });
                tracing::info!("Kanban scheduler task spawned");
            }

            // Listen for workflow_complete events to update kanban cards.
            // When a workflow tied to a card finishes (success or failure), the
            // card transitions to the review column and the error_summary is
            // stamped on the failure path.
            {
                use tauri::Listener;
                let db = state.inner().db.clone();
                let llm_manager = state.inner().llm_manager.clone();
                let tool_factory = state.inner().tool_factory.clone();
                let mcp_manager = state.inner().mcp_manager.clone();
                let app_handle_for_listener = app.handle().clone();
                app.handle().listen("workflow_complete", move |evt| {
                    let payload: serde_json::Value = match serde_json::from_str(evt.payload()) {
                        Ok(v) => v,
                        Err(e) => {
                            tracing::warn!(error = %e, "workflow_complete payload not JSON");
                            return;
                        }
                    };
                    let workflow_id = match payload["workflow_id"].as_str() {
                        Some(s) => s.to_string(),
                        None => {
                            tracing::warn!("workflow_complete missing workflow_id");
                            return;
                        }
                    };
                    let status = payload["status"].as_str().unwrap_or("").to_string();
                    let error_message = payload["error"].as_str().map(String::from);
                    let success = status == "completed";
                    let db = db.clone();
                    let llm_manager = llm_manager.clone();
                    let tool_factory = tool_factory.clone();
                    let mcp_manager = mcp_manager.clone();
                    let app_handle = app_handle_for_listener.clone();
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = commands::scheduler::mark_card_done_core(
                            &db,
                            &workflow_id,
                            success,
                            error_message.as_deref(),
                        )
                        .await
                        {
                            tracing::warn!(
                                workflow_id = %workflow_id,
                                error = %e,
                                "Failed to update kanban card after workflow completion"
                            );
                            return;
                        }

                        // C: event-driven promotion. The card we just finished
                        // freed a slot; immediately try to promote the next
                        // ready card so the user does not wait up to 60 s for
                        // the scheduler tick. Best-effort.
                        if let Err(e) = commands::scheduler::start_next_pending_card_core(
                            &db,
                            &app_handle,
                        )
                        .await
                        {
                            tracing::warn!(
                                workflow_id = %workflow_id,
                                error = %e,
                                "Event-driven promotion failed; will retry at next scheduler tick"
                            );
                        }

                        // On success, trigger auto-analyze if the Kanban agent
                        // has the flag enabled. Best-effort: any failure here is
                        // logged but does not affect the user-visible flow.
                        if !success {
                            return;
                        }
                        let card_id = match commands::scheduler::card_id_for_workflow(
                            &db,
                            &workflow_id,
                        )
                        .await
                        {
                            Ok(Some(id)) => id,
                            Ok(None) => return, // not a kanban-spawned workflow
                            Err(e) => {
                                tracing::warn!(
                                    workflow_id = %workflow_id,
                                    error = %e,
                                    "Failed to resolve card_id for auto-analyze"
                                );
                                return;
                            }
                        };
                        if let Err(e) = commands::kanban_analyzer::analyze_card_report_core(
                            &db,
                            &tool_factory,
                            &mcp_manager,
                            &llm_manager,
                            &app_handle,
                            &card_id,
                        )
                        .await
                        {
                            tracing::warn!(
                                card_id = %card_id,
                                error = %e,
                                "Auto-analyze failed (non-fatal)"
                            );
                        }
                    });
                });
                tracing::info!("Kanban workflow_complete listener registered");
            }

            // Boot-time catch-up: re-analyze cards that finished into `review`
            // but were never analyzed (app closed mid-workflow, or an earlier
            // auto-analyze failed silently). Detached so it never blocks
            // startup — each analysis fires a full LLM tool loop. Best-effort:
            // failures are logged inside the core helper.
            {
                let db = state.inner().db.clone();
                let llm_manager = state.inner().llm_manager.clone();
                let tool_factory = state.inner().tool_factory.clone();
                let mcp_manager = state.inner().mcp_manager.clone();
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    match commands::kanban_analyzer::catchup_unanalyzed_review_cards_core(
                        &db,
                        &tool_factory,
                        &mcp_manager,
                        &llm_manager,
                        &app_handle,
                    )
                    .await
                    {
                        Ok(n) if n > 0 => {
                            tracing::info!(analyzed = n, "Kanban catch-up: analyzed stuck review cards")
                        }
                        Ok(_) => tracing::debug!("Kanban catch-up: no un-analyzed review cards"),
                        Err(e) => tracing::warn!(error = %e, "Kanban catch-up failed (non-fatal)"),
                    }
                });
            }

            // Spawn the validation_audit cleanup task.
            // Honors `audit.retention_days` and runs every 24h.
            // The handle is parked in AppState so the runtime owns it (and a
            // future shutdown hook can `abort()` it deterministically).
            let cleanup_handle =
                commands::validation_audit::spawn_audit_cleanup_task(state.inner().db.clone());
            let cleanup_slot = state.inner().audit_cleanup_handle.clone();
            tauri::async_runtime::spawn(async move {
                *cleanup_slot.lock().await = Some(cleanup_handle);
            });
            tracing::info!("Validation audit cleanup task spawned");

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
                // Set app handle in ToolFactory for validation event emission
                // This is needed for sub-agents that don't have agent_context
                // Clone handle first to avoid holding guard across await
                let handle_to_set = app_handle_clone.read().ok().and_then(|g| g.clone());
                if let Some(handle) = handle_to_set {
                    tool_factory.set_app_handle(handle).await;
                }

                // Load agents from database
                let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, skills, folders, require_file_confirmation, system_prompt, max_tool_iterations, reasoning_effort, kind, auto_analyze_reports FROM agent";
                let results: Vec<serde_json::Value> = match db.db.query(query).await {
                    Ok(mut r) => r.take(0).unwrap_or_default(),
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to query agents from database");
                        return;
                    }
                };

                let mut loaded = 0;
                for row in results {
                    let config: crate::models::AgentConfig = match serde_json::from_value(row) {
                        Ok(c) => c,
                        Err(e) => {
                            tracing::warn!(error = %e, "Failed to deserialize agent, skipping");
                            continue;
                        }
                    };
                    if config.id.is_empty() {
                        continue;
                    }
                    // Clamp max_tool_iterations to safe range
                    let mut config = crate::models::AgentConfig {
                        max_tool_iterations: config.max_tool_iterations.clamp(1, 200),
                        ..config
                    };
                    // Refresh model-owned LLMConfig fields (`is_reasoning`,
                    // `context_window`) from the current `llm_model` row.
                    // Without this, an agent saved before the model was
                    // flagged `is_reasoning=true` keeps a stale snapshot —
                    // `effective_reasoning_effort` returns None, the
                    // `reasoning_effort` parameter never reaches the
                    // provider, and the Reflexion UI block never appears.
                    // See ERR_LLM_010.
                    let pre_hydrate_is_reasoning = config.llm.is_reasoning;
                    let pre_hydrate_context_window = config.llm.context_window;
                    if let Err(e) = crate::commands::agent::hydrate_llm_from_model(
                        &db,
                        &mut config.llm,
                    )
                    .await
                    {
                        tracing::warn!(
                            agent_id = %config.id,
                            error = %e,
                            "Failed to hydrate LLM snapshot from model card on startup; \
                             keeping the persisted snapshot. Reasoning may be missing \
                             until the model card is re-saved."
                        );
                    } else {
                        tracing::info!(
                            agent_id = %config.id,
                            agent_name = %config.name,
                            provider = %config.llm.provider,
                            model = %config.llm.model,
                            is_reasoning_before = pre_hydrate_is_reasoning,
                            is_reasoning_after = config.llm.is_reasoning,
                            context_window_before = ?pre_hydrate_context_window,
                            context_window_after = ?config.llm.context_window,
                            "Hydrated agent LLM snapshot from model card"
                        );
                    }
                    let id = config.id.clone();

                    // Create agent context with app_handle
                    // Note: No cancellation token during startup agent loading
                    let app_handle = app_handle_clone.read().ok().and_then(|guard| guard.clone());
                    let context = crate::tools::AgentToolContext::new(
                        registry.clone(),
                        orchestrator.clone(),
                        llm_manager.clone(),
                        Some(mcp_manager.clone()),
                        tool_factory.clone(),
                        app_handle,
                        None, // No cancellation token during startup
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
        .build(tauri::generate_context!())
        .expect("error while running tauri application");

    // Flag preventing infinite recursion: app_handle.exit(0) re-fires
    // ExitRequested, so the second pass must let Tauri terminate normally.
    let shutdown_done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));

    app.run(move |app_handle, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event {
            if shutdown_done.load(std::sync::atomic::Ordering::SeqCst) {
                return;
            }

            // main() is #[tokio::main], so this callback runs inside an
            // active tokio runtime. block_on() would panic with "Cannot
            // start a runtime from within a runtime". Defer the shutdown
            // to a spawned task and call exit(0) once it finishes; a 5s
            // timeout protects the UI from misbehaving MCP servers.
            api.prevent_exit();

            // Flip the kanban scheduler shutdown flag so the loop exits on
            // its next tick. The handle is owned by AppState; we don't need
            // to abort() it explicitly — the std::process::exit(0) below
            // tears down the runtime cleanly once MCP shutdown finishes.
            let app_state = app_handle.state::<AppState>();
            app_state
                .kanban_scheduler_shutdown
                .store(true, std::sync::atomic::Ordering::Release);

            let mcp_manager = app_state.mcp_manager.clone();
            let shutdown_done = shutdown_done.clone();

            tauri::async_runtime::spawn(async move {
                let result =
                    tokio::time::timeout(std::time::Duration::from_secs(5), mcp_manager.shutdown())
                        .await;

                match result {
                    Ok(Ok(())) => tracing::info!("MCP manager shutdown complete on exit"),
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "MCP manager shutdown returned error");
                    }
                    Err(_) => tracing::warn!("MCP manager shutdown timed out after 5s"),
                }

                shutdown_done.store(true, std::sync::atomic::Ordering::SeqCst);

                // `app_handle.exit(0)` lets Tauri unwind the drop chain
                // (AppState -> Arc<DBClient> -> Surreal<RocksDb> -> RocksDB
                // C++ destructors) while the tokio runtime is still
                // tearing down. RocksDB 2.6 systematically aborts with
                // `free(): corrupted unsorted chunks` on this path. We
                // sidestep the race with std::process::exit(0): MCP
                // children are already terminated (logged above), MCP
                // HTTP clients are disconnected, and the RocksDB
                // write-ahead log handles crash recovery on the next
                // startup so data integrity is preserved.
                tracing::info!("Exiting process after MCP cleanup");
                std::process::exit(0);
            });
        }
    });

    Ok(())
}
