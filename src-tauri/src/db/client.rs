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

use anyhow::Result;
use surrealdb::{
    engine::local::{Db, RocksDb},
    Surreal,
};
use tracing::{debug, error, info, instrument};

/// Database client for SurrealDB embedded operations
pub struct DBClient {
    pub db: Surreal<Db>,
}

impl DBClient {
    /// Creates a new database client and connects to the specified path
    #[instrument(name = "db_client_new", skip_all, fields(db_path = %path))]
    pub async fn new(path: &str) -> Result<Self> {
        info!("Initializing SurrealDB connection");

        let db = Surreal::new::<RocksDb>(path).await.map_err(|e| {
            error!(error = %e, "Failed to connect to SurrealDB");
            e
        })?;

        db.use_ns("zileo").use_db("chat").await.map_err(|e| {
            error!(error = %e, "Failed to select namespace/database");
            e
        })?;

        info!("SurrealDB connection established");
        Ok(Self { db })
    }

    /// Initializes the database schema, re-applying it only when it changed.
    ///
    /// Re-running `SCHEMA_SQL` rebuilds every index on populated tables, which
    /// costs seconds once the store holds real data (vector and B-tree indexes
    /// are rebuilt from scratch). To avoid paying that on every launch, a stable
    /// fingerprint of the schema text is stored in a small `schema_meta` record
    /// and compared on boot; an unchanged schema is skipped entirely. Editing
    /// `SCHEMA_SQL` changes the fingerprint and forces a one-time re-apply.
    ///
    /// # Returns
    /// `true` when the schema was (re)applied, `false` when it was already
    /// current and the apply was skipped.
    ///
    /// # Errors
    /// When reading the stored fingerprint or applying the schema fails.
    #[instrument(name = "db_initialize_schema", skip(self))]
    pub async fn initialize_schema(&self) -> Result<bool> {
        use super::schema::SCHEMA_SQL;

        let fingerprint = schema_fingerprint(SCHEMA_SQL);

        // `schema_meta` holds a single record remembering the last-applied
        // fingerprint. Selecting a named field (never `*`/`id`) sidesteps the
        // SDK's Thing-enum deserialization issues and is a cheap point lookup.
        let stored: Vec<SchemaMetaRow> = self
            .db
            .query("SELECT hash FROM schema_meta:current")
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to read stored schema fingerprint");
                e
            })?
            .take(0)
            .map_err(|e| {
                error!(error = %e, "Failed to deserialize stored schema fingerprint");
                e
            })?;

        if stored.first().map(|r| r.hash.as_str()) == Some(fingerprint.as_str()) {
            info!("Database schema already current; skipping re-apply");
            return Ok(false);
        }

        info!("Applying database schema (first run or definition changed)");
        let response = self.db.query(SCHEMA_SQL).await.map_err(|e| {
            error!(error = %e, "Failed to initialize schema");
            e
        })?;

        // SCHEMA_SQL is a single multi-statement query: a transport-level success
        // does not mean every DEFINE succeeded. Surface per-statement errors and
        // skip caching the fingerprint when any failed, so the schema is
        // re-applied (and re-logged) on the next boot instead of being frozen in
        // a half-applied state. Non-fatal: the app still boots best-effort.
        if let Err(e) = response.check() {
            error!(
                error = %e,
                "Schema apply reported a statement error; not caching the fingerprint so it re-applies next boot"
            );
            return Ok(true);
        }

        // MCP HTTP command-field migration is handled inside SCHEMA_SQL via
        // idempotent field redefinition; the guarded migration in
        // commands/migration.rs covers databases created before that change.

        // Cache the fingerprint only after a clean apply so the next boot can
        // skip the expensive re-apply.
        self.db
            .query("UPSERT schema_meta:current SET hash = $hash")
            .bind(("hash", fingerprint))
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to persist schema fingerprint");
                e
            })?;

        info!("Database schema initialized successfully");
        Ok(true)
    }

    /// Executes a query and returns the results as JSON Value first,
    /// then deserializes using serde_json for proper custom deserializer support.
    #[instrument(name = "db_query", skip(self), fields(query_len = query.len()))]
    pub async fn query<T>(&self, query: &str) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing query");

        let mut result = self.db.query(query).await.map_err(|e| {
            error!(error = %e, "Query execution failed");
            e
        })?;

        let data: Vec<T> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to deserialize query results");
            e
        })?;

        debug!(result_count = data.len(), "Query completed");
        Ok(data)
    }

    /// Executes a raw query and returns results as JSON Values.
    /// Use this when the standard query method fails due to SurrealDB SDK serialization issues.
    #[instrument(name = "db_query_json", skip(self), fields(query_len = query.len()))]
    pub async fn query_json(&self, query: &str) -> Result<Vec<serde_json::Value>> {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing JSON query");

        let mut result = self.db.query(query).await.map_err(|e| {
            error!(error = %e, "Query execution failed");
            e
        })?;

        let data: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to extract query results");
            e
        })?;

        debug!(result_count = data.len(), "Query completed");
        Ok(data)
    }

    /// Executes a query without deserializing the result.
    ///
    /// Use this for UPSERT, CREATE, UPDATE, DELETE operations where you don't need
    /// the returned data and want to avoid SurrealDB SDK serialization issues.
    #[instrument(name = "db_execute", skip(self), fields(query_len = query.len()))]
    pub async fn execute(&self, query: &str) -> Result<()> {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing query (no result)");

        self.db.query(query).await.map_err(|e| {
            error!(error = %e, "Query execution failed");
            e
        })?;

        debug!("Query executed successfully");
        Ok(())
    }

    /// Creates a new record in the specified table with a specific ID
    ///
    /// Uses a SurrealQL CREATE query with CONTENT to avoid SDK serialization issues.
    /// The data should NOT contain an `id` field (it's set via the record ID).
    ///
    /// NOTE: SurrealDB ASSERT constraints may silently reject records without error.
    /// This method verifies the record was actually created by checking the result.
    #[instrument(name = "db_create", skip(self, data), fields(table = %table, record_id = %id))]
    pub async fn create<T>(&self, table: &str, id: &str, data: T) -> Result<String>
    where
        T: serde::Serialize + Send + Sync + 'static,
    {
        debug!("Creating record");

        // Convert data to JSON Value first to avoid SDK serialization issues
        let json_data = serde_json::to_value(&data).map_err(|e| {
            error!(error = %e, "Failed to serialize data to JSON");
            anyhow::anyhow!("Serialization error: {}", e)
        })?;

        // Log the data being saved for debugging ASSERT constraint issues
        debug!(
            table = %table,
            record_id = %id,
            data = %json_data,
            "Attempting to create record"
        );

        // Use CREATE query with backtick-escaped ID for safety
        // Use RETURN meta::id(id) to get a string ID instead of Thing enum (SDK 2.x serialization issue)
        let query = format!(
            "CREATE {}:`{}` CONTENT $data RETURN meta::id(id) AS created_id",
            table, id
        );
        let mut result = self
            .db
            .query(&query)
            .bind(("data", json_data.clone()))
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create record");
                e
            })?;

        // Check if the record was actually created by examining the result
        // Using meta::id(id) returns a clean string instead of Thing enum
        let created: Option<serde_json::Value> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to get create result");
            anyhow::anyhow!("Failed to get create result: {}", e)
        })?;

        match created {
            Some(_) => {
                debug!(record_id = %id, "Record created successfully");
                Ok(id.to_string())
            }
            None => {
                // Record was not created - likely ASSERT constraint violation
                error!(
                    table = %table,
                    record_id = %id,
                    data = %json_data,
                    "Record was NOT created - possible ASSERT constraint violation"
                );
                Err(anyhow::anyhow!(
                    "Failed to create record in {}: record was silently rejected (check ASSERT constraints)",
                    table
                ))
            }
        }
    }

    /// Deletes a record by ID
    ///
    /// Accepts ID in format `table:uuid` (e.g., "workflow:123e4567-...")
    /// Uses raw DELETE query to avoid SurrealDB SDK 2.x serialization issues.
    #[instrument(name = "db_delete", skip(self), fields(record_id = %id))]
    pub async fn delete(&self, id: &str) -> Result<()> {
        debug!("Deleting record");

        // Parse table:uuid format
        let (table, uuid) = id.split_once(':').ok_or_else(|| {
            let msg = format!("Invalid record ID format '{}', expected 'table:uuid'", id);
            error!("{}", msg);
            anyhow::anyhow!(msg)
        })?;

        // Use raw DELETE query with backtick-escaped ID to avoid SDK issues
        let query = format!("DELETE {}:`{}`", table, uuid);
        self.db.query(&query).await.map_err(|e| {
            error!(error = %e, "Failed to delete record");
            e
        })?;

        debug!("Record deleted");
        Ok(())
    }

    /// Executes a parameterized query and returns results.
    ///
    /// Uses SurrealDB's `.bind()` method to safely bind parameters to the query.
    /// Parameters are passed as a vector of (name, value) tuples.
    ///
    /// # Arguments
    /// * `query` - The SurrealQL query with $param placeholders
    /// * `params` - Vector of (param_name, param_value) tuples
    ///
    /// # Example
    /// ```ignore
    /// let result = db.query_with_params(
    ///     "CREATE user CONTENT $data",
    ///     vec![("data".to_string(), json!({"name": "test"}))]
    /// ).await?;
    /// ```
    #[instrument(name = "db_query_with_params", skip(self, params), fields(query_len = query.len()))]
    pub async fn query_with_params<T>(
        &self,
        query: &str,
        params: Vec<(String, serde_json::Value)>,
    ) -> Result<Vec<T>>
    where
        T: serde::de::DeserializeOwned,
    {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing parameterized query");

        let mut query_builder = self.db.query(query);

        for (name, value) in params {
            query_builder = query_builder.bind((name, value));
        }

        let mut result = query_builder.await.map_err(|e| {
            error!(error = %e, "Parameterized query execution failed");
            e
        })?;

        let data: Vec<T> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to deserialize parameterized query results");
            e
        })?;

        debug!(result_count = data.len(), "Parameterized query completed");
        Ok(data)
    }

    /// Executes a parameterized query as JSON and returns results.
    ///
    /// Uses SurrealDB's `.bind()` method to safely bind parameters to the query.
    /// Returns results as JSON Value to avoid SDK serialization issues.
    ///
    /// # Arguments
    /// * `query` - The SurrealQL query with $param placeholders
    /// * `params` - Vector of (param_name, param_value) tuples
    ///
    /// # Example
    /// ```ignore
    /// let results = db.query_json_with_params(
    ///     "SELECT status FROM user_question:`$id`",
    ///     vec![("id".to_string(), json!("uuid-here"))]
    /// ).await?;
    /// ```
    #[instrument(name = "db_query_json_with_params", skip(self, params), fields(query_len = query.len()))]
    pub async fn query_json_with_params(
        &self,
        query: &str,
        params: Vec<(String, serde_json::Value)>,
    ) -> Result<Vec<serde_json::Value>> {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing parameterized JSON query");

        let mut query_builder = self.db.query(query);

        for (name, value) in params {
            query_builder = query_builder.bind((name, value));
        }

        let mut result = query_builder.await.map_err(|e| {
            error!(error = %e, "Parameterized query execution failed");
            e
        })?;

        let data: Vec<serde_json::Value> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to extract parameterized query results");
            e
        })?;

        debug!(
            result_count = data.len(),
            "Parameterized JSON query completed"
        );
        Ok(data)
    }

    /// Executes a parameterized mutation (INSERT/UPDATE/DELETE) without returning results.
    ///
    /// Uses SurrealDB's `.bind()` method to safely bind parameters to the query.
    /// Use this for write operations where you don't need the returned data.
    ///
    /// # Arguments
    /// * `query` - The SurrealQL mutation with $param placeholders
    /// * `params` - Vector of (param_name, param_value) tuples
    ///
    /// # Example
    /// ```ignore
    /// db.execute_with_params(
    ///     "DELETE FROM memory WHERE type = $type",
    ///     vec![("type".to_string(), json!("knowledge"))]
    /// ).await?;
    /// ```
    #[instrument(name = "db_execute_with_params", skip(self, params), fields(query_len = query.len()))]
    pub async fn execute_with_params(
        &self,
        query: &str,
        params: Vec<(String, serde_json::Value)>,
    ) -> Result<()> {
        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing parameterized mutation");

        let mut query_builder = self.db.query(query);

        for (name, value) in params {
            query_builder = query_builder.bind((name, value));
        }

        query_builder.await.map_err(|e| {
            error!(error = %e, "Parameterized mutation execution failed");
            e
        })?;

        debug!("Parameterized mutation executed successfully");
        Ok(())
    }
}

