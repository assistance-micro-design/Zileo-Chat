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

//! Prompt Library Commands
//!
//! Tauri IPC commands for managing prompt templates with variable interpolation.

use crate::models::prompt::{
    Prompt, PromptCreate, PromptSummary, PromptUpdate, MAX_PROMPT_CONTENT_LEN,
    MAX_PROMPT_DESCRIPTION_LEN, MAX_PROMPT_NAME_LEN,
};
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::AppState;
use tauri::State;
use tracing::{error, info, instrument, warn};

fn validate_prompt_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Prompt name cannot be empty".to_string());
    }
    if trimmed.len() > MAX_PROMPT_NAME_LEN {
        return Err(format!(
            "Prompt name exceeds maximum length of {} characters",
            MAX_PROMPT_NAME_LEN
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_prompt_description(description: &str) -> Result<String, String> {
    let trimmed = description.trim();
    if trimmed.len() > MAX_PROMPT_DESCRIPTION_LEN {
        return Err(format!(
            "Prompt description exceeds maximum length of {} characters",
            MAX_PROMPT_DESCRIPTION_LEN
        ));
    }
    Ok(trimmed.to_string())
}

fn validate_prompt_content(content: &str) -> Result<String, String> {
    if content.is_empty() {
        return Err("Prompt content cannot be empty".to_string());
    }
    if content.len() > MAX_PROMPT_CONTENT_LEN {
        return Err(format!(
            "Prompt content exceeds maximum length of {} characters",
            MAX_PROMPT_CONTENT_LEN
        ));
    }
    Ok(content.to_string())
}

/// List all prompts (returns lightweight summaries)
#[tauri::command]
#[instrument(name = "list_prompts", skip(state))]
pub async fn list_prompts(state: State<'_, AppState>) -> Result<Vec<PromptSummary>, String> {
    info!("Listing all prompts");

    let query = r#"
        SELECT
            meta::id(id) AS id,
            name,
            description,
            category,
            array::len(variables ?? []) AS variables_count,
            updated_at
        FROM prompt
        ORDER BY updated_at DESC
    "#;

    let results: Vec<serde_json::Value> = state.db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to list prompts");
        format!("Failed to list prompts: {}", e)
    })?;

    let prompts: Vec<PromptSummary> = results
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    info!(count = prompts.len(), "Listed prompts");
    Ok(prompts)
}

/// Get full prompt by ID
#[tauri::command]
#[instrument(name = "get_prompt", skip(state), fields(prompt_id = %prompt_id))]
pub async fn get_prompt(prompt_id: String, state: State<'_, AppState>) -> Result<Prompt, String> {
    info!("Getting prompt");

    let prompt_id = validate_uuid_field(&prompt_id, "prompt_id")?;

    let query = format!(
        r#"
        SELECT
            meta::id(id) AS id,
            name,
            description,
            category,
            content,
            variables,
            created_at,
            updated_at
        FROM prompt:`{}`
        "#,
        prompt_id
    );

    let results: Vec<serde_json::Value> = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to get prompt");
        format!("Failed to get prompt: {}", e)
    })?;

    let prompt: Prompt = results
        .into_iter()
        .next()
        .ok_or_else(|| format!("Prompt not found: {}", prompt_id))
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize prompt: {}", e))
        })?;

    info!("Retrieved prompt");
    Ok(prompt)
}

/// Create new prompt
#[tauri::command]
#[instrument(name = "create_prompt", skip(state, config), fields(prompt_name = %config.name))]
pub async fn create_prompt(
    config: PromptCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating new prompt");

    // Validate input
    let name = validate_prompt_name(&config.name)?;
    let description = validate_prompt_description(&config.description)?;
    let content = validate_prompt_content(&config.content)?;

    // Generate UUID
    let prompt_id = uuid::Uuid::new_v4().to_string();

    // Detect variables from content
    let variables = Prompt::detect_variables(&content);
    // Use bind parameters for all user-supplied values
    // Note: prompt_id is a uuid::Uuid::new_v4() output (safe for format!())
    // but name, description, content, category come from user input
    let query = format!(
        r#"CREATE prompt:`{}` CONTENT {{
            name: $name,
            description: $description,
            category: $category,
            content: $content,
            variables: $variables,
            created_at: time::now(),
            updated_at: time::now()
        }}"#,
        prompt_id
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
                ("variables".to_string(), serde_json::json!(variables)),
            ],
        )
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create prompt in database");
            format!("Failed to create prompt: {}", e)
        })?;

    info!(prompt_id = %prompt_id, "Prompt created successfully");
    Ok(prompt_id)
}

