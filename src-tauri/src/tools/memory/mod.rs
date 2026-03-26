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

//! # Memory Tool Module
//!
//! This module provides the MemoryTool for agent contextual persistence
//! with vector embeddings and semantic search capabilities.
//!
//! ## Architecture
//!
//! ```text
//! mod.rs          - Module exports
//! tool.rs         - Struct, constructor, Tool trait dispatch (~190 lines)
//! definition.rs   - Tool definition (schema + LLM description)
//! input.rs        - MemoryInput parsing + validation
//! operations.rs   - Operation implementations (add, get, list, search, etc.)
//! helpers.rs      - Shared core logic (used by both tool and commands)
//! ```
//!
//! ## Features
//!
//! - Store and retrieve contextual memories
//! - Semantic search using vector embeddings (HNSW index)
//! - Workflow-scoped memory isolation
//! - Multiple memory types (user_pref, context, knowledge, decision)
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::tools::memory::MemoryTool;
//! use crate::llm::embedding::{EmbeddingService, EmbeddingProvider};
//!
//! let embedding_service = EmbeddingService::with_provider(
//!     EmbeddingProvider::mistral("api-key")
//! )?;
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

mod definition;
mod helpers;
mod helpers_search;
mod input;
mod operations;
mod operations_query;
mod tool;

pub use helpers::{add_memory_core, AddMemoryParams, SearchParams};
pub use helpers_search::search_memories_core;
pub use tool::MemoryTool;
