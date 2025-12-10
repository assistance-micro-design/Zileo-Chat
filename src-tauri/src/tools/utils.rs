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

//! Database and validation utilities for tools.

use crate::db::DBClient;
use crate::tools::{ToolError, ToolResult};
use std::sync::Arc;

/// Verifies a record exists in the database.
///
/// Uses parameterized query for the ID to prevent injection.
pub async fn ensure_record_exists(
    db: &Arc<DBClient>,
    table: &str,
    id: &str,
    resource_name: &str,
) -> ToolResult<()> {
    // Note: table name is controlled by code (not user input), ID is bound as parameter
    let check_query = format!(
        "SELECT meta::id(id) AS id FROM {} WHERE meta::id(id) = $id",
        table
    );
    let existing: Vec<serde_json::Value> = db
        .query_json_with_params(
            &check_query,
            vec![("id".to_string(), serde_json::json!(id))],
        )
        .await
        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;

    if existing.is_empty() {
        return Err(ToolError::NotFound(format!(
            "{} '{}' does not exist",
            resource_name, id
        )));
    }
    Ok(())
}

/// Deletes a record with existence check.
pub async fn delete_with_check(
    db: &Arc<DBClient>,
    table: &str,
    id: &str,
    resource_name: &str,
) -> ToolResult<()> {
    ensure_record_exists(db, table, id, resource_name).await?;

    let delete_query = format!("DELETE {}:`{}`", table, id);
    db.execute(&delete_query)
        .await
        .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
    Ok(())
}

/// Converts a database error to ToolError.
#[inline]
pub fn db_error(e: impl std::fmt::Display) -> ToolError {
    ToolError::DatabaseError(e.to_string())
}

/// Validates that a string is not empty.
#[inline]
pub fn validate_not_empty(value: &str, field_name: &str) -> ToolResult<()> {
    if value.is_empty() {
        return Err(ToolError::ValidationFailed(format!(
            "{} cannot be empty",
            field_name
        )));
    }
    Ok(())
}

/// Validates string length.
#[inline]
pub fn validate_length(value: &str, max: usize, field_name: &str) -> ToolResult<()> {
    if value.len() > max {
        return Err(ToolError::ValidationFailed(format!(
            "{} is {} chars, max is {}",
            field_name,
            value.len(),
            max
        )));
    }
    Ok(())
}

/// Validates a value is within range.
#[inline]
pub fn validate_range<T: PartialOrd + std::fmt::Display>(
    value: T,
    min: T,
    max: T,
    field_name: &str,
) -> ToolResult<()> {
    if value < min || value > max {
        return Err(ToolError::ValidationFailed(format!(
            "{} {} is invalid. Use {}-{}",
            field_name, value, min, max
        )));
    }
    Ok(())
}

/// Validates a value is in a list of valid values.
#[inline]
pub fn validate_enum_value(value: &str, valid_values: &[&str], field_name: &str) -> ToolResult<()> {
    if !valid_values.contains(&value) {
        return Err(ToolError::ValidationFailed(format!(
            "Invalid {} '{}'. Valid values: {:?}",
            field_name, value, valid_values
        )));
    }
    Ok(())
}

/// Fluent builder for SurrealDB queries.
#[allow(dead_code)]
pub struct QueryBuilder {
    table: String,
    fields: Vec<String>,
    conditions: Vec<String>,
    order_by: Option<(String, bool)>,
    limit: Option<usize>,
}

#[allow(dead_code)]
impl QueryBuilder {
    pub fn new(table: &str) -> Self {
        Self {
            table: table.to_string(),
            fields: vec!["meta::id(id) AS id".to_string()],
            conditions: Vec::new(),
            order_by: None,
            limit: None,
        }
    }

    pub fn select(mut self, fields: &[&str]) -> Self {
        self.fields.extend(fields.iter().map(|f| f.to_string()));
        self
    }

    pub fn where_eq(mut self, field: &str, value: &str) -> Self {
        let escaped = serde_json::to_string(value).unwrap_or_else(|_| format!("'{}'", value));
        self.conditions.push(format!("{} = {}", field, escaped));
        self
    }

    pub fn where_clause(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
        self
    }

    pub fn order_by(mut self, field: &str, desc: bool) -> Self {
        self.order_by = Some((field.to_string(), desc));
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    pub fn build(self) -> String {
        let mut query = format!("SELECT {} FROM {}", self.fields.join(", "), self.table);

        if !self.conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.conditions.join(" AND "));
        }

        if let Some((field, desc)) = self.order_by {
            query.push_str(&format!(
                " ORDER BY {} {}",
                field,
                if desc { "DESC" } else { "ASC" }
            ));
        }

        if let Some(n) = self.limit {
            query.push_str(&format!(" LIMIT {}", n));
        }

        query
    }
}

/// OPT-MEM-9: Parameterized query builder for SQL-injection safe queries.
/// Returns both the query string and the bind parameters.
#[allow(dead_code)]
pub struct ParamQueryBuilder {
    table: String,
    fields: Vec<String>,
    conditions: Vec<String>,
    params: Vec<(String, serde_json::Value)>,
    order_by: Option<(String, bool)>,
    limit: Option<usize>,
}

