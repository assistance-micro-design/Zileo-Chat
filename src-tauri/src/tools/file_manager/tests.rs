use super::tool::FileManagerTool;
use super::trash::TRASH_DIR_NAME;
use crate::tools::{Tool, ToolError};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
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
        cached_definition: std::sync::OnceLock::new(),
    }
}

#[test]
fn test_validate_input_missing_operation() {
    let tool = FileManagerTool {
        authorized_folders: vec![],
        cleanup_done: AtomicBool::new(false),
        cached_definition: std::sync::OnceLock::new(),
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
        cached_definition: std::sync::OnceLock::new(),
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
        cached_definition: std::sync::OnceLock::new(),
    };

    // Write without content
    let result = tool.validate_input(&json!({"operation": "write", "path": "/tmp/x"}));
    assert!(result.is_err());

    // Replace without pattern
    let result =
        tool.validate_input(&json!({"operation": "replace", "path": "/tmp/x", "replacement": "b"}));
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

#[test]
fn test_definition_empty_folders() {
    let tool = FileManagerTool {
        authorized_folders: vec![],
        cleanup_done: AtomicBool::new(false),
        cached_definition: std::sync::OnceLock::new(),
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
        cached_definition: std::sync::OnceLock::new(),
    };
    let def = tool.definition();

    assert!(def.description.contains("Authorized directories:"));
    assert!(def.description.contains("/home/user/docs"));
}

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

    let content = fs::read_to_string(&file_path).expect("read");
    assert_eq!(content, "updated");
}

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
        cached_definition: std::sync::OnceLock::new(),
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
