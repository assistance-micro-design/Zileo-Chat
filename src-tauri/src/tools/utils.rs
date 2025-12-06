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
pub async fn ensure_record_exists(
    db: &Arc<DBClient>,
    table: &str,
    id: &str,
    resource_name: &str,
) -> ToolResult<()> {
    let check_query = format!(
        "SELECT meta::id(id) AS id FROM {} WHERE meta::id(id) = '{}'",
        table, id
    );
    let existing: Vec<serde_json::Value> = db
        .query(&check_query)
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
}
