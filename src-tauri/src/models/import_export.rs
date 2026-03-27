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

//! Import/Export Settings Models (Schema v1.1)
//!
//! Types for exporting and importing configuration entities:
//! Agents, MCP Servers, Models, Prompts, Skills, Custom Providers.
//! Synchronized with src/types/import-export.ts
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
//! 2. Backend validates and returns ImportValidation (with structured warnings)
//! 3. User resolves conflicts via ConflictResolution
//! 4. Backend executes import and returns ImportResult (with post-import actions)
//!
//! # Schema v1.1 Changes (backward compatible with v1.0)
//! - Added skills and custom_providers to ExportPackage
//! - Added missing agent fields (folders, require_file_confirmation, llm.is_reasoning, llm.context_window)
//! - Structured ImportWarning replaces plain string warnings
//! - Post-import actions in ImportResult
//! - Cross-entity dependency validation

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::models::agent::ReasoningEffort;

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
    /// Skill IDs to export
    #[serde(default)]
    pub skills: Vec<String>,
    /// Custom Provider names to export
    #[serde(default)]
    pub custom_providers: Vec<String>,
}

impl ExportSelection {
    /// Returns true if no entities are selected
    pub fn is_empty(&self) -> bool {
        self.agents.is_empty()
            && self.mcp_servers.is_empty()
            && self.models.is_empty()
            && self.prompts.is_empty()
            && self.skills.is_empty()
            && self.custom_providers.is_empty()
    }

    /// Returns total count of selected entities
    pub fn total_count(&self) -> usize {
        self.agents.len()
            + self.mcp_servers.len()
            + self.models.len()
            + self.prompts.len()
            + self.skills.len()
            + self.custom_providers.len()
    }
}

/// Export configuration options.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    /// Export format (JSON)
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
    #[serde(default)]
    pub skills: usize,
    #[serde(default)]
    pub custom_providers: usize,
}

/// Complete export package containing manifest and all entities.
/// Schema v1.1: Added skills and custom_providers (backward compatible with v1.0).
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
    /// Exported skill definitions (v1.1)
    #[serde(default)]
    pub skills: Vec<SkillExportData>,
    /// Exported custom provider configurations (v1.1)
    #[serde(default)]
    pub custom_providers: Vec<CustomProviderExportData>,
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
    #[serde(default)]
    pub skills: Vec<String>,
    pub system_prompt: String,
    pub max_tool_iterations: usize,
    /// Reasoning effort for thinking models (None = disabled)
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
    /// Authorized folder paths (v1.1, machine-specific)
    #[serde(default)]
    pub folders: Vec<String>,
    /// Whether file operations require user confirmation (v1.1)
    #[serde(default = "default_require_file_confirmation")]
    pub require_file_confirmation: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

fn default_require_file_confirmation() -> bool {
    true
}

/// LLM config for export.
/// v1.1: Added is_reasoning and context_window.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMConfigExport {
    pub provider: String,
    pub model: String,
    pub temperature: f64,
    pub max_tokens: usize,
    /// Whether the model supports reasoning/thinking (v1.1)
    #[serde(default)]
    pub is_reasoning: bool,
    /// Context window size override (v1.1, None = provider default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_window: Option<usize>,
}

/// MCP Server data for export.
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMModelExportData {
    pub provider: String,
    /// Model name - used as unique identifier for import conflict detection
    pub name: String,
    pub api_name: String,
    pub context_window: usize,
    pub max_output_tokens: usize,
    pub temperature_default: f64,
    pub is_builtin: bool,
    #[serde(default)]
    pub is_reasoning: bool,
    #[serde(default)]
    pub input_price_per_mtok: f64,
    #[serde(default)]
    pub output_price_per_mtok: f64,
    #[serde(default)]
    pub cache_read_price_per_mtok: f64,
    #[serde(default)]
    pub cache_write_price_per_mtok: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Prompt data for export.
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

/// Skill data for export (v1.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillExportData {
    /// Skill name - used as unique identifier for import conflict detection
    pub name: String,
    pub description: String,
    /// Category: system, coding, workflow, analysis, custom
    pub category: String,
    pub content: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

/// Custom provider data for export (v1.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderExportData {
    /// URL-safe identifier (e.g., "routerlab", "openrouter")
    pub name: String,
    /// Human-readable display name
    pub display_name: String,
    /// API base URL
    pub base_url: String,
    pub enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

/// Preview data returned before finalizing export.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreviewData {
    pub agents: Vec<AgentExportSummary>,
    pub mcp_servers: Vec<MCPServerExportSummary>,
    pub models: Vec<LLMModelExportSummary>,
    pub prompts: Vec<PromptExportSummary>,
    #[serde(default)]
    pub skills: Vec<SkillExportSummary>,
    #[serde(default)]
    pub custom_providers: Vec<CustomProviderExportSummary>,
    /// Map of server_id -> env var key names
    pub mcp_env_keys: HashMap<String, Vec<String>>,
}

