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

//! ReadSkill Tool Implementation
//!
//! Allows LLM agents to read skill documents on-demand.
//! Access is restricted to skills assigned to the agent.

use crate::db::DBClient;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Tool that allows agents to read their assigned skill documents.
///
/// The tool supports two operations:
/// - `list`: Lists available skills for the agent (name + description)
/// - `read` (default): Reads the full content of a specific skill by name
///
/// Access control: only skills assigned to the agent can be read.
pub struct ReadSkillTool {
    db: Arc<DBClient>,
    agent_skills: Vec<String>,
}

impl ReadSkillTool {
    /// Creates a new ReadSkillTool for an agent.
    ///
    /// # Arguments
    /// * `db` - Database client for querying skills
    /// * `agent_skills` - List of skill names assigned to the agent
    pub fn new(db: Arc<DBClient>, agent_skills: Vec<String>) -> Self {
        debug!(
            skills_count = agent_skills.len(),
            "ReadSkillTool created with agent skills"
        );
        Self { db, agent_skills }
    }

    /// Lists available skills for the agent (name + description only).
    async fn list_skills(&self) -> ToolResult<Value> {
        if self.agent_skills.is_empty() {
            return Ok(json!({
                "success": true,
                "skills": [],
                "message": "No skills are assigned to this agent."
            }));
        }

        let placeholders: Vec<String> = self
            .agent_skills
            .iter()
            .enumerate()
            .map(|(i, _)| format!("$name_{}", i))
            .collect();

        let query = format!(
            r#"SELECT name, description, category
               FROM skill
               WHERE name IN [{}] AND enabled = true
               ORDER BY name ASC"#,
            placeholders.join(", ")
        );

        let params: Vec<(String, serde_json::Value)> = self
            .agent_skills
            .iter()
            .enumerate()
            .map(|(i, name)| (format!("name_{}", i), json!(name)))
            .collect();

        let results: Vec<serde_json::Value> = self
            .db
            .query_json_with_params(&query, params)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("Failed to list skills: {}", e)))?;

        info!(count = results.len(), "Listed agent skills");

        Ok(json!({
            "success": true,
            "skills": results,
            "message": format!("{} skill(s) available. Use ReadSkill with a skill name to read its content.", results.len())
        }))
    }

    /// Reads the full content of a skill by name.
    async fn read_skill(&self, name: &str) -> ToolResult<Value> {
        // Access control: check if skill is assigned to the agent
        if !self.agent_skills.iter().any(|s| s == name) {
            warn!(
                skill_name = %name,
                "Agent attempted to read unassigned skill"
            );
            return Err(ToolError::PermissionDenied(format!(
                "Skill '{}' is not assigned to this agent. Use the 'list' operation to see available skills.",
                name
            )));
        }

        let query = r#"SELECT name, description, category, content
                       FROM skill
                       WHERE name = $name AND enabled = true"#;

        let results: Vec<serde_json::Value> = self
            .db
            .query_json_with_params(query, vec![("name".to_string(), json!(name))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("Failed to read skill: {}", e)))?;

        let skill = results.into_iter().next().ok_or_else(|| {
            ToolError::NotFound(format!("Skill '{}' not found or is disabled.", name))
        })?;

        info!(skill_name = %name, "Skill read successfully");

        Ok(json!({
            "success": true,
            "name": skill["name"],
            "description": skill["description"],
            "category": skill["category"],
            "content": skill["content"]
        }))
    }
}

#[async_trait]
impl Tool for ReadSkillTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "ReadSkillTool".to_string(),
            name: "ReadSkill".to_string(),
            summary: "Read skill documents containing instructions and context".to_string(),
            description: r#"Reads skill documents containing instructions and context.

USE THIS TOOL WHEN:
- You need to follow specific instructions or conventions for a task
- A skill is listed in your available skills and is relevant to your current task

OPERATIONS:
- "list": List all available skills (name + description)
- "read" (default): Read the full content of a skill by name

EXAMPLES:
1. List: {"operation": "list"}
2. Read: {"name": "coding-standards"}"#
                .to_string(),

            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["read", "list"],
                        "default": "read",
                        "description": "Operation: 'read' to get skill content, 'list' to see available skills"
                    },
                    "name": {
                        "type": "string",
                        "description": "Name of the skill to read (required for 'read' operation)"
                    }
                }
            }),

            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"},
                    "name": {"type": "string"},
                    "description": {"type": "string"},
                    "category": {"type": "string"},
                    "content": {"type": "string"},
                    "skills": {"type": "array"},
                    "message": {"type": "string"}
                }
            }),

            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let operation = input["operation"].as_str().unwrap_or("read");

        debug!(operation = %operation, "Executing ReadSkillTool");

        match operation {
            "list" => self.list_skills().await,
            "read" => {
                let name = input["name"].as_str().ok_or_else(|| {
                    ToolError::InvalidInput(
                        "Read operation requires 'name' field (skill name)".to_string(),
                    )
                })?;
                self.read_skill(name).await
            }
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Use 'read' or 'list'.",
                operation
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let operation = input["operation"].as_str().unwrap_or("read");

        match operation {
            "list" => Ok(()),
            "read" => {
                if input["name"].as_str().is_none() {
                    return Err(ToolError::InvalidInput(
                        "Read operation requires 'name' field. Use {\"operation\": \"list\"} to see available skills.".to_string(),
                    ));
                }
                Ok(())
            }
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid: 'read', 'list'.",
                operation
            ))),
        }
    }
}

#[cfg(test)]
#[path = "read_skill_tests.rs"]
mod tests;
