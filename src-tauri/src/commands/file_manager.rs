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

//! FileManager Tauri commands
//!
//! Provides IPC commands for folder validation and trash management.
//!
//! ## Commands
//!
//! - [`validate_agent_folder`] - Validate a folder path for agent authorization
//! - [`list_trash`] - List trash entries for a specific folder
//! - [`restore_from_trash_cmd`] - Restore a file from trash

use crate::tools::file_manager::security::validate_folder_for_authorization;
use crate::tools::file_manager::trash::TrashEntry;
use crate::tools::file_manager::trash_management;
use std::path::Path;
use tracing::{info, instrument, warn};

/// Validate a folder path and return its canonical form.
/// Called from frontend when user selects a folder via dialog.
///
/// # Arguments
/// * `path` - The directory path selected by the user
///
/// # Returns
/// The canonical path string if valid, or error message
#[tauri::command]
#[instrument(name = "validate_agent_folder", fields(path = %path))]
pub async fn validate_agent_folder(path: String) -> Result<String, String> {
    info!("Validating folder for agent authorization");

    let canonical = validate_folder_for_authorization(&path).map_err(|e| {
        warn!(error = %e, "Folder validation failed");
        e
    })?;

    let canonical_str = canonical
        .to_str()
        .ok_or_else(|| "Path contains non-UTF8 characters".to_string())?
        .to_string();

    info!(canonical = %canonical_str, "Folder validated successfully");
    Ok(canonical_str)
}

/// List trash entries for a specific authorized folder.
///
/// # Arguments
/// * `folder_path` - The authorized folder path to list trash for
///
/// # Returns
/// A vector of `TrashEntry` describing each trash item, or error message
#[tauri::command]
#[instrument(name = "list_trash", fields(folder = %folder_path))]
pub async fn list_trash(folder_path: String) -> Result<Vec<TrashEntry>, String> {
    info!("Listing trash entries");

    let folder = Path::new(&folder_path);
    if !folder.is_dir() {
        let msg = format!("Folder does not exist: {}", folder_path);
        warn!(%msg);
        return Err(msg);
    }

    trash_management::list_trash_entries(folder).map_err(|e| {
        warn!(error = %e, "Failed to list trash entries");
        e.to_string()
    })
}

/// Restore a file from trash to its original location.
///
/// # Arguments
/// * `trash_path` - Absolute path to the trash file to restore
/// * `folder_path` - The authorized folder to restore into
///
/// # Returns
/// The restored file path as a string, or error message
#[tauri::command]
#[instrument(name = "restore_from_trash_cmd", fields(trash = %trash_path, folder = %folder_path))]
pub async fn restore_from_trash_cmd(
    trash_path: String,
    folder_path: String,
) -> Result<String, String> {
    info!("Restoring file from trash");

    let trash = Path::new(&trash_path);
    let folder = Path::new(&folder_path);

    if !folder.is_dir() {
        let msg = format!("Folder does not exist: {}", folder_path);
        warn!(%msg);
        return Err(msg);
    }

    let restored = trash_management::restore_from_trash(trash, folder).map_err(|e| {
        warn!(error = %e, "Failed to restore from trash");
        e.to_string()
    })?;

    let restored_str = restored
        .to_str()
        .ok_or_else(|| "Restored path contains non-UTF8 characters".to_string())?
        .to_string();

    info!(restored = %restored_str, "File restored from trash");
    Ok(restored_str)
}