/// Agent summary for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub lifecycle: String,
    pub provider: String,
    pub model: String,
    pub tools_count: usize,
    pub mcp_servers_count: usize,
    #[serde(default)]
    pub skills_count: usize,
    #[serde(default)]
    pub folders_count: usize,
}

/// MCP server summary for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MCPServerExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub enabled: bool,
    pub command: String,
    pub tools_count: usize,
}

/// LLM model summary for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LLMModelExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub provider: String,
    pub api_name: String,
    pub is_builtin: bool,
}

/// Prompt summary for preview.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub category: String,
    pub variables_count: usize,
}

/// Skill summary for preview (v1.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub category: String,
    pub enabled: bool,
    pub content_length: usize,
}

/// Custom provider summary for preview (v1.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderExportSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub name: String,
    pub display_name: String,
    pub base_url: String,
}

/// Selection of entities to import.
/// Note: These are entity NAMES, not IDs (IDs are not in the export file).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSelection {
    pub agents: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub models: Vec<String>,
    pub prompts: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub custom_providers: Vec<String>,
}

/// Import conflict information.
/// Conflicts are detected by NAME only (IDs are not exported).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportConflict {
    /// Type of entity ("agent", "mcp", "model", "prompt", "skill", "custom_provider")
    pub entity_type: String,
    /// Name of the entity being imported
    pub entity_name: String,
    /// ID of the existing entity in the database
    pub existing_id: String,
}

/// How to resolve an import conflict.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConflictResolution {
    #[default]
    Skip,
    Overwrite,
    Rename,
}

/// Additional env vars/args for MCP import.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MCPAdditions {
    pub add_env: HashMap<String, String>,
    pub add_args: Vec<String>,
}

/// Structured import warning with actionable context (v1.1).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportWarning {
    /// Warning category
    pub warning_type: ImportWarningType,
    /// Severity: "info", "medium", "high"
    pub severity: String,
    /// Which entity is affected (e.g., "Agent 'CodeReviewer'")
    pub entity: String,
    /// What the problem is
    pub detail: String,
    /// What the user should do after import
    pub action: String,
}

/// Category of import warning.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ImportWarningType {
    /// A referenced entity (skill, model, MCP, provider) not found
    MissingDependency,
    /// Folder paths are machine-specific
    MachineSpecific,
    /// Fields defaulted due to v1.0 schema (is_reasoning, context_window)
    DefaultApplied,
    /// Builtin model reimport
    BuiltinModel,
}

/// Import validation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportValidation {
    pub valid: bool,
    pub schema_version: String,
    /// Validation errors (blocking)
    pub errors: Vec<String>,
    /// Structured validation warnings (non-blocking, v1.1)
    pub warnings: Vec<ImportWarning>,
    /// Entities found in the import file
    pub entities: ImportEntities,
    /// Detected conflicts
    pub conflicts: Vec<ImportConflict>,
    /// Map of server_name -> missing required env var keys
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
    #[serde(default)]
    pub skills: Vec<SkillExportSummary>,
    #[serde(default)]
    pub custom_providers: Vec<CustomProviderExportSummary>,
}

/// Import operation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success: bool,
    pub imported: ImportCounts,
    pub skipped: ImportCounts,
    pub errors: Vec<ImportError>,
    /// Actionable items for the user to check after import (v1.1)
    #[serde(default)]
    pub post_import_actions: Vec<String>,
}

/// Entity import counts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportCounts {
    pub agents: usize,
    pub mcp_servers: usize,
    pub models: usize,
    pub prompts: usize,
    #[serde(default)]
    pub skills: usize,
    #[serde(default)]
    pub custom_providers: usize,
}

/// Individual entity import error.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportError {
    pub entity_type: String,
    pub entity_id: String,
    pub error: String,
}

/// Current schema version for export packages
pub const EXPORT_SCHEMA_VERSION: &str = "1.1";

