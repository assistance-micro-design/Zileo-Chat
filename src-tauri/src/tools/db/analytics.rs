// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Analytics Tool - Aggregations and graph traversal.

use crate::db::DBClient;
use crate::models::ToolDefinition;
use crate::tools::{Tool, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use tracing::{debug, instrument};

/// Tool for analytics operations: aggregations and graph traversal
pub struct AnalyticsTool {
    /// Database client
    db: Arc<DBClient>,
}

impl AnalyticsTool {
    /// Creates a new Analytics tool
    pub fn new(db: Arc<DBClient>) -> Self {
        Self { db }
    }

    /// Performs aggregation query
    #[instrument(skip(self), fields(operation = "aggregate"))]
    async fn aggregate(&self, params: &Value) -> ToolResult<Value> {
        let table = params["table"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;

        let function = params["function"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing aggregation function".to_string()))?;

        let field = params["field"].as_str();
        let group_by = params["group_by"].as_str();

        // Validate function
        let valid_functions = ["count", "sum", "avg", "min", "max", "array::distinct"];
        if !valid_functions.contains(&function.to_lowercase().as_str()) {
            return Err(ToolError::InvalidInput(format!(
                "Invalid aggregation function: {}. Valid functions: {:?}",
                function, valid_functions
            )));
        }

        let agg_expr = match (function.to_lowercase().as_str(), field) {
            ("count", _) => "count()".to_string(),
            (f, Some(fld)) => format!("{}({})", f, fld),
            (f, None) => {
                return Err(ToolError::InvalidInput(format!(
                    "Function {} requires a field",
                    f
                )))
            }
        };

        let mut query = format!("SELECT {} as result FROM {}", agg_expr, table);

        // Add GROUP BY
        if let Some(gb) = group_by {
            query = format!("SELECT {}, {} as result FROM {} GROUP BY {}", gb, agg_expr, table, gb);
        }

        debug!(query = %query, "Executing aggregation query");

        let result: Result<Vec<Value>, _> = self.db.query(&query).await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "aggregate",
                "function": function,
                "table": table,
                "field": field,
                "group_by": group_by,
                "data": records
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Performs graph traversal
    #[instrument(skip(self), fields(operation = "traverse"))]
    async fn traverse(&self, params: &Value) -> ToolResult<Value> {
        let start = params["start"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing start node".to_string()))?;

        let relation = params["relation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing relation".to_string()))?;

        let direction = params["direction"].as_str().unwrap_or("out");
        let depth = params["depth"].as_u64().unwrap_or(1);

        // Build traversal query
        let arrow = match direction {
            "in" => "<-",
            "out" => "->",
            "both" => "<->",
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Invalid direction: {}. Use 'in', 'out', or 'both'",
                    direction
                )))
            }
        };

        // Build recursive traversal for depth > 1
        let traversal = if depth == 1 {
            format!("{}{}", arrow, relation)
        } else {
            // SurrealDB recursive traversal syntax
            format!("{}{}({}..{})", arrow, relation, 1, depth)
        };

        let query = format!("SELECT * FROM {}{}", start, traversal);

        debug!(query = %query, "Executing graph traversal");

        let result: Result<Vec<Value>, _> = self.db.query(&query).await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "traverse",
                "start": start,
                "relation": relation,
                "direction": direction,
                "depth": depth,
                "count": records.len(),
                "data": records
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }

    /// Gets statistics for a table
    #[instrument(skip(self), fields(operation = "stats"))]
    async fn table_stats(&self, table: &str) -> ToolResult<Value> {
        // Count total records
        let count_query = format!("SELECT count() as total FROM {}", table);
        let count_result: Result<Vec<Value>, _> = self.db.query(&count_query).await;

        let total = count_result
            .ok()
            .and_then(|r| r.first().cloned())
            .and_then(|v| v["total"].as_u64())
            .unwrap_or(0);

        // Get table info
        let info_query = format!("INFO FOR TABLE {}", table);
        let info_result: Result<Vec<Value>, _> = self.db.query(&info_query).await;

        let info = info_result.ok().and_then(|r| r.first().cloned());

        Ok(serde_json::json!({
            "operation": "stats",
            "table": table,
            "total_records": total,
            "schema_info": info
        }))
    }

    /// Finds connected components
    #[instrument(skip(self), fields(operation = "connections"))]
    async fn find_connections(&self, params: &Value) -> ToolResult<Value> {
        let from = params["from"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing from node".to_string()))?;

        let to = params["to"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing to node".to_string()))?;

        let max_depth = params["max_depth"].as_u64().unwrap_or(5);

        // Try to find paths between nodes
        let query = format!(
            "SELECT * FROM {} WHERE ->({}..{})->id = {}",
            from, 1, max_depth, to
        );

        debug!(query = %query, "Finding connections");

        let result: Result<Vec<Value>, _> = self.db.query(&query).await;

        match result {
            Ok(records) => Ok(serde_json::json!({
                "operation": "connections",
                "from": from,
                "to": to,
                "max_depth": max_depth,
                "connected": !records.is_empty(),
                "paths": records
            })),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }
}

#[async_trait]
impl Tool for AnalyticsTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "AnalyticsTool".to_string(),
            name: "Analytics Tool".to_string(),
            description: "Performs aggregations, graph traversal, and statistical analysis"
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["aggregate", "traverse", "stats", "connections"],
                        "description": "The analytics operation to perform"
                    },
                    "table": {
                        "type": "string",
                        "description": "Table name (for aggregate/stats)"
                    },
                    "function": {
                        "type": "string",
                        "enum": ["count", "sum", "avg", "min", "max"],
                        "description": "Aggregation function"
                    },
                    "field": {
                        "type": "string",
                        "description": "Field to aggregate"
                    },
                    "group_by": {
                        "type": "string",
                        "description": "Field to group by"
                    },
                    "start": {
                        "type": "string",
                        "description": "Start node for traversal"
                    },
                    "relation": {
                        "type": "string",
                        "description": "Relation type for traversal"
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["in", "out", "both"],
                        "description": "Traversal direction"
                    },
                    "depth": {
                        "type": "integer",
                        "description": "Traversal depth"
                    },
                    "from": {
                        "type": "string",
                        "description": "Source node for connection search"
                    },
                    "to": {
                        "type": "string",
                        "description": "Target node for connection search"
                    },
                    "max_depth": {
                        "type": "integer",
                        "description": "Maximum depth for connection search"
                    }
                },
                "required": ["operation"]
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string"},
                    "data": {"type": "array"},
                    "count": {"type": "integer"}
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

        match operation {
            "aggregate" => self.aggregate(&input).await,
            "traverse" => self.traverse(&input).await,
            "stats" => {
                let table = input["table"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("Missing table".to_string()))?;
                self.table_stats(table).await
            }
            "connections" => self.find_connections(&input).await,
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput("Input must be an object".to_string()));
        }

        if input.get("operation").is_none() {
            return Err(ToolError::InvalidInput(
                "Missing operation field".to_string(),
            ));
        }

        let operation = input["operation"].as_str().unwrap_or("");
        match operation {
            "aggregate" | "stats" => {
                if input.get("table").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing table for aggregate/stats".to_string(),
                    ));
                }
            }
            "traverse" => {
                if input.get("start").is_none() || input.get("relation").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing start or relation for traverse".to_string(),
                    ));
                }
            }
            "connections" => {
                if input.get("from").is_none() || input.get("to").is_none() {
                    return Err(ToolError::InvalidInput(
                        "Missing from or to for connections".to_string(),
                    ));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let definition = ToolDefinition {
            id: "AnalyticsTool".to_string(),
            name: "Analytics Tool".to_string(),
            description: "Performs aggregations and graph traversal".to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            requires_confirmation: false,
        };

        assert_eq!(definition.id, "AnalyticsTool");
        assert!(!definition.requires_confirmation);
    }
}
