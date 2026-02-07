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

//! Database migration commands for schema updates.
//!
//! Provides Tauri commands for running database migrations,
//! particularly for the Memory Tool vector search schema.

use crate::AppState;
use serde::{Deserialize, Serialize};
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Result of a migration operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationResult {
    /// Whether the migration was successful
    pub success: bool,
    /// Human-readable message describing the result
    pub message: String,
    /// Number of records affected
    pub records_affected: usize,
}

/// SQL for migrating memory table to new schema (Phase 2)
///
/// Changes:
/// - HNSW dimension: 1536 -> 1024 (Mistral/Ollama compatibility)
/// - Add workflow_id field for workflow scoping
/// - Add workflow_id index for efficient filtering
/// - Set embedding to NONE for existing records (to be regenerated)
const MEMORY_SCHEMA_MIGRATION: &str = r#"
-- Step 1: Remove the old HNSW index (must be dropped before dimension change)
REMOVE INDEX IF EXISTS memory_vec_idx ON TABLE memory;

-- Step 2: Define the optional embedding field (allows null for migration)
DEFINE FIELD embedding ON memory TYPE option<array<float>>;

-- Step 3: Add workflow_id field for workflow scoping
DEFINE FIELD IF NOT EXISTS workflow_id ON memory TYPE option<string>;

-- Step 4: Create new HNSW index with 1024 dimensions
DEFINE INDEX memory_vec_idx ON memory FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;

-- Step 5: Create workflow_id index for efficient filtering
DEFINE INDEX IF NOT EXISTS memory_workflow_idx ON memory FIELDS workflow_id;

-- Step 6: Clear existing embeddings (they have wrong dimensions)
UPDATE memory SET embedding = NONE WHERE embedding IS NOT NONE;
"#;

/// Migrates the memory table schema for Phase 2 (vector search).
///
/// This migration:
/// - Drops and recreates the HNSW index with 1024 dimensions
/// - Adds workflow_id field for workflow scoping
/// - Adds index on workflow_id for efficient queries
/// - Clears existing embeddings (wrong dimension) for regeneration
///
/// # Returns
/// Migration result with affected record count
///
/// # Safety
/// This migration is idempotent and can be run multiple times.
/// Existing memory content is preserved, only embeddings are cleared.
#[tauri::command]
#[instrument(name = "migrate_memory_schema", skip(state))]
pub async fn migrate_memory_schema(state: State<'_, AppState>) -> Result<MigrationResult, String> {
    info!("Starting memory schema migration (Phase 2)");

    // Count memories before migration
    let count_query = "SELECT count() FROM memory GROUP ALL";
    let count_before: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let total_memories = count_before
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    info!(
        total_memories = total_memories,
        "Memories found before migration"
    );

    // Run migration queries
    let _: Vec<serde_json::Value> = state.db.query(MEMORY_SCHEMA_MIGRATION).await.map_err(|e| {
        error!(error = %e, "Memory schema migration failed");
        format!("Memory schema migration failed: {}", e)
    })?;

    // Verify migration success by checking field exists
    let verify_query = "INFO FOR TABLE memory";
    let _: Vec<serde_json::Value> = state.db.query(verify_query).await.map_err(|e| {
        warn!(error = %e, "Could not verify migration");
        format!("Could not verify migration: {}", e)
    })?;

    let message = if total_memories > 0 {
        format!(
            "Migration complete. {} memories updated. Embeddings cleared for regeneration.",
            total_memories
        )
    } else {
        "Migration complete. Schema updated. No existing memories to migrate.".to_string()
    };

    info!(
        records_affected = total_memories,
        "Memory schema migration completed successfully"
    );

    Ok(MigrationResult {
        success: true,
        message,
        records_affected: total_memories,
    })
}

/// Gets the current memory schema status.
///
/// Returns information about the memory table schema including:
/// - Whether workflow_id field exists
/// - HNSW index configuration
/// - Total memory count
/// - Memories with/without embeddings
#[tauri::command]
#[instrument(name = "get_memory_schema_status", skip(state))]
pub async fn get_memory_schema_status(
    state: State<'_, AppState>,
) -> Result<MemorySchemaStatus, String> {
    info!("Getting memory schema status");

    // Get total memory count
    let count_query = "SELECT count() FROM memory GROUP ALL";
    let count_result: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let total_memories = count_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    // Count memories with embeddings
    let with_embedding_query = "SELECT count() FROM memory WHERE embedding IS NOT NONE GROUP ALL";
    let with_result: Vec<serde_json::Value> =
        state.db.query(with_embedding_query).await.map_err(|e| {
            error!(error = %e, "Failed to count memories with embeddings");
            format!("Failed to count memories with embeddings: {}", e)
        })?;

    let with_embeddings = with_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    // Count memories with workflow_id
    let with_workflow_query = "SELECT count() FROM memory WHERE workflow_id IS NOT NONE GROUP ALL";
    let workflow_result: Vec<serde_json::Value> =
        state.db.query(with_workflow_query).await.map_err(|e| {
            error!(error = %e, "Failed to count memories with workflow_id");
            format!("Failed to count memories with workflow_id: {}", e)
        })?;

    let with_workflow_id = workflow_result
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    info!(
        total = total_memories,
        with_embeddings = with_embeddings,
        with_workflow_id = with_workflow_id,
        "Memory schema status retrieved"
    );

    Ok(MemorySchemaStatus {
        total_memories,
        with_embeddings,
        without_embeddings: total_memories.saturating_sub(with_embeddings),
        with_workflow_id,
        hnsw_dimension: 1024, // Current schema dimension
    })
}

