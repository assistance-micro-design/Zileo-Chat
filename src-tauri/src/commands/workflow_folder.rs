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

//! Workflow folder CRUD commands for organizing workflows into groups.

use crate::{
    models::{WorkflowFolder, WorkflowFolderCreate},
    security::{serialize_for_query, validate_uuid_field, Validator},
    AppState,
};
use std::sync::LazyLock;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Maximum number of folders allowed
const MAX_FOLDERS: usize = 50;

/// Compiled regex for valid hex color (#RRGGBB)
static HEX_COLOR_REGEX: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"^#[0-9a-fA-F]{6}$").expect("valid hex color regex"));

/// Validates a hex color string.
fn validate_hex_color(color: &str) -> Result<String, String> {
    if HEX_COLOR_REGEX.is_match(color) {
        Ok(color.to_string())
    } else {
        Err(format!(
            "Invalid hex color '{}', expected format #RRGGBB",
            color
        ))
    }
}

/// Creates a new workflow folder.
///
/// # Arguments
/// * `name` - Folder display name (1-128 chars)
/// * `color` - Hex color string (#RRGGBB)
///
/// # Returns
/// The created WorkflowFolder
#[tauri::command]
#[instrument(name = "create_workflow_folder", skip(state), fields(folder_name = %name))]
pub async fn create_workflow_folder(
    name: String,
    color: String,
    state: State<'_, AppState>,
) -> Result<WorkflowFolder, String> {
    info!("Creating new workflow folder");

    let validated_name = Validator::validate_workflow_name(&name).map_err(|e| {
        warn!(error = %e, "Invalid folder name");
        format!("Invalid folder name: {}", e)
    })?;
    let validated_color = validate_hex_color(&color)?;

    // Check folder limit
    let count_query = "SELECT count() FROM workflow_folder GROUP ALL";
    let count_result: Vec<serde_json::Value> = state.db.query(count_query).await.map_err(|e| {
        error!(error = %e, "Failed to count folders");
        format!("Failed to count folders: {}", e)
    })?;

    let current_count = crate::db::extract_count(&count_result) as usize;
    if current_count >= MAX_FOLDERS {
        return Err(format!("Maximum folder limit ({}) reached", MAX_FOLDERS));
    }

    // Get next sort_order
    let sort_query = "SELECT math::max(sort_order) AS max_order FROM workflow_folder GROUP ALL";
    let sort_result: Vec<serde_json::Value> = state.db.query(sort_query).await.map_err(|e| {
        error!(error = %e, "Failed to query max sort_order");
        format!("Failed to query max sort_order: {}", e)
    })?;
    let next_order = sort_result
        .first()
        .and_then(|v| v.get("max_order").and_then(|o| o.as_i64()))
        .unwrap_or(-1)
        + 1;

    let folder_id = uuid::Uuid::new_v4().to_string();
    let folder_data = WorkflowFolderCreate {
        name: validated_name,
        color: validated_color,
        sort_order: next_order,
    };

    state
        .db
        .create("workflow_folder", &folder_id, folder_data)
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to create workflow folder");
            format!("Failed to create workflow folder: {}", e)
        })?;

    // Read back the created folder
    let query = format!(
        "SELECT meta::id(id) AS id, name, color, sort_order, created_at, updated_at FROM workflow_folder:`{}`",
        folder_id
    );
    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to read created folder");
        format!("Failed to read created folder: {}", e)
    })?;

    let folder: WorkflowFolder = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Folder not found after creation".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| {
                error!(error = %e, "Failed to deserialize folder");
                format!("Failed to deserialize folder: {}", e)
            })
        })?;

    info!(folder_id = %folder.id, "Workflow folder created successfully");
    Ok(folder)
}

/// Lists all workflow folders ordered by sort_order.
#[tauri::command]
#[instrument(name = "list_workflow_folders", skip(state))]
pub async fn list_workflow_folders(
    state: State<'_, AppState>,
) -> Result<Vec<WorkflowFolder>, String> {
    info!("Loading workflow folders");

    let query = "SELECT meta::id(id) AS id, name, color, sort_order, created_at, updated_at FROM workflow_folder ORDER BY sort_order ASC";

    let json_results = state.db.query_json(query).await.map_err(|e| {
        error!(error = %e, "Failed to load workflow folders");
        format!("Failed to load workflow folders: {}", e)
    })?;

    let folders: Vec<WorkflowFolder> = json_results
        .into_iter()
        .map(serde_json::from_value)
        .collect::<Result<Vec<WorkflowFolder>, _>>()
        .map_err(|e| {
            error!(error = %e, "Failed to deserialize workflow folders");
            format!("Failed to deserialize workflow folders: {}", e)
        })?;

    info!(count = folders.len(), "Workflow folders loaded");
    Ok(folders)
}

