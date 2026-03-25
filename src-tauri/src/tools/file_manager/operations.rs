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

//! Filesystem operation implementations for the FileManager tool.
//!
//! Each operation is an async method on `FileManagerTool`, extracted here
//! to keep `tool.rs` focused on struct definition and trait implementation.

use crate::tools::file_manager::helpers::{
    ensure_parent_exists, format_file_info, is_text_file, DEFAULT_CONTEXT_LINES, DEFAULT_LIST_MAX,
    DEFAULT_SEARCH_MAX, MAX_CONTEXT_LINES, MAX_FILE_SIZE,
};
use crate::tools::file_manager::security::{find_authorized_folder, validate_path};
use crate::tools::file_manager::trash::{backup_before_overwrite, move_to_trash, TRASH_DIR_NAME};
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use std::path::Path;
use tracing::{debug, info};

use super::tool::FileManagerTool;

/// Context for recursive content search, avoiding too many function parameters.
struct ContentSearchContext<'a> {
    pattern: &'a str,
    compiled_regex: Option<&'a regex::Regex>,
    recursive: bool,
    max_results: usize,
    context_lines: usize,
}

impl FileManagerTool {
    /// List directory contents.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`, optional `recursive` and `max_results`
    ///
    /// # Returns
    /// JSON with `path`, `entries`, `count`, `truncated`
    ///
    /// # Errors
    /// Returns error if path is not a directory or is outside authorized folders.
    pub(crate) async fn op_list(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let recursive = input["recursive"].as_bool().unwrap_or(false);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_LIST_MAX);

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.is_dir() {
            return Err(ToolError::InvalidInput(format!(
                "Path is not a directory: {}",
                validated_path.display()
            )));
        }

        let mut entries = Vec::new();
        collect_entries(&validated_path, recursive, max_results, &mut entries)?;

