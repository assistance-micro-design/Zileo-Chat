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

//! Skill Commands
//!
//! Tauri IPC commands for managing skill documents (markdown instructions for agents).

use crate::models::skill::{
    validate_skill_content, validate_skill_description, validate_skill_name, Skill, SkillCreate,
    SkillSummary, SkillUpdate,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::AppState;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// List all skills (returns lightweight summaries with content_length)
#[tauri::command]
#[instrument(name = "list_skills", skip(state))]
pub async fn list_skills(state: State<'_, AppState>) -> Result<Vec<SkillSummary>, String> {
    info!("Listing all skills");

    let query = r#"
        SELECT
            meta::id(id) AS id,
            name,
            description,
            category,
            enabled,
            string::len(content) AS content_length,
            updated_at
        FROM skill
        ORDER BY updated_at DESC
    "#;

    let results: Vec<serde_json::Value> = state.db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to list skills");
        format!("Failed to list skills: {}", e)
    })?;

    let skills: Vec<SkillSummary> = results
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    info!(count = skills.len(), "Listed skills");
    Ok(skills)
}

/// Get full skill by ID
#[tauri::command]
#[instrument(name = "get_skill", skip(state), fields(skill_id = %skill_id))]
pub async fn get_skill(skill_id: String, state: State<'_, AppState>) -> Result<Skill, String> {
    let skill_id = validate_uuid_field(&skill_id, "skill_id")?;
    info!("Getting skill");

    let query = format!(
        r#"
        SELECT
            meta::id(id) AS id,
            name,
            description,
            category,
            content,
            enabled,
            created_at,
            updated_at
        FROM skill:`{}`
        "#,
        skill_id
    );

    let results: Vec<serde_json::Value> = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to get skill");
        format!("Failed to get skill: {}", e)
    })?;

    let skill: Skill = results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Skill not found: {}", skill_id))
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize skill: {}", e))
        })?;

    info!("Retrieved skill");
    Ok(skill)
}

/// Create new skill
#[tauri::command]
#[instrument(name = "create_skill", skip(state, config), fields(skill_name = %config.name))]
pub async fn create_skill(
    config: SkillCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating new skill");

    let name = validate_skill_name(&config.name)?;
    let description = validate_skill_description(&config.description)?;
    let content = validate_skill_content(&config.content)?;

    let skill_id = uuid::Uuid::new_v4().to_string();

    let query = format!(
        r#"CREATE skill:`{}` CONTENT {{
            name: $name,
            description: $description,
            category: $category,
            content: $content,
            enabled: true,
            created_at: time::now(),
            updated_at: time::now()
        }}"#,
        skill_id
    );

    state
        .db
        .execute_with_params(
            &query,
            vec![
                ("name".to_string(), serde_json::json!(name)),
                ("description".to_string(), serde_json::json!(description)),
                (
                    "category".to_string(),
                    serde_json::json!(config.category.to_string()),
                ),
                ("content".to_string(), serde_json::json!(content)),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create skill in database");
            format!("Failed to create skill: {}", e)
        })?;

    info!(skill_id = %skill_id, "Skill created successfully");
    Ok(skill_id)
}

/// Update existing skill
#[tauri::command]
#[instrument(name = "update_skill", skip(state, config), fields(skill_id = %skill_id))]
pub async fn update_skill(
    skill_id: String,
    config: SkillUpdate,
    state: State<'_, AppState>,
) -> Result<Skill, String> {
    let skill_id = validate_uuid_field(&skill_id, "skill_id")?;
    info!("Updating skill");

    let mut set_clauses = Vec::new();

    if let Some(ref name) = config.name {
        let validated = validate_skill_name(name)?;
        let name_json = serialize_for_query(&validated, "name")?;
        set_clauses.push(format!("name = {}", name_json));
    }

    if let Some(ref description) = config.description {
        let validated = validate_skill_description(description)?;
        let desc_json = serialize_for_query(&validated, "description")?;
        set_clauses.push(format!("description = {}", desc_json));
    }

    if let Some(ref category) = config.category {
        let cat_json = serialize_for_query(&category.to_string(), "category")?;
        set_clauses.push(format!("category = {}", cat_json));
    }

    if let Some(ref content) = config.content {
        let validated = validate_skill_content(content)?;
        let content_json = serialize_for_query(&validated, "content")?;
        set_clauses.push(format!("content = {}", content_json));
    }

    if let Some(enabled) = config.enabled {
        set_clauses.push(format!("enabled = {}", enabled));
    }

    if set_clauses.is_empty() {
        warn!("No fields to update");
        return get_skill(skill_id, state).await;
    }

    set_clauses.push("updated_at = time::now()".to_string());

    let query = format!("UPDATE skill:`{}` SET {}", skill_id, set_clauses.join(", "));

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update skill in database");
        format!("Failed to update skill: {}", e)
    })?;

    info!("Skill updated successfully");
    get_skill(skill_id, state).await
}

