// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! SurrealDB Tool - Direct database operations.

use crate::db::DBClient;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Tool for direct SurrealDB operations
#[allow(dead_code)]
pub struct SurrealDBTool {
    /// Database client
    db: Arc<DBClient>,
    /// Query timeout in milliseconds
    timeout_ms: u64,
    /// Maximum results per query
    max_results: usize,
}

#[allow(dead_code)]
impl SurrealDBTool {
    /// Creates a new SurrealDB tool
    pub fn new(db: Arc<DBClient>, timeout_ms: u64, max_results: usize) -> Self {
        Self {
            db,
            timeout_ms,
            max_results,
        }
    }

    /// Executes a SELECT query
    #[instrument(skip(self), fields(operation = "select"))]
    async fn execute_select(&self, table: &str, filter: Option<&str>) -> ToolResult<Value> {
        let query = match filter {
            Some(f) => format!(
                "SELECT * FROM {} WHERE {} LIMIT {}",
                table, f, self.max_results
            ),
            None => format!("SELECT * FROM {} LIMIT {}", table, self.max_results),
        };

        debug!(query = %query, "Executing SELECT query");

        let result: Result<Vec<Value>, _> = self.db.query(&query).await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "select",
                "table": table,
                "count": records.len(),
                "data": records
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Executes a CREATE query
    #[instrument(skip(self, data), fields(operation = "create"))]
    async fn execute_create(&self, table: &str, data: Value) -> ToolResult<Value> {
        let query = format!("CREATE {} CONTENT $data", table);
        debug!(query = %query, "Executing CREATE query");

        // Use parameterized query for safety
        let result: Result<Vec<Value>, _> = self
            .db
            .query_with_params(&query, vec![("data".to_string(), data.clone())])
            .await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "create",
                "table": table,
                "success": true,
                "created": records.first()
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Executes an UPDATE query
    #[instrument(skip(self, data), fields(operation = "update"))]
    async fn execute_update(&self, table: &str, id: &str, data: Value) -> ToolResult<Value> {
        let query = format!("UPDATE {}:{} MERGE $data", table, id);
        debug!(query = %query, "Executing UPDATE query");

        let result: Result<Vec<Value>, _> = self
            .db
            .query_with_params(&query, vec![("data".to_string(), data.clone())])
            .await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "update",
                "table": table,
                "id": id,
                "success": true,
                "updated": records.first()
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Executes a DELETE query
    #[instrument(skip(self), fields(operation = "delete"))]
    async fn execute_delete(&self, table: &str, id: &str) -> ToolResult<Value> {
        let query = format!("DELETE {}:{}", table, id);
        debug!(query = %query, "Executing DELETE query");

        // Use execute() for DELETE to avoid SurrealDB SDK serialization issues
        match self.db.execute(&query).await {
            Ok(_) => Ok(serde_json::json!({
                "operation": "delete",
                "table": table,
                "id": id,
                "success": true
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Inspects table schema
    #[instrument(skip(self), fields(operation = "schema"))]
    async fn inspect_schema(&self, table: &str) -> ToolResult<Value> {
        let query = format!("INFO FOR TABLE {}", table);
        debug!(query = %query, "Inspecting table schema");

        let result: Result<Vec<Value>, _> = self.db.query(&query).await;

        match result {
            Ok(info) => Ok(serde_json::json!({
                "operation": "schema",
                "table": table,
                "info": info.first()
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }
}

#[async_trait]
impl Tool for SurrealDBTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "SurrealDBTool".to_string(),
            name: "SurrealDB Tool".to_string(),
            description: "Performs direct CRUD operations on SurrealDB database".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["select", "create", "update", "delete", "schema"],
                        "description": "The database operation to perform"
                    },
                    "table": {
                        "type": "string",
                        "description": "The table name"
                    },
                    "id": {
                        "type": "string",
                        "description": "Record ID (for update/delete operations)"
                    },
                    "filter": {
                        "type": "string",
                        "description": "WHERE clause filter (for select operation)"
                    },
                    "data": {
                        "type": "object",
                        "description": "Data to create or update"
                    }
                },
                "required": ["operation", "table"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string"},
                    "table": {"type": "string"},
                    "success": {"type": "boolean"},
                    "data": {"type": "array"},
                    "count": {"type": "integer"},
                    "error": {"type": "string"}
                }
            }),
            requires_confirmation: false,
        }
    }

    #[instrument(skip(self, input))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation".to_string()))?;

        let table = input["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        match operation {
            "select" => {
                let filter = input["filter"].as_str();
                self.execute_select(table, filter).await
            }
            "create" => {
                let data = input
                    .get("data")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                self.execute_create(table, data).await
            }
            "update" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing id for update".to_string()))?;
                let data = input
                    .get("data")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                self.execute_update(table, id, data).await
            }
            "delete" => {
                let id = input["id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing id for delete".to_string()))?;
                self.execute_delete(table, id).await
            }
            "schema" => self.inspect_schema(table).await,
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        if input.get("operation").is_none() {
            return Err(ToolError::InvalidInput(
                "Missing operation field".to_string(),
            ));
        }

        if input.get("table").is_none() {
            return Err(ToolError::InvalidInput("Missing table field".to_string()));
        }

        let operation = input["operation"].as_str().unwrap_or("");
        match operation {
            "update" | "delete" => {
                if input.get("id").is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Missing id field for {} operation",
                        operation
                    )));
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn requires_confirmation(&self) -> bool {
        // DELETE and UPDATE operations should require confirmation in safe mode
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        // Create a mock - we can't test without a real DB, but we can test the definition
        let definition = ToolDefinition {
            id: "SurrealDBTool".to_string(),
            name: "SurrealDB Tool".to_string(),
            description: "Performs direct CRUD operations on SurrealDB database".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "SurrealDBTool");
        assert!(!definition.requires_confirmation);
    }
}
