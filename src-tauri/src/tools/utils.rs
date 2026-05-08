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

use crate::agents::core::registry::AgentRegistry;
use crate::db::DBClient;
use crate::security::validate_uuid_field;
use crate::tools::{ToolError, ToolResult};
use std::sync::Arc;
use tracing::{debug, instrument};

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
///
/// Validates `id` as a strict UUID v4 before interpolation into SurrealQL.
pub async fn delete_with_check(
    db: &Arc<DBClient>,
    table: &str,
    id: &str,
    resource_name: &str,
) -> ToolResult<()> {
    let validated_id = validate_uuid_field(id, "id").map_err(ToolError::ValidationFailed)?;

    ensure_record_exists(db, table, &validated_id, resource_name).await?;

    let delete_query = format!("DELETE {}:`{}`", table, validated_id);
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

/// Parameterized query builder for SQL-injection safe queries.
/// Returns both the query string and the bind parameters.
pub struct ParamQueryBuilder {
    table: String,
    fields: Vec<String>,
    conditions: Vec<String>,
    params: Vec<(String, serde_json::Value)>,
    order_by: Option<(String, bool)>,
    limit: Option<usize>,
}

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
    pub fn where_eq_param(
        mut self,
        field: &str,
        param_name: &str,
        value: serde_json::Value,
    ) -> Self {
        self.conditions.push(format!("{} = ${}", field, param_name));
        self.params.push((param_name.to_string(), value));
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

/// Resolves an agent reference that can be either an ID or a name.
///
/// Attempts ID lookup first (fast path), then falls back to name lookup (slow path).
/// Returns the resolved agent ID.
///
/// # Arguments
/// * `registry` - The agent registry to search
/// * `agent_ref` - Agent ID (UUID) or agent name
///
/// # Errors
/// * `ToolError::InvalidInput` if `agent_ref` is empty after trimming
/// * `ToolError::NotFound` if no agent matches by ID or name
#[instrument(name = "resolve_agent_ref", skip(registry), fields(agent_ref = %agent_ref))]
pub async fn resolve_agent_ref(registry: &AgentRegistry, agent_ref: &str) -> ToolResult<String> {
    let trimmed = agent_ref.trim();
    if trimmed.is_empty() {
        return Err(ToolError::InvalidInput(
            "Agent reference cannot be empty. Provide an agent ID or name.".to_string(),
        ));
    }

    // Fast path: direct ID lookup
    if registry.get(trimmed).await.is_some() {
        debug!("Resolved by ID");
        return Ok(trimmed.to_string());
    }

    // Slow path: name lookup (case-insensitive)
    if let Some((agent_id, _)) = registry.get_by_name(trimmed).await {
        debug!(resolved_id = %agent_id, "Resolved by name");
        return Ok(agent_id);
    }

    Err(ToolError::NotFound(format!(
        "Agent '{}' not found. Use 'list_agents' to see available agents.",
        trimmed
    )))
}

/// Safely truncates a string to a maximum number of characters.
///
/// This function handles multi-byte UTF-8 characters correctly by working
/// with char boundaries instead of byte positions.
///
/// # Arguments
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters to keep
/// * `ellipsis` - Whether to append "..." if truncated
///
/// # Returns
/// The truncated string
pub fn safe_truncate(s: &str, max_chars: usize, ellipsis: bool) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars).collect();
        if ellipsis {
            format!("{}...", truncated)
        } else {
            truncated
        }
    }
}

#[cfg(test)]
#[path = "utils_tests.rs"]
mod tests;
