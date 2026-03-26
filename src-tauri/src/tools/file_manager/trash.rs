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

//! Trash Protection System for FileManager.
//!
//! Provides safe file deletion with trash-based recovery.
//! Files are moved to a `.zileo-trash/` directory within each authorized folder
//! before deletion, allowing restoration within a configurable retention period.
//!
//! ## Trash naming convention
//!
//! Trash files follow the pattern: `{ISO_timestamp}_{sanitized_relative_path}`
//! - Timestamp format: `%Y-%m-%dT%H-%M-%S` (filesystem-safe)
//! - Path separators replaced with `__` (double underscore)
//!
//! ## Example
//!
//! ```text
//! /home/user/project/
//!   .zileo-trash/
//!     2026-03-01T10-30-00_myfile.txt
//!     2026-03-01T10-31-00_subdir__nested__file.rs
//! ```

use crate::tools::{ToolError, ToolResult};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tracing::info;

/// Name of the trash directory.
pub const TRASH_DIR_NAME: &str = ".zileo-trash";

/// Default retention period in days.
pub const DEFAULT_RETENTION_DAYS: u64 = 30;

/// Maximum trash size in bytes (100 MB).
/// Used by `cleanup_trash` for size-based eviction after time-based cleanup.
pub const MAX_TRASH_SIZE: u64 = 100 * 1024 * 1024;

/// Separator used to encode path separators in trash filenames.
pub(crate) const PATH_SEPARATOR_REPLACEMENT: &str = "__";

/// Separator between timestamp and original path in trash filenames.
pub(crate) const TRASH_NAME_SEPARATOR: &str = "_";

/// Timestamp format for trash filenames (filesystem-safe ISO 8601).
pub(crate) const TRASH_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H-%M-%S";

/// A single entry in the trash directory.
#[derive(Debug, Clone, Serialize)]
pub struct TrashEntry {
    /// Full path to the file in the trash directory.
    pub trash_path: PathBuf,
    /// Original relative path from the authorized folder.
    pub original_relative_path: String,
    /// ISO timestamp when the file was deleted.
    pub deleted_at: String,
    /// Size of the trash file in bytes.
    pub size_bytes: u64,
}

/// Sanitize a relative path for use in trash filenames.
///
/// Replaces all path separators (both `/` and `\`) with `__` (double underscore).
///
/// # Arguments
/// * `relative_path` - The relative path to sanitize
///
/// # Returns
/// A string with path separators replaced by `__`
pub fn sanitize_path_for_trash(relative_path: &Path) -> String {
    relative_path
        .to_string_lossy()
        .replace(['/', '\\'], PATH_SEPARATOR_REPLACEMENT)
}

/// Parse a trash filename into its timestamp and original sanitized path components.
///
/// Expected format: `{YYYY-MM-DDTHH-MM-SS}_{sanitized_path}`
///
/// # Arguments
/// * `filename` - The trash filename to parse
///
/// # Returns
/// `Some((timestamp, sanitized_original_path))` if parsing succeeds, `None` otherwise
pub fn parse_trash_filename(filename: &str) -> Option<(String, String)> {
    // Timestamp format: YYYY-MM-DDTHH-MM-SS = 19 characters
    // Minimum filename: 19 chars timestamp + 1 char separator + 1 char path = 21
    if filename.len() < 21 {
        return None;
    }

    let timestamp = &filename[..19];

    // Validate timestamp format: YYYY-MM-DDTHH-MM-SS
    if !is_valid_trash_timestamp(timestamp) {
        return None;
    }

    // The separator after the timestamp
    if &filename[19..20] != TRASH_NAME_SEPARATOR {
        return None;
    }

    let original_path = &filename[20..];
    if original_path.is_empty() {
        return None;
    }

    Some((timestamp.to_string(), original_path.to_string()))
}

/// Generate a timestamped trash filename for a given relative path.
///
/// # Arguments
/// * `relative_path` - The original relative path of the file
///
/// # Returns
/// A filename in the format `{ISO_timestamp}_{sanitized_path}`
pub fn generate_trash_name(relative_path: &Path) -> String {
    let timestamp = chrono::Local::now().format(TRASH_TIMESTAMP_FORMAT);
    let sanitized = sanitize_path_for_trash(relative_path);
    format!("{}{}{}", timestamp, TRASH_NAME_SEPARATOR, sanitized)
}

