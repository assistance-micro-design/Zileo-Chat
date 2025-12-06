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

//! Import/Export Settings Models
//!
//! Types for exporting and importing configuration entities (Agents, MCP Servers, Models, Prompts).
//! Synchronized with src/types/importExport.ts
//!
//! # Export Flow
//! 1. User selects entities via ExportSelection
//! 2. Backend returns ExportPreviewData for preview
//! 3. User configures MCPSanitizationConfig for sensitive data
//! 4. Backend generates ExportPackage as JSON string
//! 5. Frontend triggers file download
//!
//! # Import Flow
//! 1. User uploads JSON file
//! 2. Backend validates and returns ImportValidation
//! 3. User resolves conflicts via ConflictResolution
//! 4. Backend executes import and returns ImportResult

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Export Types
// ============================================================================

/// Selection of entities to export.
/// At least one entity must be selected.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSelection {
    /// Agent IDs to export
    pub agents: Vec<String>,
    /// MCP Server IDs to export
    pub mcp_servers: Vec<String>,
    /// Model IDs to export (custom only recommended)
    pub models: Vec<String>,
    /// Prompt IDs to export
    pub prompts: Vec<String>,
}

impl ExportSelection {
    /// Returns true if at least one entity is selected
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
            && self.mcp_servers.is_empty()
            && self.models.is_empty()
            && self.prompts.is_empty()
    }

    /// Returns total count of selected entities
    pub fn total_count(&self) -> usize {
        self.agents.len() + self.mcp_servers.len() + self.models.len() + self.prompts.len()
    }
}

/// Export configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// Export format (JSON only in Phase 1)
    pub format: String,
    /// Whether to include created_at/updated_at timestamps
    pub include_timestamps: bool,
    /// Whether to enable MCP env var sanitization
    pub sanitize_mcp: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            format: "json".to_string(),
            include_timestamps: true,
            sanitize_mcp: true,
        }
    }
}

/// MCP server sanitization configuration for export.
/// Allows clearing or modifying sensitive environment variables.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPSanitizationConfig {
    /// Env var keys to clear (set to empty string)
    pub clear_env_keys: Vec<String>,
    /// Env var values to modify/override
    pub modify_env: HashMap<String, String>,
    /// Modified command args (optional)
    pub modify_args: Vec<String>,
    /// If true, skip this server entirely from export
    pub exclude_from_export: bool,
}

/// Export manifest with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    /// Schema version for compatibility checking
    pub version: String,
    /// Application version that created the export
    pub app_version: String,
    /// ISO 8601 timestamp of export
    pub exported_at: String,
    /// Optional identifier of who exported
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exported_by: Option<String>,
    /// Optional user description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Entity counts
    pub counts: ExportCounts,
}

/// Entity counts in an export package.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ExportCounts {
    pub agents: usize,
    pub mcp_servers: usize,
    pub models: usize,
    pub prompts: usize,
}

/// Complete export package containing manifest and all entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPackage {
    /// Export metadata
    pub manifest: ExportManifest,
    /// Exported agent configurations
    pub agents: Vec<AgentExportData>,
    /// Exported MCP server configurations
    pub mcp_servers: Vec<MCPServerExportData>,
    /// Exported LLM model definitions
    pub models: Vec<LLMModelExportData>,
    /// Exported prompt templates
    pub prompts: Vec<PromptExportData>,
}

/// Agent data for export.
/// Note: IDs are NOT exported - entities are identified by NAME.
/// A new UUID is generated on import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExportData {
    /// Agent name - used as unique identifier for import conflict detection
    pub name: String,
    pub lifecycle: String,
    pub llm: LLMConfigExport,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub system_prompt: String,
    pub max_tool_iterations: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// LLM config for export (simplified).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMConfigExport {
    pub provider: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: usize,
}

/// MCP Server data for export.
/// Note: IDs are NOT exported - entities are identified by NAME.
/// A new UUID is generated on import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerExportData {
    /// Server name - used as unique identifier for import conflict detection
    pub name: String,
    pub enabled: bool,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// LLM Model data for export.
/// Note: IDs are NOT exported - entities are identified by NAME.
/// A new UUID is generated on import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMModelExportData {
    pub provider: String,
    /// Model name - used as unique identifier for import conflict detection
    pub name: String,
    pub api_name: String,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub temperature_default: f32,
    pub is_builtin: bool,
    #[serde(default)]
    pub is_reasoning: bool,
    #[serde(default)]
    pub input_price_per_mtok: f64,
    #[serde(default)]
    pub output_price_per_mtok: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Prompt data for export.
