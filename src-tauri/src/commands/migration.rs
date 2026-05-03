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

use crate::db::{extract_count, DBClient};
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

/// Migration name constants
const MIGRATION_MEMORY_SCHEMA_V1: &str = "memory_schema_v1";
const MIGRATION_MEMORY_V2_SCHEMA: &str = "memory_v2_schema";
const MIGRATION_MCP_HTTP_SCHEMA: &str = "mcp_http_schema";
const MIGRATION_REASONING_EFFORT: &str = "reasoning_effort_v1";
const MIGRATION_SIDEBAR_FEATURES: &str = "sidebar_features_v1";
const MIGRATION_MCP_AUTH_V1: &str = "mcp_auth_v1";
const MIGRATION_TOKEN_COST_ACCURACY_V1: &str = "token_cost_accuracy_v1";

/// Checks if a migration has already been applied.
///
/// Queries the `migration_log` table for a record with the given name.
/// Returns `true` if the migration was already applied.
///
/// The migration name is bound as a parameter rather than interpolated, so
/// names containing SQL-special characters round-trip safely.
async fn check_migration_applied(db: &DBClient, migration_name: &str) -> Result<bool, String> {
    let results = db
        .query_json_with_params(
            "SELECT name FROM migration_log WHERE name = $name",
            vec![("name".to_string(), serde_json::json!(migration_name))],
        )
        .await
        .map_err(|e| format!("Failed to check migration status: {}", e))?;

    Ok(!results.is_empty())
}

/// Records a migration as applied in the `migration_log` table.
///
/// Creates a record with the migration name and current timestamp. The name
/// is bound as a parameter; `time::now()` is a SurrealQL function evaluated
/// server-side.
async fn record_migration_applied(db: &DBClient, migration_name: &str) -> Result<(), String> {
    db.execute_with_params(
        "CREATE migration_log SET name = $name, applied_at = time::now()",
        vec![("name".to_string(), serde_json::json!(migration_name))],
    )
    .await
    .map_err(|e| format!("Failed to record migration: {}", e))?;

    Ok(())
}

/// SQL for migrating memory table to new schema
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
DEFINE FIELD OVERWRITE embedding ON memory TYPE option<array<float>>;

-- Step 3: Add workflow_id field for workflow scoping
DEFINE FIELD IF NOT EXISTS workflow_id ON memory TYPE option<string>;

-- Step 4: Create new HNSW index with 1024 dimensions
DEFINE INDEX memory_vec_idx ON memory FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;

-- Step 5: Create workflow_id index for efficient filtering
DEFINE INDEX IF NOT EXISTS memory_workflow_idx ON memory FIELDS workflow_id;

-- Step 6: Clear existing embeddings (they have wrong dimensions)
UPDATE memory SET embedding = NONE WHERE embedding IS NOT NONE;
"#;

/// Migrates the memory table schema for vector search.
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
/// Guarded by migration_log to prevent re-execution.
/// First run clears embeddings; subsequent runs are no-ops.
#[tauri::command]
#[instrument(name = "migrate_memory_schema", skip(state))]
pub async fn migrate_memory_schema(state: State<'_, AppState>) -> Result<MigrationResult, String> {
    info!("Starting memory schema migration");

    if check_migration_applied(&state.db, MIGRATION_MEMORY_SCHEMA_V1).await? {
        info!("Memory schema migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: memory_schema_v1".to_string(),
            records_affected: 0,
        });
    }

    // Count memories before migration
    let count_query = "SELECT count() FROM memory GROUP ALL";
    let count_before: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let total_memories = extract_count(&count_before) as usize;

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

    // Record migration as applied
    record_migration_applied(&state.db, MIGRATION_MEMORY_SCHEMA_V1).await?;

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

    let total_memories = extract_count(&count_result) as usize;

    // Count memories with embeddings
    let with_embedding_query = "SELECT count() FROM memory WHERE embedding IS NOT NONE GROUP ALL";
    let with_result: Vec<serde_json::Value> =
        state.db.query(with_embedding_query).await.map_err(|e| {
            error!(error = %e, "Failed to count memories with embeddings");
            format!("Failed to count memories with embeddings: {}", e)
        })?;

    let with_embeddings = extract_count(&with_result) as usize;

    // Count memories with workflow_id
    let with_workflow_query = "SELECT count() FROM memory WHERE workflow_id IS NOT NONE GROUP ALL";
    let workflow_result: Vec<serde_json::Value> =
        state.db.query(with_workflow_query).await.map_err(|e| {
            error!(error = %e, "Failed to count memories with workflow_id");
            format!("Failed to count memories with workflow_id: {}", e)
        })?;

    let with_workflow_id = extract_count(&workflow_result) as usize;

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
DEFINE FIELD OVERWRITE importance ON memory TYPE float DEFAULT 0.5;