/// Supported schema versions for import (backward compatibility)
pub const SUPPORTED_SCHEMA_VERSIONS: &[&str] = &["1.0", "1.1"];

/// Application version (read from Cargo.toml at compile time)
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

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

impl ExportPackage {
    /// Creates a new export package with the given entities.
    pub fn new(
        agents: Vec<AgentExportData>,
        mcp_servers: Vec<MCPServerExportData>,
        models: Vec<LLMModelExportData>,
        prompts: Vec<PromptExportData>,
        skills: Vec<SkillExportData>,
        custom_providers: Vec<CustomProviderExportData>,
        description: Option<String>,
    ) -> Self {
        let counts = ExportCounts {
            agents: agents.len(),
            mcp_servers: mcp_servers.len(),
            models: models.len(),
            prompts: prompts.len(),
            skills: skills.len(),
            custom_providers: custom_providers.len(),
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
            skills,
            custom_providers,
        }
    }
}

impl ImportValidation {
    /// Creates a validation result for an invalid import.
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
                skills: Vec::new(),
                custom_providers: Vec::new(),
            },
            conflicts: Vec::new(),
            missing_mcp_env: HashMap::new(),
        }
    }
}

/// Checks if an env var key matches sensitive patterns.
pub fn is_sensitive_env_key(key: &str) -> bool {
    let upper = key.to_uppercase();
    SENSITIVE_ENV_PATTERNS
        .iter()
        .any(|pattern| upper.contains(pattern))
}

