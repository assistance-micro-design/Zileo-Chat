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

//! Memory CRUD, export/import, and embedding regeneration commands.

use crate::{
    db::sanitize_for_surrealdb,
    models::{ExportFormat, ImportResult, Memory, RegenerateResult},
    security::{serialize_for_query, validate_uuid_field},
    AppState,
};
use serde_json::json;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Updates an existing memory entry.
///
/// # Arguments
/// * `memory_id` - The ID of the memory to update
/// * `content` - New content (optional)
/// * `metadata` - New metadata (optional)
#[tauri::command]
#[instrument(name = "update_memory", skip(state, content, metadata))]
pub async fn update_memory(
    memory_id: String,
    content: Option<String>,
    metadata: Option<serde_json::Value>,
    state: State<'_, AppState>,
) -> Result<Memory, String> {
    info!(memory_id = %memory_id, "Updating memory entry");

    let memory_id = validate_uuid_field(&memory_id, "memory_id")?;

    // Build update fields
    let mut updates = Vec::new();

    if let Some(ref c) = content {
        let trimmed = c.trim();
        if trimmed.is_empty() {
            return Err("Content cannot be empty".to_string());
        }
        if trimmed.len() > 50_000 {
            return Err("Content exceeds maximum length".to_string());
        }
        let content_json = serialize_for_query(trimmed, "content")?;
        updates.push(format!("content = {}", content_json));
    }

    if let Some(ref m) = metadata {
        let meta_str = serialize_for_query(m, "metadata")?;
        updates.push(format!("metadata = {}", meta_str));
    }

    if updates.is_empty() {
        return Err("No updates provided".to_string());
    }

    let update_query = format!("UPDATE memory:`{}` SET {}", memory_id, updates.join(", "));

    state.db.execute(&update_query).await.map_err(|e| {
        error!(error = %e, "Failed to update memory");
        format!("Failed to update memory: {}", e)
    })?;

    let select_query = format!(
        "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
         FROM memory WHERE meta::id(id) = '{}'",
        memory_id
    );

    let results: Vec<Memory> = state.db.query(&select_query).await.map_err(|e| {
        error!(error = %e, "Failed to fetch updated memory");
        format!("Failed to fetch updated memory: {}", e)
    })?;

    results.into_iter().next().ok_or_else(|| {
        warn!(memory_id = %memory_id, "Memory not found");
        "Memory not found".to_string()
    })
}

/// Exports memories to JSON or CSV format.
///
/// # Arguments
/// * `format` - Export format (json or csv)
/// * `type_filter` - Optional filter by memory type
#[tauri::command]
#[instrument(name = "export_memories", skip(state))]
pub async fn export_memories(
    format: ExportFormat,
    type_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!(format = ?format, type_filter = ?type_filter, "Exporting memories");

    let memories: Vec<Memory> = match type_filter {
        Some(ref mtype) => {
            let query =
                "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
                         FROM memory WHERE type = $type ORDER BY created_at DESC";
            state
                .db
                .query_with_params(query, vec![("type".to_string(), serde_json::json!(mtype))])
                .await
                .map_err(|e| {
                    error!(error = %e, "Failed to load memories for export");
                    format!("Failed to export memories: {}", e)
                })?
        }
        None => {
            let query =
                "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
                         FROM memory ORDER BY created_at DESC";
            state.db.query(query).await.map_err(|e| {
                error!(error = %e, "Failed to load memories for export");
                format!("Failed to export memories: {}", e)
            })?
        }
    };

    let export_data = match format {
        ExportFormat::Json => serde_json::to_string_pretty(&memories).map_err(|e| {
            error!(error = %e, "Failed to serialize memories to JSON");
            format!("Failed to create JSON export: {}", e)
        })?,
        ExportFormat::Csv => {
            let mut csv = String::from("id,type,content,metadata,created_at\n");
            for mem in &memories {
                csv.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    mem.id,
                    mem.memory_type,
                    mem.content.replace('"', "\"\""),
                    serde_json::to_string(&mem.metadata)
                        .unwrap_or_default()
                        .replace('"', "\"\""),
                    mem.created_at.to_rfc3339()
                ));
            }
            csv
        }
    };

    info!(count = memories.len(), "Memories exported successfully");
    Ok(export_data)
}

