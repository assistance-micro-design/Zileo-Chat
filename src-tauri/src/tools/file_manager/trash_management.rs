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

//! Trash management operations: cleanup, list, restore, size.
//!
//! These functions operate on the `.zileo-trash/` directory within
//! authorized folders.

use super::trash::{
    parse_trash_filename, TrashEntry, MAX_TRASH_SIZE, PATH_SEPARATOR_REPLACEMENT, TRASH_DIR_NAME,
    TRASH_TIMESTAMP_FORMAT,
};
use crate::tools::{ToolError, ToolResult};
use std::path::{Path, PathBuf};
use tracing::{info, warn};

/// Clean up trash entries older than the retention period.
///
/// # Arguments
/// * `authorized_folder` - The authorized folder whose trash to clean
/// * `retention_days` - Number of days to keep trash entries
///
/// # Returns
/// The number of entries cleaned up
///
/// # Errors
/// - `ExecutionFailed` if the trash directory cannot be read or files cannot be removed
pub fn cleanup_trash(authorized_folder: &Path, retention_days: u64) -> ToolResult<usize> {
    let trash_dir = authorized_folder.join(TRASH_DIR_NAME);

    if !trash_dir.exists() {
        return Ok(0);
    }

    let now = chrono::Local::now();
    let retention = chrono::Duration::days(retention_days as i64);
    let cutoff = now - retention;
    let mut cleaned = 0;

    let entries = std::fs::read_dir(&trash_dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read trash entry: {}", e))
        })?;

        let filename = entry.file_name().to_string_lossy().to_string();

        if let Some((timestamp_str, _)) = parse_trash_filename(&filename) {
            if let Ok(timestamp) =
                chrono::NaiveDateTime::parse_from_str(&timestamp_str, TRASH_TIMESTAMP_FORMAT)
            {
                let entry_time = timestamp
                    .and_local_timezone(chrono::Local)
                    .single()
                    // Fallback: treat ambiguous as now (won't delete)
                    .unwrap_or_else(chrono::Local::now);

                if entry_time < cutoff {
                    if let Err(e) = std::fs::remove_file(entry.path()) {
                        warn!(
                            file = %entry.path().display(),
                            error = %e,
                            "Failed to remove expired trash entry"
                        );
                    } else {
                        cleaned += 1;
                    }
                }
            }
        }
        // Also handle .bak files with the same naming convention
        else if filename.ends_with(".bak") {
            let base = filename.trim_end_matches(".bak");
            if let Some((timestamp_str, _)) = parse_trash_filename(base) {
                if let Ok(timestamp) =
                    chrono::NaiveDateTime::parse_from_str(&timestamp_str, TRASH_TIMESTAMP_FORMAT)
                {
                    let entry_time = timestamp
                        .and_local_timezone(chrono::Local)
                        .single()
                        .unwrap_or_else(chrono::Local::now);

                    if entry_time < cutoff {
                        if let Err(e) = std::fs::remove_file(entry.path()) {
                            warn!(
                                file = %entry.path().display(),
                                error = %e,
                                "Failed to remove expired trash backup"
                            );
                        } else {
                            cleaned += 1;
                        }
                    }
                }
            }
        }
    }

    // Phase 2: Size-based eviction if trash exceeds MAX_TRASH_SIZE
    let total_size = get_trash_size(authorized_folder).unwrap_or(0);
    if total_size > MAX_TRASH_SIZE {
        // Collect remaining entries sorted by timestamp (oldest first)
        let mut size_entries: Vec<(String, std::path::PathBuf, u64)> = Vec::new();
        if let Ok(remaining) = std::fs::read_dir(&trash_dir) {
            for entry in remaining.flatten() {
                let filename = entry.file_name().to_string_lossy().to_string();
                let parse_name = if filename.ends_with(".bak") {
                    filename.trim_end_matches(".bak").to_string()
                } else {
                    filename.clone()
                };
                if let Some((ts, _)) = parse_trash_filename(&parse_name) {
                    if let Ok(meta) = entry.metadata() {
                        size_entries.push((ts, entry.path(), meta.len()));
                    }
                }
            }
        }
        // Sort oldest first
        size_entries.sort_by(|a, b| a.0.cmp(&b.0));

        let mut current_size = total_size;
        for (_, path, file_size) in &size_entries {
            if current_size <= MAX_TRASH_SIZE {
                break;
            }
            if let Err(e) = std::fs::remove_file(path) {
                warn!(
                    file = %path.display(),
                    error = %e,
                    "Failed to remove trash entry during size eviction"
                );
            } else {
                current_size = current_size.saturating_sub(*file_size);
                cleaned += 1;
            }
        }
    }

    info!(
        folder = %authorized_folder.display(),
        cleaned = cleaned,
        retention_days = retention_days,
        "Trash cleanup completed"
    );

    Ok(cleaned)
}

