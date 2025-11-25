// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! # Zileo-Chat-3 Backend Library
//!
//! This crate provides the backend functionality for Zileo-Chat-3,
//! a multi-agent desktop application built with Tauri.
//!
//! ## Modules
//!
//! - [`agents`] - Multi-agent system infrastructure (registry, orchestrator, agent trait)
//! - [`commands`] - Tauri IPC command handlers for frontend communication
//! - [`db`] - SurrealDB database client and schema management
//! - [`models`] - Data models shared between frontend and backend
//! - [`security`] - Input validation and secure key storage
//! - [`state`] - Application state management
//!
//! ## Architecture
//!
//! The backend follows a multi-agent architecture where:
//! - An [`AgentRegistry`](agents::core::AgentRegistry) manages agent discovery
//! - An [`AgentOrchestrator`](agents::core::AgentOrchestrator) coordinates task execution
//! - Agents implement the [`Agent`](agents::core::agent::Agent) trait
//!
//! ## Example
//!
//! ```rust,ignore
//! use zileo_chat::{AppState, agents::SimpleAgent};
//!
//! // Initialize application state
//! let state = AppState::new("/path/to/db").await?;
//!
//! // Register an agent
//! state.registry.register("my_agent".to_string(), Arc::new(agent)).await;
//! ```

pub mod agents;
pub mod commands;
pub mod db;
pub mod llm;
pub mod mcp;
pub mod models;
pub mod security;
pub mod state;

pub use commands::SecureKeyStore;
pub use llm::ProviderManager;
pub use mcp::{MCPError, MCPResult};
pub use state::AppState;
