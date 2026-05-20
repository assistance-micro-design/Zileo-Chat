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
    ensure_parent_exists, ext_to_image_mime, format_file_info, is_text_file,
    ALLOWED_IMAGE_EXTENSIONS, DEFAULT_LIST_MAX, MAX_FILE_SIZE,
};
use crate::tools::file_manager::security::{find_authorized_folder, validate_path};
use crate::tools::file_manager::trash::{backup_before_overwrite, move_to_trash, TRASH_DIR_NAME};
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use std::path::Path;
use tracing::{debug, info};

use super::tool::FileManagerTool;

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

    /// Read an image file and return it as base64 for vision analysis.
    ///
    /// Validates the path against authorized folders, restricts to a small
    /// whitelist of image extensions, enforces a hard size cap, and returns
    /// both the human-facing metadata and a `_image_attachment` marker that
    /// the agent loop converts into a synthetic multipart user turn at the
    /// next iteration so the model can actually "see" the image.
    pub(crate) async fn op_read_image(&self, input: &Value) -> ToolResult<Value> {
        const MAX_IMAGE_SIZE_BYTES: u64 = 8 * 1024 * 1024; // 8 MB

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

        let ext = validated_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        if !ALLOWED_IMAGE_EXTENSIONS.contains(&ext.as_str()) {
            return Err(ToolError::InvalidInput(format!(
                "Unsupported image extension '{}'. Allowed: {:?}",
                ext, ALLOWED_IMAGE_EXTENSIONS
            )));
        }

        let metadata = tokio::fs::metadata(&validated_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to stat image '{}': {}",
                validated_path.display(),
                e
            ))
        })?;

        if metadata.len() > MAX_IMAGE_SIZE_BYTES {
            return Err(ToolError::InvalidInput(format!(
                "Image too large ({} bytes, max {} bytes)",
                metadata.len(),
                MAX_IMAGE_SIZE_BYTES
            )));
        }

        let bytes = tokio::fs::read(&validated_path).await.map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to read image '{}': {}",
                validated_path.display(),
                e
            ))
        })?;

        use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
        let data_base64 = BASE64_STANDARD.encode(&bytes);

        // Whitelist check above guarantees a hit; fallback is defensive only.
        let mime_type = ext_to_image_mime(&ext).unwrap_or("application/octet-stream");

        let file_name = validated_path
            .file_name()
            .and_then(|n| n.to_str())
            .map(String::from);

        debug!(
            path = %validated_path.display(),
            size = metadata.len(),
            mime = %mime_type,
            "Image read"
        );

        // `_image_attachment` is the sentinel the iteration loop watches for:
        // when present in a tool result, it queues a synthetic user turn with
        // a multipart content that carries the actual image part so the next
        // assistant call sees the picture.
        Ok(json!({
            "path": validated_path.to_string_lossy(),
            "mime_type": mime_type,
            "size_bytes": metadata.len(),
            "name": file_name,
            "_image_attachment": {
                "kind": "image",
                "mime_type": mime_type,
                "data_base64": data_base64,
                "name": file_name,
                "size_bytes": metadata.len(),
            }
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
}

/// Check if a path is the `.zileo-trash` directory.
pub(crate) fn is_trash_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|name| name == TRASH_DIR_NAME)
}

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