-- Step 2: Add expires_at field for TTL
DEFINE FIELD OVERWRITE expires_at ON memory TYPE option<datetime>;

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
/// Guarded by migration_log to prevent redundant re-execution.
#[tauri::command]
#[instrument(name = "migrate_memory_v2_schema", skip(state))]
pub async fn migrate_memory_v2_schema(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Starting memory v2 schema migration (importance + TTL)");

    // Check if migration was already applied
    if check_migration_applied(&state.db, MIGRATION_MEMORY_V2_SCHEMA).await? {
        info!("Memory v2 schema migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: memory_v2_schema".to_string(),
            records_affected: 0,
        });
    }

    // Count memories before migration
    let count_query = "SELECT count() FROM memory GROUP ALL";
    let count_before: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count memories");
        format!("Failed to count memories: {}", e)
    })?;

    let total_memories = extract_count(&count_before) as usize;

    info!(
        total_memories = total_memories,
        "Memories found before v2 migration"
    );

    // Run migration queries
    let _: Vec<serde_json::Value> = state.db.query(MEMORY_V2_MIGRATION).await.map_err(|e| {
        error!(error = %e, "Memory v2 schema migration failed");
        format!("Memory v2 schema migration failed: {}", e)
    })?;

    // Record migration as applied
    record_migration_applied(&state.db, MIGRATION_MEMORY_V2_SCHEMA).await?;

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
DEFINE FIELD OVERWRITE command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx', 'http'];
"#;

/// Migrates MCP server schema to support HTTP deployment method.
///
/// Updates the command field ASSERT constraint to include 'http',
/// allowing HTTP-based MCP server connections.
///
/// # Safety
/// Guarded by migration_log to prevent redundant re-execution.
#[tauri::command]
#[instrument(name = "migrate_mcp_http_schema", skip(state))]
pub async fn migrate_mcp_http_schema(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Running MCP HTTP schema migration");

    // Check if migration was already applied
    if check_migration_applied(&state.db, MIGRATION_MCP_HTTP_SCHEMA).await? {
        info!("MCP HTTP schema migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: mcp_http_schema".to_string(),
            records_affected: 0,
        });
    }

    // Run migration query
    let _: Vec<serde_json::Value> = state.db.query(MCP_HTTP_MIGRATION).await.map_err(|e| {
        error!(error = %e, "MCP HTTP schema migration failed");
        format!("MCP HTTP schema migration failed: {}", e)
    })?;

    // Record migration as applied
    record_migration_applied(&state.db, MIGRATION_MCP_HTTP_SCHEMA).await?;

    info!("MCP HTTP schema migration completed successfully");

    Ok(MigrationResult {
        success: true,
        message: "MCP schema updated to support HTTP deployment method".to_string(),
        records_affected: 0,
    })
}

/// SQL for migrating agent table from enable_thinking (bool) to reasoning_effort (enum string).
///
/// Changes:
/// - Converts enable_thinking=true to reasoning_effort='medium'
/// - Converts enable_thinking=false/NONE to reasoning_effort=NONE
/// - Removes the old enable_thinking field
/// - Defines new reasoning_effort field with ASSERT constraint
const REASONING_EFFORT_MIGRATION: &str = r#"
-- Step 1: Define reasoning_effort field
DEFINE FIELD OVERWRITE reasoning_effort ON agent TYPE option<string>
    ASSERT $value IS NONE OR $value IN ['low', 'medium', 'high']
    DEFAULT NONE;

