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

use super::helpers::set_created_at;
use crate::{
    db::DBClient,
    llm::embedding::EmbeddingService,
    models::{ExportFormat, ImportResult, Memory, MemoryType, ReindexJobStatus},
    security::{serialize_for_query, validate_uuid_field},
    tools::memory::chunker::{split_recursive, DEFAULT_CHUNK_OVERLAP, DEFAULT_CHUNK_SIZE},
    tools::memory::helpers::{
        add_memory_core, create_memory_chunk, replace_memory_chunks, AddMemoryParams,
    },
    AppState,
};
use chrono::{DateTime, Utc};
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
    let embedding_service = state.embedding_service.read().await.clone();
    update_core(
        &state.db,
        embedding_service.as_ref(),
        &memory_id,
        content,
        metadata,
    )
    .await
}

/// Core implementation of [`update_memory`].
///
/// Beyond the obvious `UPDATE memory:<id> SET …`, this also keeps the
/// `memory_chunk` index in sync: when `content` changes, the existing
/// chunks (still indexed against the old text and embeddings) are dropped
/// and re-created from the new content — otherwise the parent row would
/// carry the new content while semantic search keeps matching the old one.
async fn update_core(
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
    raw_memory_id: &str,
    content: Option<String>,
    metadata: Option<serde_json::Value>,
) -> Result<Memory, String> {
    let memory_id = validate_uuid_field(raw_memory_id, "memory_id")?;

    // Validate + normalise content up-front so we can both UPDATE and
    // re-chunkify with the same trimmed string.
    let normalised_content: Option<String> = match content {
        Some(c) => {
            let trimmed = c.trim();
            if trimmed.is_empty() {
                return Err("Content cannot be empty".to_string());
            }
            if trimmed.len() > 50_000 {
                return Err("Content exceeds maximum length".to_string());
            }
            Some(trimmed.to_string())
        }
        None => None,
    };

    // Build UPDATE SET clause.
    let mut updates = Vec::new();
    if let Some(ref c) = normalised_content {
        let content_json = serialize_for_query(c, "content")?;
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
    db.execute(&update_query).await.map_err(|e| {
        error!(error = %e, "Failed to update memory");
        format!("Failed to update memory: {}", e)
    })?;

    // Keep the chunk index in sync with the new content. Re-chunkify ONLY
    // when content actually changed — a metadata-only update leaves the
    // (still-correct) chunks alone, saving the embedding round-trips.
    if let Some(ref c) = normalised_content {
        replace_memory_chunks(db, embedding_service, &memory_id, c).await?;
    }

    let select_query = format!(
        "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, \
         importance, expires_at, created_at \
         FROM memory WHERE meta::id(id) = '{}'",
        memory_id
    );

    let results: Vec<Memory> = db.query(&select_query).await.map_err(|e| {
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
    export_core(&state.db, format, type_filter).await
}

/// Core implementation of [`export_memories`] — separated from the Tauri
/// command so tests can exercise the real SQL + serialization without
/// instantiating `tauri::State`.
async fn export_core(
    db: &DBClient,
    format: ExportFormat,
    type_filter: Option<String>,
) -> Result<String, String> {
    let memories: Vec<Memory> = match type_filter {
        Some(ref mtype) => {
            let query = "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, \
                         importance, expires_at, created_at \
                         FROM memory WHERE type = $type ORDER BY created_at DESC";
            db.query_with_params(query, vec![("type".to_string(), serde_json::json!(mtype))])
                .await
                .map_err(|e| {
                    error!(error = %e, "Failed to load memories for export");
                    format!("Failed to export memories: {}", e)
                })?
        }
        None => {
            let query = "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, \
                         importance, expires_at, created_at \
                         FROM memory ORDER BY created_at DESC";
            db.query(query).await.map_err(|e| {
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
            let mut csv = String::from(
                "id,type,content,workflow_id,metadata,importance,expires_at,created_at\n",
            );
            for mem in &memories {
                let workflow_id = mem.workflow_id.clone().unwrap_or_default();
                let expires_at = mem.expires_at.map(|d| d.to_rfc3339()).unwrap_or_default();
                csv.push_str(&format!(
                    "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",\"{}\"\n",
                    mem.id,
                    mem.memory_type,
                    mem.content.replace('"', "\"\""),
                    workflow_id.replace('"', "\"\""),
                    serde_json::to_string(&mem.metadata)
                        .unwrap_or_default()
                        .replace('"', "\"\""),
                    mem.importance,
                    expires_at,
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
    let embedding_service = state.embedding_service.read().await.clone();
    import_core(&state.db, embedding_service.as_ref(), &data).await
}

/// Core implementation of [`import_memories`] — runs the import against an
/// arbitrary `DBClient` and optional `EmbeddingService` so the round-trip
/// can be tested end-to-end on a fresh DB without `tauri::State`.
///
/// Behavior contract (regression-locked by tests):
/// - **Type validation is strict**: missing or unknown `type` → counted in
///   `failed` with an explicit error (no silent fallback to `knowledge`).
/// - **Content validation**: missing or empty `content` → counted in `failed`.
/// - **Optional fields preserved when present**: `workflow_id`, `importance`,
///   `expires_at`, `created_at`. Absent fields fall back to schema defaults
///   (`importance = 0.5`, `created_at = time::now()`, the rest stay `None`).
/// - **Chunkification**: each successful import goes through
///   [`add_memory_core`] so parent + N chunks (+embeddings if service
///   present) are produced atomically — without this, imports would land
///   as orphan parents invisible to semantic search.
async fn import_core(
    db: &DBClient,
    embedding_service: Option<&Arc<EmbeddingService>>,
    data: &str,
) -> Result<ImportResult, String> {
    let memories: Vec<serde_json::Value> = serde_json::from_str(data).map_err(|e| {
        error!(error = %e, "Failed to parse import data");
        format!("Invalid JSON format: {}", e)
    })?;

    let mut imported = 0usize;
    let mut failed = 0usize;
    let mut errors: Vec<String> = Vec::new();

    for (idx, mem) in memories.iter().enumerate() {
        // --- Strict type validation (no silent fallback) -----------------
        let memory_type_str = match mem.get("type").and_then(|t| t.as_str()) {
            Some(s) => s,
            None => {
                failed += 1;
                errors.push(format!("Item {}: missing 'type' field", idx));
                continue;
            }
        };
        let memory_type: MemoryType = match serde_json::from_value(json!(memory_type_str)) {
            Ok(t) => t,
            Err(_) => {
                failed += 1;
                errors.push(format!(
                        "Item {}: invalid memory_type '{}' (expected one of: user_pref, context, knowledge, decision)",
                        idx, memory_type_str
                    ));
                continue;
            }
        };

        // --- Content validation ------------------------------------------
        let content = match mem.get("content").and_then(|c| c.as_str()) {
            Some(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                failed += 1;
                errors.push(format!("Item {}: missing or empty content", idx));
                continue;
            }
        };

        let metadata = mem.get("metadata").cloned().unwrap_or_else(|| json!({}));

        // --- Optional fields (Option<T>) — preserved when present --------
        let workflow_id = mem
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);

        // Invalid RFC3339 is treated as absent (tolerant policy — `None`
        // falls back to the schema default). Cast happens via
        // `<datetime>$param` in the helpers (ERR_SURREAL_007).
        let expires_at: Option<DateTime<Utc>> = mem
            .get("expires_at")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        let original_created_at: Option<DateTime<Utc>> = mem
            .get("created_at")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc));

        // `importance` falls back to the schema's DEFAULT 0.5 — matches the
        // behavior of a minimal CREATE without the field.
        let importance = mem
            .get("importance")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.5);

        let params = AddMemoryParams {
            memory_type,
            content,
            metadata,
            workflow_id,
            importance,
            expires_at,
        };

        match add_memory_core(params, db, embedding_service).await {
            Ok(result) => {
                if let Some(orig) = original_created_at {
                    // Best-effort: overriding created_at can fail in
                    // edge cases (read-only mode, schema drift). A failure
                    // here doesn't invalidate the import — the row exists
                    // with the default timestamp.
                    if let Err(e) = set_created_at(db, &result.memory_id, orig).await {
                        warn!(memory_id = %result.memory_id, error = %e, "Failed to override created_at on imported memory; keeping default");
                    }
                }
                imported += 1;
            }
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
#[tauri::command]
#[instrument(name = "reindex_memory_chunks", skip(state, app_handle))]
pub async fn reindex_memory_chunks(
    app_handle: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Spawning memory chunk reindex job");

    let service_guard = state.embedding_service.read().await;
    let service = service_guard.as_ref().cloned();
    drop(service_guard);

    let Some(service) = service else {
        // Refuse the spawn so the UI surfaces the missing-config state
        // instead of silently producing chunks with embedding = NONE.
        return Err(
            "Embedding service not configured. Please save embedding settings first.".to_string(),
        );
    };

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

#[cfg(test)]
mod export_import_tests {
    use super::*;
    use crate::test_utils::setup_test_state;
    use chrono::TimeZone;

    fn make_params(memory_type: MemoryType, content: &str) -> AddMemoryParams {
        AddMemoryParams {
            memory_type,
            content: content.to_string(),
            metadata: json!({}),
            workflow_id: None,
            importance: 0.5,
            expires_at: None,
        }
    }

    // ---- export ----------------------------------------------------------

    #[tokio::test]
    async fn export_memories_json_includes_importance_and_expires_at() {
        let (state, _guard) = setup_test_state().await;
        let expires = Utc.with_ymd_and_hms(2030, 1, 1, 0, 0, 0).unwrap();

        let params = AddMemoryParams {
            memory_type: MemoryType::Knowledge,
            content: "exported content".to_string(),
            metadata: json!({}),
            workflow_id: None,
            importance: 0.9,
            expires_at: Some(expires),
        };
        add_memory_core(params, &state.db, None).await.unwrap();

        let out = export_core(&state.db, ExportFormat::Json, None)
            .await
            .unwrap();
        let parsed: Vec<Memory> = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed.len(), 1);
        assert!((parsed[0].importance - 0.9).abs() < f64::EPSILON);
        assert_eq!(parsed[0].expires_at, Some(expires));
    }

    #[tokio::test]
    async fn export_memories_csv_includes_importance_and_expires_at_columns() {
        let (state, _guard) = setup_test_state().await;
        let expires = Utc.with_ymd_and_hms(2030, 6, 15, 12, 0, 0).unwrap();

        let params = AddMemoryParams {
            memory_type: MemoryType::Context,
            content: "csv content".to_string(),
            metadata: json!({}),
            workflow_id: Some("wf-csv".to_string()),
            importance: 0.7,
            expires_at: Some(expires),
        };
        add_memory_core(params, &state.db, None).await.unwrap();

        let out = export_core(&state.db, ExportFormat::Csv, None)
            .await
            .unwrap();
        let mut lines = out.lines();
        let header = lines.next().unwrap();
        assert_eq!(
            header,
            "id,type,content,workflow_id,metadata,importance,expires_at,created_at"
        );
        let row = lines.next().expect("expected at least one data row");
        assert!(
            row.contains("0.7"),
            "row should contain importance: {}",
            row
        );
        assert!(
            row.contains(&expires.to_rfc3339()),
            "row should contain expires_at: {}",
            row
        );
        assert!(
            row.contains("wf-csv"),
            "row should contain workflow_id: {}",
            row
        );
    }

    // ---- import ----------------------------------------------------------

    /// Helper to count chunks linked to a given parent memory id.
    async fn count_chunks(db: &DBClient, memory_id: &str) -> usize {
        let rows: Vec<serde_json::Value> = db
            .query_json(&format!(
                "SELECT count() AS count FROM memory_chunk WHERE memory_id = memory:`{}` GROUP ALL",
                memory_id
            ))
            .await
            .unwrap();
        rows.first()
            .and_then(|r| r.get("count"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize
    }

    /// Helper to fetch a freshly-imported memory by its `content` text.
    async fn fetch_by_content(db: &DBClient, content: &str) -> serde_json::Value {
        let rows: Vec<serde_json::Value> = db
            .query_json_with_params(
                "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, \
                 importance, expires_at, created_at FROM memory WHERE content = $c",
                vec![("c".to_string(), json!(content))],
            )
            .await
            .unwrap();
        rows.into_iter()
            .next()
            .unwrap_or_else(|| panic!("no memory with content '{}'", content))
    }

    #[tokio::test]
    async fn import_memories_preserves_workflow_id_importance_expires_at() {
        let (state, _guard) = setup_test_state().await;
        let expires = Utc.with_ymd_and_hms(2030, 3, 14, 0, 0, 0).unwrap();
        let payload = json!([{
            "type": "context",
            "content": "preserve me",
            "metadata": {},
            "workflow_id": "wf1",
            "importance": 0.8,
            "expires_at": expires.to_rfc3339()
        }])
        .to_string();

        let result = import_core(&state.db, None, &payload).await.unwrap();
        assert_eq!(result.imported, 1);
        assert_eq!(result.failed, 0);

        let row = fetch_by_content(&state.db, "preserve me").await;
        assert_eq!(row.get("workflow_id").and_then(|v| v.as_str()), Some("wf1"));
        assert!(
            (row.get("importance").and_then(|v| v.as_f64()).unwrap() - 0.8).abs() < f64::EPSILON
        );
        let stored_expires = row.get("expires_at").and_then(|v| v.as_str()).unwrap();
        let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(stored_expires)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed, expires);
    }

    #[tokio::test]
    async fn import_memories_creates_chunks_for_each_memory() {
        let (state, _guard) = setup_test_state().await;
        let payload = json!([{
            "type": "knowledge",
            "content": "this memory should get at least one chunk after import",
            "metadata": {}
        }])
        .to_string();

        let result = import_core(&state.db, None, &payload).await.unwrap();
        assert_eq!(result.imported, 1);

        let row = fetch_by_content(
            &state.db,
            "this memory should get at least one chunk after import",
        )
        .await;
        let id = row.get("id").and_then(|v| v.as_str()).unwrap();
        let chunk_count = count_chunks(&state.db, id).await;
        assert!(
            chunk_count >= 1,
            "expected at least 1 chunk for imported memory, got {}",
            chunk_count
        );
    }

    #[tokio::test]
    async fn import_memories_preserves_created_at_when_provided() {
        let (state, _guard) = setup_test_state().await;
        let target = Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap();
        let payload = json!([{
            "type": "knowledge",
            "content": "old memory",
            "metadata": {},
            "created_at": target.to_rfc3339()
        }])
        .to_string();

        let result = import_core(&state.db, None, &payload).await.unwrap();
        assert_eq!(result.imported, 1);

        let row = fetch_by_content(&state.db, "old memory").await;
        let stored = row.get("created_at").and_then(|v| v.as_str()).unwrap();
        let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(stored)
            .unwrap()
            .with_timezone(&Utc);
        assert_eq!(parsed, target);
    }

    #[tokio::test]
    async fn import_memories_falls_back_to_now_when_created_at_absent() {
        let (state, _guard) = setup_test_state().await;
        let before = Utc::now();
        let payload = json!([{
            "type": "knowledge",
            "content": "fresh memory",
            "metadata": {}
        }])
        .to_string();

        let result = import_core(&state.db, None, &payload).await.unwrap();
        assert_eq!(result.imported, 1);

        let row = fetch_by_content(&state.db, "fresh memory").await;
        let stored = row.get("created_at").and_then(|v| v.as_str()).unwrap();
        let parsed: DateTime<Utc> = DateTime::parse_from_rfc3339(stored)
            .unwrap()
            .with_timezone(&Utc);
        // Should be roughly `now`, definitely after `before - 1s`.
        assert!(
            parsed >= before - chrono::Duration::seconds(1),
            "created_at should be ~now, got {}",
            parsed
        );
        assert!(
            parsed <= Utc::now() + chrono::Duration::seconds(1),
            "created_at should be ~now, got {}",
            parsed
        );
    }

    #[tokio::test]
    async fn import_memories_rejects_invalid_memory_type() {
        let (state, _guard) = setup_test_state().await;
        let payload = json!([
            { "type": "knowledge", "content": "valid one", "metadata": {} },
            { "type": "foobar", "content": "bad type", "metadata": {} },
            { "content": "missing type", "metadata": {} }
        ])
        .to_string();

        let result = import_core(&state.db, None, &payload).await.unwrap();
        assert_eq!(result.imported, 1, "only the valid item should be imported");
        assert_eq!(result.failed, 2);
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("invalid memory_type 'foobar'")),
            "errors should mention foobar: {:?}",
            result.errors
        );
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.contains("missing 'type' field")),
            "errors should mention missing type: {:?}",
            result.errors
        );

        // DB should contain exactly 1 memory.
        let rows: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT count() AS count FROM memory GROUP ALL")
            .await
            .unwrap();
        let total = rows
            .first()
            .and_then(|r| r.get("count"))
            .and_then(|c| c.as_u64())
            .unwrap_or(0);
        assert_eq!(total, 1);
    }

    // ---- update_memory ----------------------------------------------------

    /// When content changes the chunk index must follow: parent rows carry
    /// the new text and chunks (with their embeddings) must match it.
    #[tokio::test]
    async fn update_memory_replaces_chunks_when_content_changes() {
        let (state, _guard) = setup_test_state().await;

        let added = add_memory_core(
            make_params(MemoryType::Knowledge, "old text body that should disappear"),
            &state.db,
            None,
        )
        .await
        .unwrap();
        let id = added.memory_id.clone();
        let chunks_before = count_chunks(&state.db, &id).await;
        assert!(chunks_before >= 1);

        let updated = update_core(
            &state.db,
            None,
            &id,
            Some("brand new content that replaces the previous body entirely".to_string()),
            None,
        )
        .await
        .unwrap();
        assert_eq!(
            updated.content,
            "brand new content that replaces the previous body entirely"
        );

        // The chunks must now reflect the new content — none should carry
        // the old phrase.
        let rows: Vec<serde_json::Value> = state
            .db
            .query_json(&format!(
                "SELECT content FROM memory_chunk WHERE memory_id = memory:`{}`",
                id
            ))
            .await
            .unwrap();
        assert!(!rows.is_empty(), "expected at least one chunk after update");
        for row in &rows {
            let chunk_content = row.get("content").and_then(|v| v.as_str()).unwrap();
            assert!(
                !chunk_content.contains("old text"),
                "stale chunk still contains old phrase: {}",
                chunk_content
            );
        }
    }

    /// A metadata-only update must not touch the chunk index — re-chunkifying
    /// would burn pointless embedding calls and disturb persisted state.
    #[tokio::test]
    async fn update_memory_preserves_chunks_when_only_metadata_changes() {
        let (state, _guard) = setup_test_state().await;

        let added = add_memory_core(
            make_params(
                MemoryType::Knowledge,
                "stable content unchanged across updates",
            ),
            &state.db,
            None,
        )
        .await
        .unwrap();
        let id = added.memory_id.clone();

        // Capture chunk ids before update.
        let ids_before: Vec<String> = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM memory_chunk WHERE memory_id = memory:`{}`",
                id
            ))
            .await
            .unwrap()
            .into_iter()
            .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();
        assert!(!ids_before.is_empty());

        update_core(
            &state.db,
            None,
            &id,
            None, // no content change
            Some(json!({ "tags": ["updated"], "agent_source": null, "priority": null })),
        )
        .await
        .unwrap();

        // Chunks must still exist with the same ids.
        let ids_after: Vec<String> = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id FROM memory_chunk WHERE memory_id = memory:`{}`",
                id
            ))
            .await
            .unwrap()
            .into_iter()
            .filter_map(|r| r.get("id").and_then(|v| v.as_str()).map(String::from))
            .collect();
        assert_eq!(
            ids_after, ids_before,
            "metadata-only update must leave chunk ids untouched"
        );
    }

    /// The IPC response must echo back every field the row stores —
    /// `importance` and `expires_at` are easy to drop from the SELECT and
    /// not notice until the UI shows the wrong values.
    #[tokio::test]
    async fn update_memory_returns_importance_and_expires_at() {
        let (state, _guard) = setup_test_state().await;
        let expires = Utc.with_ymd_and_hms(2030, 5, 5, 0, 0, 0).unwrap();

        let added = add_memory_core(
            AddMemoryParams {
                memory_type: MemoryType::Context,
                content: "scoped memory".to_string(),
                metadata: json!({}),
                workflow_id: Some("wf-up".to_string()),
                importance: 0.9,
                expires_at: Some(expires),
            },
            &state.db,
            None,
        )
        .await
        .unwrap();

        let updated = update_core(
            &state.db,
            None,
            &added.memory_id,
            Some("scoped memory new body".to_string()),
            None,
        )
        .await
        .unwrap();

        assert!(
            (updated.importance - 0.9).abs() < f64::EPSILON,
            "importance must round-trip; got {}",
            updated.importance
        );
        assert_eq!(
            updated.expires_at,
            Some(expires),
            "expires_at must round-trip"
        );
        assert_eq!(updated.workflow_id.as_deref(), Some("wf-up"));
    }

    #[tokio::test]
    async fn import_memories_round_trips_with_export() {
        // Seed two memories on DB-A, export them.
        let (state_a, _guard_a) = setup_test_state().await;
        let expires = Utc.with_ymd_and_hms(2030, 7, 1, 0, 0, 0).unwrap();
        add_memory_core(
            make_params(MemoryType::Knowledge, "round-trip knowledge"),
            &state_a.db,
            None,
        )
        .await
        .unwrap();
        add_memory_core(
            AddMemoryParams {
                memory_type: MemoryType::Context,
                content: "round-trip context".to_string(),
                metadata: json!({}),
                workflow_id: Some("wf-rt".to_string()),
                importance: 0.6,
                expires_at: Some(expires),
            },
            &state_a.db,
            None,
        )
        .await
        .unwrap();

        let exported = export_core(&state_a.db, ExportFormat::Json, None)
            .await
            .unwrap();

        // Fresh DB-B, import.
        let (state_b, _guard_b) = setup_test_state().await;
        let result = import_core(&state_b.db, None, &exported).await.unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.failed, 0);

        let row_k = fetch_by_content(&state_b.db, "round-trip knowledge").await;
        assert_eq!(
            row_k.get("type").and_then(|v| v.as_str()),
            Some("knowledge")
        );

        let row_c = fetch_by_content(&state_b.db, "round-trip context").await;
        assert_eq!(
            row_c.get("workflow_id").and_then(|v| v.as_str()),
            Some("wf-rt")
        );
        assert!(
            (row_c.get("importance").and_then(|v| v.as_f64()).unwrap() - 0.6).abs() < f64::EPSILON
        );
        let parsed_expires: DateTime<Utc> =
            DateTime::parse_from_rfc3339(row_c.get("expires_at").and_then(|v| v.as_str()).unwrap())
                .unwrap()
                .with_timezone(&Utc);
        assert_eq!(parsed_expires, expires);

        // Each imported memory should have ≥ 1 chunk.
        let kid = row_k.get("id").and_then(|v| v.as_str()).unwrap();
        let cid = row_c.get("id").and_then(|v| v.as_str()).unwrap();
        assert!(count_chunks(&state_b.db, kid).await >= 1);
        assert!(count_chunks(&state_b.db, cid).await >= 1);
    }
}