/// Delete skill
#[tauri::command]
#[instrument(name = "delete_skill", skip(state), fields(skill_id = %skill_id))]
pub async fn delete_skill(skill_id: String, state: State<'_, AppState>) -> Result<(), String> {
    let skill_id = validate_uuid_field(&skill_id, "skill_id")?;
    info!("Deleting skill");

    let query = format!("DELETE skill:`{}`", skill_id);

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to delete skill from database");
        format!("Failed to delete skill: {}", e)
    })?;

    info!("Skill deleted successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_utils::setup_test_state;

    #[tokio::test]
    async fn test_create_skill_with_bind_params() {
        let (state, _db_guard) = setup_test_state().await;
        let id = uuid::Uuid::new_v4().to_string();
        let query = format!(
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
        );
        state
            .db
            .execute_with_params(
                &query,
                vec![
                    ("name".to_string(), serde_json::json!("test-skill")),
                    (
                        "description".to_string(),
                        serde_json::json!("A test skill with 'quotes'"),
                    ),
                    ("category".to_string(), serde_json::json!("coding")),
                    (
                        "content".to_string(),
                        serde_json::json!("# Test\n\nSome markdown content"),
                    ),
                ],
            )
            .await
            .unwrap();

        let results: Vec<serde_json::Value> = state
            .db
            .query_json(&format!(
                "SELECT meta::id(id) AS id, name, enabled FROM skill:`{}`",
                id
            ))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["name"], "test-skill");
        assert_eq!(results[0]["enabled"], true);
    }

    #[tokio::test]
    async fn test_list_skills_returns_content_length() {
        let (state, _db_guard) = setup_test_state().await;
        let id = uuid::Uuid::new_v4().to_string();
        let content = "Hello, world!"; // 13 chars
        let query = format!(
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
        );
        state
            .db
            .execute_with_params(
                &query,
                vec![
                    ("name".to_string(), serde_json::json!("len-skill")),
                    (
                        "description".to_string(),
                        serde_json::json!("Testing length"),
                    ),
                    ("category".to_string(), serde_json::json!("custom")),
                    ("content".to_string(), serde_json::json!(content)),
                ],
            )
            .await
            .unwrap();

        let results: Vec<serde_json::Value> = state
            .db
            .query_json(
                r#"SELECT meta::id(id) AS id, name, string::len(content) AS content_length, updated_at
                   FROM skill ORDER BY updated_at DESC"#,
            )
            .await
            .unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["content_length"], 13);
    }

    #[tokio::test]
    async fn test_skill_enabled_default() {
        let (state, _db_guard) = setup_test_state().await;
        let id = uuid::Uuid::new_v4().to_string();
        let query = format!(
            r#"CREATE skill:`{}` CONTENT {{
                name: $name,
                description: $description,
                category: $category,
                content: $content,
                created_at: time::now(),
                updated_at: time::now()
            }}"#,
            id
        );
        state
            .db
            .execute_with_params(
                &query,
                vec![
                    ("name".to_string(), serde_json::json!("default-enabled")),
                    ("description".to_string(), serde_json::json!("Test")),
                    ("category".to_string(), serde_json::json!("custom")),
                    ("content".to_string(), serde_json::json!("Content")),
                ],
            )
            .await
            .unwrap();

        let results: Vec<serde_json::Value> = state
            .db
            .query_json(&format!("SELECT enabled FROM skill:`{}`", id))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0]["enabled"], true);
    }
}
