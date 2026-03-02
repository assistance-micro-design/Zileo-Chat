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

//! FileManager Tool implementation.
//!
//! Provides 10 filesystem operations within sandboxed (authorized) directories:
//! list, read, write, replace, create, delete, move, rename, search_glob, search_content.
//!
//! All paths are validated against the agent's configured authorized folders
//! before any operation is performed.

use crate::tools::file_manager::helpers::{
    ensure_parent_exists, format_file_info, is_text_file, DEFAULT_CONTEXT_LINES, DEFAULT_LIST_MAX,
    DEFAULT_SEARCH_MAX, MAX_CONTEXT_LINES, MAX_FILE_SIZE,
};
use crate::tools::file_manager::security::{find_authorized_folder, validate_path};
use crate::tools::file_manager::trash::{
    backup_before_overwrite, cleanup_trash, move_to_trash, TRASH_DIR_NAME,
};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::{debug, info, warn};

/// FileManager tool for sandboxed filesystem operations.
///
/// All operations are restricted to the agent's configured authorized folders.
/// Destructive operations use trash-based safety (no permanent deletion).
pub struct FileManagerTool {
    authorized_folders: Vec<PathBuf>,
    cleanup_done: AtomicBool,
}

impl FileManagerTool {
    /// Creates a new FileManagerTool with the given authorized folders.
    ///
    /// Trash cleanup is deferred to the first destructive operation to avoid
    /// adding latency on tool creation (which happens every LLM turn).
    ///
    /// # Arguments
    /// * `authorized_folders` - Canonical paths to authorized directories
    pub fn new(authorized_folders: Vec<PathBuf>) -> Self {
        Self {
            authorized_folders,
            cleanup_done: AtomicBool::new(false),
        }
    }

