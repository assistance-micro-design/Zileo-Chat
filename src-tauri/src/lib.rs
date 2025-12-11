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

//! # Zileo Chat Backend Library
//!
//! This crate provides the backend functionality for Zileo Chat,
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
pub mod tools;

pub use commands::SecureKeyStore;
pub use llm::ProviderManager;
pub use mcp::{MCPClient, MCPError, MCPManager, MCPResult, MCPServerHandle};
pub use state::AppState;
