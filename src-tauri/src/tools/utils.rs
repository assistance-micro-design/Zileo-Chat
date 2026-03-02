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

/// Parameterized query builder for SQL-injection safe queries.
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
    pub fn where_conditions(
        mut self,
        conditions: Vec<String>,
        params: Vec<(String, serde_json::Value)>,
    ) -> Self {
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

/// Generates common sub-agent tool description sections.
///
/// Returns a formatted string containing:
/// - PRIMARY_AGENT_ONLY restriction notice
/// - Sub-agent limit from constants
/// - Response format specification
///
/// # Arguments
/// * `tool_specific_text` - The tool-specific description to wrap
///
/// # Usage
/// ```rust,ignore
/// let description = sub_agent_description_template(
///     "Spawns temporary sub-agents to execute tasks in parallel or sequence."
/// );
/// ```
pub fn sub_agent_description_template(tool_specific_text: &str) -> String {
    use crate::tools::constants::sub_agent::MAX_SUB_AGENTS;

    format!(
        r#"{}

PRIMARY AGENT ONLY:
- Only the primary/root agent can use this tool
- Sub-agents cannot use sub-agent tools (max depth: 1)
- Maximum {} sub-agent operations per workflow

RESPONSE FORMAT:
Sub-agents return structured JSON with:
- success: boolean
- result: string (summary or error message)
- metrics: execution time, tokens used"#,
        tool_specific_text, MAX_SUB_AGENTS
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agents::core::agent::{Agent, Report, ReportMetrics, ReportStatus, Task};
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use async_trait::async_trait;

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
    fn test_param_query_builder_simple() {
        let (query, params) = ParamQueryBuilder::new("memory")
            .select(&["content", "type"])
            .build();
        assert_eq!(
            query,
            "SELECT meta::id(id) AS id, content, type FROM memory"
        );
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

    // --- SA-020/P3: resolve_agent_ref tests ---

    /// Minimal test agent for resolve_agent_ref tests
    struct TestAgent {
        config: AgentConfig,
    }

    impl TestAgent {
        fn new(id: &str, lifecycle: Lifecycle) -> Self {
            Self {
                config: AgentConfig {
                    id: id.to_string(),
                    name: format!("Test Agent {}", id),
                    lifecycle,
                    llm: LLMConfig {
                        provider: "Test".to_string(),
                        model: "test-model".to_string(),
                        temperature: 0.7,
                        max_tokens: 100,
                        is_reasoning: false,
                    },
                    tools: vec![],
                    mcp_servers: vec![],
                    skills: vec![],
                    folders: vec![],
                    require_file_confirmation: true,
                    system_prompt: "Test prompt".to_string(),
                    max_tool_iterations: 50,
                    enable_thinking: true,
                },
            }
        }
    }

    #[async_trait]
    impl Agent for TestAgent {
        async fn execute(&self, task: Task) -> anyhow::Result<Report> {
            Ok(Report {
                task_id: task.id,
                status: ReportStatus::Success,
                content: "Test report".to_string(),
                response: "Test report".to_string(),
                metrics: ReportMetrics {
                    duration_ms: 10,
                    tokens_input: 0,
                    tokens_output: 0,
                    tools_used: vec![],
                    mcp_calls: vec![],
                    tool_executions: vec![],
                    reasoning_steps: vec![],
                },
                system_prompt: None,
                tools_json: None,
            })
        }

        fn capabilities(&self) -> Vec<String> {
            vec!["test".to_string()]
        }

        fn lifecycle(&self) -> Lifecycle {
            self.config.lifecycle.clone()
        }

        fn tools(&self) -> Vec<String> {
            self.config.tools.clone()
        }

        fn mcp_servers(&self) -> Vec<String> {
            self.config.mcp_servers.clone()
        }

        fn system_prompt(&self) -> String {
            self.config.system_prompt.clone()
        }

        fn config(&self) -> &AgentConfig {
            &self.config
        }
    }

    #[tokio::test]
    async fn test_resolve_agent_ref_by_id() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("agent-uuid-1", Lifecycle::Permanent));
        registry.register("agent-uuid-1".to_string(), agent).await;

        let result = resolve_agent_ref(&registry, "agent-uuid-1").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "agent-uuid-1");
    }

    #[tokio::test]
    async fn test_resolve_agent_ref_by_name() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("agent-uuid-2", Lifecycle::Permanent));
        registry.register("agent-uuid-2".to_string(), agent).await;

        let result = resolve_agent_ref(&registry, "Test Agent agent-uuid-2").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "agent-uuid-2");
    }

    #[tokio::test]
    async fn test_resolve_agent_ref_not_found() {
        let registry = AgentRegistry::new();
        let agent = Arc::new(TestAgent::new("agent-uuid-3", Lifecycle::Permanent));
        registry.register("agent-uuid-3".to_string(), agent).await;

        let result = resolve_agent_ref(&registry, "ghost").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_resolve_agent_ref_empty_input() {
        let registry = AgentRegistry::new();

        let result = resolve_agent_ref(&registry, "").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));

        // Whitespace-only should also be rejected
        let result = resolve_agent_ref(&registry, "   ").await;
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[test]
    fn test_safe_truncate_utf8_multibyte() {
        // Test with French accented characters
        let text = "Ceci est un texte en francais avec des accents: e, a, o, i, u";
        let truncated = safe_truncate(text, 50, true);
        assert!(truncated.ends_with("..."));
        assert!(!truncated.contains("\\u")); // No escaped unicode

        // Test with text where byte 100 is inside a multi-byte char
        // This is the exact scenario that caused the panic
        let mission_text = "# MISSION\nRechercher sources fiables sur ACTUALITE pour: Mistral AI nouveautes 2025 actualites recentes lancements produits";
        let truncated = safe_truncate(mission_text, 100, true);
        assert!(truncated.ends_with("..."));

        // Test with emojis (4-byte UTF-8)
        let emoji_text =
            "Test avec emojis X et Y et beaucoup de texte apres pour depasser la limite";
        let truncated = safe_truncate(emoji_text, 30, true);
        assert!(truncated.ends_with("..."));

        // Test short text (no truncation needed)
        let short_text = "Court";
        let not_truncated = safe_truncate(short_text, 100, false);
        assert_eq!(not_truncated, "Court");
    }
}