    /// Run trash cleanup once (on first destructive operation).
    ///
    /// Uses an atomic flag to ensure cleanup runs at most once per tool instance.
    /// Warnings are logged but never fatal.
    fn ensure_cleanup(&self) {
        if self
            .cleanup_done
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            for folder in &self.authorized_folders {
                if let Err(e) = cleanup_trash(
                    folder,
                    crate::tools::file_manager::trash::DEFAULT_RETENTION_DAYS,
                ) {
                    warn!(folder = %folder.display(), error = %e, "Trash cleanup failed");
                }
            }
        }
    }

    /// List directory contents.
    async fn op_list(&self, input: &Value) -> ToolResult<Value> {
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
        self.collect_entries(&validated_path, recursive, max_results, &mut entries)?;

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

    /// Recursively collect directory entries, filtering out trash directory.
    fn collect_entries(
        &self,
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

            // Filter out .zileo-trash/ directory
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                if name == TRASH_DIR_NAME {
                    continue;
                }
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
                self.collect_entries(&entry_path, true, max_results, entries)?;
            }
        }

        Ok(())
    }

    /// Read file contents.
    async fn op_read(&self, input: &Value) -> ToolResult<Value> {
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

        // Check file size
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

        // Check if text file
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
    async fn op_write(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let content = input["content"].as_str().unwrap_or("");
        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        // If file exists, create a backup before overwriting
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
    async fn op_replace(&self, input: &Value) -> ToolResult<Value> {
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

        // Read current content
        let content = tokio::fs::read_to_string(&validated_path)
            .await
            .map_err(|e| {
                ToolError::ExecutionFailed(format!(
                    "Failed to read file '{}': {}",
                    validated_path.display(),
                    e
                ))
            })?;

        // Perform replacement
        let (new_content, count) = if is_regex {
            // Build regex with size limit for ReDoS safety
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

        // Create backup before writing changes
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
    async fn op_create(&self, input: &Value) -> ToolResult<Value> {
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
    async fn op_delete(&self, input: &Value) -> ToolResult<Value> {
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
    async fn op_move(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let dest_str = input["destination"].as_str().unwrap_or("");

        // Validate both source and destination
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

        // Try rename first (same filesystem), fall back to copy+delete
        match tokio::fs::rename(&source_path, &dest_path).await {
            Ok(()) => {}
            Err(_) => {
                // Cross-device move: copy then delete
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
                        // Try to clean up destination on failure
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
    async fn op_rename(&self, input: &Value) -> ToolResult<Value> {
        let path_str = input["path"].as_str().unwrap_or("");
        let new_name = input["new_name"].as_str().unwrap_or("");

        let validated_path = validate_path(path_str, &self.authorized_folders)?;

        if !validated_path.exists() {
            return Err(ToolError::NotFound(format!(
                "Path not found: {}",
                validated_path.display()
            )));
        }

        // Validate new_name doesn't contain path separators
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

        // Build new path (same parent + new name)
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
    async fn op_search_glob(&self, input: &Value) -> ToolResult<Value> {
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
        self.search_glob_recursive(&validated_path, &glob, recursive, max_results, &mut matches)?;

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

    /// Recursively search for files matching a glob.
    fn search_glob_recursive(
        &self,
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

            // Skip trash directory
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                if name == TRASH_DIR_NAME {
                    continue;
                }
            }

            // Match against the file name
            if let Some(file_name) = entry_path.file_name() {
                if glob.is_match(file_name) {
                    let metadata = entry.metadata().map_err(|e| {
                        ToolError::ExecutionFailed(format!("Failed to read metadata: {}", e))
                    })?;
                    matches.push(format_file_info(&entry_path, &metadata));
                }
            }

            if recursive && entry_path.is_dir() && matches.len() < max_results {
                self.search_glob_recursive(&entry_path, glob, true, max_results, matches)?;
            }
        }

        Ok(())
    }

    /// Search file contents for a pattern.
    async fn op_search_content(&self, input: &Value) -> ToolResult<Value> {
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

        // Compile regex if needed (with size limit for ReDoS safety)
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

        let mut matches = Vec::new();
        self.search_content_recursive(
            &validated_path,
            pattern,
            compiled_regex.as_ref(),
            recursive,
            max_results,
            context_lines,
            &mut matches,
        )?;

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

    /// Recursively search file contents.
    #[allow(clippy::too_many_arguments)]
    fn search_content_recursive(
        &self,
        dir: &Path,
        pattern: &str,
        compiled_regex: Option<&regex::Regex>,
        recursive: bool,
        max_results: usize,
        context_lines: usize,
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

            // Skip trash directory
            if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                if name == TRASH_DIR_NAME {
                    continue;
                }
            }

            if entry_path.is_file() && is_text_file(&entry_path) {
                // Read and search file
                if let Ok(content) = std::fs::read_to_string(&entry_path) {
                    let lines: Vec<&str> = content.lines().collect();

                    for (line_idx, line) in lines.iter().enumerate() {
                        if matches.len() >= max_results {
                            break;
                        }

                        let is_match = if let Some(re) = compiled_regex {
                            re.is_match(line)
                        } else {
                            line.contains(pattern)
                        };

                        if is_match {
                            let start = line_idx.saturating_sub(context_lines);
                            let end = (line_idx + context_lines + 1).min(lines.len());

                            let context_before: Vec<String> = lines[start..line_idx]
                                .iter()
                                .map(|s| s.to_string())
                                .collect();
                            let context_after: Vec<String> = lines
                                [(line_idx + 1).min(lines.len())..end]
                                .iter()
                                .map(|s| s.to_string())
                                .collect();

                            matches.push(json!({
                                "path": entry_path.to_string_lossy(),
                                "line": line_idx + 1,
                                "content": line,
                                "context_before": context_before,
                                "context_after": context_after
                            }));
                        }
                    }
                }
            } else if recursive && entry_path.is_dir() && matches.len() < max_results {
                self.search_content_recursive(
                    &entry_path,
                    pattern,
                    compiled_regex,
                    true,
                    max_results,
                    context_lines,
                    matches,
                )?;
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Tool for FileManagerTool {
    fn definition(&self) -> ToolDefinition {
        let folders_list: Vec<String> = self
            .authorized_folders
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        let folders_desc = if folders_list.is_empty() {
            "No authorized directories configured.".to_string()
        } else {
            format!("Authorized directories: {}", folders_list.join(", "))
        };

        ToolDefinition {
            id: "FileManagerTool".to_string(),
            name: "File Manager".to_string(),
            description: format!(
                "Manage files within authorized directories. Operations: list, read, \
                write, replace, create, delete, move, rename, search_glob, search_content. \
                All paths must be within the authorized folders. {}",
                folders_desc
            ),
            input_schema: json!({
                "type": "object",
                "required": ["operation"],
                "properties": {
                    "operation": {
                        "type": "string",
                        "enum": ["list", "read", "write", "replace", "create", "delete",
                                 "move", "rename", "search_glob", "search_content"]
                    },
                    "path": { "type": "string", "description": "File or directory path" },
                    "content": { "type": "string", "description": "File content for write/create" },
                    "destination": { "type": "string", "description": "Destination path for move" },
                    "new_name": { "type": "string", "description": "New name for rename" },
                    "pattern": { "type": "string", "description": "Search pattern (glob or text/regex)" },
                    "replacement": { "type": "string", "description": "Replacement text for replace" },
                    "is_regex": { "type": "boolean", "description": "Whether pattern is regex (for replace/search_content)" },
                    "recursive": { "type": "boolean", "description": "Recursive listing/search" },
                    "max_results": { "type": "integer", "description": "Max results (default: 500 for list, 100 for search)" },
                    "context_lines": { "type": "integer", "description": "Context lines for search_content (default: 3, max: 10)" },
                    "create_type": { "type": "string", "enum": ["file", "directory"], "description": "Type to create" }
                }
            }),
            output_schema: json!({"type": "object"}),
            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let operation = input["operation"].as_str().unwrap_or("");

        // Run lazy trash cleanup before any destructive operation
        if matches!(
            operation,
            "write" | "replace" | "delete" | "move" | "rename"
        ) {
            self.ensure_cleanup();
        }

        match operation {
            "list" => self.op_list(&input).await,
            "read" => self.op_read(&input).await,
            "write" => self.op_write(&input).await,
            "replace" => self.op_replace(&input).await,
            "create" => self.op_create(&input).await,
            "delete" => self.op_delete(&input).await,
            "move" => self.op_move(&input).await,
            "rename" => self.op_rename(&input).await,
            "search_glob" => self.op_search_glob(&input).await,
            "search_content" => self.op_search_content(&input).await,
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                operation
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let operation = input
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| {
                ToolError::InvalidInput("Missing required field: 'operation'".to_string())
            })?;

        let valid_ops = [
            "list",
            "read",
            "write",
            "replace",
            "create",
            "delete",
            "move",
            "rename",
            "search_glob",
            "search_content",
        ];
        if !valid_ops.contains(&operation) {
            return Err(ToolError::InvalidInput(format!(
                "Invalid operation: '{}'. Must be one of: {}",
                operation,
                valid_ops.join(", ")
            )));
        }

        // Validate required params per operation
        match operation {
            "list" | "read" | "delete" => {
                if input.get("path").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Operation '{}' requires 'path' parameter",
                        operation
                    )));
                }
            }
            "write" => {
                if input.get("path").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(
                        "Write requires 'path' parameter".to_string(),
                    ));
                }
                if input.get("content").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(
                        "Write requires 'content' parameter".to_string(),
                    ));
                }
            }
            "replace" => {
                for field in &["path", "pattern", "replacement"] {
                    if input.get(*field).and_then(|v| v.as_str()).is_none() {
                        return Err(ToolError::InvalidInput(format!(
                            "Replace requires '{}' parameter",
                            field
                        )));
                    }
                }
            }
            "create" => {
                if input.get("path").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(
                        "Create requires 'path' parameter".to_string(),
                    ));
                }
            }
            "move" => {
                if input.get("path").and_then(|v| v.as_str()).is_none()
                    || input.get("destination").and_then(|v| v.as_str()).is_none()
                {
                    return Err(ToolError::InvalidInput(
                        "Move requires 'path' and 'destination' parameters".to_string(),
                    ));
                }
            }
            "rename" => {
                if input.get("path").and_then(|v| v.as_str()).is_none()
                    || input.get("new_name").and_then(|v| v.as_str()).is_none()
                {
                    return Err(ToolError::InvalidInput(
                        "Rename requires 'path' and 'new_name' parameters".to_string(),
                    ));
                }
            }
            "search_glob" | "search_content" => {
                if input.get("path").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Operation '{}' requires 'path' parameter",
                        operation
                    )));
                }
                if input.get("pattern").and_then(|v| v.as_str()).is_none() {
                    return Err(ToolError::InvalidInput(format!(
                        "Operation '{}' requires 'pattern' parameter",
                        operation
                    )));
                }
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Create a temp dir in $HOME to avoid /tmp system directory issues.
    fn create_test_dir() -> TempDir {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        TempDir::new_in(&home).expect("Failed to create temp dir")
    }

    fn canonical(path: &std::path::Path) -> PathBuf {
        path.canonicalize().expect("canonicalize")
    }

    fn create_tool_with_dir(tmp: &TempDir) -> FileManagerTool {
        let authorized = vec![canonical(tmp.path())];
        FileManagerTool {
            authorized_folders: authorized,
            cleanup_done: AtomicBool::new(false),
        }
    }

    // ========================================================
    // validate_input tests
    // ========================================================

    #[test]
    fn test_validate_input_missing_operation() {
        let tool = FileManagerTool {
            authorized_folders: vec![],
            cleanup_done: AtomicBool::new(false),
        };
        let input = json!({});
        let result = tool.validate_input(&input);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("operation"));
    }

    #[test]
    fn test_validate_input_invalid_operation() {
        let tool = FileManagerTool {
            authorized_folders: vec![],
            cleanup_done: AtomicBool::new(false),
        };
        let input = json!({"operation": "invalid"});
        let result = tool.validate_input(&input);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid operation"));
    }

    #[test]
    fn test_validate_input_all_valid_operations() {
        let tmp = create_test_dir();
        let tool = create_tool_with_dir(&tmp);
        let path_str = tmp.path().to_string_lossy().to_string();

        // Operations that need just path
        for op in &["list", "read", "delete"] {
            let input = json!({"operation": op, "path": path_str});
            assert!(tool.validate_input(&input).is_ok(), "Failed for op: {}", op);
        }

        // Write needs path + content
        let input = json!({"operation": "write", "path": path_str, "content": "test"});
        assert!(tool.validate_input(&input).is_ok());

        // Replace needs path + pattern + replacement
        let input =
            json!({"operation": "replace", "path": path_str, "pattern": "a", "replacement": "b"});
        assert!(tool.validate_input(&input).is_ok());

        // Create needs path
        let input = json!({"operation": "create", "path": path_str});
        assert!(tool.validate_input(&input).is_ok());

        // Move needs path + destination
        let input = json!({"operation": "move", "path": path_str, "destination": path_str});
        assert!(tool.validate_input(&input).is_ok());

        // Rename needs path + new_name
        let input = json!({"operation": "rename", "path": path_str, "new_name": "test.txt"});
        assert!(tool.validate_input(&input).is_ok());

        // Search glob/content need path + pattern
        for op in &["search_glob", "search_content"] {
            let input = json!({"operation": op, "path": path_str, "pattern": "*.rs"});
            assert!(tool.validate_input(&input).is_ok(), "Failed for op: {}", op);
        }
    }

    #[test]
    fn test_validate_input_missing_required_params() {
        let tool = FileManagerTool {
            authorized_folders: vec![],
            cleanup_done: AtomicBool::new(false),
        };

        // Write without content
        let result = tool.validate_input(&json!({"operation": "write", "path": "/tmp/x"}));
        assert!(result.is_err());

        // Replace without pattern
        let result = tool
            .validate_input(&json!({"operation": "replace", "path": "/tmp/x", "replacement": "b"}));
        assert!(result.is_err());

        // Move without destination
        let result = tool.validate_input(&json!({"operation": "move", "path": "/tmp/x"}));
        assert!(result.is_err());

        // Rename without new_name
        let result = tool.validate_input(&json!({"operation": "rename", "path": "/tmp/x"}));
        assert!(result.is_err());

        // Search without pattern
        let result = tool.validate_input(&json!({"operation": "search_glob", "path": "/tmp/x"}));
        assert!(result.is_err());
    }

    // ========================================================
    // definition tests
    // ========================================================

    #[test]
    fn test_definition_empty_folders() {
        let tool = FileManagerTool {
            authorized_folders: vec![],
            cleanup_done: AtomicBool::new(false),
        };
        let def = tool.definition();

        assert_eq!(def.id, "FileManagerTool");
        assert_eq!(def.name, "File Manager");
        assert!(!def.requires_confirmation);
        assert!(def
            .description
            .contains("No authorized directories configured"));
    }

    #[test]
    fn test_definition_with_folders() {
        let tool = FileManagerTool {
            authorized_folders: vec![PathBuf::from("/home/user/docs")],
            cleanup_done: AtomicBool::new(false),
        };
        let def = tool.definition();

        assert!(def.description.contains("Authorized directories:"));
        assert!(def.description.contains("/home/user/docs"));
    }

    // ========================================================
    // op_list tests
    // ========================================================

    #[tokio::test]
    async fn test_op_list_basic() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::write(base.join("file1.txt"), "content1").expect("write");
        fs::write(base.join("file2.txt"), "content2").expect("write");
        fs::create_dir(base.join("subdir")).expect("mkdir");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "list",
                "path": base.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["count"], 3);
        assert!(!val["truncated"].as_bool().unwrap_or(true));
    }

    #[tokio::test]
    async fn test_op_list_filters_trash() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::write(base.join("file.txt"), "content").expect("write");
        fs::create_dir(base.join(TRASH_DIR_NAME)).expect("mkdir trash");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "list",
                "path": base.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        // Should only see file.txt, not .zileo-trash
        assert_eq!(val["count"], 1);
    }

    #[tokio::test]
    async fn test_op_list_recursive() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::create_dir_all(base.join("a/b")).expect("mkdir");
        fs::write(base.join("a/file.txt"), "in a").expect("write");
        fs::write(base.join("a/b/deep.txt"), "deep").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "list",
                "path": base.to_string_lossy(),
                "recursive": true
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        // a/ + a/file.txt + a/b/ + a/b/deep.txt = 4
        assert_eq!(val["count"], 4);
    }

    #[tokio::test]
    async fn test_op_list_not_directory() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("not_a_dir.txt");
        fs::write(&file_path, "content").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "list",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
    }

    // ========================================================
    // op_read tests
    // ========================================================

    #[tokio::test]
    async fn test_op_read_text_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("hello.txt");
        fs::write(&file_path, "Hello, World!").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "read",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["content"], "Hello, World!");
        assert_eq!(val["size"], 13);
        assert_eq!(val["truncated"], false);
    }

    #[tokio::test]
    async fn test_op_read_binary_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("binary.bin");
        fs::write(&file_path, [0x00, 0x01, 0x02]).expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "read",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("binary or non-UTF8"));
    }

    #[tokio::test]
    async fn test_op_read_nonexistent() {
        let tmp = create_test_dir();
        // The path for validate_path must have an existing parent
        let file_path = tmp.path().join("ghost.txt");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "read",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
    }

    // ========================================================
    // op_write tests
    // ========================================================

    #[tokio::test]
    async fn test_op_write_new_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("new_file.txt");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "write",
                "path": file_path.to_string_lossy(),
                "content": "new content"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["written"], true);
        assert!(val["backup"].is_null());

        // Verify file was written
        let content = fs::read_to_string(&file_path).expect("read");
        assert_eq!(content, "new content");
    }

    #[tokio::test]
    async fn test_op_write_overwrite_with_backup() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("existing.txt");
        fs::write(&file_path, "original").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "write",
                "path": file_path.to_string_lossy(),
                "content": "updated"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["written"], true);
        assert!(val["backup"].is_string());

        // Verify file was updated
        let content = fs::read_to_string(&file_path).expect("read");
        assert_eq!(content, "updated");
    }

    // ========================================================
    // op_replace tests
    // ========================================================

    #[tokio::test]
    async fn test_op_replace_literal() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("replace_me.txt");
        fs::write(&file_path, "hello world hello").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "replace",
                "path": file_path.to_string_lossy(),
                "pattern": "hello",
                "replacement": "Hi"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["replacements"], 2);
        assert!(val["backup"].is_string());

        let content = fs::read_to_string(&file_path).expect("read");
        assert_eq!(content, "Hi world Hi");
    }

    #[tokio::test]
    async fn test_op_replace_regex() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("regex.txt");
        fs::write(&file_path, "foo123 bar456").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "replace",
                "path": file_path.to_string_lossy(),
                "pattern": "\\d+",
                "replacement": "NUM",
                "is_regex": true
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["replacements"], 2);

        let content = fs::read_to_string(&file_path).expect("read");
        assert_eq!(content, "fooNUM barNUM");
    }

    #[tokio::test]
    async fn test_op_replace_no_match() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("nope.txt");
        fs::write(&file_path, "nothing matches here").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "replace",
                "path": file_path.to_string_lossy(),
                "pattern": "ZZZZZ",
                "replacement": "YYY"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["replacements"], 0);
        assert!(val["backup"].is_null());
    }

    // ========================================================
    // op_create tests
    // ========================================================

    #[tokio::test]
    async fn test_op_create_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("created.txt");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "create",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["created"], true);
        assert_eq!(val["type"], "file");
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_op_create_directory() {
        let tmp = create_test_dir();
        let dir_path = tmp.path().join("new_dir");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "create",
                "path": dir_path.to_string_lossy(),
                "create_type": "directory"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["created"], true);
        assert_eq!(val["type"], "directory");
        assert!(dir_path.is_dir());
    }

    #[tokio::test]
    async fn test_op_create_already_exists() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("exists.txt");
        fs::write(&file_path, "already here").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "create",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    // ========================================================
    // op_delete tests
    // ========================================================

    #[tokio::test]
    async fn test_op_delete_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("deleteme.txt");
        fs::write(&file_path, "goodbye").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "delete",
                "path": file_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["deleted"], true);
        assert!(val["trash_path"].is_string());
        assert!(!file_path.exists()); // Original should be gone
    }

    #[tokio::test]
    async fn test_op_delete_directory_rejected() {
        let tmp = create_test_dir();
        let dir_path = tmp.path().join("a_dir");
        fs::create_dir(&dir_path).expect("mkdir");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "delete",
                "path": dir_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Only files can be deleted"));
    }

    // ========================================================
    // op_move tests
    // ========================================================

    #[tokio::test]
    async fn test_op_move_file() {
        let tmp = create_test_dir();
        let src = tmp.path().join("src.txt");
        fs::write(&src, "move me").expect("write");
        let dst_path = tmp.path().join("dst.txt");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "move",
                "path": src.to_string_lossy(),
                "destination": dst_path.to_string_lossy()
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["moved"], true);
        assert!(!src.exists());
        assert!(dst_path.exists());
        assert_eq!(fs::read_to_string(&dst_path).unwrap(), "move me");
    }

    #[tokio::test]
    async fn test_op_move_destination_exists() {
        let tmp = create_test_dir();
        let src = tmp.path().join("a.txt");
        let dst = tmp.path().join("b.txt");
        fs::write(&src, "a").expect("write");
        fs::write(&dst, "b").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "move",
                "path": src.to_string_lossy(),
                "destination": dst.to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("already exists"));
    }

    // ========================================================
    // op_rename tests
    // ========================================================

    #[tokio::test]
    async fn test_op_rename_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("old_name.txt");
        fs::write(&file_path, "content").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "rename",
                "path": file_path.to_string_lossy(),
                "new_name": "new_name.txt"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["renamed"], true);
        assert!(!file_path.exists());
        assert!(tmp.path().join("new_name.txt").exists());
    }

    #[tokio::test]
    async fn test_op_rename_rejects_path_separators() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("file.txt");
        fs::write(&file_path, "content").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "rename",
                "path": file_path.to_string_lossy(),
                "new_name": "sub/dir/name.txt"
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path separators"));
    }

    #[tokio::test]
    async fn test_op_rename_empty_name() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("file.txt");
        fs::write(&file_path, "content").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "rename",
                "path": file_path.to_string_lossy(),
                "new_name": ""
            }))
            .await;

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty"));
    }

    // ========================================================
    // op_search_glob tests
    // ========================================================

    #[tokio::test]
    async fn test_op_search_glob() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::write(base.join("file1.rs"), "fn main() {}").expect("write");
        fs::write(base.join("file2.rs"), "fn test() {}").expect("write");
        fs::write(base.join("readme.md"), "# Readme").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "search_glob",
                "path": base.to_string_lossy(),
                "pattern": "*.rs"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["count"], 2);
    }

    #[tokio::test]
    async fn test_op_search_glob_recursive() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::create_dir_all(base.join("src")).expect("mkdir");
        fs::write(base.join("top.rs"), "top").expect("write");
        fs::write(base.join("src/nested.rs"), "nested").expect("write");
        fs::write(base.join("src/other.txt"), "other").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "search_glob",
                "path": base.to_string_lossy(),
                "pattern": "*.rs",
                "recursive": true
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["count"], 2);
    }

    // ========================================================
    // op_search_content tests
    // ========================================================

    #[tokio::test]
    async fn test_op_search_content_literal() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::write(base.join("file.txt"), "line one\nfind me here\nline three").expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "search_content",
                "path": base.to_string_lossy(),
                "pattern": "find me"
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["count"], 1);
        let matches = val["matches"].as_array().expect("matches array");
        assert_eq!(matches[0]["line"], 2);
        assert_eq!(matches[0]["content"], "find me here");
    }

    #[tokio::test]
    async fn test_op_search_content_regex() {
        let tmp = create_test_dir();
        let base = tmp.path();

        fs::write(
            base.join("code.rs"),
            "fn main() {}\nfn test_something() {}\nstruct Foo;",
        )
        .expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "search_content",
                "path": base.to_string_lossy(),
                "pattern": "fn \\w+\\(",
                "is_regex": true
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        assert_eq!(val["count"], 2);
    }

    #[tokio::test]
    async fn test_op_search_content_with_context() {
        let tmp = create_test_dir();
        let base = tmp.path();

        let content = "line1\nline2\nline3\nTARGET\nline5\nline6\nline7";
        fs::write(base.join("ctx.txt"), content).expect("write");

        let tool = create_tool_with_dir(&tmp);
        let result = tool
            .execute(json!({
                "operation": "search_content",
                "path": base.to_string_lossy(),
                "pattern": "TARGET",
                "context_lines": 2
            }))
            .await;

        assert!(result.is_ok());
        let val = result.unwrap();
        let matches = val["matches"].as_array().expect("matches array");
        assert_eq!(matches.len(), 1);

        let m = &matches[0];
        assert_eq!(m["line"], 4);
        assert_eq!(m["content"], "TARGET");

        let before = m["context_before"].as_array().expect("context_before");
        assert_eq!(before.len(), 2);
        assert_eq!(before[0], "line2");
        assert_eq!(before[1], "line3");

        let after = m["context_after"].as_array().expect("context_after");
        assert_eq!(after.len(), 2);
        assert_eq!(after[0], "line5");
        assert_eq!(after[1], "line6");
    }

    // ========================================================
    // Permission tests (uses security module)
    // ========================================================

    #[tokio::test]
    async fn test_operation_outside_authorized() {
        let tmp = create_test_dir();
        let base = tmp.path();
        let authorized_dir = base.join("allowed");
        let outside_dir = base.join("outside");
        fs::create_dir(&authorized_dir).expect("mkdir");
        fs::create_dir(&outside_dir).expect("mkdir");
        fs::write(outside_dir.join("secret.txt"), "secret").expect("write");

        let tool = FileManagerTool {
            authorized_folders: vec![canonical(&authorized_dir)],
            cleanup_done: AtomicBool::new(false),
        };

        let result = tool
            .execute(json!({
                "operation": "read",
                "path": outside_dir.join("secret.txt").to_string_lossy()
            }))
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::PermissionDenied(_)
        ));
    }
}
