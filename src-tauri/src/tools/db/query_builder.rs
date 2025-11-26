// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Query Builder Tool - SQL/SurrealQL generation.

use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use tracing::{debug, instrument};

/// Tool for building SurrealQL queries from natural language or structured input
#[allow(dead_code)]
pub struct QueryBuilderTool;

#[allow(dead_code)]
impl QueryBuilderTool {
    /// Creates a new QueryBuilder tool
    pub fn new() -> Self {
        Self
    }

    /// Builds a SELECT query from structured parameters
    fn build_select(&self, params: &Value) -> ToolResult<String> {
        let table = params["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        let fields = params["fields"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_else(|| "*".to_string());

        let mut query = format!("SELECT {} FROM {}", fields, table);

        // Add WHERE clause
        if let Some(filter) = params.get("where") {
            if let Some(conditions) = filter.as_array() {
                let where_clauses: Vec<String> = conditions
                    .iter()
                    .filter_map(|c| {
                        let field = c["field"].as_str()?;
                        let op = c["operator"].as_str().unwrap_or("=");
                        let value = &c["value"];
                        Some(format_condition(field, op, value))
                    })
                    .collect();

                if !where_clauses.is_empty() {
                    query.push_str(" WHERE ");
                    query.push_str(&where_clauses.join(" AND "));
                }
            }
        }

        // Add ORDER BY
        if let Some(order) = params.get("order_by") {
            if let Some(field) = order["field"].as_str() {
                let direction = order["direction"].as_str().unwrap_or("ASC");
                query.push_str(&format!(" ORDER BY {} {}", field, direction.to_uppercase()));
            }
        }

        // Add LIMIT
        if let Some(limit) = params.get("limit").and_then(|l| l.as_u64()) {
            query.push_str(&format!(" LIMIT {}", limit));
        }

        // Add OFFSET
        if let Some(offset) = params.get("offset").and_then(|o| o.as_u64()) {
            query.push_str(&format!(" START {}", offset));
        }

        debug!(query = %query, "Built SELECT query");
        Ok(query)
    }

    /// Builds a CREATE query
    fn build_create(&self, params: &Value) -> ToolResult<String> {
        let table = params["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        let id = params.get("id").and_then(|i| i.as_str());

        let query = match id {
            Some(record_id) => format!("CREATE {}:{} CONTENT $data", table, record_id),
            None => format!("CREATE {} CONTENT $data", table),
        };

        debug!(query = %query, "Built CREATE query");
        Ok(query)
    }

    /// Builds an UPDATE query
    fn build_update(&self, params: &Value) -> ToolResult<String> {
        let table = params["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        let id = params["id"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing id".to_string()))?;

        let merge_or_set = params["mode"].as_str().unwrap_or("merge");

        let query = match merge_or_set {
            "set" => format!("UPDATE {}:{} SET $data", table, id),
            _ => format!("UPDATE {}:{} MERGE $data", table, id),
        };

        debug!(query = %query, "Built UPDATE query");
        Ok(query)
    }

    /// Builds a DELETE query
    fn build_delete(&self, params: &Value) -> ToolResult<String> {
        let table = params["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        let query = if let Some(id) = params.get("id").and_then(|i| i.as_str()) {
            format!("DELETE {}:{}", table, id)
        } else if let Some(filter) = params.get("where") {
            if let Some(conditions) = filter.as_array() {
                let where_clauses: Vec<String> = conditions
                    .iter()
                    .filter_map(|c| {
                        let field = c["field"].as_str()?;
                        let op = c["operator"].as_str().unwrap_or("=");
                        let value = &c["value"];
                        Some(format_condition(field, op, value))
                    })
                    .collect();

                format!(
                    "DELETE FROM {} WHERE {}",
                    table,
                    where_clauses.join(" AND ")
                )
            } else {
                return Err(ToolError::InvalidInput(
                    "Delete requires id or where conditions".to_string(),
                ));
            }
        } else {
            return Err(ToolError::InvalidInput(
                "Delete requires id or where conditions".to_string(),
            ));
        };

        debug!(query = %query, "Built DELETE query");
        Ok(query)
    }

    /// Builds a RELATE query for graph relations
    fn build_relate(&self, params: &Value) -> ToolResult<String> {
        let from = params["from"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing from".to_string()))?;
        let relation = params["relation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing relation".to_string()))?;
        let to = params["to"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing to".to_string()))?;

        let query = if params.get("data").is_some() {
            format!("RELATE {}->{}-> {} CONTENT $data", from, relation, to)
        } else {
            format!("RELATE {}->{}->{}", from, relation, to)
        };

        debug!(query = %query, "Built RELATE query");
        Ok(query)
    }
}

impl Default for QueryBuilderTool {
    fn default() -> Self {
        Self::new()
    }
}

/// Formats a condition for WHERE clause
#[allow(dead_code)]
fn format_condition(field: &str, operator: &str, value: &Value) -> String {
    let formatted_value = match value {
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "NULL".to_string(),
        _ => value.to_string(),
    };

    let op = match operator.to_lowercase().as_str() {
        "eq" | "=" | "==" => "=",
        "ne" | "!=" | "<>" => "!=",
        "gt" | ">" => ">",
        "gte" | ">=" => ">=",
        "lt" | "<" => "<",
        "lte" | "<=" => "<=",
        "contains" => "CONTAINS",
        "containsall" => "CONTAINSALL",
        "containsany" => "CONTAINSANY",
        "like" => "~",
        _ => operator,
    };

    format!("{} {} {}", field, op, formatted_value)
}

#[async_trait]
impl Tool for QueryBuilderTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "QueryBuilderTool".to_string(),
            name: "Query Builder Tool".to_string(),
            description: "Builds SurrealQL queries from structured parameters".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query_type": {
                        "type": "string",
                        "enum": ["select", "create", "update", "delete", "relate"],
                        "description": "The type of query to build"
                    },
                    "table": {
                        "type": "string",
                        "description": "The table name"
                    },
                    "fields": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Fields to select (default: *)"
                    },
                    "where": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "field": {"type": "string"},
                                "operator": {"type": "string"},
                                "value": {}
                            }
                        },
                        "description": "WHERE conditions"
                    },
                    "order_by": {
                        "type": "object",
                        "properties": {
                            "field": {"type": "string"},
                            "direction": {"type": "string", "enum": ["ASC", "DESC"]}
                        }
                    },
                    "limit": {"type": "integer"},
                    "offset": {"type": "integer"},
                    "id": {"type": "string"},
                    "data": {"type": "object"},
                    "from": {"type": "string"},
                    "relation": {"type": "string"},
                    "to": {"type": "string"}
                },
                "required": ["query_type", "table"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "query_type": {"type": "string"},
                    "table": {"type": "string"}
                },
                "required": ["query"]
            }),
            requires_confirmation: false,
        }
    }

    #[instrument(skip(self, input))]
    async fn execute(&self, input: Value) -> ToolResult<Value> {
        let query_type = input["query_type"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing query_type".to_string()))?;

        let query = match query_type {
            "select" => self.build_select(&input)?,
            "create" => self.build_create(&input)?,
            "update" => self.build_update(&input)?,
            "delete" => self.build_delete(&input)?,
            "relate" => self.build_relate(&input)?,
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown query type: {}",
                    query_type
                )))
            }
        };

        Ok(serde_json::json!({
            "query": query,
            "query_type": query_type,
            "table": input["table"].as_str().unwrap_or("")
        }))
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        if input.get("query_type").is_none() {
            return Err(ToolError::InvalidInput(
                "Missing query_type field".to_string(),
            ));
        }

        if input.get("table").is_none() {
            return Err(ToolError::InvalidInput("Missing table field".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_build_select_simple() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "select",
                "table": "users"
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("SELECT * FROM users"));
    }

    #[tokio::test]
    async fn test_build_select_with_fields() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "select",
                "table": "users",
                "fields": ["id", "name", "email"]
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("SELECT id, name, email FROM users"));
    }

    #[tokio::test]
    async fn test_build_select_with_where() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "select",
                "table": "users",
                "where": [
                    {"field": "status", "operator": "=", "value": "active"}
                ]
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("WHERE status = 'active'"));
    }

    #[tokio::test]
    async fn test_build_select_with_order_and_limit() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "select",
                "table": "users",
                "order_by": {"field": "created_at", "direction": "DESC"},
                "limit": 10
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(query.contains("LIMIT 10"));
    }

    #[tokio::test]
    async fn test_build_create() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "create",
                "table": "users"
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("CREATE users CONTENT $data"));
    }

    #[tokio::test]
    async fn test_build_update() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "update",
                "table": "users",
                "id": "user123"
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("UPDATE users:user123 MERGE $data"));
    }

    #[tokio::test]
    async fn test_build_delete() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "delete",
                "table": "users",
                "id": "user123"
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("DELETE users:user123"));
    }

    #[tokio::test]
    async fn test_build_relate() {
        let tool = QueryBuilderTool::new();
        let result = tool
            .execute(serde_json::json!({
                "query_type": "relate",
                "table": "follows",
                "from": "user:1",
                "relation": "follows",
                "to": "user:2"
            }))
            .await
            .unwrap();

        let query = result["query"].as_str().unwrap();
        assert!(query.contains("RELATE user:1->follows->user:2"));
    }

    #[test]
    fn test_format_condition() {
        assert_eq!(
            format_condition("name", "=", &Value::String("John".to_string())),
            "name = 'John'"
        );
        assert_eq!(
            format_condition("age", ">=", &serde_json::json!(18)),
            "age >= 18"
        );
        assert_eq!(
            format_condition("active", "=", &serde_json::json!(true)),
            "active = true"
        );
    }
}
