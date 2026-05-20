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

//! FileManager Tauri commands
//!
//! Provides IPC commands for folder validation and trash management.
//!
//! ## Commands
//!
//! - [`validate_agent_folder`] - Validate a folder path for agent authorization
//! - [`list_trash`] - List trash entries for a specific folder
//! - [`restore_from_trash_cmd`] - Restore a file from trash

use crate::tools::file_manager::helpers::{ext_to_image_mime, ALLOWED_IMAGE_EXTENSIONS};
use crate::tools::file_manager::security::validate_folder_for_authorization;
use crate::tools::file_manager::trash::TrashEntry;
use crate::tools::file_manager::trash_management;
use serde::Serialize;
use tracing::{info, instrument, warn};

/// Payload returned by [`read_image_for_attachment`].
#[derive(Debug, Serialize)]
pub struct ImageReadResult {
    /// Base64-encoded image bytes (raw, no `data:` prefix).
    pub data_base64: String,
    /// IANA MIME type derived from the file extension.
    pub mime_type: String,
    /// File size in bytes (post-encoding the encoder produces ~33% more).
    pub size_bytes: u64,
    /// Original filename (display only).
    pub name: String,
}

/// Maximum image size in bytes accepted from the picker. Mirrors the per-
/// attachment cap enforced by `save_message_core` (4 MB binary → ~5.33 MB
/// base64). Hard-coded here because this command runs before the canonical
/// validation in `save_message`, so a fast-fail at the picker is friendlier.
const MAX_PICKER_IMAGE_SIZE_BYTES: u64 = 4 * 1024 * 1024;

/// Reads an image file selected by the user through the Tauri dialog and
/// encodes it as base64 for use as a `MessageAttachment`.
///
/// The Tauri dialog already enforces user consent at OS level, so this
/// command intentionally does NOT cross-check the path against the agent's
/// authorized folders (it is invoked from the ChatInput, not from an agent
/// tool). It only:
///
/// - Validates the extension whitelist (`png`, `jpg`, `jpeg`, `webp`, `gif`).
/// - Caps the file size at 4 MB (defence in depth — the frontend also gates
///   at the same threshold, the server-side validation in `save_message`
///   accepts up to ~5.33 MB base64).
/// - Reads the file and returns `(data_base64, mime_type, size_bytes, name)`.
#[tauri::command]
#[instrument(name = "read_image_for_attachment", fields(path = %path))]
pub async fn read_image_for_attachment(path: String) -> Result<ImageReadResult, String> {
    use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};

    let path_buf = std::path::PathBuf::from(&path);
    let file_name = path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .map(String::from)
        .unwrap_or_else(|| "image".to_string());

    let ext = path_buf
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();

    let mime_type = match ext_to_image_mime(&ext) {
        Some(m) => m,
        None => {
            warn!(extension = %ext, "Picker received unsupported image extension");
            return Err(format!(
                "Unsupported image extension '{}'. Allowed: {}",
                ext,
                ALLOWED_IMAGE_EXTENSIONS.join(", ")
            ));
        }
    };

    let metadata = tokio::fs::metadata(&path_buf).await.map_err(|e| {
        warn!(error = %e, "Failed to stat picker image");
        format!("Failed to read image metadata: {}", e)
    })?;

    if metadata.len() > MAX_PICKER_IMAGE_SIZE_BYTES {
        return Err(format!(
            "Image too large ({} bytes, max {} bytes)",
            metadata.len(),
            MAX_PICKER_IMAGE_SIZE_BYTES
        ));
    }

    let bytes = tokio::fs::read(&path_buf).await.map_err(|e| {
        warn!(error = %e, "Failed to read picker image");
        format!("Failed to read image: {}", e)
    })?;

    let data_base64 = BASE64_STANDARD.encode(&bytes);

    info!(
        size_bytes = metadata.len(),
        mime = %mime_type,
        "Picker image encoded"
    );

    Ok(ImageReadResult {
        data_base64,
        mime_type: mime_type.to_string(),
        size_bytes: metadata.len(),
        name: file_name,
    })
}

