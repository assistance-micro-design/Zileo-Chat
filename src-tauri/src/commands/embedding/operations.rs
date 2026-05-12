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
    models::{ExportFormat, ImportResult, Memory, ReindexJobStatus},
    security::{serialize_for_query, validate_uuid_field},
    tools::memory::chunker::{split_recursive, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE},
    tools::memory::helpers::create_memory_chunk,
    AppState,
};
use chrono::Utc;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

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

/// Tauri event name emitted by the reindex background task.
const REINDEX_PROGRESS_EVENT: &str = "reindex-progress";

/// Number of seconds to keep a terminal job in `AppState.reindex_jobs`
/// before the auto-cleanup task removes it. 10 minutes gives the UI plenty
/// of time to remount and read the terminal status retroactively.
const REINDEX_JOB_RETENTION_SECS: u64 = 600;

/// Spawns a background reindex of all unindexed memories and returns
/// immediately with a `job_id`.
///
/// The task creates one `memory_chunk` row per chunk per pending parent,
/// emitting a `reindex-progress` event after each parent (granularity:
/// one memory = N chunks atomic). The job can be cancelled via
/// `cancel_reindex_job`; its live status can be polled via
/// `get_reindex_job_status` (useful on UI remount).
///
/// # Arguments
/// * `force` - reserved for parity with the previous sync API; the
///   migration_log guard is not consulted on the background path since
///   every run starts from "parents without chunks" anyway.
#[tauri::command]
#[instrument(name = "reindex_memory_chunks", skip(state, app_handle))]
pub async fn reindex_memory_chunks(
    force: bool,
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!(force = force, "Spawning memory chunk reindex job");

    let service_guard = state.embedding_service.read().await;
    let service = service_guard.as_ref().cloned();
    drop(service_guard);

    if service.is_none() {
        // Refuse the spawn so the UI surfaces the missing-config state
        // instead of silently producing chunks with embedding = NONE.
        return Err(
            "Embedding service not configured. Please save embedding settings first.".to_string(),
        );
    }

    let job_id = Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    state
        .reindex_cancellations
        .lock()
        .await
        .insert(job_id.clone(), token.clone());

    let initial = ReindexJobStatus {
        job_id: job_id.clone(),
        status: "running".to_string(),
        processed: 0,
        total: 0,
        chunks_created: 0,
        current_memory_id: None,
        error_message: None,
        started_at: Utc::now(),
        finished_at: None,
    };
    state
        .reindex_jobs
        .lock()
        .await
        .insert(job_id.clone(), initial);

    let db = state.db.clone();
    let cancellations = state.reindex_cancellations.clone();
    let jobs = state.reindex_jobs.clone();
    let job_id_task = job_id.clone();
    let service = service.expect("service Some checked above");

    tokio::spawn(async move {
        run_reindex_with_progress(
            db,
            service,
            app_handle,
            job_id_task,
            token,
            cancellations,
            jobs,
        )
        .await;
    });

    Ok(job_id)
}

/// Cancels a running reindex job. Idempotent — unknown ids are a no-op.
#[tauri::command]
#[instrument(name = "cancel_reindex_job", skip(state))]
pub async fn cancel_reindex_job(job_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!(job_id = %job_id, "Cancel reindex job requested");
    if let Some(token) = state.reindex_cancellations.lock().await.get(&job_id) {
        token.cancel();
    }
    Ok(())
}

/// Returns the live status of a reindex job, or `None` when unknown.
///
/// Terminal entries are auto-purged from the map after being read, so the
/// UI can rely on a single round-trip to consume a "retroactive toast"
/// without leaving stale state in the map.
#[tauri::command]
#[instrument(name = "get_reindex_job_status", skip(state))]
pub async fn get_reindex_job_status(
    job_id: String,
    state: State<'_, AppState>,
) -> Result<Option<ReindexJobStatus>, String> {
    let mut jobs = state.reindex_jobs.lock().await;
    let Some(status) = jobs.get(&job_id).cloned() else {
        return Ok(None);
    };
    if status.status != "running" {
        jobs.remove(&job_id);
    }
    Ok(Some(status))
}