/// Note: IDs are NOT exported - entities are identified by NAME.
/// A new UUID is generated on import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptExportData {
    /// Prompt name - used as unique identifier for import conflict detection
    pub name: String,
    pub description: String,
    pub category: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Preview data returned before finalizing export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreviewData {
    /// Agent summaries
    pub agents: Vec<AgentExportSummary>,
    /// MCP server summaries
    pub mcp_servers: Vec<MCPServerExportSummary>,
    /// Model summaries
    pub models: Vec<LLMModelExportSummary>,
    /// Prompt summaries
    pub prompts: Vec<PromptExportSummary>,
    /// Map of server_id -> env var key names
    pub mcp_env_keys: HashMap<String, Vec<String>>,
}

/// Agent summary for preview (export preview has ID, import preview doesn't).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExportSummary {
    /// ID is present for export preview (from DB), absent for import preview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Name is the unique identifier for import
    pub name: String,
    pub lifecycle: String,
    pub provider: String,
    pub model: String,
    pub tools_count: usize,
    pub mcp_servers_count: usize,
}

/// MCP server summary for preview (export preview has ID, import preview doesn't).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerExportSummary {
    /// ID is present for export preview (from DB), absent for import preview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Name is the unique identifier for import
    pub name: String,
    pub enabled: bool,
    pub command: String,
    pub tools_count: usize,
}

/// LLM model summary for preview (export preview has ID, import preview doesn't).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMModelExportSummary {
    /// ID is present for export preview (from DB), absent for import preview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Name is the unique identifier for import
    pub name: String,
    pub provider: String,
    pub api_name: String,
    pub is_builtin: bool,
}

/// Prompt summary for preview (export preview has ID, import preview doesn't).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptExportSummary {
    /// ID is present for export preview (from DB), absent for import preview
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Name is the unique identifier for import
    pub name: String,
    pub description: String,
    pub category: String,
    pub variables_count: usize,
}

// ============================================================================
// Import Types
// ============================================================================

/// Selection of entities to import.
/// Note: These are entity NAMES, not IDs (IDs are not in the export file).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelection {
    /// Agent names to import
    pub agents: Vec<String>,
    /// MCP server names to import
    pub mcp_servers: Vec<String>,
    /// Model names to import
    pub models: Vec<String>,
    /// Prompt names to import
    pub prompts: Vec<String>,
}

/// Import conflict information.
/// Conflicts are detected by NAME only (IDs are not exported).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    /// Type of entity ("agent", "mcp", "model", "prompt")
    pub entity_type: String,
    /// Name of the entity being imported - used as unique identifier
    pub entity_name: String,
    /// ID of the existing entity in the database
    pub existing_id: String,
}

/// How to resolve an import conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    /// Skip importing this entity
    Skip,
    /// Overwrite the existing entity
    Overwrite,
    /// Rename the imported entity (new ID generated)
    Rename,
}

impl Default for ConflictResolution {
    fn default() -> Self {
        Self::Skip
    }
}

/// Additional env vars/args for MCP import.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPAdditions {
    /// Additional environment variables
    pub add_env: HashMap<String, String>,
    /// Additional command arguments
    pub add_args: Vec<String>,
}

/// Import validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportValidation {
    /// Whether the import file is valid
    pub valid: bool,
    /// Schema version of the import file
    pub schema_version: String,
    /// Validation errors (blocking)
    pub errors: Vec<String>,
    /// Validation warnings (non-blocking)
    pub warnings: Vec<String>,
    /// Entities found in the import file
    pub entities: ImportEntities,
    /// Detected conflicts
    pub conflicts: Vec<ImportConflict>,
    /// Map of server_id -> missing required env var keys
    pub missing_mcp_env: HashMap<String, Vec<String>>,
}

/// Entity summaries from import file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntities {
    pub agents: Vec<AgentExportSummary>,
    pub mcp_servers: Vec<MCPServerExportSummary>,
    pub models: Vec<LLMModelExportSummary>,
    pub prompts: Vec<PromptExportSummary>,
}

/// Import operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    /// Whether import completed (may have partial failures)
    pub success: bool,
    /// Number of entities successfully imported
    pub imported: ImportCounts,
    /// Number of entities skipped
    pub skipped: ImportCounts,
    /// Import errors for individual entities
    pub errors: Vec<ImportError>,
}

/// Entity import counts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportCounts {
    pub agents: usize,
    pub mcp_servers: usize,
    pub models: usize,
    pub prompts: usize,
}

/// Individual entity import error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub entity_type: String,
    pub entity_id: String,
    pub error: String,
}

// ============================================================================
// Constants
// ============================================================================

