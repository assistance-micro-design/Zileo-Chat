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

//! # Database Module
//!
//! Provides SurrealDB embedded database functionality for Zileo-Chat-3.
//!
//! ## Overview
//!
//! This module contains:
//! - [`DBClient`] - Database client for CRUD operations
//! - [`schema`] - Database schema definitions (7 tables)
//!
//! ## Database Engine
//!
//! Uses SurrealDB with embedded RocksDB backend for:
//! - Local data persistence
//! - Vector search capabilities (HNSW index)
//! - Graph relations between entities
//!
//! ## Tables
//!
//! - `workflow` - User workflows
//! - `agent_state` - Agent configurations and metrics
//! - `message` - Conversation messages
//! - `memory` - Vector embeddings for RAG
//! - `validation_request` - Human-in-the-loop validations
//! - `task` - Task decomposition tracking

pub mod client;
pub mod queries;
pub mod schema;

pub use client::DBClient;