        debug!(
            path = %validated_path.display(),
            count = entries.len(),
            recursive = recursive,
            "Directory listed"
        );

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "entries": entries,
            "count": entries.len(),
            "truncated": entries.len() >= max_results
        }))
    }

    /// Read file contents.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`
    ///
    /// # Returns
    /// JSON with `path`, `content`, `size`, `truncated`
    ///
    /// # Errors
    /// Returns error if file does not exist, is binary, too large, or outside authorized folders.
    pub(crate) async fn op_read(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.exists() {
            return Err(ToolError::NotFound(format!(
                "File not found: {}",
                validated_path.display()
            )));
        }

        if !validated_path.is_file() {
            return Err(ToolError::InvalidInput(format!(
                "Path is not a file: {}",
                validated_path.display()
            )));
        }

        let metadata = std::fs::metadata(&validated_path).map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to read metadata for '{}': {}",
                validated_path.display(),
                e
            ))
        })?;

        if metadata.len() > MAX_FILE_SIZE {
            return Err(ToolError::InvalidInput(format!(
                "File too large ({} bytes). Maximum allowed: {} bytes",
                metadata.len(),
                MAX_FILE_SIZE
            )));
        }

        if !is_text_file(&validated_path) {
            return Err(ToolError::InvalidInput(
                "File appears to be binary or non-UTF8".to_string(),
            ));
        }

        let content = tokio::fs::read_to_string(&validated_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read file '{}': {}",
                    validated_path.display(),
                    e
                ))
            })?;

        debug!(path = %validated_path.display(), size = metadata.len(), "File read");

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "content": content,
            "size": metadata.len(),
            "truncated": false
        }))
    }

    /// Write content to a file (creates or overwrites).
    ///
    /// If the file already exists, a backup is created in the trash directory
    /// before overwriting.
    ///
    /// # Arguments
    /// * `input` - JSON with `path` and `content`
    ///
    /// # Returns
    /// JSON with `path`, `written`, `backup` (path or null)
    ///
    /// # Errors
    /// Returns error if path is outside authorized folders or write fails.
    pub(crate) async fn op_write(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");
        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        let backup_path = if validated_path.exists() {
            let auth_folder = find_authorized_folder(&validated_path, &self.authorized_folders)
                .ok_or_else(|| {
                    ToolError::PermissionDenied(
                        "Cannot find authorized folder for backup".to_string(),
                    )
                })?;
            let backup = backup_before_overwrite(&validated_path, auth_folder)?;
            Some(backup.to_string_lossy().to_string())
        } else {
            ensure_parent_exists(&validated_path).map_err(ToolError::ExecutionFailed)?;
            None
        };

        tokio::fs::write(&validated_path, content)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to write file '{}': {}",
                    validated_path.display(),
                    e
                ))
            })?;

        info!(path = %validated_path.display(), "File written");

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "written": true,
            "backup": backup_path
        }))
    }

    /// Replace content in a file using literal or regex matching.
    ///
    /// Creates a backup before applying changes. If no matches are found,
    /// returns without modifying the file.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`, `pattern`, `replacement`, optional `is_regex`
    ///
    /// # Returns
    /// JSON with `path`, `replacements` count, `backup` (path or null)
    ///
    /// # Errors
    /// Returns error if file not found, regex invalid, or outside authorized folders.
    pub(crate) async fn op_replace(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let pattern = input["pattern"].as_str().unwrap_or("");
        let replacement = input["replacement"].as_str().unwrap_or("");
        let is_regex = input["is_regex"].as_bool().unwrap_or(false);

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.exists() {
            return Err(ToolError::NotFound(format!(
                "File not found: {}",
                validated_path.display()
            )));
        }

        let content = tokio::fs::read_to_string(&validated_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read file '{}': {}",
                    validated_path.display(),
                    e
                ))
            })?;

        let (new_content, count) = if is_regex {
            let re = regex::RegexBuilder::new(pattern)
                .size_limit(256 * 1024)
                .build()
                .map_err(|e| ToolError::InvalidInput(format!("Invalid regex pattern: {}", e)))?;

            let count = re.find_iter(&content).count();
            let replaced = re.replace_all(&content, replacement).to_string();
            (replaced, count)
        } else {
            let count = content.matches(pattern).count();
            let replaced = content.replace(pattern, replacement);
            (replaced, count)
        };

        if count == 0 {
            return Ok(json!({
                "path": validated_path.to_string_lossy(),
                "replacements": 0,
                "backup": null
            }));
        }

        let auth_folder = find_authorized_folder(&validated_path, &self.authorized_folders)
            .ok_or_else(|| {
                ToolError::PermissionDenied("Cannot find authorized folder for backup".to_string())
            })?;
        let backup = backup_before_overwrite(&validated_path, auth_folder)?;

        tokio::fs::write(&validated_path, &new_content)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to write file '{}': {}",
                    validated_path.display(),
                    e
                ))
            })?;

        info!(
            path = %validated_path.display(),
            replacements = count,
            "Content replaced"
        );

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "replacements": count,
            "backup": backup.to_string_lossy()
        }))
    }

    /// Create a new file or directory.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`, optional `create_type` ("file"|"directory"), optional `content`
    ///
    /// # Returns
    /// JSON with `path`, `created`, `type`
    ///
    /// # Errors
    /// Returns error if path already exists, invalid type, or outside authorized folders.
    pub(crate) async fn op_create(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let create_type = input["create_type"].as_str().unwrap_or("file");
        let content = input["content"].as_str().unwrap_or("");

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if validated_path.exists() {
            return Err(ToolError::InvalidInput(format!(
                "Path already exists: {}",
                validated_path.display()
            )));
        }

        match create_type {
            "file" => {
                ensure_parent_exists(&validated_path).map_err(ToolError::ExecutionFailed)?;
                tokio::fs::write(&validated_path, content)
                    .await
                    .map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed to create file '{}': {}",
                            validated_path.display(),
                            e
                        ))
                    })?;
            }
            "directory" => {
                tokio::fs::create_dir_all(&validated_path)
                    .await
                    .map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed to create directory '{}': {}",
                            validated_path.display(),
                            e
                        ))
                    })?;
            }
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Invalid create_type: '{}'. Must be 'file' or 'directory'",
                    create_type
                )));
            }
        }

        info!(
            path = %validated_path.display(),
            create_type = create_type,
            "Created"
        );

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "created": true,
            "type": create_type
        }))
    }

    /// Delete a file (moves to trash, not permanent deletion).
    ///
    /// # Arguments
    /// * `input` - JSON with `path`
    ///
    /// # Returns
    /// JSON with `path`, `deleted`, `trash_path`
    ///
    /// # Errors
    /// Returns error if path is a directory, not found, or outside authorized folders.
    pub(crate) async fn op_delete(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.exists() {
            return Err(ToolError::NotFound(format!(
                "File not found: {}",
                validated_path.display()
            )));
        }

        if !validated_path.is_file() {
            return Err(ToolError::InvalidInput(format!(
                "Only files can be deleted, not directories: {}",
                validated_path.display()
            )));
        }

        let auth_folder = find_authorized_folder(&validated_path, &self.authorized_folders)
            .ok_or_else(|| {
                ToolError::PermissionDenied("Cannot find authorized folder for trash".to_string())
            })?;

        let trash_path = move_to_trash(&validated_path, auth_folder)?;

        info!(
            path = %validated_path.display(),
            trash = %trash_path.display(),
            "File moved to trash"
        );

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "deleted": true,
            "trash_path": trash_path.to_string_lossy()
        }))
    }

    /// Move a file or directory to a new location.
    ///
    /// Tries an atomic rename first; falls back to copy+delete for cross-device moves.
    /// Cross-device directory moves are not supported.
    ///
    /// # Arguments
    /// * `input` - JSON with `path` and `destination`
    ///
    /// # Returns
    /// JSON with `source`, `destination`, `moved`
    ///
    /// # Errors
    /// Returns error if source not found, destination exists, or outside authorized folders.
    pub(crate) async fn op_move(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let dest_str = input["destination"].as_str().unwrap_or("");

        let source_path = validate_path(path_str, &self.authorized_folders)?;
        let dest_path = validate_path(dest_str, &self.authorized_folders)?;

        if !source_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Source not found: {}",
                source_path.display()
            )));
        }

        if dest_path.exists() {
            return Err(ToolError::InvalidInput(format!(
                "Destination already exists: {}",
                dest_path.display()
            )));
        }

        ensure_parent_exists(&dest_path).map_err(ToolError::ExecutionFailed)?;

        if let Err(_rename_err) = tokio::fs::rename(&source_path, &dest_path).await {
            // Cross-device move: copy then delete (files only)
            if source_path.is_file() {
                tokio::fs::copy(&source_path, &dest_path)
                    .await
                    .map_err(|e| {
                        ToolError::ExecutionFailed(format!(
                            "Failed to copy '{}' to '{}': {}",
                            source_path.display(),
                            dest_path.display(),
                            e
                        ))
                    })?;
                tokio::fs::remove_file(&source_path).await.map_err(|e| {
                    let _ = std::fs::remove_file(&dest_path);
                    ToolError::ExecutionFailed(format!(
                        "Failed to remove source after copy: {}",
                        e
                    ))
                })?;
            } else {
                return Err(ToolError::ExecutionFailed(
                    "Cross-device directory move is not supported".to_string(),
                ));
            }
        }

        info!(
            source = %source_path.display(),
            destination = %dest_path.display(),
            "File moved"
        );

        Ok(json!({
            "source": source_path.to_string_lossy(),
            "destination": dest_path.to_string_lossy(),
            "moved": true
        }))
    }

    /// Rename a file or directory.
    ///
    /// # Arguments
    /// * `input` - JSON with `path` and `new_name`
    ///
    /// # Returns
    /// JSON with `path`, `new_path`, `renamed`
    ///
    /// # Errors
    /// Returns error if path not found, name contains separators, or name already taken.
    pub(crate) async fn op_rename(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let new_name = input["new_name"].as_str().unwrap_or("");

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Path not found: {}",
                validated_path.display()
            )));
        }

        if new_name.contains('/') || new_name.contains('\\') {
            return Err(ToolError::InvalidInput(
                "New name must not contain path separators".to_string(),
            ));
        }

        if new_name.is_empty() {
            return Err(ToolError::InvalidInput(
                "New name must not be empty".to_string(),
            ));
        }

        let parent = validated_path.parent().ok_or_else(|| {
            ToolError::ExecutionFailed("Cannot determine parent directory".to_string())
        })?;
        let new_path = parent.join(new_name);

        if new_path.exists() {
            return Err(ToolError::InvalidInput(format!(
                "A file or directory with that name already exists: {}",
                new_path.display()
            )));
        }

        tokio::fs::rename(&validated_path, &new_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to rename '{}' to '{}': {}",
                    validated_path.display(),
                    new_path.display(),
                    e
                ))
            })?;

        info!(
            old = %validated_path.display(),
            new = %new_path.display(),
            "Renamed"
        );

        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "new_path": new_path.to_string_lossy(),
            "renamed": true
        }))
    }

    /// Search for files matching a glob pattern.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`, `pattern`, optional `recursive` and `max_results`
    ///
    /// # Returns
    /// JSON with `pattern`, `matches`, `count`, `truncated`
    ///
    /// # Errors
    /// Returns error if path is not a directory, glob is invalid, or outside authorized folders.
    pub(crate) async fn op_search_glob(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let pattern = input["pattern"].as_str().unwrap_or("");
        let recursive = input["recursive"].as_bool().unwrap_or(true);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_SEARCH_MAX);

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.is_dir() {
            return Err(ToolError::InvalidInput(format!(
                "Path is not a directory: {}",
                validated_path.display()
            )));
        }

        let glob = globset::GlobBuilder::new(pattern)
            .literal_separator(false)
            .build()
            .map_err(|e| ToolError::InvalidInput(format!("Invalid glob pattern: {}", e)))?
            .compile_matcher();

        let mut matches = Vec::new();
        search_glob_recursive(&validated_path, &glob, recursive, max_results, &mut matches)?;

        debug!(
            path = %validated_path.display(),
            pattern = pattern,
            matches = matches.len(),
            "Glob search completed"
        );

        Ok(json!({
            "pattern": pattern,
            "matches": matches,
            "count": matches.len(),
            "truncated": matches.len() >= max_results
        }))
    }

    /// Search file contents for a pattern.
    ///
    /// # Arguments
    /// * `input` - JSON with `path`, `pattern`, optional `is_regex`, `recursive`,
    ///   `max_results`, `context_lines`
    ///
    /// # Returns
    /// JSON with `pattern`, `matches` (with line, content, context), `count`, `truncated`
    ///
    /// # Errors
    /// Returns error if path is not a directory, regex invalid, or outside authorized folders.
    pub(crate) async fn op_search_content(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let pattern = input["pattern"].as_str().unwrap_or("");
        let is_regex = input["is_regex"].as_bool().unwrap_or(false);
        let recursive = input["recursive"].as_bool().unwrap_or(true);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_SEARCH_MAX);
        let context_lines = input["context_lines"]
            .as_u64()
            .map(|n| n as usize)
            .unwrap_or(DEFAULT_CONTEXT_LINES)
            .min(MAX_CONTEXT_LINES);

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.is_dir() {
            return Err(ToolError::InvalidInput(format!(
                "Path is not a directory: {}",
                validated_path.display()
            )));
        }

        let compiled_regex = if is_regex {
            Some(
                regex::RegexBuilder::new(pattern)
                    .size_limit(256 * 1024)
                    .build()
                    .map_err(|e| {
                        ToolError::InvalidInput(format!("Invalid regex pattern: {}", e))
                    })?,
            )
        } else {
            None
        };

        let ctx = ContentSearchContext {
            pattern,
            compiled_regex: compiled_regex.as_ref(),
            recursive,
            max_results,
            context_lines,
        };

        let mut matches = Vec::new();
        search_content_recursive(&validated_path, &ctx, &mut matches)?;

        debug!(
            path = %validated_path.display(),
            pattern = pattern,
            matches = matches.len(),
            "Content search completed"
        );

        Ok(json!({
            "pattern": pattern,
            "matches": matches,
            "count": matches.len(),
            "truncated": matches.len() >= max_results
        }))
    }
}