-- Step 2: Convert enable_thinking=true to reasoning_effort='medium'
UPDATE agent SET reasoning_effort = 'medium' WHERE enable_thinking = true;

-- Step 3: Ensure false/NONE -> NONE
UPDATE agent SET reasoning_effort = NONE WHERE enable_thinking = false OR enable_thinking IS NONE;

-- Step 4: Remove old field
REMOVE FIELD IF EXISTS enable_thinking ON agent;
"#;

/// Migrates agent table from enable_thinking (bool) to reasoning_effort (enum).
///
/// Converts:
/// - `enable_thinking: true` -> `reasoning_effort: 'medium'`
/// - `enable_thinking: false/NONE` -> `reasoning_effort: NONE`
///
/// # Safety
/// Guarded by migration_log to prevent re-execution.
#[tauri::command]
#[instrument(name = "migrate_reasoning_effort", skip(state))]
pub async fn migrate_reasoning_effort(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Starting reasoning effort migration (enable_thinking -> reasoning_effort)");

    if check_migration_applied(&state.db, MIGRATION_REASONING_EFFORT).await? {
        info!("Reasoning effort migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: reasoning_effort_v1".to_string(),
            records_affected: 0,
        });
    }

    // Count agents before migration
    let count_query = "SELECT count() FROM agent GROUP ALL";
    let count_result: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count agents");
        format!("Failed to count agents: {}", e)
    })?;

    let total_agents = extract_count(&count_result) as usize;

    info!(total_agents = total_agents, "Agents found before migration");

    // Run migration
    let _: Vec<serde_json::Value> =
        state
            .db
            .query(REASONING_EFFORT_MIGRATION)
            .await
            .map_err(|e| {
                error!(error = %e, "Reasoning effort migration failed");
                format!("Reasoning effort migration failed: {}", e)
            })?;

    // Record migration
    record_migration_applied(&state.db, MIGRATION_REASONING_EFFORT).await?;

    let message = if total_agents > 0 {
        format!(
            "Reasoning effort migration complete. {} agents converted.",
            total_agents
        )
    } else {
        "Reasoning effort migration complete. No agents to convert.".to_string()
    };

    info!(
        records_affected = total_agents,
        "Reasoning effort migration completed successfully"
    );

    Ok(MigrationResult {
        success: true,
        message,
        records_affected: total_agents,
    })
}

/// SQL for adding HTTP authentication fields to mcp_server (v1.2).
///
/// All three fields are optional and stored as JSON strings (ERR_SURREAL_001).
/// Existing rows keep working: the values default to NONE so servers without
/// auth behave exactly as before.
const MCP_AUTH_V1_MIGRATION: &str = r#"
DEFINE FIELD OVERWRITE auth_type ON mcp_server TYPE option<string>
    ASSERT $value IS NONE OR $value IN ['none', 'bearer', 'apikey', 'basic'];
DEFINE FIELD OVERWRITE auth_metadata ON mcp_server TYPE option<string>;
DEFINE FIELD OVERWRITE extra_headers ON mcp_server TYPE option<string>;
"#;

