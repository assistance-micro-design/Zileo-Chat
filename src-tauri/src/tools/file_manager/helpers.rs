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

//! Shared helper functions for the FileManager tool.
//!
//! Provides constants and utility functions used across file operations:
//! - File metadata formatting
//! - Text file detection
//! - Parent directory creation

use serde_json::{json, Value};
use std::fs::Metadata;
use std::path::Path;

/// Maximum file size for read operations (10 MB).
pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;

/// Default max results for list operations.
pub const DEFAULT_LIST_MAX: usize = 500;

/// Default max results for search operations.
pub const DEFAULT_SEARCH_MAX: usize = 100;

/// Default context lines for search_content.
pub const DEFAULT_CONTEXT_LINES: usize = 3;

/// Maximum context lines allowed.
pub const MAX_CONTEXT_LINES: usize = 10;

/// Number of bytes to read for text file detection.
const TEXT_DETECTION_BUFFER_SIZE: usize = 8192;

/// Format file metadata as a JSON value.
///
/// # Arguments
/// * `path` - The file or directory path
/// * `metadata` - The filesystem metadata for the path
///
/// # Returns
/// A JSON object with name, path, size, is_directory, is_file, and modified_at fields.
pub fn format_file_info(path: &Path, metadata: &Metadata) -> Value {
    json!({
        "name": path.file_name().and_then(|n| n.to_str()).unwrap_or(""),
        "path": path.to_string_lossy(),
        "size": metadata.len(),
        "is_directory": metadata.is_dir(),
        "is_file": metadata.is_file(),
        "modified_at": metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
    })
}

/// Check if a file is likely a text file (UTF-8 compatible).
///
/// Reads the first 8192 bytes and checks for null bytes, which
/// are a strong indicator of binary content.
///
/// # Arguments
/// * `path` - The file path to check
///
/// # Returns
/// `true` if the file appears to be text, `false` if binary or unreadable.
pub fn is_text_file(path: &Path) -> bool {
    use std::io::Read;

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return false,
    };

    let mut reader = std::io::BufReader::new(file);
    let mut buffer = vec![0u8; TEXT_DETECTION_BUFFER_SIZE];

    let bytes_read = match reader.read(&mut buffer) {
        Ok(n) => n,
        Err(_) => return false,
    };

    // Empty files are considered text
    if bytes_read == 0 {
        return true;
    }

    let slice = &buffer[..bytes_read];

    // Check for null bytes (strong binary indicator)
    if slice.contains(&0) {
        return false;
    }

    // Check if it's valid UTF-8
    std::str::from_utf8(slice).is_ok()
}

/// Create parent directories if they do not exist.
///
/// # Arguments
/// * `path` - The file path whose parent directories should be created
///
/// # Returns
/// `Ok(())` on success, or an error message if directory creation fails.
///
/// # Errors
/// Returns an error string if `create_dir_all` fails.
pub fn ensure_parent_exists(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent directory: {}", e))?;
        }
    }
    Ok(())
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

    // ========================================================
    // format_file_info tests
    // ========================================================

    #[test]
    fn test_format_file_info_for_file() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("test.txt");
        fs::write(&file_path, "hello world").expect("write");

        let metadata = fs::metadata(&file_path).expect("metadata");
        let info = format_file_info(&file_path, &metadata);

        assert_eq!(info["name"], "test.txt");
        assert_eq!(info["size"], 11); // "hello world" = 11 bytes
        assert_eq!(info["is_file"], true);
        assert_eq!(info["is_directory"], false);
        assert!(info["modified_at"].is_number());
    }

    #[test]
    fn test_format_file_info_for_directory() {
        let tmp = create_test_dir();
        let dir_path = tmp.path().join("subdir");
        fs::create_dir(&dir_path).expect("mkdir");

        let metadata = fs::metadata(&dir_path).expect("metadata");
        let info = format_file_info(&dir_path, &metadata);

        assert_eq!(info["name"], "subdir");
        assert_eq!(info["is_file"], false);
        assert_eq!(info["is_directory"], true);
    }

    #[test]
    fn test_format_file_info_path_field() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("data.json");
        fs::write(&file_path, "{}").expect("write");

        let metadata = fs::metadata(&file_path).expect("metadata");
        let info = format_file_info(&file_path, &metadata);

        let path_str = info["path"].as_str().expect("path should be string");
        assert!(path_str.ends_with("data.json"));
    }

    // ========================================================
    // is_text_file tests
    // ========================================================

    #[test]
    fn test_is_text_file_with_text() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("readme.md");
        fs::write(&file_path, "# Hello World\n\nThis is text.").expect("write");

        assert!(is_text_file(&file_path));
    }

    #[test]
    fn test_is_text_file_with_empty() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("empty.txt");
        fs::write(&file_path, "").expect("write");

        assert!(is_text_file(&file_path));
    }

    #[test]
    fn test_is_text_file_with_binary() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("binary.bin");
        // Write bytes with null characters (binary indicator)
        let binary_data: Vec<u8> = vec![0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00];
        fs::write(&file_path, &binary_data).expect("write");

        assert!(!is_text_file(&file_path));
    }

    #[test]
    fn test_is_text_file_with_utf8() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("unicode.txt");
        fs::write(&file_path, "Bonjour le monde! L'eau est fraiche.").expect("write");

        assert!(is_text_file(&file_path));
    }

    #[test]
    fn test_is_text_file_nonexistent() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("nonexistent.txt");

        assert!(!is_text_file(&file_path));
    }

    // ========================================================
    // ensure_parent_exists tests
    // ========================================================

    #[test]
    fn test_ensure_parent_exists_creates_parents() {
        let tmp = create_test_dir();
        let file_path = tmp.path().join("a/b/c/deep.txt");

        assert!(!tmp.path().join("a").exists());

        let result = ensure_parent_exists(&file_path);
        assert!(result.is_ok());
        assert!(tmp.path().join("a/b/c").exists());
    }

    #[test]
    fn test_ensure_parent_exists_already_exists() {
        let tmp = create_test_dir();
        let dir_path = tmp.path().join("existing");
        fs::create_dir(&dir_path).expect("mkdir");
        let file_path = dir_path.join("file.txt");

        let result = ensure_parent_exists(&file_path);
        assert!(result.is_ok());
    }

    #[test]
    fn test_ensure_parent_exists_root_path() {
        // A root-level path (no parent needing creation)
        let result = ensure_parent_exists(Path::new("/file.txt"));
        assert!(result.is_ok());
    }

    // ========================================================
    // Constants tests
    // ========================================================

    #[test]
    fn test_constants_values() {
        assert_eq!(MAX_FILE_SIZE, 10 * 1024 * 1024);
        assert_eq!(DEFAULT_LIST_MAX, 500);
        assert_eq!(DEFAULT_SEARCH_MAX, 100);
        assert_eq!(DEFAULT_CONTEXT_LINES, 3);
        assert_eq!(MAX_CONTEXT_LINES, 10);
    }
}
