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
use tracing::{info, warn};

/// Name of the trash directory.
pub const TRASH_DIR_NAME: &str = ".zileo-trash";

/// Default retention period in days.
pub const DEFAULT_RETENTION_DAYS: u64 = 30;

/// Maximum trash size in bytes (100 MB).
/// Used by `cleanup_trash` for size-based eviction after time-based cleanup.
pub const MAX_TRASH_SIZE: u64 = 100 * 1024 * 1024;

/// Separator used to encode path separators in trash filenames.
const PATH_SEPARATOR_REPLACEMENT: &str = "__";

/// Separator between timestamp and original path in trash filenames.
const TRASH_NAME_SEPARATOR: &str = "_";

/// Timestamp format for trash filenames (filesystem-safe ISO 8601).
const TRASH_TIMESTAMP_FORMAT: &str = "%Y-%m-%dT%H-%M-%S";

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

/// Validate that a timestamp string matches the expected trash timestamp format.
fn is_valid_trash_timestamp(s: &str) -> bool {
    if s.len() != 19 {
        return false;
    }
    chrono::NaiveDateTime::parse_from_str(s, TRASH_TIMESTAMP_FORMAT).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a temp dir in $HOME to avoid /tmp permission issues.
    fn create_test_dir() -> TempDir {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        TempDir::new_in(&home).expect("Failed to create temp dir")
    }

    #[test]
    fn test_sanitize_path_for_trash() {
        // Simple filename - no separators
        assert_eq!(
            sanitize_path_for_trash(Path::new("myfile.txt")),
            "myfile.txt"
        );

        // Single directory depth
        assert_eq!(
            sanitize_path_for_trash(Path::new("subdir/file.rs")),
            "subdir__file.rs"
        );

        // Deep nested path
        assert_eq!(
            sanitize_path_for_trash(Path::new("a/b/c/deep.txt")),
            "a__b__c__deep.txt"
        );

        // Empty components are handled
        assert_eq!(sanitize_path_for_trash(Path::new("dir/file")), "dir__file");
    }

    #[test]
    fn test_parse_trash_filename() {
        // Valid filename
        let result = parse_trash_filename("2026-03-01T10-30-00_myfile.txt");
        assert!(result.is_some());
        let (ts, path) = result.unwrap();
        assert_eq!(ts, "2026-03-01T10-30-00");
        assert_eq!(path, "myfile.txt");

        // Valid with nested path separators
        let result = parse_trash_filename("2026-03-01T10-31-00_subdir__nested__file.rs");
        assert!(result.is_some());
        let (ts, path) = result.unwrap();
        assert_eq!(ts, "2026-03-01T10-31-00");
        assert_eq!(path, "subdir__nested__file.rs");

        // Too short
        assert!(parse_trash_filename("short").is_none());

        // Invalid timestamp format
        assert!(parse_trash_filename("not-a-timestampXX_file.txt").is_none());

        // Missing separator after timestamp
        assert!(parse_trash_filename("2026-03-01T10-30-00Xfile.txt").is_none());

        // Empty path after timestamp
        assert!(parse_trash_filename("2026-03-01T10-30-00_").is_none());
    }

    #[test]
    fn test_generate_trash_name() {
        let name = generate_trash_name(Path::new("myfile.txt"));

        // Should contain underscore separator
        assert!(name.contains('_'));

        // Should end with the sanitized filename
        assert!(name.ends_with("myfile.txt"));

        // Should be parseable
        let parsed = parse_trash_filename(&name);
        assert!(parsed.is_some());
        let (_, path) = parsed.unwrap();
        assert_eq!(path, "myfile.txt");
    }

    #[test]
    fn test_generate_trash_name_nested_path() {
        let name = generate_trash_name(Path::new("sub/nested/file.rs"));

        // Should contain encoded path separators
        assert!(name.contains("sub__nested__file.rs"));

        // Should be parseable
        let parsed = parse_trash_filename(&name);
        assert!(parsed.is_some());
        let (_, path) = parsed.unwrap();
        assert_eq!(path, "sub__nested__file.rs");
    }

    #[test]
    fn test_move_to_trash_creates_dir_and_moves() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create a file to trash
        let file_path = base_path.join("testfile.txt");
        fs::write(&file_path, "hello world").unwrap();

        // Move to trash
        let result = move_to_trash(&file_path, base_path);
        assert!(result.is_ok());

        let trash_path = result.unwrap();

        // Original should be gone
        assert!(!file_path.exists());

        // Trash file should exist
        assert!(trash_path.exists());

        // Trash dir should exist
        assert!(base_path.join(TRASH_DIR_NAME).exists());

        // Content should be preserved
        let content = fs::read_to_string(&trash_path).unwrap();
        assert_eq!(content, "hello world");
    }

    #[test]
    fn test_move_to_trash_nonexistent_file() {
        let base = create_test_dir();
        let base_path = base.path();

        let nonexistent = base_path.join("does_not_exist.txt");
        let result = move_to_trash(&nonexistent, base_path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_backup_before_overwrite() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create a file to back up
        let file_path = base_path.join("config.toml");
        fs::write(&file_path, "original content").unwrap();

        // Create backup
        let result = backup_before_overwrite(&file_path, base_path);
        assert!(result.is_ok());

        let backup_path = result.unwrap();

        // Original should still exist (not removed by backup)
        assert!(file_path.exists());

        // Backup should exist
        assert!(backup_path.exists());

        // Backup should have .bak suffix
        let backup_name = backup_path.file_name().unwrap().to_string_lossy();
        assert!(backup_name.ends_with(".bak"));

        // Backup content should match original
        let backup_content = fs::read_to_string(&backup_path).unwrap();
        assert_eq!(backup_content, "original content");
    }

    #[test]
    fn test_cleanup_trash_removes_old_entries() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create trash dir with an old entry (simulate old timestamp)
        let trash_dir = base_path.join(TRASH_DIR_NAME);
        fs::create_dir_all(&trash_dir).unwrap();

        // Create a file with a very old timestamp (2020)
        let old_name = "2020-01-01T00-00-00_oldfile.txt";
        fs::write(trash_dir.join(old_name), "old content").unwrap();

        // Cleanup with 30-day retention
        let result = cleanup_trash(base_path, 30);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 1);

        // Old file should be gone
        assert!(!trash_dir.join(old_name).exists());
    }

    #[test]
    fn test_cleanup_trash_keeps_recent() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create trash dir with a recent entry
        let trash_dir = base_path.join(TRASH_DIR_NAME);
        fs::create_dir_all(&trash_dir).unwrap();

        // Create a file with current timestamp
        let recent_name = generate_trash_name(Path::new("recent.txt"));
        fs::write(trash_dir.join(&recent_name), "recent content").unwrap();

        // Cleanup with 30-day retention
        let result = cleanup_trash(base_path, 30);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0);

        // Recent file should still be there
        assert!(trash_dir.join(&recent_name).exists());
    }

    #[test]
    fn test_list_trash_entries() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create trash dir with entries
        let trash_dir = base_path.join(TRASH_DIR_NAME);
        fs::create_dir_all(&trash_dir).unwrap();

        let name1 = "2026-03-01T10-30-00_file1.txt";
        let name2 = "2026-03-01T10-31-00_subdir__file2.rs";
        fs::write(trash_dir.join(name1), "content1").unwrap();
        fs::write(trash_dir.join(name2), "content two").unwrap();

        let result = list_trash_entries(base_path);
        assert!(result.is_ok());

        let entries = result.unwrap();
        assert_eq!(entries.len(), 2);

        // Should be sorted by timestamp descending
        assert_eq!(entries[0].deleted_at, "2026-03-01T10-31-00");
        assert_eq!(entries[1].deleted_at, "2026-03-01T10-30-00");

        // Check original path reconstruction
        // The second entry (index 1) has the earlier timestamp
        let file1_entry = entries
            .iter()
            .find(|e| e.deleted_at == "2026-03-01T10-30-00")
            .unwrap();
        assert_eq!(file1_entry.original_relative_path, "file1.txt");
        assert_eq!(file1_entry.size_bytes, 8); // "content1" = 8 bytes

        let file2_entry = entries
            .iter()
            .find(|e| e.deleted_at == "2026-03-01T10-31-00")
            .unwrap();
        // Path separator replacement depends on OS
        let expected_sep = std::path::MAIN_SEPARATOR_STR;
        assert_eq!(
            file2_entry.original_relative_path,
            format!("subdir{}file2.rs", expected_sep)
        );
        assert_eq!(file2_entry.size_bytes, 11); // "content two" = 11 bytes
    }

    #[test]
    fn test_list_trash_empty() {
        let base = create_test_dir();
        let base_path = base.path();

        // No trash dir exists at all
        let result = list_trash_entries(base_path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());

        // Create empty trash dir
        fs::create_dir_all(base_path.join(TRASH_DIR_NAME)).unwrap();
        let result = list_trash_entries(base_path);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_restore_from_trash() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create a file, move it to trash, then restore
        let file_path = base_path.join("restore_me.txt");
        fs::write(&file_path, "restore this content").unwrap();

        let trash_path = move_to_trash(&file_path, base_path).unwrap();
        assert!(!file_path.exists());
        assert!(trash_path.exists());

        // Restore
        let result = restore_from_trash(&trash_path, base_path);
        assert!(result.is_ok());

        let restored_path = result.unwrap();
        assert_eq!(restored_path, file_path);

        // Restored file should exist with original content
        assert!(restored_path.exists());
        let content = fs::read_to_string(&restored_path).unwrap();
        assert_eq!(content, "restore this content");

        // Trash file should be gone
        assert!(!trash_path.exists());
    }

    #[test]
    fn test_restore_nonexistent_trash() {
        let base = create_test_dir();
        let base_path = base.path();

        let fake_trash = base_path
            .join(TRASH_DIR_NAME)
            .join("2026-03-01T10-30-00_ghost.txt");

        let result = restore_from_trash(&fake_trash, base_path);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
        assert!(err.to_string().contains("does not exist"));
    }

    #[test]
    fn test_get_trash_size() {
        let base = create_test_dir();
        let base_path = base.path();

        // No trash dir -> 0
        assert_eq!(get_trash_size(base_path).unwrap(), 0);

        // Create trash with known sizes
        let trash_dir = base_path.join(TRASH_DIR_NAME);
        fs::create_dir_all(&trash_dir).unwrap();

        fs::write(
            trash_dir.join("2026-03-01T10-30-00_a.txt"),
            "12345", // 5 bytes
        )
        .unwrap();
        fs::write(
            trash_dir.join("2026-03-01T10-31-00_b.txt"),
            "1234567890", // 10 bytes
        )
        .unwrap();

        let size = get_trash_size(base_path).unwrap();
        assert_eq!(size, 15);
    }

    #[test]
    fn test_nested_path_trash() {
        let base = create_test_dir();
        let base_path = base.path();

        // Create nested directory structure
        let nested_dir = base_path.join("src").join("utils");
        fs::create_dir_all(&nested_dir).unwrap();

        let file_path = nested_dir.join("helper.rs");
        fs::write(&file_path, "fn helper() {}").unwrap();

        // Move to trash
        let trash_path = move_to_trash(&file_path, base_path).unwrap();
        assert!(!file_path.exists());
        assert!(trash_path.exists());

        // Verify the trash filename contains encoded path
        let trash_name = trash_path.file_name().unwrap().to_string_lossy();
        assert!(trash_name.contains("src__utils__helper.rs"));

        // Restore and verify the directory structure is recreated
        let restored = restore_from_trash(&trash_path, base_path).unwrap();
        assert!(restored.exists());
        assert_eq!(fs::read_to_string(&restored).unwrap(), "fn helper() {}");

        // Should be in the same nested location
        assert!(restored.starts_with(&nested_dir));
    }
}