#[allow(dead_code)]
impl ParamQueryBuilder {
    /// Create a new parameterized query builder for the given table.
    /// Automatically includes `meta::id(id) AS id` in SELECT.
    pub fn new(table: &str) -> Self {
        Self {
            table: table.to_string(),
            fields: vec!["meta::id(id) AS id".to_string()],
            conditions: Vec::new(),
            params: Vec::new(),
            order_by: None,
            limit: None,
        }
    }

    /// Add fields to SELECT clause.
    pub fn select(mut self, fields: &[&str]) -> Self {
        self.fields.extend(fields.iter().map(|f| f.to_string()));
        self
    }

    /// Add a parameterized equality condition.
    /// Creates `field = $param_name` and stores the value.
    pub fn where_eq_param(mut self, field: &str, param_name: &str, value: serde_json::Value) -> Self {
        self.conditions.push(format!("{} = ${}", field, param_name));
        self.params.push((param_name.to_string(), value));
        self
    }

    /// Add a raw WHERE condition (for complex expressions like IS NONE).
    pub fn where_clause(mut self, condition: &str) -> Self {
        self.conditions.push(condition.to_string());
        self
    }

    /// Add a pre-built condition with its associated parameter.
    /// Useful when condition is built externally (e.g., scope conditions).
    pub fn where_with_param(mut self, condition: &str, param: (String, serde_json::Value)) -> Self {
        self.conditions.push(condition.to_string());
        self.params.push(param);
        self
    }

    /// Add multiple conditions and params at once.
    pub fn where_conditions(mut self, conditions: Vec<String>, params: Vec<(String, serde_json::Value)>) -> Self {
        self.conditions.extend(conditions);
        self.params.extend(params);
        self
    }

    /// Set ORDER BY clause.
    pub fn order_by(mut self, field: &str, desc: bool) -> Self {
        self.order_by = Some((field.to_string(), desc));
        self
    }

    /// Set LIMIT clause.
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Build the query string and parameters.
    /// Returns (query_string, params_vec) for use with query_with_params().
    pub fn build(self) -> (String, Vec<(String, serde_json::Value)>) {
        let mut query = format!("SELECT {} FROM {}", self.fields.join(", "), self.table);

        if !self.conditions.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&self.conditions.join(" AND "));
        }

        if let Some((field, desc)) = self.order_by {
            query.push_str(&format!(
                " ORDER BY {} {}",
                field,
                if desc { "DESC" } else { "ASC" }
            ));
        }

        if let Some(n) = self.limit {
            query.push_str(&format!(" LIMIT {}", n));
        }

        (query, self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_not_empty_valid() {
        assert!(validate_not_empty("hello", "field").is_ok());
    }

    #[test]
    fn test_validate_not_empty_invalid() {
        let result = validate_not_empty("", "field");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::ValidationFailed(_)
        ));
    }

    #[test]
    fn test_validate_length_valid() {
        assert!(validate_length("hello", 10, "field").is_ok());
    }

    #[test]
    fn test_validate_length_invalid() {
        let result = validate_length("hello world", 5, "field");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_range_valid() {
        assert!(validate_range(5, 1, 10, "field").is_ok());
    }

    #[test]
    fn test_validate_range_invalid() {
        let result = validate_range(15, 1, 10, "field");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_enum_value_valid() {
        assert!(validate_enum_value("pending", &["pending", "done"], "status").is_ok());
    }

    #[test]
    fn test_validate_enum_value_invalid() {
        let result = validate_enum_value("invalid", &["pending", "done"], "status");
        assert!(result.is_err());
    }

    #[test]
    fn test_query_builder_simple() {
        let query = QueryBuilder::new("memory")
            .select(&["content", "memory_type"])
            .build();
        assert_eq!(
            query,
            "SELECT meta::id(id) AS id, content, memory_type FROM memory"
        );
    }

    #[test]
    fn test_query_builder_with_conditions() {
        let query = QueryBuilder::new("memory")
            .select(&["content"])
            .where_eq("memory_type", "knowledge")
            .order_by("created_at", true)
            .limit(10)
            .build();
        assert!(query.contains("WHERE memory_type ="));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(query.contains("LIMIT 10"));
    }

    #[test]
    fn test_param_query_builder_simple() {
        let (query, params) = ParamQueryBuilder::new("memory")
            .select(&["content", "type"])
            .build();
        assert_eq!(query, "SELECT meta::id(id) AS id, content, type FROM memory");
        assert!(params.is_empty());
    }

    #[test]
    fn test_param_query_builder_with_params() {
        let (query, params) = ParamQueryBuilder::new("memory")
            .select(&["content"])
            .where_eq_param("type", "type_filter", serde_json::json!("knowledge"))
            .order_by("created_at", true)
            .limit(10)
            .build();
        assert!(query.contains("WHERE type = $type_filter"));
        assert!(query.contains("ORDER BY created_at DESC"));
        assert!(query.contains("LIMIT 10"));
        assert_eq!(params.len(), 1);
        assert_eq!(params[0].0, "type_filter");
        assert_eq!(params[0].1, serde_json::json!("knowledge"));
    }

    #[test]
    fn test_param_query_builder_multiple_conditions() {
        let (query, params) = ParamQueryBuilder::new("memory")
            .select(&["content"])
            .where_clause("workflow_id IS NONE")
            .where_eq_param("type", "mem_type", serde_json::json!("context"))
            .build();
        assert!(query.contains("workflow_id IS NONE AND type = $mem_type"));
        assert_eq!(params.len(), 1);
    }
}