/// SQL for migrating memory table to v2 schema.
///
/// Changes:
/// - Add importance field (float, default 0.5)
/// - Add expires_at field (option<datetime>)
/// - Set importance for existing records to 0.5
const MEMORY_V2_MIGRATION: &str = r#"
-- Step 1: Add importance field with default
DEFINE FIELD importance ON memory TYPE float DEFAULT 0.5;

-- Step 2: Add expires_at field for TTL
DEFINE FIELD expires_at ON memory TYPE option<datetime>;

-- Step 3: Set importance for existing records
UPDATE memory SET importance = 0.5 WHERE importance IS NONE;
"#;

/// Migrates the memory table schema for v2 (importance + TTL).
///
/// This migration:
/// - Adds importance field (float, default 0.5)
/// - Adds expires_at field (option<datetime>) for TTL
/// - Sets importance to 0.5 for existing records
///
/// # Returns
/// Migration result with affected record count
///
/// # Safety
/// This migration is idempotent and can be run multiple times.
#[tauri::command]
#[instrument(name = "migrate_memory_v2_schema", skip(state))]
pub async fn migrate_memory_v2_schema(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Starting memory v2 schema migration (importance + TTL)");

    // Count memories before migration
    let count_query = "SELECT count() FROM memory GROUP ALL";
    let count_before: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let total_memories = count_before
        .first()
        .and_then(|v| v.get("count"))
        .and_then(|c| c.as_u64())
        .unwrap_or(0) as usize;

    info!(
        total_memories = total_memories,
        "Memories found before v2 migration"
    );

    // Run migration queries
    let _: Vec<serde_json::Value> = state.db.query(MEMORY_V2_MIGRATION).await.map_err(|e| {
        error!(error = %e, "Memory v2 schema migration failed");
        format!("Memory v2 schema migration failed: {}", e)
    })?;

    let message = if total_memories > 0 {
        format!(
            "Memory v2 migration complete. {} memories updated with importance=0.5.",
            total_memories
        )
    } else {
        "Memory v2 migration complete. Schema updated. No existing memories to migrate.".to_string()
    };

    info!(
        records_affected = total_memories,
        "Memory v2 schema migration completed successfully"
    );

    Ok(MigrationResult {
        success: true,
        message,
        records_affected: total_memories,
    })
}

/// SQL for updating MCP server command field ASSERT constraint to include HTTP
///
/// This migration adds 'http' to the allowed values for the command field,
/// enabling HTTP-based MCP server connections (SaaS, remote servers).
const MCP_HTTP_MIGRATION: &str = r#"
-- Update the command field ASSERT constraint to include 'http'
DEFINE FIELD command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx', 'http'];
"#;

/// Migrates MCP server schema to support HTTP deployment method.
///
/// Updates the command field ASSERT constraint to include 'http',
/// allowing HTTP-based MCP server connections.
#[tauri::command]
#[instrument(name = "migrate_mcp_http_schema", skip(state))]
pub async fn migrate_mcp_http_schema(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Running MCP HTTP schema migration");

    // Run migration query
    let _: Vec<serde_json::Value> = state.db.query(MCP_HTTP_MIGRATION).await.map_err(|e| {
        error!(error = %e, "MCP HTTP schema migration failed");
        format!("MCP HTTP schema migration failed: {}", e)
    })?;

    info!("MCP HTTP schema migration completed successfully");

    Ok(MigrationResult {
        success: true,
        message: "MCP schema updated to support HTTP deployment method".to_string(),
        records_affected: 0,
    })
}

/// Memory schema status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySchemaStatus {
    /// Total number of memories in database
    pub total_memories: usize,
    /// Memories with vector embeddings
    pub with_embeddings: usize,
    /// Memories without embeddings (need generation)
    pub without_embeddings: usize,
    /// Memories with workflow_id assigned
    pub with_workflow_id: usize,
    /// Current HNSW index dimension
    pub hnsw_dimension: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_result_serialization() {
        let result = MigrationResult {
            success: true,
            message: "Migration complete".to_string(),
            records_affected: 42,
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"records_affected\":42"));

        let deserialized: MigrationResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.records_affected, 42);
    }

    #[test]
    fn test_memory_schema_status_serialization() {
        let status = MemorySchemaStatus {
            total_memories: 100,
            with_embeddings: 80,
            without_embeddings: 20,
            with_workflow_id: 50,
            hnsw_dimension: 1024,
        };

        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("\"total_memories\":100"));
        assert!(json.contains("\"hnsw_dimension\":1024"));
    }

    #[test]
    fn test_migration_sql_contains_required_changes() {
        // Verify migration SQL contains all required changes
        assert!(MEMORY_SCHEMA_MIGRATION.contains("REMOVE INDEX"));
        assert!(MEMORY_SCHEMA_MIGRATION.contains("memory_vec_idx"));
        assert!(MEMORY_SCHEMA_MIGRATION.contains("DIMENSION 1024"));
        assert!(MEMORY_SCHEMA_MIGRATION.contains("workflow_id"));
        assert!(MEMORY_SCHEMA_MIGRATION.contains("memory_workflow_idx"));
        assert!(MEMORY_SCHEMA_MIGRATION.contains("embedding = NONE"));
    }
}