/// Renames a workflow folder.
///
/// # Arguments
/// * `folder_id` - The folder ID to rename
/// * `name` - The new folder name
#[tauri::command]
#[instrument(name = "rename_workflow_folder", skip(state), fields(folder_id = %folder_id))]
pub async fn rename_workflow_folder(
    folder_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<WorkflowFolder, String> {
    info!("Renaming workflow folder");

    let validated_id = validate_uuid_field(&folder_id, "folder_id")?;
    let validated_name = Validator::validate_workflow_name(&name).map_err(|e| {
        warn!(error = %e, "Invalid folder name");
        format!("Invalid folder name: {}", e)
    })?;
    let name_json = serialize_for_query(&validated_name, "name")?;

    let query = format!(
        "UPDATE workflow_folder:`{}` SET name = {}, updated_at = time::now() RETURN meta::id(id) AS id, name, color, sort_order, created_at, updated_at",
        validated_id, name_json
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to rename workflow folder");
        format!("Failed to rename workflow folder: {}", e)
    })?;

    let folder: WorkflowFolder = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Folder not found".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize folder: {}", e))
        })?;

    info!("Workflow folder renamed successfully");
    Ok(folder)
}

/// Updates a workflow folder's color.
///
/// # Arguments
/// * `folder_id` - The folder ID to update
/// * `color` - The new hex color (#RRGGBB)
#[tauri::command]
#[instrument(name = "update_folder_color", skip(state), fields(folder_id = %folder_id))]
pub async fn update_folder_color(
    folder_id: String,
    color: String,
    state: State<'_, AppState>,
) -> Result<WorkflowFolder, String> {
    info!("Updating workflow folder color");

    let validated_id = validate_uuid_field(&folder_id, "folder_id")?;
    let validated_color = validate_hex_color(&color)?;
    let color_json = serialize_for_query(&validated_color, "color")?;

    let query = format!(
        "UPDATE workflow_folder:`{}` SET color = {}, updated_at = time::now() RETURN meta::id(id) AS id, name, color, sort_order, created_at, updated_at",
        validated_id, color_json
    );

    let json_results = state.db.query_json(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update folder color");
        format!("Failed to update folder color: {}", e)
    })?;

    let folder: WorkflowFolder = json_results
        .into_iter()
        .next()
        .ok_or_else(|| "Folder not found".to_string())
        .and_then(|v| {
            serde_json::from_value(v).map_err(|e| format!("Failed to deserialize folder: {}", e))
        })?;

    info!("Workflow folder color updated successfully");
    Ok(folder)
}

/// Deletes a workflow folder.
///
/// Workflows in the folder are orphaned (folder_id set to NULL).
///
/// # Arguments
/// * `folder_id` - The folder ID to delete
#[tauri::command]
#[instrument(name = "delete_workflow_folder", skip(state), fields(folder_id = %folder_id))]
pub async fn delete_workflow_folder(
    folder_id: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!("Deleting workflow folder");

    let validated_id = validate_uuid_field(&folder_id, "folder_id")?;

    // Orphan workflows in this folder (set folder_id to NONE)
    let orphan_query = format!(
        "UPDATE workflow SET folder_id = NONE, updated_at = time::now() WHERE folder_id = '{}'",
        validated_id
    );
    state.db.execute(&orphan_query).await.map_err(|e| {
        error!(error = %e, "Failed to orphan workflows from folder");
        format!("Failed to orphan workflows: {}", e)
    })?;

    // Delete the folder
    state
        .db
        .delete(&format!("workflow_folder:{}", validated_id))
        .await
        .map_err(|e| {
            error!(error = %e, "Failed to delete workflow folder");
            format!("Failed to delete workflow folder: {}", e)
        })?;

    info!("Workflow folder deleted successfully");
    Ok(())
}

/// Reorders workflow folders by updating sort_order.
///
/// # Arguments
/// * `folder_ids` - Ordered list of folder IDs (index = new sort_order)
#[tauri::command]
#[instrument(name = "reorder_workflow_folders", skip(state), fields(count = folder_ids.len()))]
pub async fn reorder_workflow_folders(
    folder_ids: Vec<String>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    info!(count = folder_ids.len(), "Reordering workflow folders");

    for (i, id) in folder_ids.iter().enumerate() {
        let validated_id = validate_uuid_field(id, "folder_id")?;
        let query = format!(
            "UPDATE workflow_folder:`{}` SET sort_order = {}, updated_at = time::now()",
            validated_id, i
        );
        state.db.execute(&query).await.map_err(|e| {
            error!(error = %e, folder_id = %validated_id, "Failed to update folder sort_order");
            format!("Failed to reorder folder {}: {}", validated_id, e)
        })?;
    }

    info!("Workflow folders reordered successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_hex_color_valid() {
        assert!(validate_hex_color("#3b82f6").is_ok());
        assert!(validate_hex_color("#000000").is_ok());
        assert!(validate_hex_color("#FFFFFF").is_ok());
        assert!(validate_hex_color("#aAbBcC").is_ok());
    }

    #[test]
    fn test_validate_hex_color_invalid() {
        assert!(validate_hex_color("3b82f6").is_err()); // missing #
        assert!(validate_hex_color("#3b82f").is_err()); // too short
        assert!(validate_hex_color("#3b82f6a").is_err()); // too long
        assert!(validate_hex_color("#gggggg").is_err()); // invalid chars
        assert!(validate_hex_color("").is_err());
        assert!(validate_hex_color("red").is_err());
    }

    #[test]
    fn test_workflow_folder_create_serialization() {
        let data = WorkflowFolderCreate {
            name: "Test Folder".to_string(),
            color: "#ef4444".to_string(),
            sort_order: 2,
        };

        let json = serde_json::to_string(&data).unwrap();
        assert!(json.contains("\"name\":\"Test Folder\""));
        assert!(json.contains("\"color\":\"#ef4444\""));
        assert!(json.contains("\"sort_order\":2"));
    }
}
