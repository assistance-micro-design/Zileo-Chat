// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Embedding configuration commands for Memory Tool settings.
//!
//! Provides Tauri commands for managing embedding settings and memory operations
//! from the Settings UI.

use crate::{
    commands::SecureKeyStore,
    llm::embedding::{EmbeddingProvider, EmbeddingService},
    models::{
        CategoryTokenStats, EmbeddingConfigSettings, EmbeddingTestResult, ExportFormat,
        ImportResult, Memory, MemoryStats, MemoryTokenStats, RegenerateResult,
    },
    AppState,
};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Storage key for embedding configuration in the database
const EMBEDDING_CONFIG_KEY: &str = "settings:embedding_config";

/// Gets the current embedding configuration.
///
/// Returns the stored configuration or default values if not configured.
#[tauri::command]
#[instrument(name = "get_embedding_config", skip(state))]
pub async fn get_embedding_config(
    state: State<'_, AppState>,
) -> Result<EmbeddingConfigSettings, String> {
    info!("Getting embedding configuration");

    // Try to load from database using direct record access
    // Note: Using backtick-escaped ID for direct access instead of WHERE clause
    // to ensure correct record ID matching in SurrealDB
    // Note: Using query_json and SELECT config (not SELECT *) to avoid
    // SurrealDB SDK 2.x serialization issues with Thing enum type in id field
    let query = format!("SELECT config FROM settings:`{}`", EMBEDDING_CONFIG_KEY);

    let results: Vec<serde_json::Value> = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to query embedding config");
        format!("Failed to load embedding config: {}", e)
    })?;

    if let Some(row) = results.first() {
        if let Some(config_value) = row.get("config") {
            if let Ok(config) =
                serde_json::from_value::<EmbeddingConfigSettings>(config_value.clone())
            {
                info!("Loaded embedding config from database");
                return Ok(config);
            }
        }
    }

    // Return default config if not stored
    info!("No stored config found, returning default");
    Ok(EmbeddingConfigSettings::default())
}

/// Saves the embedding configuration.
///
/// # Arguments
/// * `config` - The embedding configuration to save
#[tauri::command]
#[instrument(name = "save_embedding_config", skip(state, keystore, config))]
pub async fn save_embedding_config(
    config: EmbeddingConfigSettings,
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!(
        provider = %config.provider,
        model = %config.model,
        dimension = config.dimension,
        "Saving embedding configuration"
    );

    // Validate configuration
    if config.provider.is_empty() {
        return Err("Provider cannot be empty".to_string());
    }
    if config.model.is_empty() {
        return Err("Model cannot be empty".to_string());
    }
    if config.chunk_size < 100 || config.chunk_size > 10000 {
        return Err("Chunk size must be between 100 and 10000".to_string());
    }
    if config.chunk_overlap >= config.chunk_size {
        return Err("Chunk overlap must be less than chunk size".to_string());
    }

    // Serialize config to JSON string for embedding in query
    // Note: Using raw query instead of query_with_params due to SurrealDB SDK 2.x
    // serialization issues with complex types (see CLAUDE.md SurrealDB patterns)
    let config_json_str = serde_json::to_string(&config).map_err(|e| {
        error!(error = %e, "Failed to serialize embedding config");
        format!("Failed to serialize config: {}", e)
    })?;

    // Upsert configuration using raw query with embedded JSON
    // Use execute() to avoid SurrealDB SDK serialization issues with UPSERT return type
    let upsert_query = format!(
        "UPSERT settings:`{}` CONTENT {{ id: '{}', config: {} }}",
        EMBEDDING_CONFIG_KEY, EMBEDDING_CONFIG_KEY, config_json_str
    );

    state.db.execute(&upsert_query).await.map_err(|e| {
        error!(error = %e, "Failed to save embedding config");
        format!("Failed to save config: {}", e)
    })?;

    // Update the EmbeddingService in AppState
    // Note: For Mistral, the API key is retrieved from SecureKeyStore (OS keychain)
    update_embedding_service_internal(&config, &state, &keystore).await;

    info!("Embedding configuration saved successfully");
    Ok(())
}

