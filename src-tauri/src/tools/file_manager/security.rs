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

//! Security module for FileManager tool.
//!
//! Provides path validation, sandboxing, and authorization checks.
//! This module is the security boundary for all filesystem operations.

use crate::tools::{ToolError, ToolResult};
use std::path::{Path, PathBuf};
use tracing::warn;

/// Forbidden system directories (Linux).
/// Single source of truth for system directory blocking.
#[cfg(target_os = "linux")]
pub const FORBIDDEN_SYSTEM_DIRS: &[&str] = &[
    "/etc", "/sys", "/proc", "/dev", "/boot", "/root", "/var", "/tmp", "/usr", "/sbin", "/bin",
];

/// Forbidden system directories (macOS).
#[cfg(target_os = "macos")]
pub const FORBIDDEN_SYSTEM_DIRS: &[&str] = &[
    "/etc", "/sys", "/dev", "/var", "/tmp", "/usr", "/sbin", "/bin", "/System", "/Library",
    "/private",
];

/// Forbidden system directories (Windows).
#[cfg(target_os = "windows")]
pub const FORBIDDEN_SYSTEM_DIRS: &[&str] = &[
    "C:\\Windows",
    "C:\\Windows\\System32",
    "C:\\Program Files",
    "C:\\Program Files (x86)",
    "C:\\ProgramData",
];

/// Validate that a path is within one of the authorized directories.
/// Resolves symlinks, rejects traversal, null bytes, and system paths.
///
/// # Arguments
/// * `requested_path` - The path the LLM wants to access
/// * `authorized_folders` - List of canonical authorized directory paths
///
/// # Returns
/// The validated canonical PathBuf, or ToolError::PermissionDenied
///
/// # Security
/// 1. Reject null bytes in path string
/// 2. Reject ".." components
/// 3. Reject system directories
/// 4. Canonicalize path (resolves symlinks to real target)
/// 5. Check canonicalized path starts_with() any authorized folder
/// 6. If path doesn't exist yet (create op), canonicalize parent + check
pub fn validate_path(requested_path: &str, authorized_folders: &[PathBuf]) -> ToolResult<PathBuf> {
    // 1. Reject null bytes
    if requested_path.contains('\0') {
        warn!(path = %requested_path.replace('\0', "\\0"), "Null byte in path rejected");
        return Err(ToolError::PermissionDenied(
            "Path contains null bytes".to_string(),
        ));
    }

    // 2. Reject ".." components
    let path = Path::new(requested_path);
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            warn!(path = %requested_path, "Path traversal attempt rejected");
            return Err(ToolError::PermissionDenied(
                "Path traversal (..) is not allowed".to_string(),
            ));
        }
    }

    // 3. Reject system directories
    if is_system_directory(requested_path) {
        warn!(path = %requested_path, "System directory access rejected");
        return Err(ToolError::PermissionDenied(
            "Access to system directories is not allowed".to_string(),
        ));
    }

    // 4. Canonicalize and check authorization
    if path.exists() {
        // Path exists: canonicalize directly
        let canonical = path.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to resolve path '{}': {}",
                requested_path, e
            ))
        })?;

        check_path_authorized(&canonical, authorized_folders)?;
        Ok(canonical)
    } else {
        // 6. Path doesn't exist: canonicalize parent directory
        let parent = path.parent().ok_or_else(|| {
            ToolError::PermissionDenied("Cannot determine parent directory".to_string())
        })?;

        if !parent.exists() {
            return Err(ToolError::PermissionDenied(format!(
                "Parent directory '{}' does not exist",
                parent.display()
            )));
        }

        let canonical_parent = parent.canonicalize().map_err(|e| {
            ToolError::ExecutionFailed(format!(
                "Failed to resolve parent '{}': {}",
                parent.display(),
                e
            ))
        })?;

        check_path_authorized(&canonical_parent, authorized_folders)?;

        // Reconstruct the full canonical path
        let file_name = path
            .file_name()
            .ok_or_else(|| ToolError::PermissionDenied("Cannot determine file name".to_string()))?;

        Ok(canonical_parent.join(file_name))
    }
}

/// Check if a canonical path is within any authorized folder.
fn check_path_authorized(canonical_path: &Path, authorized_folders: &[PathBuf]) -> ToolResult<()> {
    if authorized_folders.is_empty() {
        return Err(ToolError::PermissionDenied(
            "No authorized folders configured for this agent".to_string(),
        ));
    }

    for folder in authorized_folders {
        if canonical_path.starts_with(folder) {
            return Ok(());
        }
    }

    Err(ToolError::PermissionDenied(format!(
        "Path '{}' is outside all authorized directories",
        canonical_path.display()
    )))
}

/// Check if a path is in a system directory.
fn is_system_directory(path_str: &str) -> bool {
    let path = Path::new(path_str);

    // Check each component and build path progressively
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        let current_str = current.to_string_lossy();

        for forbidden in FORBIDDEN_SYSTEM_DIRS {
            if current_str == *forbidden || current_str.starts_with(&format!("{}/", forbidden)) {
                return true;
            }
        }
    }

    false
}

/// Validate that a folder path is suitable for authorization.
/// Called when user adds a folder in agent settings.
///
/// # Checks
/// - Path exists and is a directory
/// - Path is readable by current process
/// - Path is not a system directory
/// - Canonicalize and store canonical form
pub fn validate_folder_for_authorization(path: &str) -> Result<PathBuf, String> {
    // Check null bytes
    if path.contains('\0') {
        return Err("Path contains null bytes".to_string());
    }

    // Check system directory
    if is_system_directory(path) {
        return Err(format!("Cannot authorize system directory: {}", path));
    }

    let path_buf = PathBuf::from(path);

    // Check exists
    if !path_buf.exists() {
        return Err(format!("Directory does not exist: {}", path));
    }

    // Check is directory
    if !path_buf.is_dir() {
        return Err(format!("Path is not a directory: {}", path));
    }

    // Check permissions
    check_directory_permissions(&path_buf)?;

    // Canonicalize
    let canonical = path_buf
        .canonicalize()
        .map_err(|e| format!("Failed to resolve path: {}", e))?;

    // Double-check canonicalized path isn't a system dir (symlink resolution)
    let canonical_str = canonical.to_string_lossy();
    if is_system_directory(&canonical_str) {
        return Err(format!(
            "Path resolves to a system directory: {}",
            canonical_str
        ));
    }

    Ok(canonical)
}

/// Check if process has read+write permissions on directory.
pub fn check_directory_permissions(path: &Path) -> Result<(), String> {
    use std::fs;

    // Check read permission by listing directory
    fs::read_dir(path).map_err(|e| {
        format!(
            "No read permission on directory '{}': {}",
            path.display(),
            e
        )
    })?;

    // Check write permission by testing metadata (best effort without modifying)
    let metadata = fs::metadata(path).map_err(|e| {
        format!(
            "Cannot access directory metadata '{}': {}",
            path.display(),
            e
        )
    })?;

    if metadata.permissions().readonly() {
        return Err(format!("Directory is read-only: {}", path.display()));
    }

    Ok(())
}

/// Find which authorized folder contains the given path.
/// Returns the authorized folder that is the parent of the path.
pub fn find_authorized_folder<'a>(
    path: &Path,
    authorized_folders: &'a [PathBuf],
) -> Option<&'a PathBuf> {
    authorized_folders
        .iter()
        .find(|folder| path.starts_with(folder))
}

#[cfg(test)]
#[path = "security_tests.rs"]
mod tests;
