use super::*;
use crate::tools::file_manager::trash_management::{
    cleanup_trash, get_trash_size, list_trash_entries, restore_from_trash,
};
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