/// Background task body — owned, no `&State` references survive the spawn.
#[allow(clippy::too_many_arguments)]
async fn run_reindex_with_progress(
    db: Arc<crate::db::DBClient>,
    embed: Arc<crate::llm::embedding::EmbeddingService>,
    app_handle: AppHandle,
    job_id: String,
    token: CancellationToken,
    cancellations: Arc<tokio::sync::Mutex<std::collections::HashMap<String, CancellationToken>>>,
    jobs: Arc<tokio::sync::Mutex<std::collections::HashMap<String, ReindexJobStatus>>>,
) {
    // 1. List pending parents (resumable: skip already-chunked).
    let pending: Vec<serde_json::Value> = match db
        .query_json(
            "SELECT meta::id(id) AS id, content FROM memory \
             WHERE id NOT IN (SELECT VALUE memory_id FROM memory_chunk)",
        )
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            finalize_job(
                &jobs,
                &cancellations,
                &app_handle,
                &job_id,
                "error",
                0,
                0,
                0,
                Some(e.to_string()),
            )
            .await;
            return;
        }
    };

    let total = pending.len();
    update_job(&jobs, &job_id, |s| {
        s.total = total;
    })
    .await;
    emit_progress(&app_handle, &jobs, &job_id).await;

    let mut processed: usize = 0;
    let mut chunks_created: usize = 0;

    for row in &pending {
        if token.is_cancelled() {
            finalize_job(
                &jobs,
                &cancellations,
                &app_handle,
                &job_id,
                "cancelled",
                processed,
                total,
                chunks_created,
                None,
            )
            .await;
            schedule_purge(jobs.clone(), job_id.clone());
            return;
        }

        let Some(mem_id) = row.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let Some(content) = row.get("content").and_then(|v| v.as_str()) else {
            continue;
        };

        let chunks = split_recursive(content, DEFAULT_CHUNK_SIZE, DEFAULT_CHUNK_OVERLAP);
        let chunk_count = chunks.len();

        for (idx, chunk_text) in chunks.iter().enumerate() {
            let embedding = match embed.embed(chunk_text).await {
                Ok(v) => Some(v),
                Err(e) => {
                    warn!(memory_id = %mem_id, chunk_index = idx, error = %e, "Embedding failed; chunk stored without embedding");
                    None
                }
            };
            if let Err(e) =
                create_memory_chunk(&db, mem_id, idx, chunk_count, chunk_text, embedding).await
            {
                finalize_job(
                    &jobs,
                    &cancellations,
                    &app_handle,
                    &job_id,
                    "error",
                    processed,
                    total,
                    chunks_created,
                    Some(e),
                )
                .await;
                schedule_purge(jobs.clone(), job_id.clone());
                return;
            }
            chunks_created += 1;
        }
        processed += 1;
        update_job(&jobs, &job_id, |s| {
            s.processed = processed;
            s.chunks_created = chunks_created;
            s.current_memory_id = Some(mem_id.to_string());
        })
        .await;
        emit_progress(&app_handle, &jobs, &job_id).await;
    }

    finalize_job(
        &jobs,
        &cancellations,
        &app_handle,
        &job_id,
        "completed",
        processed,
        total,
        chunks_created,
        None,
    )
    .await;
    schedule_purge(jobs, job_id);
}

/// Updates a job entry under the shared lock.
async fn update_job(
    jobs: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, ReindexJobStatus>>>,
    job_id: &str,
    mutator: impl FnOnce(&mut ReindexJobStatus),
) {
    let mut map = jobs.lock().await;
    if let Some(entry) = map.get_mut(job_id) {
        mutator(entry);
    }
}

/// Emits the current job snapshot as a `reindex-progress` event.
async fn emit_progress(
    app_handle: &AppHandle,
    jobs: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, ReindexJobStatus>>>,
    job_id: &str,
) {
    let snapshot = jobs.lock().await.get(job_id).cloned();
    if let Some(s) = snapshot {
        if let Err(e) = app_handle.emit(REINDEX_PROGRESS_EVENT, &s) {
            warn!(error = %e, "Failed to emit reindex-progress event");
        }
    }
}

/// Marks a job terminal: updates the status, emits the final event, removes
/// the cancellation token from the map.
#[allow(clippy::too_many_arguments)]
async fn finalize_job(
    jobs: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, ReindexJobStatus>>>,
    cancellations: &Arc<tokio::sync::Mutex<std::collections::HashMap<String, CancellationToken>>>,
    app_handle: &AppHandle,
    job_id: &str,
    status: &str,
    processed: usize,
    total: usize,
    chunks_created: usize,
    error_message: Option<String>,
) {
    update_job(jobs, job_id, |s| {
        s.status = status.to_string();
        s.processed = processed;
        s.total = total;
        s.chunks_created = chunks_created;
        s.error_message = error_message.clone();
        s.finished_at = Some(Utc::now());
        s.current_memory_id = None;
    })
    .await;
    emit_progress(app_handle, jobs, job_id).await;
    cancellations.lock().await.remove(job_id);
    info!(job_id = %job_id, status = %status, processed, total, chunks_created, "Reindex job terminal");
}

/// Schedules purge of a terminal job entry from the map after a delay so
/// late readers can still see the final state via `get_reindex_job_status`.
fn schedule_purge(
    jobs: Arc<tokio::sync::Mutex<std::collections::HashMap<String, ReindexJobStatus>>>,
    job_id: String,
) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(REINDEX_JOB_RETENTION_SECS)).await;
        jobs.lock().await.remove(&job_id);
    });
}