/// List all trash entries for an authorized folder.
///
/// # Arguments
/// * `authorized_folder` - The authorized folder whose trash to list
///
/// # Returns
/// A vector of `TrashEntry` structs describing each trash item
///
/// # Errors
/// - `ExecutionFailed` if the trash directory cannot be read
pub fn list_trash_entries(authorized_folder: &Path) -> ToolResult<Vec<TrashEntry>> {
    let trash_dir = authorized_folder.join(TRASH_DIR_NAME);

    if !trash_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(&trash_dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    let mut result = Vec::new();

    for entry in entries {
        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read trash entry: {}", e))
        })?;

        let filename = entry.file_name().to_string_lossy().to_string();

        // Try parsing as regular trash file or .bak file
        let parse_result = if filename.ends_with(".bak") {
            parse_trash_filename(filename.trim_end_matches(".bak"))
        } else {
            parse_trash_filename(&filename)
        };

        if let Some((timestamp, sanitized_path)) = parse_result {
            let original_relative_path =
                sanitized_path.replace(PATH_SEPARATOR_REPLACEMENT, std::path::MAIN_SEPARATOR_STR);

            let metadata = entry.metadata().map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read metadata for {}: {}",
                    entry.path().display(),
                    e
                ))
            })?;

            result.push(TrashEntry {
                trash_path: entry.path(),
                original_relative_path,
                deleted_at: timestamp,
                size_bytes: metadata.len(),
            });
        }
    }

    // Sort by timestamp descending (most recent first)
    result.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at));

    Ok(result)
}

/// Restore a file from trash to its original location.
///
/// # Arguments
/// * `trash_path` - Absolute path to the trash file
/// * `authorized_folder` - The authorized folder to restore into
///
/// # Returns
/// The path where the file was restored
///
/// # Errors
/// - `InvalidInput` if the trash file does not exist
/// - `ExecutionFailed` if the restore operation fails
pub fn restore_from_trash(trash_path: &Path, authorized_folder: &Path) -> ToolResult<PathBuf> {
    // Validate trash file exists
    if !trash_path.exists() {
        return Err(ToolError::InvalidInput(format!(
            "Trash file does not exist: {}",
            trash_path.display()
        )));
    }

    // Parse the trash filename to get original path
    let filename = trash_path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| {
            ToolError::InvalidInput(format!("Invalid trash file path: {}", trash_path.display()))
        })?;

    // Handle .bak suffix
    let parse_name = if filename.ends_with(".bak") {
        filename.trim_end_matches(".bak")
    } else {
        filename
    };

    let (_, sanitized_path) = parse_trash_filename(parse_name).ok_or_else(|| {
        ToolError::InvalidInput(format!("Cannot parse trash filename: {}", filename))
    })?;

    // Reconstruct the original path
    let original_relative =
        sanitized_path.replace(PATH_SEPARATOR_REPLACEMENT, std::path::MAIN_SEPARATOR_STR);
    let restore_path = authorized_folder.join(&original_relative);

    // Create parent directories if needed
    if let Some(parent) = restore_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to create parent directories for {}: {}",
                restore_path.display(),
                e
            ))
        })?;
    }

    // Copy from trash to original location
    std::fs::copy(trash_path, &restore_path).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to restore file to {}: {}",
            restore_path.display(),
            e
        ))
    })?;

    // Remove from trash
    std::fs::remove_file(trash_path).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to remove trash file {}: {}",
            trash_path.display(),
            e
        ))
    })?;

    info!(
        trash = %trash_path.display(),
        restored = %restore_path.display(),
        "File restored from trash"
    );

    Ok(restore_path)
}

/// Check total size of trash directory.
///
/// Used by `cleanup_trash` for size-based eviction and by the `list_trash` command.
///
/// # Arguments
/// * `authorized_folder` - The authorized folder whose trash size to check
///
/// # Returns
/// Total size in bytes of all files in the trash directory
///
/// # Errors
/// - `ExecutionFailed` if the trash directory cannot be read
pub fn get_trash_size(authorized_folder: &Path) -> ToolResult<u64> {
    let trash_dir = authorized_folder.join(TRASH_DIR_NAME);

    if !trash_dir.exists() {
        return Ok(0);
    }

    let entries = std::fs::read_dir(&trash_dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    let mut total_size: u64 = 0;

    for entry in entries {
        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read trash entry: {}", e))
        })?;

        let metadata = entry.metadata().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to read metadata for {}: {}",
                entry.path().display(),
                e
            ))
        })?;

        if metadata.is_file() {
            total_size = total_size.saturating_add(metadata.len());
        }
    }

    Ok(total_size)
}
