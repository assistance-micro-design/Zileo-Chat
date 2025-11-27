// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! # Tauri Commands Module
//!
//! IPC command handlers for frontend-backend communication.
//!
//! ## Overview
//!
//! This module provides Tauri commands invoked via `invoke()` from the frontend:
//!
//! ### Workflow Commands ([`workflow`])
//! - `create_workflow` - Create new workflow
//! - `execute_workflow` - Execute workflow with message
//! - `load_workflows` - List all workflows
//! - `delete_workflow` - Delete workflow by ID
//!
//! ### Agent Commands ([`agent`])
//! - `list_agents` - List registered agent IDs
//! - `get_agent_config` - Get agent configuration
//!
//! ### Security Commands ([`security`])
//! - `save_api_key` - Securely store API key
//! - `get_api_key` - Retrieve stored API key
//! - `delete_api_key` - Remove stored API key
//!
//! ### LLM Commands ([`llm`])
//! - `get_llm_config` - Get current LLM configuration
//! - `configure_mistral` - Configure Mistral provider
//! - `configure_ollama` - Configure Ollama provider
//! - `set_active_provider` - Set active LLM provider
//! - `set_default_model` - Set default model for provider
//! - `get_available_models` - Get available models
//! - `test_ollama_connection` - Test Ollama connectivity
//! - `test_mistral_connection` - Test Mistral connectivity
//! - `test_llm_completion` - Test LLM completion
//!
//! ### Model Commands ([`models`])
//! - `list_models` - List all LLM models (builtin + custom)
//! - `get_model` - Get a single model by ID
//! - `create_model` - Create a custom model
//! - `update_model` - Update an existing model
//! - `delete_model` - Delete a custom model
//! - `get_provider_settings` - Get provider settings
//! - `update_provider_settings` - Update provider settings
//! - `test_provider_connection` - Test provider connection
//! - `seed_builtin_models` - Seed database with builtin models
//!
//! ### Validation Commands ([`validation`])
//! - `create_validation_request` - Create human-in-the-loop validation
//! - `list_pending_validations` - List pending validations
//! - `list_workflow_validations` - List validations for workflow
//! - `approve_validation` - Approve a validation request
//! - `reject_validation` - Reject a validation request
//! - `delete_validation` - Delete a validation request
//!
//! ### Memory Commands ([`memory`])
//! - `add_memory` - Add memory entry
//! - `list_memories` - List memories with optional filter
//! - `get_memory` - Get single memory by ID
//! - `delete_memory` - Delete memory entry
//! - `search_memories` - Search memories by text
//! - `clear_memories_by_type` - Clear all memories of a type
//!
//! ### Streaming Commands ([`streaming`])
//! - `execute_workflow_streaming` - Execute workflow with real-time events
//! - `cancel_workflow_streaming` - Cancel streaming execution
//!
//! ### Message Commands ([`message`]) - Phase 6
//! - `save_message` - Persist a message to the database
//! - `load_workflow_messages` - Load all messages for a workflow
//! - `delete_message` - Delete a single message
//! - `clear_workflow_messages` - Delete all messages for a workflow
//!
//! ### Task Commands ([`task`])
//! - `create_task` - Create a new task for a workflow
//! - `get_task` - Get a single task by ID
//! - `list_workflow_tasks` - List all tasks for a workflow
//! - `list_tasks_by_status` - List tasks filtered by status
//! - `update_task` - Update task fields (partial)
//! - `update_task_status` - Update task status specifically
//! - `complete_task` - Mark task as completed with duration
//! - `delete_task` - Delete a task
//!
//! ### MCP Commands ([`mcp`])
//! - `list_mcp_servers` - List all configured MCP servers
//! - `get_mcp_server` - Get a single MCP server by ID
//! - `create_mcp_server` - Create a new MCP server configuration
//! - `update_mcp_server` - Update an existing MCP server
//! - `delete_mcp_server` - Delete an MCP server configuration
//! - `test_mcp_server` - Test MCP server connection
//! - `start_mcp_server` - Start an MCP server
//! - `stop_mcp_server` - Stop a running MCP server
//! - `list_mcp_tools` - List available tools from a server
//! - `call_mcp_tool` - Execute a tool on an MCP server
//!
//! ### Migration Commands ([`migration`])
//! - `migrate_memory_schema` - Migrate memory table for vector search
//! - `get_memory_schema_status` - Get memory schema status
//!
//! ### Embedding Commands ([`embedding`])
//! - `get_embedding_config` - Get current embedding configuration
//! - `save_embedding_config` - Save embedding configuration
//! - `test_embedding` - Test embedding generation
//! - `get_memory_stats` - Get memory statistics for dashboard
//! - `update_memory` - Update an existing memory entry
//! - `export_memories` - Export memories to JSON/CSV
//! - `import_memories` - Import memories from JSON
//! - `regenerate_embeddings` - Regenerate embeddings for existing memories
//!
//! ## Input Validation
//!
//! All commands validate inputs using the [`crate::security::Validator`]
//! to prevent injection attacks and ensure data integrity.

pub mod agent;
pub mod embedding;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod message;
pub mod migration;
pub mod models;
pub mod security;
pub mod streaming;
pub mod task;
pub mod validation;
pub mod workflow;

pub use security::SecureKeyStore;
