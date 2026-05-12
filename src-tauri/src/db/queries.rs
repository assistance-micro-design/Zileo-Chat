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
        sub_agent_cost_usd, \
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

    /// Deletes `memory_chunk` rows whose parent memory belongs to the given
    /// workflow. Reached via the record link `memory_id.workflow_id`.
    ///
    /// MUST run BEFORE deleting the parent `memory` rows — once the parent is
    /// gone, the traversal `memory_id.workflow_id` yields NONE and the WHERE
    /// clause never matches, which would leave the chunks orphan in the
    /// HNSW index forever.
    pub async fn delete_memory_chunks_by_workflow_id(db: &Arc<DBClient>, workflow_id: &str) {
        let query = "DELETE memory_chunk WHERE memory_id.workflow_id = $wf_id";
        match db
            .execute_with_params(
                query,
                vec![(
                    "wf_id".to_string(),
                    serde_json::Value::String(workflow_id.to_string()),
                )],
            )
            .await
        {
            Ok(_) => info!(workflow_id = %workflow_id, "Cascade deleted memory_chunk rows"),
            Err(e) => {
                warn!(error = %e, workflow_id = %workflow_id, "memory_chunk cascade delete failed")
            }
        }
    }

    /// Performs cascade delete on all related tables for a workflow.
    ///
    /// `memory_chunk` is deleted SEQUENTIALLY FIRST because its parent link
    /// `memory_id.workflow_id` must still be reachable. The remaining tables
    /// in `CASCADE_DELETE_TABLES` are independent of each other and run in
    /// parallel.
    ///
    /// # Arguments
    /// * `db` - Database client Arc reference
    /// * `workflow_id` - The workflow ID to cascade delete
    pub async fn delete_workflow_related(db: &Arc<DBClient>, workflow_id: &str) {
        use super::workflow::CASCADE_DELETE_TABLES;

        // Step 1: drop chunks before their parents.
        delete_memory_chunks_by_workflow_id(db, workflow_id).await;

        // Step 2: independent tables in parallel (includes `memory` itself).
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

        join_all(futures).await;

        info!(workflow_id = %workflow_id, "Cascade delete completed for all related tables");
    }
}

#[cfg(test)]
mod tests {
    use super::cascade::delete_workflow_related;
    use crate::test_utils::setup_test_state;
    use std::sync::Arc;

    /// Inserts a parent `memory` row with `workflow_id = $wf_id` and one
    /// `memory_chunk` linked to it. Used to assert cascade behaviour without
    /// dragging in the full embedding service.
    async fn seed_memory_with_chunk_in_workflow(
        db: &Arc<crate::db::DBClient>,
        workflow_id: &str,
    ) -> String {
        let mem_id = uuid::Uuid::new_v4().to_string();
        let chunk_id = uuid::Uuid::new_v4().to_string();

        db.db
            .query(format!(
                "CREATE memory:`{}` SET \
                    type = 'context', \
                    content = 'parent', \
                    workflow_id = $wf_id, \
                    metadata = {{}}, \
                    importance = 0.5",
                mem_id
            ))
            .bind(("wf_id", workflow_id.to_string()))
            .await
            .unwrap()
            .check()
            .unwrap();

        db.db
            .query(format!(
                "CREATE memory_chunk:`{chunk_id}` SET \
                    memory_id = memory:`{mem_id}`, \
                    chunk_index = 0, \
                    chunk_count = 1, \
                    content = 'chunk content', \
                    embedding = NONE"
            ))
            .await
            .unwrap()
            .check()
            .unwrap();

        mem_id
    }

    async fn count(db: &Arc<crate::db::DBClient>, query: &str) -> u64 {
        let rows: Vec<serde_json::Value> = db.query_json(query).await.unwrap();
        rows.first()
            .and_then(|r| r.get("c"))
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
    }

    #[tokio::test]
    async fn cascade_drops_memory_chunks_when_workflow_is_deleted() {
        // Resumability + bug regression: a previous version of the cascade
        // dropped only `memory` rows and left `memory_chunk` rows pointing at
        // a now-vanished parent, bloating the HNSW index. The fix
        // pre-deletes chunks via the `memory_id.workflow_id` traversal.
        let (state, _db_guard) = setup_test_state().await;
        let wf_id = uuid::Uuid::new_v4().to_string();
        let mem_id = seed_memory_with_chunk_in_workflow(&state.db, &wf_id).await;

        let chunks_before = count(
            &state.db,
            &format!(
                "SELECT count() AS c FROM memory_chunk \
                 WHERE memory_id = memory:`{}` GROUP ALL",
                mem_id
            ),
        )
        .await;
        assert_eq!(chunks_before, 1, "fixture must seed exactly 1 chunk");

        delete_workflow_related(&state.db, &wf_id).await;

        let chunks_after =
            count(&state.db, "SELECT count() AS c FROM memory_chunk GROUP ALL").await;
        let mems_after = count(
            &state.db,
            &format!(
                "SELECT count() AS c FROM memory WHERE workflow_id = '{}' GROUP ALL",
                wf_id
            ),
        )
        .await;
        assert_eq!(
            chunks_after, 0,
            "cascade must leave zero orphan memory_chunk rows"
        );
        assert_eq!(
            mems_after, 0,
            "cascade must also drop the parent memory rows (sanity)"
        );
    }

    #[tokio::test]
    async fn cascade_leaves_other_workflows_chunks_untouched() {
        let (state, _db_guard) = setup_test_state().await;
        let wf_a = uuid::Uuid::new_v4().to_string();
        let wf_b = uuid::Uuid::new_v4().to_string();
        seed_memory_with_chunk_in_workflow(&state.db, &wf_a).await;
        seed_memory_with_chunk_in_workflow(&state.db, &wf_b).await;

        delete_workflow_related(&state.db, &wf_a).await;

        let remaining = count(&state.db, "SELECT count() AS c FROM memory_chunk GROUP ALL").await;
        assert_eq!(remaining, 1, "only workflow A's chunks must be removed");
    }
}