/// Move a file to the trash directory before deletion.
///
/// Creates the trash directory if it does not exist. The file is copied to the
/// trash with a timestamped name, then the original is removed.
///
/// # Arguments
/// * `file_path` - Absolute path to the file to trash
/// * `authorized_folder` - The authorized folder containing the file
///
/// # Returns
/// The path to the trash copy
///
/// # Errors
/// - `InvalidInput` if the file does not exist or is not within the authorized folder
/// - `ExecutionFailed` if the copy or delete operation fails
pub fn move_to_trash(file_path: &Path, authorized_folder: &Path) -> ToolResult<PathBuf> {
    // Validate file exists
    if !file_path.exists() {
        return Err(ToolError::InvalidInput(format!(
            "File does not exist: {}",
            file_path.display()
        )));
    }

    // Validate file is within authorized folder
    let canonical_file = file_path.canonicalize().map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to canonicalize file path {}: {}",
            file_path.display(),
            e
        ))
    })?;
    let canonical_folder = authorized_folder.canonicalize().map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to canonicalize folder path {}: {}",
            authorized_folder.display(),
            e
        ))
    })?;

    if !canonical_file.starts_with(&canonical_folder) {
        return Err(ToolError::InvalidInput(format!(
            "File {} is not within authorized folder {}",
            file_path.display(),
            authorized_folder.display()
        )));
    }

    // Compute relative path from authorized folder
    let relative_path = canonical_file
        .strip_prefix(&canonical_folder)
        .map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to compute relative path: {}", e))
        })?;

    // Create trash directory
    let trash_dir = canonical_folder.join(TRASH_DIR_NAME);
    std::fs::create_dir_all(&trash_dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to create trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    // Generate trash filename and copy
    let trash_name = generate_trash_name(relative_path);
    let trash_path = trash_dir.join(&trash_name);

    std::fs::copy(&canonical_file, &trash_path).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to copy file to trash {}: {}",
            trash_path.display(),
            e
        ))
    })?;

    // Remove original
    std::fs::remove_file(&canonical_file).map_err(|e| {
        // Try to clean up the trash copy if the original removal fails
        let _ = std::fs::remove_file(&trash_path);
        ToolError::ExecutionFailed(format!(
            "Failed to remove original file {}: {}",
            canonical_file.display(),
            e
        ))
    })?;

    info!(
        file = %file_path.display(),
        trash = %trash_path.display(),
        "File moved to trash"
    );

    Ok(trash_path)
}

/// Create a backup of a file before overwriting.
///
/// Copies the file to the trash directory with a `.bak` suffix.
/// The original file is NOT removed (it will be overwritten by the caller).
///
/// # Arguments
/// * `file_path` - Absolute path to the file to back up
/// * `authorized_folder` - The authorized folder containing the file
///
/// # Returns
/// The path to the backup copy
///
/// # Errors
/// - `InvalidInput` if the file does not exist or is not within the authorized folder
/// - `ExecutionFailed` if the copy operation fails
pub fn backup_before_overwrite(file_path: &Path, authorized_folder: &Path) -> ToolResult<PathBuf> {
    // Validate file exists
    if !file_path.exists() {
        return Err(ToolError::InvalidInput(format!(
            "File does not exist: {}",
            file_path.display()
        )));
    }

    // Validate file is within authorized folder
    let canonical_file = file_path.canonicalize().map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to canonicalize file path {}: {}",
            file_path.display(),
            e
        ))
    })?;
    let canonical_folder = authorized_folder.canonicalize().map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to canonicalize folder path {}: {}",
            authorized_folder.display(),
            e
        ))
    })?;

    if !canonical_file.starts_with(&canonical_folder) {
        return Err(ToolError::InvalidInput(format!(
            "File {} is not within authorized folder {}",
            file_path.display(),
            authorized_folder.display()
        )));
    }

    // Compute relative path
    let relative_path = canonical_file
        .strip_prefix(&canonical_folder)
        .map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to compute relative path: {}", e))
        })?;

    // Create trash directory
    let trash_dir = canonical_folder.join(TRASH_DIR_NAME);
    std::fs::create_dir_all(&trash_dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to create trash directory {}: {}",
            trash_dir.display(),
            e
        ))
    })?;

    // Generate backup filename with .bak suffix
    let trash_name = format!("{}.bak", generate_trash_name(relative_path));
    let backup_path = trash_dir.join(&trash_name);

    std::fs::copy(&canonical_file, &backup_path).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to create backup at {}: {}",
            backup_path.display(),
            e
        ))
    })?;

    info!(
        file = %file_path.display(),
        backup = %backup_path.display(),
        "Backup created before overwrite"
    );

    Ok(backup_path)
}

/// Validate that a timestamp string matches the expected trash timestamp format.
fn is_valid_trash_timestamp(s: &str) -> bool {
    if s.len() != 19 {
        return false;
    }
    chrono::NaiveDateTime::parse_from_str(s, TRASH_TIMESTAMP_FORMAT).is_ok()
}

#[cfg(test)]
#[path = "trash_tests.rs"]
mod tests;
