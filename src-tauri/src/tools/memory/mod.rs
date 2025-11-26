// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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

mod tool;

pub use tool::MemoryTool;