/// Row shape for reading the stored schema fingerprint from `schema_meta`.
#[derive(serde::Deserialize)]
struct SchemaMetaRow {
    hash: String,
}

/// Computes a stable fingerprint of the schema text.
///
/// Any edit to `SCHEMA_SQL` changes the fingerprint, which forces a one-time
/// re-apply on the next boot. The same input always yields the same output
/// within and across runs, so an unchanged schema is reliably detected.
///
/// Note: `DefaultHasher` is not guaranteed stable across Rust toolchains, so a
/// compiler upgrade may change the fingerprint and trigger one extra re-apply.
/// That is harmless — every schema statement is idempotent.
fn schema_fingerprint(sql: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    sql.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::test_tempdir;

    #[tokio::test]
    async fn test_db_client_new() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("test_db");
        let db_path_str = db_path.to_str().unwrap();

        let result = DBClient::new(db_path_str).await;
        assert!(result.is_ok(), "DBClient creation should succeed");
    }

    #[tokio::test]
    async fn test_db_client_invalid_path() {
        let result = DBClient::new("/nonexistent/path/that/cannot/be/created/db").await;
        assert!(result.is_err(), "Should fail with invalid path");
    }

    #[tokio::test]
    async fn test_db_initialize_schema() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("schema_test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = DBClient::new(db_path_str)
            .await
            .expect("DB creation failed");
        let result = db.initialize_schema().await;
        assert!(result.is_ok(), "Schema initialization should succeed");
    }

    #[test]
    fn schema_fingerprint_is_stable_and_sensitive() {
        let a = schema_fingerprint("DEFINE TABLE x SCHEMAFULL;");
        assert_eq!(
            a,
            schema_fingerprint("DEFINE TABLE x SCHEMAFULL;"),
            "same schema text must yield the same fingerprint"
        );
        assert_ne!(
            a,
            schema_fingerprint("DEFINE TABLE y SCHEMAFULL;"),
            "a changed schema must yield a different fingerprint"
        );
    }

    #[tokio::test]
    async fn schema_reapply_is_skipped_when_unchanged() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("schema_gate_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = DBClient::new(db_path_str)
            .await
            .expect("DB creation failed");

        // First boot applies the schema and records its fingerprint.
        let applied = db
            .initialize_schema()
            .await
            .expect("first schema init failed");
        assert!(applied, "first run should apply the schema");

        // Second boot with identical schema text must skip the expensive
        // re-apply (the whole point of the fingerprint gate).
        let applied_again = db
            .initialize_schema()
            .await
            .expect("second schema init failed");
        assert!(
            !applied_again,
            "an unchanged schema must be skipped on the next boot"
        );
    }

    #[tokio::test]
    async fn test_db_query_empty_result() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("query_test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = DBClient::new(db_path_str)
            .await
            .expect("DB creation failed");
        db.initialize_schema().await.expect("Schema init failed");

        let result: Vec<serde_json::Value> = db
            .query("SELECT * FROM workflow")
            .await
            .expect("Query failed");

        assert!(result.is_empty(), "Empty table should return empty result");
    }

    #[tokio::test]
    async fn test_db_info_query() {
        let temp_dir = test_tempdir();
        let db_path = temp_dir.path().join("info_test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = DBClient::new(db_path_str)
            .await
            .expect("DB creation failed");
        db.initialize_schema().await.expect("Schema init failed");

        // Test INFO FOR DB query which doesn't require serialization
        let result: Vec<serde_json::Value> = db.query("INFO FOR DB").await.expect("Query failed");

        // INFO query returns database info
        assert!(!result.is_empty(), "INFO query should return database info");
    }
}