/// Migrates the `mcp_server` table to support HTTP authentication (v1.2).
///
/// Adds three optional fields (`auth_type`, `auth_metadata`, `extra_headers`)
/// stored as JSON strings. Existing rows are unaffected — they keep the
/// no-auth behavior. Secrets remain in the OS keychain, never in DB.
///
/// # Safety
/// Guarded by migration_log to prevent redundant re-execution.
/// All fields use OVERWRITE for idempotency (ERR_SURREAL_011).
#[tauri::command]
#[instrument(name = "migrate_mcp_auth_v1", skip(state))]
pub async fn migrate_mcp_auth_v1(state: State<'_, AppState>) -> Result<MigrationResult, String> {
    info!("Running MCP auth v1 schema migration");

    if check_migration_applied(&state.db, MIGRATION_MCP_AUTH_V1).await? {
        info!("MCP auth v1 migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: mcp_auth_v1".to_string(),
            records_affected: 0,
        });
    }

    let _: Vec<serde_json::Value> = state.db.query(MCP_AUTH_V1_MIGRATION).await.map_err(|e| {
        error!(error = %e, "MCP auth v1 migration failed");
        format!("MCP auth v1 migration failed: {}", e)
    })?;

    record_migration_applied(&state.db, MIGRATION_MCP_AUTH_V1).await?;

    info!("MCP auth v1 migration completed successfully");

    Ok(MigrationResult {
        success: true,
        message:
            "MCP schema extended with HTTP auth fields (auth_type, auth_metadata, extra_headers)"
                .to_string(),
        records_affected: 0,
    })
}

/// SQL for migrating workflow table to support folders and pinning.
///
/// Changes:
/// - Creates workflow_folder table
/// - Adds folder_id field on workflow (optional)
/// - Adds pinned field on workflow (default false)
const SIDEBAR_FEATURES_MIGRATION: &str = r#"
-- Step 1: Create workflow_folder table
DEFINE TABLE OVERWRITE workflow_folder SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON workflow_folder TYPE string;
DEFINE FIELD OVERWRITE name ON workflow_folder TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE color ON workflow_folder TYPE string
    ASSERT $value = /^#[0-9a-fA-F]{6}$/;
DEFINE FIELD OVERWRITE sort_order ON workflow_folder TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE created_at ON workflow_folder TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON workflow_folder TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE unique_folder_id ON workflow_folder FIELDS id UNIQUE;

-- Step 2: Add folder_id field on workflow
DEFINE FIELD OVERWRITE folder_id ON workflow TYPE option<string>;

-- Step 3: Add pinned field on workflow
DEFINE FIELD OVERWRITE pinned ON workflow TYPE bool DEFAULT false;

-- Step 4: Backfill pinned on existing workflows (DEFAULT only applies to new records)
UPDATE workflow SET pinned = false WHERE pinned IS NONE;
"#;

/// Migrates database schema to support sidebar folder and pinning features.
///
/// This migration:
/// - Creates workflow_folder table for organizing workflows
/// - Adds folder_id field on workflow (optional, links to folder)
/// - Adds pinned field on workflow (boolean, default false)
///
/// # Safety
/// Guarded by migration_log to prevent re-execution.
/// All fields use OVERWRITE for idempotency.
#[tauri::command]
#[instrument(name = "migrate_sidebar_features", skip(state))]
pub async fn migrate_sidebar_features(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    info!("Starting sidebar features migration (folders + pinning)");

    if check_migration_applied(&state.db, MIGRATION_SIDEBAR_FEATURES).await? {
        info!("Sidebar features migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: "Already applied: sidebar_features_v1".to_string(),
            records_affected: 0,
        });
    }

    // Run migration queries
    let _: Vec<serde_json::Value> =
        state
            .db
            .query(SIDEBAR_FEATURES_MIGRATION)
            .await
            .map_err(|e| {
                error!(error = %e, "Sidebar features migration failed");
                format!("Sidebar features migration failed: {}", e)
            })?;

    // Record migration as applied
    record_migration_applied(&state.db, MIGRATION_SIDEBAR_FEATURES).await?;

    info!("Sidebar features migration completed successfully");

    Ok(MigrationResult {
        success: true,
        message: "Sidebar features migration complete. workflow_folder table created, folder_id and pinned fields added to workflow.".to_string(),
        records_affected: 0,
    })
}

