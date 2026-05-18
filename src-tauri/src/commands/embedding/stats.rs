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

//! Memory statistics commands.

use super::helpers::{count_parents_with_chunks, count_parents_with_chunks_by_type};
use crate::{
    db::{extract_count, DBClient},
    models::{CategoryTokenStats, MemoryStats, MemoryTokenStats},
    AppState,
};
use std::collections::HashMap;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Gets memory statistics for the settings dashboard.
#[tauri::command]
#[instrument(name = "get_memory_stats", skip(state))]
pub async fn get_memory_stats(state: State<'_, AppState>) -> Result<MemoryStats, String> {
    info!("Getting memory statistics");
    get_stats_core(&state.db).await
}

/// Core implementation of [`get_memory_stats`] — separated from the Tauri
/// command so tests can exercise the real SQL without instantiating
/// `tauri::State`.
async fn get_stats_core(db: &DBClient) -> Result<MemoryStats, String> {
    // Get total count
    let total_query = "SELECT count() AS count FROM memory GROUP ALL";
    let total_result: Vec<serde_json::Value> = db.query(total_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to get memory count: {}", e)
    })?;

    let total = extract_count(&total_result) as usize;

    // Parents that have at least one `memory_chunk` row count as "indexed"
    // — chunks own the embeddings, the parent only owns the content.
    let with_embeddings = count_parents_with_chunks(db).await?;

    // Get count by type
    let by_type_query = "SELECT type, count() AS count FROM memory GROUP BY type";
    let type_result: Vec<serde_json::Value> = db.query(by_type_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories by type");
        format!("Failed to count memories by type: {}", e)
    })?;

    let mut by_type = HashMap::new();
    for row in type_result {
        if let (Some(t), Some(c)) = (
            row.get("type").and_then(|t| t.as_str()),
            row.get("count").and_then(|c| c.as_u64()),
        ) {
            by_type.insert(t.to_string(), c as usize);
        }
    }

    // Get count by agent source from metadata
    let by_agent_query =
        "SELECT metadata.agent_source AS agent, count() AS count FROM memory WHERE metadata.agent_source != NONE GROUP BY metadata.agent_source";
    let agent_result: Vec<serde_json::Value> = db.query(by_agent_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories by agent source");
        format!("Failed to count memories by agent source: {}", e)
    })?;

    let mut by_agent = HashMap::new();
    for row in agent_result {
        if let (Some(a), Some(c)) = (
            row.get("agent").and_then(|a| a.as_str()),
            row.get("count").and_then(|c| c.as_u64()),
        ) {
            by_agent.insert(a.to_string(), c as usize);
        }
    }

    let stats = MemoryStats {
        total,
        with_embeddings,
        without_embeddings: total.saturating_sub(with_embeddings),
        by_type,
        by_agent,
    };

    info!(
        total = stats.total,
        with_embeddings = stats.with_embeddings,
        "Memory statistics retrieved"
    );

    Ok(stats)
}

/// Gets token/character statistics per memory category
#[tauri::command]
#[instrument(name = "get_memory_token_stats", skip(state))]
pub async fn get_memory_token_stats(
    state: State<'_, AppState>,
    type_filter: Option<String>,
) -> Result<MemoryTokenStats, String> {
    info!(type_filter = ?type_filter, "Getting memory token statistics");
    get_token_stats_core(&state.db, type_filter).await
}

/// Core implementation of [`get_memory_token_stats`] — see `get_stats_core`
/// for the rationale of the split.
async fn get_token_stats_core(
    db: &DBClient,
    type_filter: Option<String>,
) -> Result<MemoryTokenStats, String> {
    // Aggregate the actual word counts (via SurrealDB
    // `array::len(string::words(...))`) so per-category token estimates use
    // the same `words × 1.5` heuristic as `crate::llm::utils::estimate_tokens`,
    // not the looser `chars / 4` approximation we used before.
    let base_query = r#"SELECT
            type,
            count() AS count,
            math::sum(string::len(content)) AS total_chars,
            math::sum(array::len(string::words(content))) AS total_words
        FROM memory"#;

    let results: Vec<serde_json::Value> = if let Some(ref mem_type) = type_filter {
        let query = format!("{} WHERE type = $mtype GROUP BY type", base_query);
        db.query_json_with_params(
            &query,
            vec![("mtype".to_string(), serde_json::json!(mem_type))],
        )
        .await
    } else {
        let query = format!("{} GROUP BY type", base_query);
        db.query_json(&query).await
    }
    .map_err(|e| {
        error!(error = %e, "Failed to get token stats");
        format!("Failed to get token statistics: {}", e)
    })?;

    // Resolve `with_embeddings` separately via the chunk-aware helper —
    // one extra query but it keeps the per-category aggregation honest.
    let indexed_by_type = count_parents_with_chunks_by_type(db).await?;

    let mut categories = Vec::new();
    let mut total_chars: usize = 0;
    let mut total_memories: usize = 0;
    let mut total_words: usize = 0;

    for row in results {
        let memory_type = row
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or_else(|| {
                warn!(row = ?row, "Unexpected DB result: missing 'type' field");
                "unknown"
            })
            .to_string();

        let count = row
            .get("count")
            .and_then(|c| c.as_u64())
            .unwrap_or_else(|| {
                warn!(row = ?row, "Unexpected DB result: missing or invalid 'count' field");
                0
            }) as usize;

        let chars = row
            .get("total_chars")
            .and_then(|c| c.as_u64())
            .unwrap_or_else(|| {
                warn!(row = ?row, "Unexpected DB result: missing or invalid 'total_chars' field");
                0
            }) as usize;

        let with_embeddings = indexed_by_type.get(&memory_type).copied().unwrap_or(0);

        let words = row.get("total_words").and_then(|w| w.as_u64()).unwrap_or(0) as usize;

        let avg_chars = chars.checked_div(count).unwrap_or(0);
        // Match crate::llm::utils::estimate_tokens (words × 1.5, ceil).
        let estimated_tokens = ((words as f64) * 1.5).ceil() as usize;

        categories.push(CategoryTokenStats {
            memory_type,
            count,
            total_chars: chars,
            estimated_tokens,
            avg_chars,
            with_embeddings,
        });

        total_chars += chars;
        total_memories += count;
        total_words += words;
    }

    let stats = MemoryTokenStats {
        categories,
        total_chars,
        total_estimated_tokens: ((total_words as f64) * 1.5).ceil() as usize,
        total_memories,
    };

    info!(
        total_memories = stats.total_memories,
        total_chars = stats.total_chars,
        total_tokens = stats.total_estimated_tokens,
        "Token statistics retrieved"
    );

    Ok(stats)
}

