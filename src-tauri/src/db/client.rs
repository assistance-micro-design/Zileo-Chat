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
use tracing::{debug, error, info, instrument, warn};

// =========================================================================
// OPT-DB-10: Query Statistics (Monitoring/Diagnostic)
// =========================================================================

/// Statistics returned from a query execution.
///
/// Used for monitoring and performance analysis.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct QueryStats {
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Number of rows returned
    pub row_count: usize,
}

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

    /// Initializes the database schema
    #[instrument(name = "db_initialize_schema", skip(self))]
    pub async fn initialize_schema(&self) -> Result<()> {
        use super::schema::SCHEMA_SQL;

        info!("Initializing database schema");

        self.db.query(SCHEMA_SQL).await.map_err(|e| {
            error!(error = %e, "Failed to initialize schema");
            e
        })?;

        // Run MCP HTTP migration to ensure command field supports 'http' value
        // Must use REMOVE FIELD + DEFINE FIELD to force ASSERT constraint update
        // (SurrealDB does not update ASSERT constraints on existing fields with just DEFINE)
        let mcp_http_migration = r#"
            REMOVE FIELD IF EXISTS command ON TABLE mcp_server;
            DEFINE FIELD command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx', 'http'];
        "#;
        self.db.query(mcp_http_migration).await.map_err(|e| {
            warn!(error = %e, "MCP HTTP migration query failed (may be expected if table doesn't exist yet)");
            e
        })?;

        info!("Database schema initialized successfully");
        Ok(())
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

    /// Updates a record by ID (prepared for future phases)
    #[allow(dead_code)]
    #[instrument(name = "db_update", skip(self, data), fields(record_id = %id))]
    pub async fn update<T>(&self, id: &str, data: T) -> Result<()>
    where
        T: serde::Serialize + Send + Sync + 'static,
    {
        debug!("Updating record");

        let _: Vec<serde_json::Value> = self.db.update(id).content(data).await.map_err(|e| {
            error!(error = %e, "Failed to update record");
            e
        })?;

        debug!("Record updated");
        Ok(())
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
    #[allow(dead_code)] // May be used by future tools
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

    /// Executes a series of operations within a database transaction.
    ///
    /// If any operation fails, the transaction is rolled back (CANCEL TRANSACTION).
    /// If all operations succeed, the transaction is committed (COMMIT TRANSACTION).
    ///
    /// # Arguments
    /// * `queries` - Vector of SurrealQL queries to execute within the transaction
    ///
    /// # Example
    /// ```ignore
    /// db.transaction(vec![
    ///     "CREATE workflow:`123` CONTENT { name: 'Test' }".to_string(),
    ///     "CREATE message:`456` CONTENT { workflow_id: '123', content: 'Hello' }".to_string(),
    /// ]).await?;
    /// ```
    ///
    /// # Note
    /// For complex transactions with bind parameters, use `transaction_with_params`.
    #[allow(dead_code)] // Prepared for future use in complex multi-table operations
    #[instrument(name = "db_transaction", skip(self, queries), fields(query_count = queries.len()))]
    pub async fn transaction(&self, queries: Vec<String>) -> Result<()> {
        debug!("Starting transaction with {} queries", queries.len());

        // Begin transaction
        if let Err(e) = self.db.query("BEGIN TRANSACTION").await {
            error!(error = %e, "Failed to begin transaction");
            return Err(e.into());
        }

        // Execute all queries
        for (i, query) in queries.iter().enumerate() {
            debug!(query_index = i, "Executing transaction query");
            if let Err(e) = self.db.query(query).await {
                error!(error = %e, query_index = i, "Transaction query failed, rolling back");
                // Attempt to cancel the transaction
                if let Err(cancel_err) = self.db.query("CANCEL TRANSACTION").await {
                    warn!(error = %cancel_err, "Failed to cancel transaction after error");
                }
                return Err(e.into());
            }
        }

        // Commit transaction
        if let Err(e) = self.db.query("COMMIT TRANSACTION").await {
            error!(error = %e, "Failed to commit transaction");
            // Attempt to cancel on commit failure
            if let Err(cancel_err) = self.db.query("CANCEL TRANSACTION").await {
                warn!(error = %cancel_err, "Failed to cancel transaction after commit failure");
            }
            return Err(e.into());
        }

        info!("Transaction committed successfully");
        Ok(())
    }

    // =========================================================================
    // OPT-DB-10: Query with Stats (Monitoring/Diagnostic)
    // =========================================================================

    /// Executes a query with timing statistics for monitoring.
    ///
    /// Returns both the results and execution statistics.
    /// Useful for performance profiling and query optimization analysis.
    ///
    /// # Arguments
    /// * `query` - The SurrealQL query to execute
    ///
    /// # Example
    /// ```ignore
    /// let (results, stats) = db.query_with_stats::<Workflow>("SELECT * FROM workflow").await?;
    /// println!("Query returned {} rows in {}ms", stats.row_count, stats.execution_time_ms);
    /// ```
    #[allow(dead_code)] // Prepared for monitoring/diagnostic use
    #[instrument(name = "db_query_with_stats", skip(self), fields(query_len = query.len()))]
    pub async fn query_with_stats<T>(&self, query: &str) -> Result<(Vec<T>, QueryStats)>
    where
        T: serde::de::DeserializeOwned,
    {
        use std::time::Instant;

        debug!(query_preview = %query.chars().take(100).collect::<String>(), "Executing query with stats");

        let start = Instant::now();

        let mut result = self.db.query(query).await.map_err(|e| {
            error!(error = %e, "Query execution failed");
            e
        })?;

        let data: Vec<T> = result.take(0).map_err(|e| {
            error!(error = %e, "Failed to deserialize query results");
            e
        })?;

        let elapsed = start.elapsed();
        let stats = QueryStats {
            execution_time_ms: elapsed.as_millis() as u64,
            row_count: data.len(),
        };

        debug!(
            result_count = stats.row_count,
            execution_time_ms = stats.execution_time_ms,
            "Query with stats completed"
        );
        Ok((data, stats))
    }

    // =========================================================================
    // Transaction Support
    // =========================================================================

    /// Executes a series of parameterized operations within a database transaction.
    ///
    /// Each operation is a tuple of (query, params) allowing bind parameters.
    /// If any operation fails, the transaction is rolled back.
    ///
    /// # Arguments
    /// * `operations` - Vector of (query, params) tuples
    ///
    /// # Example
    /// ```ignore
    /// db.transaction_with_params(vec![
    ///     (
    ///         "CREATE workflow:`123` CONTENT $data".to_string(),
    ///         vec![("data".to_string(), json!({"name": "Test"}))]
    ///     ),
    ///     (
    ///         "CREATE message:`456` CONTENT $data".to_string(),
    ///         vec![("data".to_string(), json!({"workflow_id": "123"}))]
    ///     ),
    /// ]).await?;
    /// ```
    #[allow(dead_code)] // Prepared for future use in complex multi-table operations
    #[instrument(name = "db_transaction_with_params", skip(self, operations), fields(op_count = operations.len()))]
    pub async fn transaction_with_params(
        &self,
        operations: Vec<(String, Vec<(String, serde_json::Value)>)>,
    ) -> Result<()> {
        debug!(
            "Starting parameterized transaction with {} operations",
            operations.len()
        );

        // Begin transaction
        if let Err(e) = self.db.query("BEGIN TRANSACTION").await {
            error!(error = %e, "Failed to begin transaction");
            return Err(e.into());
        }

        // Execute all operations with their parameters
        for (i, (query, params)) in operations.iter().enumerate() {
            debug!(op_index = i, "Executing transaction operation");

            let mut query_builder = self.db.query(query);
            for (name, value) in params {
                query_builder = query_builder.bind((name.clone(), value.clone()));
            }

            if let Err(e) = query_builder.await {
                error!(error = %e, op_index = i, "Transaction operation failed, rolling back");
                if let Err(cancel_err) = self.db.query("CANCEL TRANSACTION").await {
                    warn!(error = %cancel_err, "Failed to cancel transaction after error");
                }
                return Err(e.into());
            }
        }

        // Commit transaction
        if let Err(e) = self.db.query("COMMIT TRANSACTION").await {
            error!(error = %e, "Failed to commit transaction");
            if let Err(cancel_err) = self.db.query("CANCEL TRANSACTION").await {
                warn!(error = %cancel_err, "Failed to cancel transaction after commit failure");
            }
            return Err(e.into());
        }

        info!("Parameterized transaction committed successfully");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_db_client_new() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("schema_test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = DBClient::new(db_path_str)
            .await
            .expect("DB creation failed");
        let result = db.initialize_schema().await;
        assert!(result.is_ok(), "Schema initialization should succeed");
    }

    #[tokio::test]
    async fn test_db_query_empty_result() {
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
        let temp_dir = tempdir().expect("Failed to create temp dir");
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