/// Validate a folder path and return its canonical form.
/// Called from frontend when user selects a folder via dialog.
///
/// # Arguments
/// * `path` - The directory path selected by the user
///
/// # Returns
/// The canonical path string if valid, or error message
#[tauri::command]
#[instrument(name = "validate_agent_folder", fields(path = %path))]
pub async fn validate_agent_folder(path: String) -> Result<String, String> {
    info!("Validating folder for agent authorization");

    let canonical = validate_folder_for_authorization(&path).map_err(|e| {
        warn!(error = %e, "Folder validation failed");
        e
    })?;

    let canonical_str = canonical
        .to_str()
        .ok_or_else(|| "Path contains non-UTF8 characters".to_string())?
        .to_string();

    info!(canonical = %canonical_str, "Folder validated successfully");
    Ok(canonical_str)
}

/// List trash entries for a specific authorized folder.
///
/// # Arguments
/// * `folder_path` - The authorized folder path to list trash for
///
/// # Returns
/// A vector of `TrashEntry` describing each trash item, or error message
#[tauri::command]
#[instrument(name = "list_trash", fields(folder = %folder_path))]
pub async fn list_trash(folder_path: String) -> Result<Vec<TrashEntry>, String> {
    info!("Listing trash entries");

    // Authorize against the same allow-list `authorize_folder` uses. Without
    // this check, list_trash exposed `<folder>/.zileo-trash/` for any folder
    // the user could name — bypassing the agent sandbox.
    let canonical = validate_folder_for_authorization(&folder_path).map_err(|e| {
        warn!(error = %e, "Folder validation failed for list_trash");
        e.to_string()
    })?;

    trash_management::list_trash_entries(&canonical).map_err(|e| {
        warn!(error = %e, "Failed to list trash entries");
        e.to_string()
    })
}

/// Restore a file from trash to its original location.
///
/// # Arguments
/// * `trash_path` - Absolute path to the trash file to restore
/// * `folder_path` - The authorized folder to restore into
///
/// # Returns
/// The restored file path as a string, or error message
#[tauri::command]
#[instrument(name = "restore_from_trash_cmd", fields(trash = %trash_path, folder = %folder_path))]
pub async fn restore_from_trash_cmd(
    trash_path: String,
    folder_path: String,
) -> Result<String, String> {
    info!("Restoring file from trash");

    // Authorize the restore destination against the agent sandbox.
    let folder = validate_folder_for_authorization(&folder_path).map_err(|e| {
        warn!(error = %e, "Folder validation failed for restore_from_trash_cmd");
        e.to_string()
    })?;

    // The trash path must canonicalize to a child of the authorized folder's
    // .zileo-trash/. Without this check, an attacker controlling trash_path
    // could name any path on disk and escape the sandbox at the cost of an
    // overwrite (NB: restore_from_trash itself will reject obvious cases via
    // its own checks, but the path chunking is layered defense).
    let trash_canonical = std::fs::canonicalize(&trash_path).map_err(|e| {
        warn!(error = %e, trash = %trash_path, "Trash path canonicalization failed");
        format!("Invalid trash path: {}", e)
    })?;
    let trash_root = folder.join(".zileo-trash");
    let trash_root_canonical = std::fs::canonicalize(&trash_root).map_err(|e| {
        warn!(error = %e, "Trash root canonicalization failed");
        format!("Trash directory not found: {}", e)
    })?;
    if !trash_canonical.starts_with(&trash_root_canonical) {
        let msg = "Trash path is outside the authorized folder's trash directory".to_string();
        warn!(%msg, trash = ?trash_canonical, expected_root = ?trash_root_canonical);
        return Err(msg);
    }

    let restored =
        trash_management::restore_from_trash(&trash_canonical, &folder).map_err(|e| {
            warn!(error = %e, "Failed to restore from trash");
            e.to_string()
        })?;

    let restored_str = restored
        .to_str()
        .ok_or_else(|| "Restored path contains non-UTF8 characters".to_string())?
        .to_string();

    info!(restored = %restored_str, "File restored from trash");
    Ok(restored_str)
}