#[cfg(test)]
mod tests {
    use crate::models::MemoryType;
    use crate::test_utils::setup_test_state;
    use crate::tools::memory::helpers::{add_memory_core, AddMemoryParams};
    use serde_json::json;
    use uuid::Uuid;

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

    /// `with_embeddings` must count parents that have at least one
    /// `memory_chunk` row — orphan parents (no chunks) count as
    /// `without_embeddings`.
    #[tokio::test]
    async fn get_memory_stats_with_embeddings_counts_only_chunked_parents() {
        let (state, _guard) = setup_test_state().await;

        // 3 chunked parents (via add_memory_core).
        for content in ["alpha", "beta", "gamma"] {
            add_memory_core(params(MemoryType::Knowledge, content), &state.db, None)
                .await
                .unwrap();
        }

        // 1 orphan parent (no memory_chunk row).
        let orphan_id = Uuid::new_v4().to_string();
        state
            .db
            .execute_with_params(
                &format!(
                    "CREATE memory:`{}` CONTENT {{ type: 'knowledge', content: 'orphan', metadata: {{}} }}",
                    orphan_id
                ),
                vec![],
            )
            .await
            .unwrap();

        // Invoke via the inner helper path (Tauri State wrapper isn't
        // mock-friendly; we exercise the same SQL the command runs).
        let stats = super::get_stats_core(&state.db).await.unwrap();
        assert_eq!(stats.total, 4, "4 total parents");
        assert_eq!(
            stats.with_embeddings, 3,
            "only chunked parents count as 'with_embeddings'"
        );
        assert_eq!(
            stats.without_embeddings, 1,
            "orphan parent counts as without_embeddings"
        );
    }

    /// Per-category `with_embeddings` is grouped via record-link traversal
    /// `memory_id.type` over the `memory_chunk` table.
    #[tokio::test]
    async fn get_memory_token_stats_per_category_indexed_count() {
        let (state, _guard) = setup_test_state().await;

        add_memory_core(params(MemoryType::Knowledge, "k1 content"), &state.db, None)
            .await
            .unwrap();
        add_memory_core(params(MemoryType::Knowledge, "k2 content"), &state.db, None)
            .await
            .unwrap();
        add_memory_core(params(MemoryType::Context, "c1 content"), &state.db, None)
            .await
            .unwrap();

        // One decision parent without chunks (orphan).
        let orphan_id = Uuid::new_v4().to_string();
        state
            .db
            .execute_with_params(
                &format!(
                    "CREATE memory:`{}` CONTENT {{ type: 'decision', content: 'orphan d1', metadata: {{}} }}",
                    orphan_id
                ),
                vec![],
            )
            .await
            .unwrap();

        let stats = super::get_token_stats_core(&state.db, None).await.unwrap();

        let mut by_type: std::collections::HashMap<String, usize> = Default::default();
        for cat in &stats.categories {
            by_type.insert(cat.memory_type.clone(), cat.with_embeddings);
        }
        assert_eq!(by_type.get("knowledge").copied(), Some(2));
        assert_eq!(by_type.get("context").copied(), Some(1));
        assert_eq!(
            by_type.get("decision").copied(),
            Some(0),
            "decision parent without chunks → with_embeddings = 0"
        );

        // Non-regression: total_chars and counts should remain populated.
        let total_count: usize = stats.categories.iter().map(|c| c.count).sum();
        assert_eq!(total_count, stats.total_memories);
        assert_eq!(total_count, 4);
    }
}