// ---------------------------------------------------------------------------
// Free helper functions (no &self needed)
// ---------------------------------------------------------------------------

/// Recursively collect directory entries, filtering out the trash directory.
fn collect_entries(
    dir: &Path,
    recursive: bool,
    max_results: usize,
    entries: &mut Vec<Value>,
) -> ToolResult<()> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read directory '{}': {}",
            dir.display(),
            e
        ))
    })?;

    for entry in read_dir {
        if entries.len() >= max_results {
            break;
        }

        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read directory entry: {}", e))
        })?;

        let entry_path = entry.path();

        if is_trash_dir(&entry_path) {
            continue;
        }

        let metadata = entry.metadata().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to read metadata for '{}': {}",
                entry_path.display(),
                e
            ))
        })?;

        entries.push(format_file_info(&entry_path, &metadata));

        if recursive && metadata.is_dir() && entries.len() < max_results {
            collect_entries(&entry_path, true, max_results, entries)?;
        }
    }

    Ok(())
}

/// Recursively search for files matching a glob pattern.
fn search_glob_recursive(
    dir: &Path,
    glob: &globset::GlobMatcher,
    recursive: bool,
    max_results: usize,
    matches: &mut Vec<Value>,
) -> ToolResult<()> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read directory '{}': {}",
            dir.display(),
            e
        ))
    })?;

    for entry in read_dir {
        if matches.len() >= max_results {
            break;
        }

        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read directory entry: {}", e))
        })?;

        let entry_path = entry.path();

        if is_trash_dir(&entry_path) {
            continue;
        }

        if let Some(file_name) = entry_path.file_name() {
            if glob.is_match(file_name) {
                let metadata = entry.metadata().map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to read metadata: {}", e))
                })?;
                matches.push(format_file_info(&entry_path, &metadata));
            }
        }

        if recursive && entry_path.is_dir() && matches.len() < max_results {
            search_glob_recursive(&entry_path, glob, true, max_results, matches)?;
        }
    }

    Ok(())
}

