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
//! ## Input Validation
//!
//! All commands validate inputs using the [`crate::security::Validator`]
//! to prevent injection attacks and ensure data integrity.

pub mod agent;
pub mod security;
pub mod workflow;

pub use security::SecureKeyStore;