/// SQL for the token-cost-accuracy v1 data backfill.
///
/// Schema for the new fields lives in `SCHEMA_SQL` (DEFINE FIELD OVERWRITE),
/// so on a fresh DB this migration is a no-op. On an existing DB whose
/// `workflow` rows predate the refactor, the new TYPE float field
/// `sub_agent_cost_usd` was never written and thus reads as NONE despite the
/// DEFAULT clause (DEFAULT only applies on CREATE). This UPDATE materializes
/// it so frontends and aggregations can rely on `(... ?? 0.0)` consistently.
///
/// `total_cached_tokens` and `total_cache_write_tokens` follow the same logic
/// for the cache columns added during this refactor.
const TOKEN_COST_ACCURACY_V1_MIGRATION: &str = r#"
-- Backfill sub-agent cost on legacy workflow rows. The token-cost-accuracy
-- refactor added this column with DEFAULT 0.0, but DEFAULT only applies on
-- CREATE so existing rows are still missing the field. This makes them
-- readable as `0.0` rather than NONE.
UPDATE workflow SET sub_agent_cost_usd = 0.0 WHERE sub_agent_cost_usd IS NONE;

-- Same logic for the cache totals introduced earlier in the same refactor.
UPDATE workflow SET total_cached_tokens = 0 WHERE total_cached_tokens IS NONE;
UPDATE workflow SET total_cache_write_tokens = 0 WHERE total_cache_write_tokens IS NONE;
"#;

/// Backfills new `workflow` columns added by the token-cost-accuracy refactor.
///
/// Called automatically once at startup from `AppState::new` so users never
/// need to invoke it manually; gated by `migration_log` for idempotency.
/// Also exposed as a Tauri command for completeness / power-user re-runs.
///
/// On a fresh database, completes in microseconds (zero rows touched).
#[tauri::command]
#[instrument(name = "migrate_token_cost_accuracy_v1", skip(state))]
pub async fn migrate_token_cost_accuracy_v1(
    state: State<'_, AppState>,
) -> Result<MigrationResult, String> {
    run_token_cost_accuracy_v1(&state.db).await
}

