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

//! Move, rename, and search operations for the FileManager tool.

use crate::tools::file_manager::helpers::{
    ensure_parent_exists, format_file_info, is_text_file, DEFAULT_CONTEXT_LINES,
    DEFAULT_SEARCH_MAX, MAX_CONTEXT_LINES,
};
use crate::tools::file_manager::operations::is_trash_dir;
use crate::tools::file_manager::security::validate_path;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

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
                    ToolError::ExecutionFailed(format!("Failed to remove source after copy: {}", e))
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
        search_glob_recursive(
            &validated_path,
            &glob,
            recursive,
            max_results,
            &mut matches,
            &self.authorized_folders,
        )?;

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
        search_content_recursive(
            &validated_path,
            &ctx,
            &mut matches,
            &self.authorized_folders,
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
}

// ---------------------------------------------------------------------------
// Free helper functions (no &self needed)
// ---------------------------------------------------------------------------

/// Re-canonicalize a directory entry and verify it remains within the agent's
/// authorized sandbox. Defends against TOCTOU attacks where an attacker swaps
/// a directory entry for a symlink pointing outside the sandbox between the
/// initial `validate_path()` and the moment we read or descend into it.
///
/// Returns `Some(canonical_path)` if the entry is safe to access,
/// or `None` if it must be skipped (broken link, unreachable, or escapes sandbox).
fn canonicalize_within_sandbox(
    entry_path: &Path,
    authorized_folders: &[PathBuf],
) -> Option<PathBuf> {
    let canonical = match entry_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            debug!(
                path = %entry_path.display(),
                error = %e,
                "Skipping entry: cannot canonicalize"
            );
            return None;
        }
    };

    if !authorized_folders.iter().any(|f| canonical.starts_with(f)) {
        warn!(
            path = %entry_path.display(),
            canonical = %canonical.display(),
            "Skipping entry: resolves outside sandbox (TOCTOU defense)"
        );
        return None;
    }

    Some(canonical)
}

/// Recursively search for files matching a glob pattern.
fn search_glob_recursive(
    dir: &Path,
    glob: &globset::GlobMatcher,
    recursive: bool,
    max_results: usize,
    matches: &mut Vec<Value>,
    authorized_folders: &[PathBuf],
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

        // TOCTOU defense: re-canonicalize each entry every time we touch it
        // (vs. relying on the initial validate_path on `dir` only).
        let Some(canonical_entry) = canonicalize_within_sandbox(&entry_path, authorized_folders)
        else {
            continue;
        };

        if let Some(file_name) = entry_path.file_name() {
            if glob.is_match(file_name) {
                let metadata = entry.metadata().map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to read metadata: {}", e))
                })?;
                matches.push(format_file_info(&entry_path, &metadata));
            }
        }

        if recursive && canonical_entry.is_dir() && matches.len() < max_results {
            search_glob_recursive(
                &canonical_entry,
                glob,
                true,
                max_results,
                matches,
                authorized_folders,
            )?;
        }
    }

    Ok(())
}

/// Recursively search file contents for a pattern, collecting matches with context.
fn search_content_recursive(
    dir: &Path,
    ctx: &ContentSearchContext<'_>,
    matches: &mut Vec<Value>,
    authorized_folders: &[PathBuf],
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

        // TOCTOU defense: re-canonicalize each entry before reading file content
        // or descending. Defends against an attacker swapping a directory entry
        // for a symlink to a sensitive file (e.g. /etc/passwd) between the
        // initial sandbox check and the actual filesystem access.
        let Some(canonical_entry) = canonicalize_within_sandbox(&entry_path, authorized_folders)
        else {
            continue;
        };

        if canonical_entry.is_file() && is_text_file(&canonical_entry) {
            if let Ok(content) = std::fs::read_to_string(&canonical_entry) {
                search_file_content(&entry_path, &content, ctx, matches);
            }
        } else if ctx.recursive && canonical_entry.is_dir() && matches.len() < ctx.max_results {
            search_content_recursive(&canonical_entry, ctx, matches, authorized_folders)?;
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
