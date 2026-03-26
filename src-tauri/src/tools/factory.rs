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

//! Tool Factory for dynamic tool instantiation.
//!
//! This module provides a factory pattern for creating tool instances
//! based on their string identifiers from agent configuration.
//!
//! # Supported Tools
//!
//! | Tool ID | Module | Description |
//! |---------|--------|-------------|
//! | `MemoryTool` | [`memory`] | Contextual memory with semantic search |
//! | `TodoTool` | [`todo`] | Task management for workflows |
//!
//! # Architecture
//!
//! - `factory.rs` - Struct, constructor, utility methods, registry delegation
//! - `factory_creation.rs` - Tool creation methods and DB resolvers
//!
//! # Usage
//!
//! ```ignore
//! use crate::tools::factory::ToolFactory;
//!
//! let factory = ToolFactory::new(
//!     db.clone(),
//!     Some(embedding_service.clone()),
//! );
//!
//! // Create a specific tool for an agent
//! let memory_tool = factory.create_tool(
//!     "MemoryTool",
//!     Some("wf_001".to_string()),
//!     "db_agent".to_string(),
//! )?;
//! ```

use crate::db::DBClient;
use crate::llm::embedding::EmbeddingService;
use crate::tools::registry::TOOL_REGISTRY;
use std::sync::Arc;
use tracing::info;

/// Factory for creating tool instances.
///
/// The factory holds shared dependencies (database, embedding service)
/// and creates tool instances on demand based on their string identifiers.
pub struct ToolFactory {
    /// Database client shared by all tools
    pub(crate) db: Arc<DBClient>,
    /// Dynamic embedding service reference for MemoryTool
    /// Uses RwLock to allow runtime configuration via Settings UI
    pub(crate) embedding_service: Arc<tokio::sync::RwLock<Option<Arc<EmbeddingService>>>>,
    /// Tauri app handle for event emission (set after app initialization)
    pub(crate) app_handle: Arc<tokio::sync::RwLock<Option<tauri::AppHandle>>>,
}

impl ToolFactory {
    /// Creates a new tool factory with dependencies.
    ///
    /// # Arguments
    /// * `db` - Database client for persistence
    /// * `embedding_service` - Dynamic embedding service reference (reads current state)
    ///
    /// # Example
    /// ```ignore
    /// let embedding_ref = Arc::new(RwLock::new(None));
    /// let factory = ToolFactory::new(db.clone(), embedding_ref);
    /// // Later: *embedding_ref.write().await = Some(embed_svc);
    /// ```
    pub fn new(
        db: Arc<DBClient>,
        embedding_service: Arc<tokio::sync::RwLock<Option<Arc<EmbeddingService>>>>,
    ) -> Self {
        info!("ToolFactory initialized with dynamic embedding service");
        Self {
            db,
            embedding_service,
            app_handle: Arc::new(tokio::sync::RwLock::new(None)),
        }
    }

    /// Sets the app handle for event emission.
    ///
    /// This should be called during app setup after the AppHandle is available.
    pub async fn set_app_handle(&self, handle: tauri::AppHandle) {
        let mut app_handle = self.app_handle.write().await;
        *app_handle = Some(handle);
        info!("ToolFactory app_handle configured");
    }

    /// Gets the app handle if available.
    ///
    /// Used by ValidationHelper as a fallback when agent_context is not available.
    pub async fn get_app_handle(&self) -> Option<tauri::AppHandle> {
        self.app_handle.read().await.clone()
    }

    /// Gets the current embedding service (reads from dynamic reference)
    pub(crate) async fn get_embedding_service(&self) -> Option<Arc<EmbeddingService>> {
        self.embedding_service.read().await.clone()
    }

    /// Returns a reference to the database client.
    ///
    /// Used by components that need direct DB access for operations
    /// like validation that are not tied to a specific tool.
    pub fn get_db(&self) -> Arc<DBClient> {
        self.db.clone()
    }

    /// Returns list of available tool names.
    ///
    /// This includes both basic tools (MemoryTool, TodoTool) and
    /// sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool).
    ///
    /// Note: Sub-agent tools require `AgentToolContext` and should be
    /// created using `create_tool_with_context()`.
    pub fn available_tools() -> Vec<&'static str> {
        TOOL_REGISTRY.available_tools()
    }

    /// Returns list of basic tool names (those not requiring AgentToolContext).
    pub fn basic_tools() -> Vec<&'static str> {
        TOOL_REGISTRY.basic_tools()
    }

    /// Returns list of sub-agent tool names (those requiring AgentToolContext).
    ///
    /// These tools are only available to the primary workflow agent
    /// and are NOT provided to sub-agents (to prevent chaining).
    pub fn sub_agent_tools() -> Vec<&'static str> {
        TOOL_REGISTRY.sub_agent_tools()
    }

    /// Checks if a tool name is valid.
    pub fn is_valid_tool(name: &str) -> bool {
        TOOL_REGISTRY.has_tool(name)
    }

    /// Checks if a tool requires AgentToolContext.
    pub fn requires_context(name: &str) -> bool {
        TOOL_REGISTRY.requires_context(name)
    }
}

#[cfg(test)]
#[path = "factory_tests.rs"]
mod tests;