/// Current schema version for export packages
pub const EXPORT_SCHEMA_VERSION: &str = "1.0";

/// Application version (should be read from Cargo.toml in production)
pub const APP_VERSION: &str = "0.1.0";

/// Maximum import file size in bytes (10MB)
pub const MAX_IMPORT_FILE_SIZE: usize = 10 * 1024 * 1024;

/// Sensitive env var key patterns to warn about
pub const SENSITIVE_ENV_PATTERNS: &[&str] = &[
    "API_KEY",
    "SECRET",
    "TOKEN",
    "PASSWORD",
    "CREDENTIAL",
    "PRIVATE_KEY",
];

// ============================================================================
// Helper Functions
// ============================================================================

impl ExportPackage {
    /// Creates a new export package with the given entities
    pub fn new(
        agents: Vec<AgentExportData>,
        mcp_servers: Vec<MCPServerExportData>,
        models: Vec<LLMModelExportData>,
        prompts: Vec<PromptExportData>,
        description: Option<String>,
    ) -> Self {
        let counts = ExportCounts {
            agents: agents.len(),
            mcp_servers: mcp_servers.len(),
            models: models.len(),
            prompts: prompts.len(),
        };

        let manifest = ExportManifest {
            version: EXPORT_SCHEMA_VERSION.to_string(),
            app_version: APP_VERSION.to_string(),
            exported_at: Utc::now().to_rfc3339(),
            exported_by: None,
            description,
            counts,
        };

        Self {
            manifest,
            agents,
            mcp_servers,
            models,
            prompts,
        }
    }
}

impl ImportValidation {
    /// Creates a validation result for an invalid import
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            schema_version: String::new(),
            errors,
            warnings: Vec::new(),
            entities: ImportEntities {
                agents: Vec::new(),
                mcp_servers: Vec::new(),
                models: Vec::new(),
                prompts: Vec::new(),
            },
            conflicts: Vec::new(),
            missing_mcp_env: HashMap::new(),
        }
    }
}

/// Checks if an env var key matches sensitive patterns
pub fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    SENSITIVE_ENV_PATTERNS
        .iter()
        .any(|pattern| upper.contains(pattern))
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_export_selection_is_empty() {
        let empty = ExportSelection {
            agents: vec![],
            mcp_servers: vec![],
            models: vec![],
            prompts: vec![],
        };
        assert!(empty.is_empty());

        let with_agent = ExportSelection {
            agents: vec!["agent1".to_string()],
            mcp_servers: vec![],
            models: vec![],
            prompts: vec![],
        };
        assert!(!with_agent.is_empty());
    }

    #[test]
    fn test_export_selection_total_count() {
        let selection = ExportSelection {
            agents: vec!["a1".to_string(), "a2".to_string()],
            mcp_servers: vec!["m1".to_string()],
            models: vec![],
            prompts: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
        };
        assert_eq!(selection.total_count(), 6);
    }

    #[test]
    fn test_is_sensitive_env_key() {
        assert!(is_sensitive_env_key("API_KEY"));
        assert!(is_sensitive_env_key("MISTRAL_API_KEY"));
        assert!(is_sensitive_env_key("secret_token"));
        assert!(is_sensitive_env_key("DB_PASSWORD"));
        assert!(!is_sensitive_env_key("DEBUG"));
        assert!(!is_sensitive_env_key("LOG_LEVEL"));
    }

    #[test]
    fn test_conflict_resolution_serialization() {
        let skip = ConflictResolution::Skip;
        let json = serde_json::to_string(&skip).unwrap();
        assert_eq!(json, "\"skip\"");

        let overwrite: ConflictResolution = serde_json::from_str("\"overwrite\"").unwrap();
        assert_eq!(overwrite, ConflictResolution::Overwrite);

        let rename: ConflictResolution = serde_json::from_str("\"rename\"").unwrap();
        assert_eq!(rename, ConflictResolution::Rename);
    }

    #[test]
    fn test_export_package_creation() {
        let package = ExportPackage::new(
            vec![],
            vec![],
            vec![],
            vec![],
            Some("Test export".to_string()),
        );

        assert_eq!(package.manifest.version, EXPORT_SCHEMA_VERSION);
        assert_eq!(package.manifest.app_version, APP_VERSION);
        assert!(package.manifest.description.is_some());
        assert_eq!(package.manifest.counts.agents, 0);
    }

    #[test]
    fn test_import_validation_invalid() {
        let validation = ImportValidation::invalid(vec!["Invalid JSON".to_string()]);
        assert!(!validation.valid);
        assert_eq!(validation.errors.len(), 1);
        assert!(validation.entities.agents.is_empty());
    }
}
