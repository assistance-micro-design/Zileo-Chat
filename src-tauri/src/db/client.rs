// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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

    /// Initializes the database schema
    #[instrument(name = "db_initialize_schema", skip(self))]
    pub async fn initialize_schema(&self) -> Result<()> {
        use super::schema::SCHEMA_SQL;

        info!("Initializing database schema");

        self.db.query(SCHEMA_SQL).await.map_err(|e| {
            error!(error = %e, "Failed to initialize schema");
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

        // Use CREATE query with backtick-escaped ID for safety
        let query = format!("CREATE {}:`{}` CONTENT $data", table, id);
        self.db
            .query(&query)
            .bind(("data", json_data))
            .await
            .map_err(|e| {
                error!(error = %e, "Failed to create record");
                e
            })?;

        debug!(record_id = %id, "Record created");
        Ok(id.to_string())
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
    #[instrument(name = "db_delete", skip(self), fields(record_id = %id))]
    pub async fn delete(&self, id: &str) -> Result<()> {
        debug!("Deleting record");

        let _: Vec<serde_json::Value> = self.db.delete(id).await.map_err(|e| {
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
    #[allow(dead_code)] // Used by SurrealDBTool which is prepared for Phase 6+
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
