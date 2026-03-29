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

use crate::{
    db::extract_count,
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

    // Get total count
    let total_query = "SELECT count() AS count FROM memory GROUP ALL";
    let total_result: Vec<serde_json::Value> = state.db.query(total_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to get memory count: {}", e)
    })?;

    let total = extract_count(&total_result) as usize;

    // Get count with embeddings
    let with_embeddings_query =
        "SELECT count() AS count FROM memory WHERE embedding != NONE GROUP ALL";
    let with_result: Vec<serde_json::Value> = state
        .db
        .query(with_embeddings_query)
        .await
        .unwrap_or_default();

    let with_embeddings = extract_count(&with_result) as usize;

    // Get count by type
    let by_type_query = "SELECT type, count() AS count FROM memory GROUP BY type";
    let type_result: Vec<serde_json::Value> =
        state.db.query(by_type_query).await.unwrap_or_default();

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
    let agent_result: Vec<serde_json::Value> =
        state.db.query(by_agent_query).await.unwrap_or_default();

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

    let base_query = r#"SELECT
            type,
            count() AS count,
            math::sum(string::len(content)) AS total_chars,
            count(embedding != NONE) AS with_embeddings
        FROM memory"#;

    let results: Vec<serde_json::Value> = if let Some(ref mem_type) = type_filter {
        let query = format!("{} WHERE type = $mtype GROUP BY type", base_query);
        state
            .db
            .query_json_with_params(
                &query,
                vec![("mtype".to_string(), serde_json::json!(mem_type))],
            )
            .await
    } else {
        let query = format!("{} GROUP BY type", base_query);
        state.db.query_json(&query).await
    }
    .map_err(|e| {
        error!(error = %e, "Failed to get token stats");
        format!("Failed to get token statistics: {}", e)
    })?;

    let mut categories = Vec::new();
    let mut total_chars: usize = 0;
    let mut total_memories: usize = 0;

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

        let with_embeddings = row
            .get("with_embeddings")
            .and_then(|c| c.as_u64())
            .unwrap_or_else(|| {
                warn!(row = ?row, "Unexpected DB result: missing or invalid 'with_embeddings' field");
                0
            }) as usize;

        let avg_chars = if count > 0 { chars / count } else { 0 };
        let estimated_tokens = chars / 4; // Standard approximation

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
    }

    let stats = MemoryTokenStats {
        categories,
        total_chars,
        total_estimated_tokens: total_chars / 4,
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