/// Updates the EmbeddingService in AppState based on config.
/// Note: For Mistral, requires API key to be pre-configured in Provider settings (OS keychain).
async fn update_embedding_service_internal(
    config: &EmbeddingConfigSettings,
    state: &State<'_, AppState>,
    keystore: &State<'_, SecureKeyStore>,
) {
    // For Ollama, we can create the service directly (no API key needed)
    // For Mistral, we need to get the API key from SecureKeyStore (OS keychain)
    let provider = match config.provider.as_str() {
        "ollama" => Some(EmbeddingProvider::ollama_with_config(
            "http://localhost:11434",
            &config.model,
        )),
        "mistral" => {
            // Get API key from SecureKeyStore (same as test_provider_connection)
            if let Some(api_key) = keystore.get_key("Mistral") {
                Some(EmbeddingProvider::mistral_with_model(
                    &api_key,
                    &config.model,
                ))
            } else {
                warn!(
                    "Mistral API key not available - please configure in Provider settings first"
                );
                None
            }
        }
        _ => {
            warn!(provider = %config.provider, "Unknown embedding provider");
            None
        }
    };

    if let Some(provider) = provider {
        let service = EmbeddingService::with_provider(provider);
        let mut guard = state.embedding_service.write().await;
        *guard = Some(Arc::new(service));
        info!("Embedding service updated successfully");
    }
}

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

    let total = total_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    // Get count with embeddings
    let with_embeddings_query =
        "SELECT count() AS count FROM memory WHERE embedding != NONE GROUP ALL";
    let with_result: Vec<serde_json::Value> = state
        .db
        .query(with_embeddings_query)
        .await
        .unwrap_or_default();

    let with_embeddings = with_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

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

    // Validate memory ID format
    if memory_id.is_empty() {
        return Err("Memory ID cannot be empty".to_string());
    }

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
        // Use JSON string encoding to properly escape all special characters
        let content_json =
            serde_json::to_string(trimmed).map_err(|e| format!("Invalid content: {}", e))?;
        updates.push(format!("content = {}", content_json));
    }

    if let Some(ref m) = metadata {
        let meta_str = serde_json::to_string(m).map_err(|e| format!("Invalid metadata: {}", e))?;
        updates.push(format!("metadata = {}", meta_str));
    }

    if updates.is_empty() {
        return Err("No updates provided".to_string());
    }

    // Use execute() for UPDATE to avoid SurrealDB SDK serialization issues
    let update_query = format!("UPDATE memory:`{}` SET {}", memory_id, updates.join(", "));

    state.db.execute(&update_query).await.map_err(|e| {
        error!(error = %e, "Failed to update memory");
        format!("Failed to update memory: {}", e)
    })?;

    // Fetch the updated record with explicit field selection
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

    // Use explicit field selection with meta::id(id) to avoid SurrealDB SDK
    // serialization issues with internal Thing type (see CLAUDE.md)
    let query = match type_filter {
        Some(ref mtype) => format!(
            "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
             FROM memory WHERE type = '{}' ORDER BY created_at DESC",
            mtype
        ),
        None => "SELECT meta::id(id) AS id, type, content, workflow_id, metadata, created_at \
                 FROM memory ORDER BY created_at DESC"
            .to_string(),
    };

    let memories: Vec<Memory> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load memories for export");
        format!("Failed to export memories: {}", e)
    })?;

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

    // Parse JSON data
    let memories: Vec<serde_json::Value> = serde_json::from_str(&data).map_err(|e| {
        error!(error = %e, "Failed to parse import data");
        format!("Invalid JSON format: {}", e)
    })?;

    let mut imported = 0;
    let mut failed = 0;
    let mut errors = Vec::new();

    for (idx, mem) in memories.iter().enumerate() {
        // Extract required fields
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

        // Create memory
        let memory_id = uuid::Uuid::new_v4().to_string();
        let create_query = format!(
            "CREATE memory:`{}` CONTENT {{ type: '{}', content: '{}', metadata: {} }}",
            memory_id,
            memory_type,
            content.replace('\'', "''"),
            serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string())
        );

        match state.db.query::<serde_json::Value>(&create_query).await {
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

    // Get embedding service
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

    // Load memories (no ORDER BY needed for regeneration, just need id and content)
    let query = match type_filter {
        Some(ref mtype) => format!(
            "SELECT meta::id(id) AS id, content FROM memory WHERE type = '{}'",
            mtype
        ),
        None => "SELECT meta::id(id) AS id, content FROM memory".to_string(),
    };

    let memories: Vec<serde_json::Value> = state.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to load memories for regeneration");
        format!("Failed to load memories: {}", e)
    })?;

    let mut processed = 0;
    let mut success = 0;
    let mut failed = 0;

    for mem in &memories {
        processed += 1;

        let id = match mem.get("id").and_then(|i| i.as_str()) {
            Some(i) => i,
            None => {
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

        // Generate embedding
        match service.embed(content).await {
            Ok(embedding) => {
                // Update memory with new embedding using execute() to avoid serialization issues
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

/// Helper function to get config internally
async fn get_embedding_config_internal(
    state: &State<'_, AppState>,
) -> Result<EmbeddingConfigSettings, String> {
    let query = format!("SELECT config FROM settings:`{}`", EMBEDDING_CONFIG_KEY);

    let results: Vec<serde_json::Value> = state
        .db
        .query_json(&query)
        .await
        .map_err(|e| format!("Failed to query config: {}", e))?;

    if let Some(row) = results.first() {
        if let Some(config) = row.get("config") {
            return serde_json::from_value(config.clone())
                .map_err(|e| format!("Failed to parse config: {}", e));
        }
    }

    Ok(EmbeddingConfigSettings::default())
}

/// Reinitializes the embedding service with current config
#[tauri::command]
#[instrument(name = "reinit_embedding_service", skip(state, keystore))]
pub async fn reinit_embedding_service(
    state: State<'_, AppState>,
    keystore: State<'_, SecureKeyStore>,
) -> Result<(), String> {
    info!("Reinitializing embedding service");
    let config = get_embedding_config_internal(&state).await?;
    update_embedding_service_internal(&config, &state, &keystore).await;
    Ok(())
}

/// Tests embedding generation with current configuration
#[tauri::command]
#[instrument(name = "test_embedding", skip(state))]
pub async fn test_embedding(
    state: State<'_, AppState>,
    text: String,
) -> Result<EmbeddingTestResult, String> {
    info!(text_len = text.len(), "Testing embedding generation");

    // Validate input
    if text.is_empty() {
        return Err("Test text cannot be empty".to_string());
    }

    if text.len() > 10000 {
        return Err("Test text too long (max 10000 chars)".to_string());
    }

    // Get embedding service
    let embed_service = state.embedding_service.read().await;
    let service = embed_service.as_ref().ok_or_else(|| {
        "Embedding service not configured. Please save embedding settings first.".to_string()
    })?;

    let start = Instant::now();

    match service.embed(&text).await {
        Ok(embedding) => {
            let duration_ms = start.elapsed().as_millis() as u64;
            let dimension = embedding.len();
            let preview: Vec<f32> = embedding.iter().take(5).cloned().collect();

            // Get provider/model from config (since EmbeddingService doesn't expose it directly)
            let config = get_embedding_config_internal(&state).await;
            let (provider, model) = match config {
                Ok(c) => (c.provider, c.model),
                Err(_) => ("unknown".to_string(), "unknown".to_string()),
            };

            info!(
                dimension = dimension,
                duration_ms = duration_ms,
                "Embedding test successful"
            );

            Ok(EmbeddingTestResult {
                success: true,
                dimension,
                preview,
                duration_ms,
                provider,
                model,
                error: None,
            })
        }
        Err(e) => {
            let config = get_embedding_config_internal(&state).await;
            let (provider, model) = match config {
                Ok(c) => (c.provider, c.model),
                Err(_) => ("unknown".to_string(), "unknown".to_string()),
            };

            warn!(error = %e, "Embedding test failed");

            Ok(EmbeddingTestResult {
                success: false,
                dimension: 0,
                preview: vec![],
                duration_ms: start.elapsed().as_millis() as u64,
                provider,
                model,
                error: Some(e.to_string()),
            })
        }
    }
}

/// Gets token/character statistics per memory category
#[tauri::command]
#[instrument(name = "get_memory_token_stats", skip(state))]
pub async fn get_memory_token_stats(
    state: State<'_, AppState>,
    type_filter: Option<String>,
) -> Result<MemoryTokenStats, String> {
    info!(type_filter = ?type_filter, "Getting memory token statistics");

    // Build query with optional filter
    let query = if let Some(ref mem_type) = type_filter {
        format!(
            r#"SELECT
                type,
                count() AS count,
                math::sum(string::len(content)) AS total_chars,
                count(embedding != NONE) AS with_embeddings
            FROM memory
            WHERE type = '{}'
            GROUP BY type"#,
            mem_type
        )
    } else {
        r#"SELECT
            type,
            count() AS count,
            math::sum(string::len(content)) AS total_chars,
            count(embedding != NONE) AS with_embeddings
        FROM memory
        GROUP BY type"#
            .to_string()
    };

    let results: Vec<serde_json::Value> = state.db.query_json(&query).await.map_err(|e| {
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
            .unwrap_or("unknown")
            .to_string();

        let count = row.get("count").and_then(|c| c.as_u64()).unwrap_or(0) as usize;

        let chars = row.get("total_chars").and_then(|c| c.as_u64()).unwrap_or(0) as usize;

        let with_embeddings = row
            .get("with_embeddings")
            .and_then(|c| c.as_u64())
            .unwrap_or(0) as usize;

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_config_settings_default() {
        let config = EmbeddingConfigSettings::default();
        assert_eq!(config.provider, "mistral");
        assert_eq!(config.model, "mistral-embed");
        assert_eq!(config.dimension, 1024);
    }

    #[test]
    fn test_export_format_serialization() {
        let json = ExportFormat::Json;
        let csv = ExportFormat::Csv;

        assert_eq!(serde_json::to_string(&json).unwrap(), "\"json\"");
        assert_eq!(serde_json::to_string(&csv).unwrap(), "\"csv\"");
    }
}
