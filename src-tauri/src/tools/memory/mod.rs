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

// Allow dead code until Phase 4: Agent Integration
#![allow(dead_code)]

//! # Memory Tool Module
//!
//! This module provides the MemoryTool for agent contextual persistence
//! with vector embeddings and semantic search capabilities.
//!
//! ## Features
//!
//! - Store and retrieve contextual memories
//! - Semantic search using vector embeddings (HNSW index)
//! - Workflow-scoped memory isolation
//! - Multiple memory types (user_pref, context, knowledge, decision)
//!
//! ## Architecture
//!
//! ```text
//! Agent
//!   |
//!   v
//! MemoryTool
//!   |
//!   +---> EmbeddingService (Mistral/Ollama)
//!   |
//!   +---> DBClient (SurrealDB)
//!          |
//!          +---> HNSW Vector Index (1024D)
//! ```
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::tools::memory::MemoryTool;
//! use crate::llm::embedding::{EmbeddingService, EmbeddingProvider};
//!
//! let embedding_service = EmbeddingService::with_provider(
//!     EmbeddingProvider::mistral("api-key")
//! );
//!
//! let tool = MemoryTool::new(
//!     db.clone(),
//!     Some(Arc::new(embedding_service)),
//!     Some("workflow_123".to_string()),
//!     "db_agent".to_string(),
//! );
//!
//! let result = tool.execute(json!({
//!     "operation": "add",
//!     "type": "knowledge",
//!     "content": "SurrealDB supports HNSW vector indexing"
//! })).await?;
//! ```

mod helpers;
mod tool;

pub use helpers::{add_memory_core, AddMemoryParams};
pub use tool::MemoryTool;
