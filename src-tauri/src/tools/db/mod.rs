// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Database tools for the DB Agent.
//!
//! This module provides tools for database operations:
//! - SurrealDBTool: Direct CRUD operations
//! - QueryBuilderTool: SQL/SurrealQL generation
//! - AnalyticsTool: Aggregations and graph traversal

mod analytics;
mod query_builder;
mod surrealdb_tool;

pub use analytics::AnalyticsTool;
pub use query_builder::QueryBuilderTool;
pub use surrealdb_tool::SurrealDBTool;