/// Recursively search file contents for a pattern, collecting matches with context.
fn search_content_recursive(
    dir: &Path,
    ctx: &ContentSearchContext<'_>,
    matches: &mut Vec<Value>,
) -> ToolResult<()> {
    let read_dir = std::fs::read_dir(dir).map_err(|e| {
        ToolError::ExecutionFailed(format!(
            "Failed to read directory '{}': {}",
            dir.display(),
            e
        ))
    })?;

    for entry in read_dir {
        if matches.len() >= ctx.max_results {
            break;
        }

        let entry = entry.map_err(|e| {
            ToolError::ExecutionFailed(format!("Failed to read directory entry: {}", e))
        })?;

        let entry_path = entry.path();

        if is_trash_dir(&entry_path) {
            continue;
        }

        if entry_path.is_file() && is_text_file(&entry_path) {
            if let Ok(content) = std::fs::read_to_string(&entry_path) {
                search_file_content(&entry_path, &content, ctx, matches);
            }
        } else if ctx.recursive && entry_path.is_dir() && matches.len() < ctx.max_results {
            search_content_recursive(&entry_path, ctx, matches)?;
        }
    }

    Ok(())
}

/// Search a single file's content for pattern matches, appending results with context.
fn search_file_content(
    path: &Path,
    content: &str,
    ctx: &ContentSearchContext<'_>,
    matches: &mut Vec<Value>,
) {
    let lines: Vec<&str> = content.lines().collect();

    for (line_idx, line) in lines.iter().enumerate() {
        if matches.len() >= ctx.max_results {
            break;
        }

        let is_match = if let Some(re) = ctx.compiled_regex {
            re.is_match(line)
        } else {
            line.contains(ctx.pattern)
        };

        if is_match {
            let start = line_idx.saturating_sub(ctx.context_lines);
            let end = (line_idx + ctx.context_lines + 1).min(lines.len());

            let context_before: Vec<String> = lines[start..line_idx]
                .iter()
                .map(|s| s.to_string())
                .collect();
            let context_after: Vec<String> = lines[(line_idx + 1).min(lines.len())..end]
                .iter()
                .map(|s| s.to_string())
                .collect();

            matches.push(json!({
                "path": path.to_string_lossy(),
                "line": line_idx + 1,
                "content": line,
                "context_before": context_before,
                "context_after": context_after
            }));
        }
    }
}

/// Check if a path is the `.zileo-trash` directory.
fn is_trash_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name == TRASH_DIR_NAME)
}
