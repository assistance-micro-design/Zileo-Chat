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
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// Helper: create a temp directory with a structure.
    /// Uses the user's home directory as the base to avoid FORBIDDEN_SYSTEM_DIRS (/tmp).
    fn setup_test_dir() -> TempDir {
        let base_dir = std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::current_dir().expect("current dir"));
        let tmp = TempDir::new_in(base_dir).expect("Failed to create temp dir");
        let base = tmp.path();

        // Create subdirectories
        fs::create_dir_all(base.join("project/src")).expect("mkdir");
        fs::create_dir_all(base.join("project/data")).expect("mkdir");
        fs::write(base.join("project/src/main.rs"), "fn main() {}").expect("write");
        fs::write(base.join("project/data/config.json"), "{}").expect("write");

        tmp
    }

    fn canonical(path: &Path) -> PathBuf {
        path.canonicalize().expect("canonicalize")
    }

    // ========================================================
    // validate_path: Happy path tests
    // ========================================================

    #[test]
    fn test_validate_path_existing_file() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let result = validate_path(
            tmp.path().join("project/src/main.rs").to_str().unwrap(),
            &authorized,
        );
        assert!(
            result.is_ok(),
            "Should allow file in authorized dir: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_existing_directory() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let result = validate_path(
            tmp.path().join("project/src").to_str().unwrap(),
            &authorized,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_path_new_file_in_existing_dir() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        // File doesn't exist yet, but parent does
        let result = validate_path(
            tmp.path().join("project/src/new_file.rs").to_str().unwrap(),
            &authorized,
        );
        assert!(
            result.is_ok(),
            "Should allow new file in authorized dir: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_path_multiple_authorized_folders() {
        let tmp = setup_test_dir();
        let authorized = vec![
            canonical(tmp.path().join("project/src").as_path()),
            canonical(tmp.path().join("project/data").as_path()),
        ];

        // File in first authorized folder
        let result1 = validate_path(
            tmp.path().join("project/src/main.rs").to_str().unwrap(),
            &authorized,
        );
        assert!(result1.is_ok());

        // File in second authorized folder
        let result2 = validate_path(
            tmp.path()
                .join("project/data/config.json")
                .to_str()
                .unwrap(),
            &authorized,
        );
        assert!(result2.is_ok());
    }

    // ========================================================
    // validate_path: Security rejection tests
    // ========================================================

    #[test]
    fn test_validate_path_rejects_null_bytes() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let malicious = format!("{}/src/file.rs\0.txt", tmp.path().join("project").display());
        let result = validate_path(&malicious, &authorized);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::PermissionDenied(_)
        ));
    }

    #[test]
    fn test_validate_path_rejects_traversal() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let malicious = format!(
            "{}/src/../../etc/passwd",
            tmp.path().join("project").display()
        );
        let result = validate_path(&malicious, &authorized);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
        assert!(err.to_string().contains("traversal"));
    }

    #[test]
    fn test_validate_path_rejects_system_dirs() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let result = validate_path("/etc/passwd", &authorized);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::PermissionDenied(_)
        ));
    }

    #[test]
    fn test_validate_path_rejects_outside_authorized() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project/src").as_path())];

        // data/ is NOT in authorized (only src/ is)
        let result = validate_path(
            tmp.path()
                .join("project/data/config.json")
                .to_str()
                .unwrap(),
            &authorized,
        );
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::PermissionDenied(_)
        ));
    }

    #[test]
    fn test_validate_path_rejects_empty_authorized() {
        let tmp = setup_test_dir();
        let authorized: Vec<PathBuf> = vec![];

        let result = validate_path(
            tmp.path().join("project/src/main.rs").to_str().unwrap(),
            &authorized,
        );
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("No authorized folders"));
    }

    #[test]
    fn test_validate_path_rejects_nonexistent_parent() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        let result = validate_path(
            tmp.path()
                .join("project/nonexistent/dir/file.txt")
                .to_str()
                .unwrap(),
            &authorized,
        );
        assert!(result.is_err());
    }

    #[cfg(unix)]
    #[test]
    fn test_validate_path_symlink_escape() {
        let tmp = setup_test_dir();
        let authorized = vec![canonical(tmp.path().join("project").as_path())];

        // Create a directory outside the authorized area
        let outside = tmp.path().join("outside");
        fs::create_dir_all(&outside).expect("mkdir outside");
        fs::write(outside.join("secret.txt"), "secret data").expect("write secret");

        // Create a symlink inside authorized dir pointing outside
        let symlink_path = tmp.path().join("project/src/link_to_outside");
        std::os::unix::fs::symlink(&outside, &symlink_path).expect("symlink");

        // Attempt to read through symlink - should be rejected because
        // canonicalize resolves to path outside authorized dirs
        let result = validate_path(
            symlink_path.join("secret.txt").to_str().unwrap(),
            &authorized,
        );
        assert!(
            result.is_err(),
            "Symlink escape should be blocked: {:?}",
            result
        );
    }

    // ========================================================
    // validate_folder_for_authorization tests
    // ========================================================

    #[test]
    fn test_validate_folder_valid() {
        let tmp = setup_test_dir();
        let result =
            validate_folder_for_authorization(tmp.path().join("project").to_str().unwrap());
        assert!(
            result.is_ok(),
            "Valid folder should be accepted: {:?}",
            result
        );
    }

    #[test]
    fn test_validate_folder_rejects_file() {
        let tmp = setup_test_dir();
        let result = validate_folder_for_authorization(
            tmp.path().join("project/src/main.rs").to_str().unwrap(),
        );
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not a directory"));
    }

    #[test]
    fn test_validate_folder_rejects_nonexistent() {
        let result = validate_folder_for_authorization("/nonexistent/path/xyz");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    fn test_validate_folder_rejects_system_dir() {
        // /etc exists on Linux/macOS
        #[cfg(not(target_os = "windows"))]
        {
            let result = validate_folder_for_authorization("/etc");
            assert!(result.is_err());
            assert!(result.unwrap_err().contains("system directory"));
        }
    }

    #[test]
    fn test_validate_folder_rejects_null_bytes() {
        let result = validate_folder_for_authorization("/tmp/test\0dir");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("null bytes"));
    }

    // ========================================================
    // is_system_directory tests
    // ========================================================

    #[test]
    fn test_is_system_directory() {
        #[cfg(not(target_os = "windows"))]
        {
            assert!(is_system_directory("/etc"));
            assert!(is_system_directory("/etc/passwd"));
            assert!(is_system_directory("/sys/kernel"));
            assert!(is_system_directory("/proc/1"));
            assert!(is_system_directory("/dev/null"));
            assert!(is_system_directory("/boot"));
            assert!(is_system_directory("/usr/bin"));

            assert!(!is_system_directory("/home/user/documents"));
            assert!(!is_system_directory("/opt/myapp"));
        }
    }

    // ========================================================
    // check_directory_permissions tests
    // ========================================================

    #[test]
    fn test_check_permissions_valid_dir() {
        let tmp = setup_test_dir();
        let result = check_directory_permissions(tmp.path().join("project").as_path());
        assert!(result.is_ok());
    }

    #[test]
    fn test_check_permissions_nonexistent() {
        let result = check_directory_permissions(Path::new("/nonexistent/xyz"));
        assert!(result.is_err());
    }

    // ========================================================
    // find_authorized_folder tests
    // ========================================================

    #[test]
    fn test_find_authorized_folder_found() {
        let tmp = setup_test_dir();
        let src = canonical(tmp.path().join("project/src").as_path());
        let data = canonical(tmp.path().join("project/data").as_path());
        let authorized = vec![src.clone(), data.clone()];

        let file_path = src.join("main.rs");
        let result = find_authorized_folder(&file_path, &authorized);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &src);
    }

    #[test]
    fn test_find_authorized_folder_not_found() {
        let tmp = setup_test_dir();
        let src = canonical(tmp.path().join("project/src").as_path());
        let authorized = vec![src];

        let outside = PathBuf::from("/home/user/other");
        let result = find_authorized_folder(&outside, &authorized);
        assert!(result.is_none());
    }
}
