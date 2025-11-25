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
//! - `test_llm_completion` - Test LLM completion
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
//! ## Input Validation
//!
//! All commands validate inputs using the [`crate::security::Validator`]
//! to prevent injection attacks and ensure data integrity.

pub mod agent;
pub mod llm;
pub mod mcp;
pub mod memory;
pub mod security;
pub mod streaming;
pub mod validation;
pub mod workflow;

pub use security::SecureKeyStore;
