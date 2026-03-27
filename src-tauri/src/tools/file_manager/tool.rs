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

//! FileManager Tool - struct definition and trait implementation.
//!
//! The 10 filesystem operations are implemented in `operations.rs`.
//! This file contains only the struct, constructor, trash cleanup,
//! the `Tool` trait implementation (definition, execute, validate_input).

use crate::tools::file_manager::trash_management::cleanup_trash;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing::warn;

/// FileManager tool for sandboxed filesystem operations.
///
/// All operations are restricted to the agent's configured authorized folders.
/// Destructive operations use trash-based safety (no permanent deletion).
pub struct FileManagerTool {
    pub(crate) authorized_folders: Vec<PathBuf>,
    pub(crate) cleanup_done: AtomicBool,
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
    pub(crate) fn ensure_cleanup(&self) {
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
            summary: "Read, write, search, and organize files within authorized directories"
                .to_string(),
            description: format!(
                r#"Manages files within authorized directories with sandboxed access.

USE THIS TOOL WHEN:
- You need to read, write, or modify files for the user
- You need to search for files by name pattern or content
- You need to organize files (move, rename, delete)

DO NOT USE THIS TOOL WHEN:
- The path is outside authorized directories (will be rejected)
- You need to execute files (use appropriate tool instead)

OPERATIONS:
- list: List directory contents
- read: Read file content (with optional offset/limit)
- write: Create or overwrite a file
- replace: Replace text in a file (old_text -> new_text)
- create: Create a new empty file or directory
- delete: Move to trash (.zileo-trash/)
- move: Move file to another authorized directory
- rename: Rename file in place
- search_glob: Find files matching a glob pattern
- search_content: Search text content across files

EXAMPLES:
1. List: {{"operation": "list", "path": "/project/src"}}
2. Read: {{"operation": "read", "path": "/project/main.rs"}}
3. Search: {{"operation": "search_content", "path": "/project", "pattern": "TODO"}}

{}"#,
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

        validate_required_params(operation, input)
    }
}

/// Validate that required parameters are present for each operation.
fn validate_required_params(operation: &str, input: &Value) -> ToolResult<()> {
    /// Check that a string field exists and is non-null.
    fn require_str(input: &Value, field: &str, operation: &str) -> ToolResult<()> {
        if input.get(field).and_then(|v| v.as_str()).is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Operation '{}' requires '{}' parameter",
                operation, field
            )));
        }
        Ok(())
    }

    match operation {
        "list" | "read" | "delete" | "create" => {
            require_str(input, "path", operation)?;
        }
        "write" => {
            require_str(input, "path", operation)?;
            require_str(input, "content", operation)?;
        }
        "replace" => {
            require_str(input, "path", operation)?;
            require_str(input, "pattern", operation)?;
            require_str(input, "replacement", operation)?;
        }
        "move" => {
            require_str(input, "path", operation)?;
            require_str(input, "destination", operation)?;
        }
        "rename" => {
            require_str(input, "path", operation)?;
            require_str(input, "new_name", operation)?;
        }
        "search_glob" | "search_content" => {
            require_str(input, "path", operation)?;
            require_str(input, "pattern", operation)?;
        }
        _ => {}
    }

    Ok(())
}
