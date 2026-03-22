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

//! Workflow folder model for organizing workflows into named groups.

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Workflow folder for organizing workflows into groups.
///
/// Folders have a name, color (hex), and sort order for display.
/// Deleting a folder orphans its workflows (folder_id set to NULL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowFolder {
    /// Unique identifier (deserialized from SurrealDB Thing type)
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    /// Folder display name
    pub name: String,
    /// Hex color for the folder indicator (e.g. "#3b82f6")
    pub color: String,
    /// Position in the folder list (lower = higher)
    #[serde(default)]
    pub sort_order: i64,
    /// Creation timestamp
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,
    /// Last update timestamp
    #[serde(default = "Utc::now")]
    pub updated_at: DateTime<Utc>,
}

/// Payload for creating a new workflow folder.
///
/// ID is generated separately and passed via table:id format.
/// Timestamps are set by database defaults.
#[derive(Debug, Clone, Serialize)]
pub struct WorkflowFolderCreate {
    /// Folder display name
    pub name: String,
    /// Hex color for the folder indicator
    pub color: String,
    /// Position in the folder list
    pub sort_order: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_folder_serialization() {
        let folder = WorkflowFolder {
            id: "folder_001".to_string(),
            name: "Project Alpha".to_string(),
            color: "#3b82f6".to_string(),
            sort_order: 0,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let json = serde_json::to_string(&folder).unwrap();
        let deserialized: WorkflowFolder = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, folder.id);
        assert_eq!(deserialized.name, folder.name);
        assert_eq!(deserialized.color, folder.color);
        assert_eq!(deserialized.sort_order, 0);
    }

    #[test]
    fn test_workflow_folder_create_serialization() {
        let create = WorkflowFolderCreate {
            name: "Test Folder".to_string(),
            color: "#ef4444".to_string(),
            sort_order: 1,
        };

        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"name\":\"Test Folder\""));
        assert!(json.contains("\"color\":\"#ef4444\""));
        assert!(json.contains("\"sort_order\":1"));
    }

    #[test]
    fn test_workflow_folder_default_sort_order() {
        let json = r##"{"id": "f1", "name": "Folder", "color": "#000000", "created_at": "2026-01-01T00:00:00Z", "updated_at": "2026-01-01T00:00:00Z"}"##;
        let folder: WorkflowFolder = serde_json::from_str(json).unwrap();
        assert_eq!(folder.sort_order, 0);
    }
}
