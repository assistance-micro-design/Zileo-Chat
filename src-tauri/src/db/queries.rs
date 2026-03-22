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

//! # Centralized Query Constants
//!
//! Contains SQL query templates for SurrealDB to eliminate duplication
//! and ensure consistent field selection across commands.
//!
//! ## Usage
//!
//! ```rust,ignore
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
    /// Core field list shared by SELECT, RETURN, and list queries.
    /// Single source of truth for workflow field selection with NULL coalescing.
    const FIELDS: &str = "\
        meta::id(id) AS id, \
        name, \
        agent_id, \
        status, \
        created_at, \
        updated_at, \
        completed_at, \
        (total_tokens_input ?? 0) AS total_tokens_input, \
        (total_tokens_output ?? 0) AS total_tokens_output, \
        (total_cost_usd ?? 0.0) AS total_cost_usd, \
        model_id, \
        (current_context_tokens ?? 0) AS current_context_tokens, \
        (sub_agent_tokens_input ?? 0) AS sub_agent_tokens_input, \
        (sub_agent_tokens_output ?? 0) AS sub_agent_tokens_output, \
        folder_id, \
        (pinned ?? false) AS pinned";

    /// Base SELECT for workflow queries (no WHERE/ORDER BY).
    /// Use with `format!("{} WHERE meta::id(id) = '{}'", SELECT_BASE, id)`.
    pub static SELECT_BASE: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| format!("SELECT {} FROM workflow", FIELDS));

    /// SELECT query for listing all workflows ordered by update time.
    pub static SELECT_LIST: std::sync::LazyLock<String> =
        std::sync::LazyLock::new(|| format!("{} ORDER BY updated_at DESC", *SELECT_BASE));

    /// RETURN clause fields for UPDATE commands (rename, move, toggle pin).
    /// Use with `format!("UPDATE workflow:`{}` SET ... RETURN {}", id, RETURN_FIELDS)`.
    pub const RETURN_FIELDS: &str = FIELDS;

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

    /// Tables that have workflow_id foreign key and need cascade delete.
    /// Order doesn't matter as these are deleted in parallel.
    pub const CASCADE_DELETE_TABLES: &[&str] = &[
        "task",
        "message",
        "tool_execution",
        "thinking_step",
        "sub_agent_execution",
        "validation_request",
        "memory",
        "user_question",
    ];
}

/// Cascade delete helpers.
pub mod cascade {
    use crate::db::DBClient;
    use futures_util::future::join_all;
    use std::sync::Arc;
    use tracing::{info, warn};

    /// Deletes all records from a table that reference the given workflow_id.
    ///
    /// Used for cascade delete operations to clean up related entities.
    /// Logs success or failure but does not propagate errors (best-effort cleanup).
    ///
    /// # Arguments
    /// * `db` - Database client Arc reference
    /// * `table` - Table name from CASCADE_DELETE_TABLES constant (hardcoded, not user input)
    /// * `workflow_id` - The workflow ID to match (parameterized to prevent injection)
    pub async fn delete_by_workflow_id(db: &Arc<DBClient>, table: &str, workflow_id: &str) {
        // Table names come from the hardcoded CASCADE_DELETE_TABLES constant.
        // workflow_id is parameterized via $wf_id bind variable.
        let query = format!("DELETE {} WHERE workflow_id = $wf_id", table);
        match db
            .execute_with_params(
                &query,
                vec![(
                    "wf_id".to_string(),
                    serde_json::Value::String(workflow_id.to_string()),
                )],
            )
            .await
        {
            Ok(_) => info!(table = %table, workflow_id = %workflow_id, "Cascade deleted records"),
            Err(e) => warn!(error = %e, table = %table, "Cascade delete failed (may not exist)"),
        }
    }

    /// Performs cascade delete on all related tables for a workflow.
    ///
    /// Uses `tokio::join!` to execute all deletes in parallel for efficiency.
    /// This eliminates the need for 8 Arc clones + 8 ID clones by using a single
    /// helper function.
    ///
    /// # Arguments
    /// * `db` - Database client Arc reference
    /// * `workflow_id` - The workflow ID to cascade delete
    pub async fn delete_workflow_related(db: &Arc<DBClient>, workflow_id: &str) {
        use super::workflow::CASCADE_DELETE_TABLES;

        // Create futures for all cascade deletes
        let futures: Vec<_> = CASCADE_DELETE_TABLES
            .iter()
            .map(|table| {
                let db = Arc::clone(db);
                let table = *table;
                let wf_id = workflow_id.to_string();
                async move {
                    delete_by_workflow_id(&db, table, &wf_id).await;
                }
            })
            .collect();

        // Execute all in parallel using join_all
        join_all(futures).await;

        info!(workflow_id = %workflow_id, "Cascade delete completed for all related tables");
    }
}
