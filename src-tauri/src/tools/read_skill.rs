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
            description: r#"Reads skill documents containing instructions and context.

USE THIS TOOL WHEN:
- You need to follow specific instructions or conventions for a task
- A skill is listed in your available skills and is relevant to your current task
- You need detailed guidelines before performing an action

OPERATIONS:
- "list": List all available skills (name + description)
- "read" (default): Read the full content of a skill by name

BEST PRACTICES:
- Read relevant skills BEFORE performing related actions
- Use "list" first if unsure which skills are available
- Skills contain markdown instructions - follow them carefully

EXAMPLES:
1. List skills: {"operation": "list"}
2. Read a skill: {"name": "coding-standards"}
3. Read with explicit operation: {"operation": "read", "name": "git-workflow"}"#
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

    fn requires_confirmation(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn create_test_db() -> Arc<DBClient> {
        let temp = tempfile::tempdir().unwrap();
        let db = DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap();
        db.initialize_schema().await.unwrap();
        Arc::new(db)
    }

    #[tokio::test]
    async fn test_definition() {
        let db = create_test_db().await;
        let tool = ReadSkillTool::new(db, vec!["skill1".to_string()]);
        let def = tool.definition();
        assert_eq!(def.id, "ReadSkillTool");
        assert_eq!(def.name, "ReadSkill");
        assert!(!def.requires_confirmation);
    }

    #[tokio::test]
    async fn test_validate_list_operation() {
        let db = create_test_db().await;
        let tool = ReadSkillTool::new(db, vec![]);
        assert!(tool.validate_input(&json!({"operation": "list"})).is_ok());
    }

    #[tokio::test]
    async fn test_validate_read_missing_name() {
        let db = create_test_db().await;
        let tool = ReadSkillTool::new(db, vec![]);
        assert!(tool.validate_input(&json!({"operation": "read"})).is_err());
        assert!(tool.validate_input(&json!({})).is_err());
    }

    #[tokio::test]
    async fn test_validate_read_with_name() {
        let db = create_test_db().await;
        let tool = ReadSkillTool::new(db, vec![]);
        assert!(tool.validate_input(&json!({"name": "my-skill"})).is_ok());
    }

    #[tokio::test]
    async fn test_validate_invalid_operation() {
        let db = create_test_db().await;
        let tool = ReadSkillTool::new(db, vec![]);
        assert!(tool
            .validate_input(&json!({"operation": "delete"}))
            .is_err());
    }

    #[tokio::test]
    async fn test_list_empty_skills() {
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            DBClient::new(temp.path().join("test").to_str().unwrap())
                .await
                .unwrap(),
        );
        db.initialize_schema().await.unwrap();

        let tool = ReadSkillTool::new(db, vec![]);
        let result = tool.execute(json!({"operation": "list"})).await.unwrap();
        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["skills"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_read_unassigned_skill() {
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            DBClient::new(temp.path().join("test").to_str().unwrap())
                .await
                .unwrap(),
        );
        db.initialize_schema().await.unwrap();

        let tool = ReadSkillTool::new(db, vec!["allowed-skill".to_string()]);
        let result = tool.execute(json!({"name": "forbidden-skill"})).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::PermissionDenied(_)) => {}
            other => panic!("Expected PermissionDenied, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_read_assigned_but_missing_skill() {
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            DBClient::new(temp.path().join("test").to_str().unwrap())
                .await
                .unwrap(),
        );
        db.initialize_schema().await.unwrap();

        let tool = ReadSkillTool::new(db, vec!["nonexistent-skill".to_string()]);
        let result = tool.execute(json!({"name": "nonexistent-skill"})).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::NotFound(_)) => {}
            other => panic!("Expected NotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_read_existing_skill() {
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            DBClient::new(temp.path().join("test").to_str().unwrap())
                .await
                .unwrap(),
        );
        db.initialize_schema().await.unwrap();

        // Seed a skill
        let id = uuid::Uuid::new_v4().to_string();
        db.execute_with_params(
            &format!(
                r#"CREATE skill:`{}` CONTENT {{
                    name: $name,
                    description: $description,
                    category: $category,
                    content: $content,
                    enabled: true,
                    created_at: time::now(),
                    updated_at: time::now()
                }}"#,
                id
            ),
            vec![
                ("name".to_string(), json!("test-skill")),
                ("description".to_string(), json!("A test skill")),
                ("category".to_string(), json!("coding")),
                (
                    "content".to_string(),
                    json!("# Test Skill\n\nFollow these rules."),
                ),
            ],
        )
        .await
        .unwrap();

        let tool = ReadSkillTool::new(db, vec!["test-skill".to_string()]);
        let result = tool.execute(json!({"name": "test-skill"})).await.unwrap();

        assert!(result["success"].as_bool().unwrap());
        assert_eq!(result["name"], "test-skill");
        assert_eq!(result["content"], "# Test Skill\n\nFollow these rules.");
    }

    #[tokio::test]
    async fn test_read_disabled_skill() {
        let temp = tempfile::tempdir().unwrap();
        let db = Arc::new(
            DBClient::new(temp.path().join("test").to_str().unwrap())
                .await
                .unwrap(),
        );
        db.initialize_schema().await.unwrap();

        // Seed a disabled skill
        let id = uuid::Uuid::new_v4().to_string();
        db.execute_with_params(
            &format!(
                r#"CREATE skill:`{}` CONTENT {{
                    name: $name,
                    description: $description,
                    category: $category,
                    content: $content,
                    enabled: false,
                    created_at: time::now(),
                    updated_at: time::now()
                }}"#,
                id
            ),
            vec![
                ("name".to_string(), json!("disabled-skill")),
                ("description".to_string(), json!("Disabled")),
                ("category".to_string(), json!("custom")),
                ("content".to_string(), json!("Content")),
            ],
        )
        .await
        .unwrap();

        let tool = ReadSkillTool::new(db, vec!["disabled-skill".to_string()]);
        let result = tool.execute(json!({"name": "disabled-skill"})).await;
        assert!(result.is_err());
        match result {
            Err(ToolError::NotFound(_)) => {}
            other => panic!("Expected NotFound for disabled skill, got {:?}", other),
        }
    }
}
