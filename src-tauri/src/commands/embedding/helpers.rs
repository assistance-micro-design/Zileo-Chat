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

//! Private helpers shared by the `commands/embedding` submodules.
//!
//! These helpers factor out queries that would otherwise live (and drift)
//! in parallel between `stats.rs`, `operations.rs`, and `migration.rs`.

use crate::db::{extract_count, DBClient};
use chrono::{DateTime, Utc};
use serde_json::json;
use std::collections::HashMap;

/// Counts parent `memory` rows that have at least one `memory_chunk` row.
///
/// Uses a `DISTINCT memory_id` subquery so a parent with `N` chunks counts
/// as one.
pub(super) async fn count_parents_with_chunks(db: &DBClient) -> Result<usize, String> {
    let query =
        "SELECT count() FROM (SELECT memory_id FROM memory_chunk GROUP BY memory_id) GROUP ALL";
    let result: Vec<serde_json::Value> = db
        .query(query)
        .await
        .map_err(|e| format!("count_parents_with_chunks failed: {}", e))?;
    Ok(extract_count(&result) as usize)
}

/// Counts parent `memory` rows that have at least one chunk, grouped by the
/// parent's `type`.
///
/// Resolves the parent type via record-link traversal (`memory_id.type`),
/// which is supported in SELECT contexts (ERR_SURREAL_013 only applies to
/// WHERE/DELETE).
///
/// Returns a `HashMap<memory_type, count>` — empty on empty DB.
pub(super) async fn count_parents_with_chunks_by_type(
    db: &DBClient,
) -> Result<HashMap<String, usize>, String> {
    let query = "SELECT memory_id.type AS type, count() AS count FROM \
                 (SELECT memory_id FROM memory_chunk GROUP BY memory_id) GROUP BY type";
    let rows: Vec<serde_json::Value> = db
        .query_json(query)
        .await
        .map_err(|e| format!("count_parents_with_chunks_by_type failed: {}", e))?;

    let mut map = HashMap::new();
    for row in rows {
        if let (Some(t), Some(c)) = (
            row.get("type").and_then(|t| t.as_str()),
            row.get("count").and_then(|c| c.as_u64()),
        ) {
            map.insert(t.to_string(), c as usize);
        }
    }
    Ok(map)
}

/// Overrides the `created_at` of an existing memory row with the given
/// timestamp.
///
/// Used by `import_memories` to preserve a memory's original creation date
/// when round-tripping through an export file. The schema's
/// `DEFAULT time::now()` would otherwise overwrite it on CREATE.
///
/// SurrealDB SCHEMAFULL rejects ISO 8601 strings for `datetime` fields when
/// passed via JSON CONTENT (ERR_SURREAL_007); this helper uses a
/// `<datetime>` cast inside the UPDATE, mirroring `set_expires_at_if_present`
/// in `tools/memory/helpers.rs`.
pub(super) async fn set_created_at(
    db: &DBClient,
    memory_id: &str,
    created_at: DateTime<Utc>,
) -> Result<(), String> {
    let query = format!(
        "UPDATE memory:`{}` SET created_at = <datetime>$created_at",
        memory_id
    );
    db.execute_with_params(
        &query,
        vec![("created_at".to_string(), json!(created_at.to_rfc3339()))],
    )
    .await
    .map_err(|e| format!("Failed to override created_at: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MemoryType;
    use crate::test_utils::setup_test_state;
    use crate::tools::memory::helpers::{add_memory_core, AddMemoryParams};
    use chrono::TimeZone;

    /// Builds default `AddMemoryParams` for a given type/content — used by
    /// the helper tests to seed parents (+chunks) cheaply.
    fn params(memory_type: MemoryType, content: &str) -> AddMemoryParams {
        AddMemoryParams {
            memory_type,
            content: content.to_string(),
            metadata: json!({}),
            workflow_id: None,
            importance: 0.5,
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn count_parents_with_chunks_returns_zero_on_empty_db() {
        let (state, _guard) = setup_test_state().await;
        let count = count_parents_with_chunks(&state.db).await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn count_parents_with_chunks_counts_distinct_parents() {
        let (state, _guard) = setup_test_state().await;

        // Seed 2 parents via add_memory_core (each creates 1 parent + N chunks).
        // Even if a parent yields multiple chunks, the DISTINCT memory_id
        // subquery must collapse them to one.
        add_memory_core(
            params(MemoryType::Knowledge, "first parent content"),
            &state.db,
            None,
        )
        .await
        .unwrap();
        add_memory_core(
            params(MemoryType::Context, "second parent content"),
            &state.db,
            None,
        )
        .await
        .unwrap();

        let count = count_parents_with_chunks(&state.db).await.unwrap();
        assert_eq!(count, 2, "two parents → count = 2 (chunks collapsed)");
    }

    #[tokio::test]
    async fn count_parents_with_chunks_by_type_groups_correctly() {
        let (state, _guard) = setup_test_state().await;

        add_memory_core(params(MemoryType::Knowledge, "k1"), &state.db, None)
            .await
            .unwrap();
        add_memory_core(params(MemoryType::Context, "c1"), &state.db, None)
            .await
            .unwrap();
        add_memory_core(params(MemoryType::Context, "c2"), &state.db, None)
            .await
            .unwrap();

        let map = count_parents_with_chunks_by_type(&state.db).await.unwrap();
        assert_eq!(map.get("knowledge").copied(), Some(1));
        assert_eq!(map.get("context").copied(), Some(2));
        assert_eq!(map.get("decision").copied(), None);
        assert_eq!(map.get("user_pref").copied(), None);
    }

    #[tokio::test]
    async fn count_parents_with_chunks_by_type_empty_db_returns_empty_map() {
        let (state, _guard) = setup_test_state().await;
        let map = count_parents_with_chunks_by_type(&state.db).await.unwrap();
        assert!(map.is_empty(), "expected empty HashMap, got {:?}", map);
    }

    #[tokio::test]
    async fn set_created_at_overrides_default_timestamp() {
        let (state, _guard) = setup_test_state().await;
        let result = add_memory_core(params(MemoryType::Knowledge, "test"), &state.db, None)
            .await
            .unwrap();

        let target = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        set_created_at(&state.db, &result.memory_id, target)
            .await
            .unwrap();

        let rows: Vec<serde_json::Value> = state
            .db
            .query_json(&format!(
                "SELECT created_at FROM memory:`{}`",
                result.memory_id
            ))
            .await
            .unwrap();
        let stored = rows
            .first()
            .and_then(|r| r.get("created_at"))
            .and_then(|c| c.as_str())
            .unwrap();
        let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(stored)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed, target);
    }
}