/// Extracts the custom provider name from a provider string.
/// Returns Some("name") for "Custom(name)", None for builtin providers.
pub fn extract_custom_provider_name(provider: &str) -> Option<String> {
    if provider.starts_with("Custom(") && provider.ends_with(')') {
        Some(provider[7..provider.len() - 1].to_string())
    } else {
        None
    }
}

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
            skills: vec![],
            custom_providers: vec![],
        };
        assert!(empty.is_empty());

        let with_agent = ExportSelection {
            agents: vec!["agent1".to_string()],
            mcp_servers: vec![],
            models: vec![],
            prompts: vec![],
            skills: vec![],
            custom_providers: vec![],
        };
        assert!(!with_agent.is_empty());

        let with_skill_only = ExportSelection {
            agents: vec![],
            mcp_servers: vec![],
            models: vec![],
            prompts: vec![],
            skills: vec!["s1".to_string()],
            custom_providers: vec![],
        };
        assert!(!with_skill_only.is_empty());
    }

    #[test]
    fn test_export_selection_total_count() {
        let selection = ExportSelection {
            agents: vec!["a1".to_string(), "a2".to_string()],
            mcp_servers: vec!["m1".to_string()],
            models: vec![],
            prompts: vec!["p1".to_string(), "p2".to_string(), "p3".to_string()],
            skills: vec!["s1".to_string()],
            custom_providers: vec!["cp1".to_string()],
        };
        assert_eq!(selection.total_count(), 8);
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
            vec![],
            vec![],
            Some("Test export".to_string()),
        );

        assert_eq!(package.manifest.version, EXPORT_SCHEMA_VERSION);
        assert_eq!(package.manifest.version, "1.1");
        assert_eq!(package.manifest.app_version, env!("CARGO_PKG_VERSION"));
        assert!(package.manifest.description.is_some());
        assert_eq!(package.manifest.counts.agents, 0);
        assert_eq!(package.manifest.counts.skills, 0);
        assert_eq!(package.manifest.counts.custom_providers, 0);
    }

    #[test]
    fn test_import_validation_invalid() {
        let validation = ImportValidation::invalid(vec!["Invalid JSON".to_string()]);
        assert!(!validation.valid);
        assert_eq!(validation.errors.len(), 1);
        assert!(validation.entities.agents.is_empty());
        assert!(validation.entities.skills.is_empty());
        assert!(validation.entities.custom_providers.is_empty());
    }

    #[test]
    fn test_v1_0_backward_compat_deserialization() {
        // Simulate a v1.0 export package JSON (no skills, no custom_providers, no agent folders)
        let v1_0_json = r#"{
            "manifest": {
                "version": "1.0",
                "appVersion": "0.1.0",
                "exportedAt": "2026-01-01T00:00:00Z",
                "counts": { "agents": 1, "mcpServers": 0, "models": 0, "prompts": 0 }
            },
            "agents": [{
                "name": "TestAgent",
                "lifecycle": "permanent",
                "llm": { "provider": "Mistral", "model": "mistral-small", "temperature": 0.7, "maxTokens": 4096 },
                "tools": [],
                "mcpServers": [],
                "skills": ["old-skill"],
                "systemPrompt": "test",
                "maxToolIterations": 50
            }],
            "mcpServers": [],
            "models": [],
            "prompts": []
        }"#;

        let package: ExportPackage = serde_json::from_str(v1_0_json).unwrap();
        assert_eq!(package.manifest.version, "1.0");
        // v1.0 has no skills/custom_providers fields -> default to empty
        assert!(package.skills.is_empty());
        assert!(package.custom_providers.is_empty());
        // Agent should have defaults for new fields
        let agent = &package.agents[0];
        assert!(agent.folders.is_empty());
        assert!(agent.require_file_confirmation); // defaults to true
        assert!(!agent.llm.is_reasoning); // defaults to false
        assert!(agent.llm.context_window.is_none());
    }

    #[test]
    fn test_import_warning_serialization() {
        let warning = ImportWarning {
            warning_type: ImportWarningType::MissingDependency,
            severity: "high".to_string(),
            entity: "Agent 'CodeReviewer'".to_string(),
            detail: "skill 'python-bp' not found".to_string(),
            action: "Create the skill after import".to_string(),
        };
        let json = serde_json::to_string(&warning).unwrap();
        assert!(json.contains("\"warningType\":\"missing_dependency\""));
        assert!(json.contains("\"severity\":\"high\""));

        let warning2 = ImportWarning {
            warning_type: ImportWarningType::MachineSpecific,
            severity: "info".to_string(),
            entity: "Agent 'FileHelper'".to_string(),
            detail: "3 folder paths".to_string(),
            action: "Verify paths".to_string(),
        };
        let json2 = serde_json::to_string(&warning2).unwrap();
        assert!(json2.contains("\"warningType\":\"machine_specific\""));
    }

    #[test]
    fn test_extract_custom_provider_name() {
        assert_eq!(
            extract_custom_provider_name("Custom(routerlab)"),
            Some("routerlab".to_string())
        );
        assert_eq!(
            extract_custom_provider_name("Custom(openrouter)"),
            Some("openrouter".to_string())
        );
        assert_eq!(extract_custom_provider_name("Mistral"), None);
        assert_eq!(extract_custom_provider_name("Ollama"), None);
        assert_eq!(extract_custom_provider_name(""), None);
    }

    #[test]
    fn test_import_result_with_post_actions() {
        let result = ImportResult {
            success: true,
            imported: ImportCounts {
                agents: 2,
                skills: 1,
                ..Default::default()
            },
            skipped: ImportCounts::default(),
            errors: vec![],
            post_import_actions: vec![
                "Agent 'X': verify folder paths".to_string(),
                "Agent 'Y': create skill 'z'".to_string(),
            ],
        };
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("postImportActions"));
        assert!(json.contains("verify folder paths"));
    }

    #[test]
    fn test_supported_schema_versions() {
        assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&"1.0"));
        assert!(SUPPORTED_SCHEMA_VERSIONS.contains(&"1.1"));
        assert!(!SUPPORTED_SCHEMA_VERSIONS.contains(&"2.0"));
    }

    #[test]
    fn test_skill_export_data_serialization() {
        let skill = SkillExportData {
            name: "coding-standards".to_string(),
            description: "Coding standards guide".to_string(),
            category: "coding".to_string(),
            content: "# Standards\n...".to_string(),
            enabled: true,
            created_at: None,
            updated_at: None,
        };
        let json = serde_json::to_string(&skill).unwrap();
        assert!(json.contains("\"name\":\"coding-standards\""));
        assert!(json.contains("\"enabled\":true"));
        // created_at/updated_at should be omitted when None
        assert!(!json.contains("createdAt"));
    }

    #[test]
    fn test_custom_provider_export_data_serialization() {
        let provider = CustomProviderExportData {
            name: "routerlab".to_string(),
            display_name: "RouterLab".to_string(),
            base_url: "https://api.routerlab.ch/v1".to_string(),
            enabled: true,
            created_at: None,
        };
        let json = serde_json::to_string(&provider).unwrap();
        assert!(json.contains("\"name\":\"routerlab\""));
        assert!(json.contains("\"displayName\":\"RouterLab\""));
        assert!(json.contains("\"baseUrl\":\"https://api.routerlab.ch/v1\""));
    }
}
