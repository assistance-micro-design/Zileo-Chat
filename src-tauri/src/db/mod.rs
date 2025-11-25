// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
pub mod schema;

pub use client::DBClient;
