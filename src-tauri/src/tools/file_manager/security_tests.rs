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

#[test]
fn test_validate_folder_valid() {
    let tmp = setup_test_dir();
    let result = validate_folder_for_authorization(tmp.path().join("project").to_str().unwrap());
    assert!(
        result.is_ok(),
        "Valid folder should be accepted: {:?}",
        result
    );
}

#[test]
fn test_validate_folder_rejects_file() {
    let tmp = setup_test_dir();
    let result =
        validate_folder_for_authorization(tmp.path().join("project/src/main.rs").to_str().unwrap());
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