/// Internal entrypoint for `migrate_token_cost_accuracy_v1`, callable without
/// a Tauri `State` handle (used from `AppState::new` at startup).
pub async fn run_token_cost_accuracy_v1(db: &DBClient) -> Result<MigrationResult, String> {
    if check_migration_applied(db, MIGRATION_TOKEN_COST_ACCURACY_V1).await? {
        info!("Token cost accuracy migration already applied, skipping");
        return Ok(MigrationResult {
            success: true,
            message: format!("Already applied: {}", MIGRATION_TOKEN_COST_ACCURACY_V1),
            records_affected: 0,
        });
    }

    let _: Vec<serde_json::Value> =
        db.query(TOKEN_COST_ACCURACY_V1_MIGRATION)
            .await
            .map_err(|e| {
                error!(error = %e, "Token cost accuracy migration failed");
                format!("Token cost accuracy migration failed: {}", e)
            })?;

    record_migration_applied(db, MIGRATION_TOKEN_COST_ACCURACY_V1).await?;

    info!("Token cost accuracy migration completed successfully");

    Ok(MigrationResult {
        success: true,
        message: "Backfilled sub_agent_cost_usd and cache token totals on legacy workflow rows."
            .to_string(),
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
    use crate::test_utils::{seed_test_memory, seed_test_memory_with_embedding, setup_test_state};

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

    #[tokio::test]
    async fn test_check_migration_not_applied() {
        let (state, _db_guard) = setup_test_state().await;
        let applied = check_migration_applied(&state.db, "nonexistent_migration")
            .await
            .unwrap();
        assert!(!applied, "Migration should not be marked as applied");
    }

    #[tokio::test]
    async fn test_record_and_check_migration() {
        let (state, _db_guard) = setup_test_state().await;

        // Record migration
        record_migration_applied(&state.db, "test_migration_v1")
            .await
            .unwrap();

        // Check it's now applied
        let applied = check_migration_applied(&state.db, "test_migration_v1")
            .await
            .unwrap();
        assert!(applied, "Migration should be marked as applied");
    }

    #[tokio::test]
    async fn check_and_record_migration_handle_sql_special_chars() {
        // Locks in the parameterised-binding contract: a migration name
        // containing characters that would break a `format!("'{}'", name)`
        // SQL literal must round-trip cleanly. A future regression to string
        // interpolation would either fail the CREATE (syntax error) or skip
        // the SELECT match — both observable here.
        let (state, _db_guard) = setup_test_state().await;
        let weird_name = "weird'name_v1";

        let applied_before = check_migration_applied(&state.db, weird_name)
            .await
            .unwrap();
        assert!(!applied_before);

        record_migration_applied(&state.db, weird_name)
            .await
            .unwrap();

        let applied_after = check_migration_applied(&state.db, weird_name)
            .await
            .unwrap();
        assert!(
            applied_after,
            "Migration name with apostrophe must round-trip via parameterised binding"
        );
    }

    #[tokio::test]
    async fn test_check_migration_does_not_cross_contaminate() {
        let (state, _db_guard) = setup_test_state().await;

        record_migration_applied(&state.db, "migration_a")
            .await
            .unwrap();

        let applied_a = check_migration_applied(&state.db, "migration_a")
            .await
            .unwrap();
        let applied_b = check_migration_applied(&state.db, "migration_b")
            .await
            .unwrap();

        assert!(applied_a, "migration_a should be applied");
        assert!(!applied_b, "migration_b should NOT be applied");
    }

    #[tokio::test]
    async fn test_memory_migration_first_run_clears_embeddings() {
        let (state, _db_guard) = setup_test_state().await;

        // Seed a memory with an embedding
        seed_test_memory_with_embedding(&state.db).await;

        // Verify embedding exists before migration
        let before: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT embedding FROM memory WHERE embedding IS NOT NONE")
            .await
            .unwrap();
        assert!(
            !before.is_empty(),
            "Should have a memory with embedding before migration"
        );

        // Run the migration SQL directly (bypassing Tauri State)
        let _: Vec<serde_json::Value> = state.db.query(MEMORY_SCHEMA_MIGRATION).await.unwrap();
        record_migration_applied(&state.db, MIGRATION_MEMORY_SCHEMA_V1)
            .await
            .unwrap();

        // Verify embeddings are cleared
        let after: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT embedding FROM memory WHERE embedding IS NOT NONE")
            .await
            .unwrap();
        assert!(
            after.is_empty(),
            "Embeddings should be cleared after first migration"
        );
    }

    #[tokio::test]
    async fn test_memory_migration_second_run_preserves_embeddings() {
        let (state, _db_guard) = setup_test_state().await;

        // First migration: seed, run, record
        seed_test_memory(&state.db).await;
        let _: Vec<serde_json::Value> = state.db.query(MEMORY_SCHEMA_MIGRATION).await.unwrap();
        record_migration_applied(&state.db, MIGRATION_MEMORY_SCHEMA_V1)
            .await
            .unwrap();

        // Now seed a NEW memory with embedding (simulating regeneration)
        seed_test_memory_with_embedding(&state.db).await;

        // Verify embedding exists
        let before: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT embedding FROM memory WHERE embedding IS NOT NONE")
            .await
            .unwrap();
        assert!(
            !before.is_empty(),
            "Should have a memory with embedding after regeneration"
        );

        // Second migration attempt: guard should prevent it
        let already_applied = check_migration_applied(&state.db, MIGRATION_MEMORY_SCHEMA_V1)
            .await
            .unwrap();
        assert!(already_applied, "Migration should be marked as applied");

        // Embeddings should still be intact
        let after: Vec<serde_json::Value> = state
            .db
            .query_json("SELECT embedding FROM memory WHERE embedding IS NOT NONE")
            .await
            .unwrap();
        assert!(
            !after.is_empty(),
            "Embeddings should survive when migration guard prevents re-run"
        );
    }

    #[tokio::test]
    async fn test_memory_v2_migration_guard() {
        let (state, _db_guard) = setup_test_state().await;

        // First run: should not be applied yet
        let applied = check_migration_applied(&state.db, MIGRATION_MEMORY_V2_SCHEMA)
            .await
            .unwrap();
        assert!(!applied);

        // Run the migration
        let _: Vec<serde_json::Value> = state.db.query(MEMORY_V2_MIGRATION).await.unwrap();
        record_migration_applied(&state.db, MIGRATION_MEMORY_V2_SCHEMA)
            .await
            .unwrap();

        // Second run: should be applied
        let applied = check_migration_applied(&state.db, MIGRATION_MEMORY_V2_SCHEMA)
            .await
            .unwrap();
        assert!(applied, "Memory v2 migration should be marked as applied");
    }

    #[tokio::test]
    async fn test_mcp_auth_v1_migration_guard() {
        let (state, _db_guard) = setup_test_state().await;

        // First run: not applied yet
        let applied = check_migration_applied(&state.db, MIGRATION_MCP_AUTH_V1)
            .await
            .unwrap();
        assert!(!applied);

        // Run the migration SQL directly and record it
        let _: Vec<serde_json::Value> = state.db.query(MCP_AUTH_V1_MIGRATION).await.unwrap();
        record_migration_applied(&state.db, MIGRATION_MCP_AUTH_V1)
            .await
            .unwrap();

        // Second run: should be applied
        let applied = check_migration_applied(&state.db, MIGRATION_MCP_AUTH_V1)
            .await
            .unwrap();
        assert!(applied, "MCP auth v1 migration should be marked as applied");
    }

    #[test]
    fn test_mcp_auth_v1_migration_sql_contains_required_fields() {
        assert!(MCP_AUTH_V1_MIGRATION.contains("auth_type"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("auth_metadata"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("extra_headers"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("DEFINE FIELD OVERWRITE"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("'bearer'"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("'apikey'"));
        assert!(MCP_AUTH_V1_MIGRATION.contains("'basic'"));
    }

    #[tokio::test]
    async fn test_mcp_http_migration_guard() {
        let (state, _db_guard) = setup_test_state().await;

        // First run: should not be applied yet
        let applied = check_migration_applied(&state.db, MIGRATION_MCP_HTTP_SCHEMA)
            .await
            .unwrap();
        assert!(!applied);

        // Run the migration
        let _: Vec<serde_json::Value> = state.db.query(MCP_HTTP_MIGRATION).await.unwrap();
        record_migration_applied(&state.db, MIGRATION_MCP_HTTP_SCHEMA)
            .await
            .unwrap();

        // Second run: should be applied
        let applied = check_migration_applied(&state.db, MIGRATION_MCP_HTTP_SCHEMA)
            .await
            .unwrap();
        assert!(applied, "MCP HTTP migration should be marked as applied");
    }

    // -------------------------------------------------------------------------
    // token_cost_accuracy_v1 — backfill migration tests
    // -------------------------------------------------------------------------

    /// Inserts a workflow row that LACKS the new columns by writing only the
    /// minimum fields required by the schema. Used to simulate legacy DB rows
    /// that predate the token-cost-accuracy refactor. Returns the workflow id.
    async fn seed_legacy_workflow_row(db: &DBClient) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let query = format!(
            "CREATE workflow:`{id}` SET \
                name = 'Legacy WF', \
                agent_id = 'legacy-agent', \
                status = 'completed', \
                pinned = false"
        );
        db.db
            .query(&query)
            .await
            .expect("Query execution failed")
            .check()
            .expect("CREATE workflow failed validation");
        id
    }

    #[tokio::test]
    async fn token_cost_accuracy_v1_backfills_legacy_workflow_rows() {
        let (state, _db_guard) = setup_test_state().await;
        let workflow_id = seed_legacy_workflow_row(&state.db).await;

        // The schema's DEFAULT 0.0 only fires on CREATE. Whether the freshly
        // initialized schema in the test backfills the row is implementation
        // dependent; what we care about is the migration's POST-state.
        let result = run_token_cost_accuracy_v1(&state.db).await.unwrap();
        assert!(result.success, "Migration must succeed");

        let rows = state
            .db
            .query_json(&format!(
                "SELECT \
                    sub_agent_cost_usd, total_cached_tokens, total_cache_write_tokens \
                 FROM workflow:`{}`",
                workflow_id
            ))
            .await
            .unwrap();
        let row = rows.into_iter().next().expect("Workflow row missing");

        assert_eq!(
            row.get("sub_agent_cost_usd").and_then(|v| v.as_f64()),
            Some(0.0),
            "sub_agent_cost_usd must be materialized as 0.0 (was NONE on legacy row)"
        );
        assert_eq!(
            row.get("total_cached_tokens").and_then(|v| v.as_i64()),
            Some(0),
            "total_cached_tokens must be materialized as 0"
        );
        assert_eq!(
            row.get("total_cache_write_tokens").and_then(|v| v.as_i64()),
            Some(0),
            "total_cache_write_tokens must be materialized as 0"
        );
    }

    #[tokio::test]
    async fn token_cost_accuracy_v1_is_idempotent() {
        let (state, _db_guard) = setup_test_state().await;
        let _ = seed_legacy_workflow_row(&state.db).await;

        let first = run_token_cost_accuracy_v1(&state.db).await.unwrap();
        let second = run_token_cost_accuracy_v1(&state.db).await.unwrap();

        assert!(first.success);
        assert!(second.success);
        assert!(
            second.message.contains("Already applied"),
            "Second run must short-circuit via migration_log, got: {}",
            second.message
        );
    }

    #[tokio::test]
    async fn token_cost_accuracy_v1_does_not_overwrite_existing_values() {
        let (state, _db_guard) = setup_test_state().await;

        // Workflow already has non-zero values written. Migration must NOT
        // reset them to 0 — it only touches rows where the field IS NONE.
        let workflow_id = uuid::Uuid::new_v4().to_string();
        let query = format!(
            "CREATE workflow:`{id}` SET \
                name = 'Has Cost', \
                agent_id = 'a', \
                status = 'completed', \
                pinned = false, \
                sub_agent_cost_usd = 1.23, \
                total_cached_tokens = 500, \
                total_cache_write_tokens = 100",
            id = workflow_id
        );
        state.db.db.query(&query).await.unwrap().check().unwrap();

        run_token_cost_accuracy_v1(&state.db).await.unwrap();

        let rows = state
            .db
            .query_json(&format!(
                "SELECT sub_agent_cost_usd, total_cached_tokens, total_cache_write_tokens \
                 FROM workflow:`{}`",
                workflow_id
            ))
            .await
            .unwrap();
        let row = rows.into_iter().next().unwrap();
        assert_eq!(
            row.get("sub_agent_cost_usd").and_then(|v| v.as_f64()),
            Some(1.23),
            "Existing value must be preserved"
        );
        assert_eq!(
            row.get("total_cached_tokens").and_then(|v| v.as_i64()),
            Some(500)
        );
        assert_eq!(
            row.get("total_cache_write_tokens").and_then(|v| v.as_i64()),
            Some(100)
        );
    }

    #[test]
    fn token_cost_accuracy_v1_sql_targets_only_none_rows() {
        // Defence: a future edit that drops the WHERE clause would silently
        // wipe production cost data. Lock the SQL contract via assertions.
        assert!(
            TOKEN_COST_ACCURACY_V1_MIGRATION.contains("WHERE sub_agent_cost_usd IS NONE"),
            "sub_agent_cost_usd UPDATE must guard with IS NONE"
        );
        assert!(
            TOKEN_COST_ACCURACY_V1_MIGRATION.contains("WHERE total_cached_tokens IS NONE"),
            "total_cached_tokens UPDATE must guard with IS NONE"
        );
        assert!(
            TOKEN_COST_ACCURACY_V1_MIGRATION.contains("WHERE total_cache_write_tokens IS NONE"),
            "total_cache_write_tokens UPDATE must guard with IS NONE"
        );
    }
}
