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

//! # Centralized Query Constants (OPT-WF-1)
//!
//! Contains SQL query templates for SurrealDB to eliminate duplication
//! and ensure consistent field selection across commands.
//!
//! ## Usage
//!
//! ```rust
//! use crate::db::queries::workflow;
//!
//! // For listing all workflows (no WHERE clause)
//! let query = workflow::SELECT_LIST;
//!
//! // For loading single workflow by ID
//! let query = format!("{} WHERE meta::id(id) = '{}'", workflow::SELECT_BASE, id);
//! ```

/// Workflow query constants.
///
/// Two patterns are provided:
/// - `SELECT_BASE`: Core fields without WHERE/ORDER BY (for single workflow lookup)
/// - `SELECT_LIST`: Full query for listing workflows (with ORDER BY)
pub mod workflow {
    /// Base SELECT fields for workflow queries.
    /// Use with `format!("{} WHERE meta::id(id) = '{}'", SELECT_BASE, id)` for single lookup.
    ///
    /// Includes token metrics with NULL coalescing for backwards compatibility.
    pub const SELECT_BASE: &str = r#"SELECT
        meta::id(id) AS id,
        name,
        agent_id,
        status,
        created_at,
        updated_at,
        completed_at,
        (total_tokens_input ?? 0) AS total_tokens_input,
        (total_tokens_output ?? 0) AS total_tokens_output,
        (total_cost_usd ?? 0.0) AS total_cost_usd,
        model_id
    FROM workflow"#;

    /// SELECT query for listing all workflows ordered by update time.
    /// Use directly: `db.query_json(SELECT_LIST)`.
    pub const SELECT_LIST: &str = r#"SELECT
        meta::id(id) AS id,
        name,
        agent_id,
        status,
        created_at,
        updated_at,
        completed_at,
        (total_tokens_input ?? 0) AS total_tokens_input,
        (total_tokens_output ?? 0) AS total_tokens_output,
        (total_cost_usd ?? 0.0) AS total_cost_usd,
        model_id
    FROM workflow
    ORDER BY updated_at DESC"#;

    /// Minimal SELECT fields for basic workflow validation (existence check).
    /// Used in execute_workflow/streaming where token metrics aren't needed yet.
    pub const SELECT_BASIC: &str = r#"SELECT
        meta::id(id) AS id,
        name,
        agent_id,
        status,
        created_at,
        updated_at,
        completed_at
    FROM workflow"#;
}