/// Imports memories from JSON data.
///
/// # Arguments
/// * `data` - JSON string containing array of memories to import
#[tauri::command]
#[instrument(name = "import_memories", skip(state, data), fields(data_len = data.len()))]
pub async fn import_memories(
    data: String,
    state: State<'_, AppState>,
) -> Result<ImportResult, String> {
    info!("Importing memories");

    let memories: Vec<serde_json::Value> = serde_json::from_str(&data).map_err(|e| {
        error!(error = %e, "Failed to parse import data");
        format!("Invalid JSON format: {}", e)
    })?;

    let mut imported = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for (idx, mem) in memories.iter().enumerate() {
        let memory_type = mem
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("knowledge");

        let content = match mem.get("content").and_then(|c| c.as_str()) {
            Some(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                failed += 1;
                errors.push(format!("Item {}: Missing or empty content", idx));
                continue;
            }
        };

        let metadata = mem.get("metadata").cloned().unwrap_or_else(|| json!({}));

        let memory_id = uuid::Uuid::new_v4().to_string();
        // Use the centralized sanitizer rather than ad-hoc \0 stripping.
        // It also enforces the depth limit and handles nested JSON consistently
        // — required because `metadata` is opaque user-provided JSON that may
        // contain null bytes anywhere in the tree (ERR_SURREAL_006).
        let sanitized_content = sanitize_for_surrealdb(serde_json::json!(content));
        let sanitized_metadata = sanitize_for_surrealdb(metadata.clone());
        let create_query = format!(
            "CREATE memory:`{}` CONTENT {{ type: $mtype, content: $content, metadata: $metadata }}",
            memory_id
        );

        match state
            .db
            .execute_with_params(
                &create_query,
                vec![
                    ("mtype".to_string(), serde_json::json!(memory_type)),
                    ("content".to_string(), sanitized_content),
                    ("metadata".to_string(), sanitized_metadata),
                ],
            )
            .await
        {
            Ok(_) => imported += 1,
            Err(e) => {
                failed += 1;
                errors.push(format!("Item {}: {}", idx, e));
            }
        }
    }

    info!(imported, failed, "Memory import completed");

    Ok(ImportResult {
        imported,
        failed,
        errors,
    })
}

/// Regenerates embeddings for existing memories.
///
/// # Arguments
/// * `type_filter` - Optional filter to only regenerate for specific type
#[tauri::command]
#[instrument(name = "regenerate_embeddings", skip(state))]
pub async fn regenerate_embeddings(
    type_filter: Option<String>,
    state: State<'_, AppState>,
) -> Result<RegenerateResult, String> {
    info!(type_filter = ?type_filter, "Regenerating embeddings");

    let service_guard = state.embedding_service.read().await;
    let service = match service_guard.as_ref() {
        Some(s) => s.clone(),
        None => {
            return Err(
                "Embedding service not configured. Please save embedding settings first."
                    .to_string(),
            );
        }
    };
    drop(service_guard);

    let memories: Vec<serde_json::Value> = match type_filter {
        Some(ref mtype) => {
            state
                .db
                .query_json_with_params(
                    "SELECT meta::id(id) AS id, content FROM memory WHERE type = $mtype",
                    vec![("mtype".to_string(), serde_json::json!(mtype))],
                )
                .await
        }
        None => {
            state
                .db
                .query_json("SELECT meta::id(id) AS id, content FROM memory")
                .await
        }
    }
    .map_err(|e| {
        error!(error = %e, "Failed to load memories for regeneration");
        format!("Failed to load memories: {}", e)
    })?;

    let mut processed = 0;
    let mut success = 0;
    let mut failed = 0;

    for mem in &memories {
        processed += 1;

        let id_raw = match mem.get("id").and_then(|i| i.as_str()) {
            Some(i) => i,
            None => {
                failed += 1;
                continue;
            }
        };

        // Defense-in-depth: validate the id even though it comes from our DB.
        // Skips rather than aborts the whole batch on malformed rows.
        let id = match validate_uuid_field(id_raw, "memory_id") {
            Ok(v) => v,
            Err(e) => {
                warn!(memory_id = %id_raw, error = %e, "Skipping memory with invalid id");
                failed += 1;
                continue;
            }
        };

        let content = match mem.get("content").and_then(|c| c.as_str()) {
            Some(c) => c,
            None => {
                failed += 1;
                continue;
            }
        };

        match service.embed(content).await {
            Ok(embedding) => {
                let embedding_json = serde_json::to_string(&embedding).unwrap_or_default();
                let update_query =
                    format!("UPDATE memory:`{}` SET embedding = {}", id, embedding_json);

                match state.db.execute(&update_query).await {
                    Ok(_) => success += 1,
                    Err(e) => {
                        warn!(memory_id = %id, error = %e, "Failed to update embedding");
                        failed += 1;
                    }
                }
            }
            Err(e) => {
                warn!(memory_id = %id, error = %e, "Failed to generate embedding");
                failed += 1;
            }
        }
    }

    info!(
        processed,
        success, failed, "Embedding regeneration completed"
    );

    Ok(RegenerateResult {
        processed,
        success,
        failed,
    })
}