/// Update existing prompt
#[tauri::command]
#[instrument(name = "update_prompt", skip(state, config), fields(prompt_id = %prompt_id))]
pub async fn update_prompt(
    prompt_id: String,
    config: PromptUpdate,
    state: State<'_, AppState>,
) -> Result<Prompt, String> {
    info!("Updating prompt");

    let prompt_id = validate_uuid_field(&prompt_id, "prompt_id")?;

    // Build SET clauses for non-None fields
    let mut set_clauses = Vec::new();

    if let Some(ref name) = config.name {
        let validated = validate_prompt_name(name)?;
        let name_json = serialize_for_query(&validated, "name")?;
        set_clauses.push(format!("name = {}", name_json));
    }

    if let Some(ref description) = config.description {
        let validated = validate_prompt_description(description)?;
        let desc_json = serialize_for_query(&validated, "description")?;
        set_clauses.push(format!("description = {}", desc_json));
    }

    if let Some(ref category) = config.category {
        let cat_json = serialize_for_query(&category.to_string(), "category")?;
        set_clauses.push(format!("category = {}", cat_json));
    }

    if let Some(ref content) = config.content {
        let validated = validate_prompt_content(content)?;
        let content_json = serialize_for_query(&validated, "content")?;

        // Re-detect variables when content changes
        let variables = Prompt::detect_variables(&validated);
        let variables_json = serialize_for_query(&variables, "variables")?;

        set_clauses.push(format!("content = {}", content_json));
        set_clauses.push(format!("variables = {}", variables_json));
    }

    if set_clauses.is_empty() {
        warn!("No fields to update");
        return get_prompt(prompt_id, state).await;
    }

    // Always update timestamp
    set_clauses.push("updated_at = time::now()".to_string());

    let query = format!(
        "UPDATE prompt:`{}` SET {}",
        prompt_id,
        set_clauses.join(", ")
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update prompt in database");
        format!("Failed to update prompt: {}", e)
    })?;

    info!("Prompt updated successfully");
    get_prompt(prompt_id, state).await
}

/// Delete prompt
#[tauri::command]
#[instrument(name = "delete_prompt", skip(state), fields(prompt_id = %prompt_id))]
pub async fn delete_prompt(prompt_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting prompt");

    let prompt_id = validate_uuid_field(&prompt_id, "prompt_id")?;

    let query = format!("DELETE prompt:`{}`", prompt_id);

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to delete prompt from database");
        format!("Failed to delete prompt: {}", e)
    })?;

    info!("Prompt deleted successfully");
    Ok(())
}

/// Search prompts by query and/or category
#[tauri::command]
#[instrument(name = "search_prompts", skip(state))]
pub async fn search_prompts(
    query: Option<String>,
    category: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<PromptSummary>, String> {
    info!(query = ?query, category = ?category, "Searching prompts");

    let mut conditions = Vec::new();
    let mut params: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(ref q) = query {
        if !q.trim().is_empty() {
            let search_term = q.trim().to_lowercase();
            conditions.push(
                "(string::lowercase(name) CONTAINS $search OR string::lowercase(description) CONTAINS $search)"
                    .to_string(),
            );
            params.push(("search".to_string(), serde_json::json!(search_term)));
        }
    }

    if let Some(ref cat) = category {
        if !cat.trim().is_empty() {
            conditions.push("category = $category".to_string());
            params.push(("category".to_string(), serde_json::json!(cat.trim())));
        }
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let db_query = format!(
        r#"
        SELECT
            meta::id(id) AS id,
            name,
            description,
            category,
            array::len(variables ?? []) AS variables_count,
            updated_at
        FROM prompt
        {}
        ORDER BY updated_at DESC
        "#,
        where_clause
    );

    let results: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(&db_query, params)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to search prompts");
            format!("Failed to search prompts: {}", e)
        })?;

    let prompts: Vec<PromptSummary> = results
        .into_iter()
        .filter_map(|v| serde_json::from_value(v).ok())
        .collect();

    info!(count = prompts.len(), "Search completed");
    Ok(prompts)
}

#[cfg(test)]
#[path = "prompt_tests.rs"]
mod tests;
