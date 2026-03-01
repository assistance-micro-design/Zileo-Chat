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

//! Skill Models
//!
//! Types for managing reusable skill documents (markdown instructions for agents).
//! Skills are assigned to agents and read on-demand by the LLM via ReadSkillTool.
//! Synchronized with src/types/skill.ts

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// Regex pattern for validating skill names: only alphanumeric, underscore, and hyphen.
static SKILL_NAME_PATTERN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zA-Z0-9_-]+$").expect("Invalid skill name regex pattern"));

// ===== Enums =====

/// Category for organizing skills
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum SkillCategory {
    System,
    Coding,
    Workflow,
    Analysis,
    #[default]
    Custom,
}

impl std::fmt::Display for SkillCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::Coding => write!(f, "coding"),
            Self::Workflow => write!(f, "workflow"),
            Self::Analysis => write!(f, "analysis"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

// ===== Structs =====

/// Full skill entity (from database)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub content: String,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight skill for list display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub enabled: bool,
    pub content_length: usize,
    pub updated_at: DateTime<Utc>,
}

/// Data for creating a new skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillCreate {
    pub name: String,
    pub description: String,
    pub category: SkillCategory,
    pub content: String,
}

/// Data for updating an existing skill (all fields optional)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<SkillCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ===== Validation =====

/// Maximum length for skill name
pub const MAX_SKILL_NAME_LEN: usize = 128;
/// Maximum length for skill description
pub const MAX_SKILL_DESCRIPTION_LEN: usize = 500;
/// Maximum length for skill content
pub const MAX_SKILL_CONTENT_LEN: usize = 50000;

/// Validates a skill name: trimmed, non-empty, 1-128 chars, [a-zA-Z0-9_-] only.
pub fn validate_skill_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Skill name cannot be empty".to_string());
    }
    if trimmed.len() > MAX_SKILL_NAME_LEN {
        return Err(format!(
            "Skill name exceeds maximum length of {} characters",
            MAX_SKILL_NAME_LEN
        ));
    }
    if !SKILL_NAME_PATTERN.is_match(trimmed) {
        return Err(
            "Skill name can only contain letters, digits, underscores, and hyphens".to_string(),
        );
    }
    Ok(trimmed.to_string())
}

/// Validates a skill description: trimmed, 1-500 chars.
pub fn validate_skill_description(description: &str) -> Result<String, String> {
    let trimmed = description.trim();
    if trimmed.is_empty() {
        return Err("Skill description cannot be empty".to_string());
    }
    if trimmed.len() > MAX_SKILL_DESCRIPTION_LEN {
        return Err(format!(
            "Skill description exceeds maximum length of {} characters",
            MAX_SKILL_DESCRIPTION_LEN
        ));
    }
    Ok(trimmed.to_string())
}

/// Validates skill content: non-empty, 1-50000 chars.
pub fn validate_skill_content(content: &str) -> Result<String, String> {
    if content.is_empty() {
        return Err("Skill content cannot be empty".to_string());
    }
    if content.len() > MAX_SKILL_CONTENT_LEN {
        return Err(format!(
            "Skill content exceeds maximum length of {} characters",
            MAX_SKILL_CONTENT_LEN
        ));
    }
    Ok(content.to_string())
}

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    // -- validate_skill_name tests --

    #[test]
    fn test_validate_skill_name_valid() {
        assert_eq!(
            validate_skill_name("coding-standards").unwrap(),
            "coding-standards"
        );
        assert_eq!(validate_skill_name("git_workflow").unwrap(), "git_workflow");
        assert_eq!(validate_skill_name("MySkill123").unwrap(), "MySkill123");
    }

    #[test]
    fn test_validate_skill_name_trims() {
        assert_eq!(validate_skill_name("  my-skill  ").unwrap(), "my-skill");
    }

    #[test]
    fn test_validate_skill_name_empty() {
        assert!(validate_skill_name("").is_err());
        assert!(validate_skill_name("   ").is_err());
    }

    #[test]
    fn test_validate_skill_name_too_long() {
        let long_name = "a".repeat(MAX_SKILL_NAME_LEN + 1);
        assert!(validate_skill_name(&long_name).is_err());
    }

    #[test]
    fn test_validate_skill_name_invalid_chars() {
        assert!(validate_skill_name("has spaces").is_err());
        assert!(validate_skill_name("has.dots").is_err());
        assert!(validate_skill_name("has/slash").is_err());
        assert!(validate_skill_name("has@at").is_err());
    }

    #[test]
    fn test_validate_skill_name_max_len() {
        let max_name = "a".repeat(MAX_SKILL_NAME_LEN);
        assert!(validate_skill_name(&max_name).is_ok());
    }

    // -- validate_skill_description tests --

    #[test]
    fn test_validate_skill_description_valid() {
        assert_eq!(
            validate_skill_description("Coding rules and conventions").unwrap(),
            "Coding rules and conventions"
        );
    }

    #[test]
    fn test_validate_skill_description_empty() {
        assert!(validate_skill_description("").is_err());
        assert!(validate_skill_description("   ").is_err());
    }

    #[test]
    fn test_validate_skill_description_too_long() {
        let long_desc = "a".repeat(MAX_SKILL_DESCRIPTION_LEN + 1);
        assert!(validate_skill_description(&long_desc).is_err());
    }

    // -- validate_skill_content tests --

    #[test]
    fn test_validate_skill_content_valid() {
        assert!(validate_skill_content("# My Skill\n\nSome content").is_ok());
    }

    #[test]
    fn test_validate_skill_content_empty() {
        assert!(validate_skill_content("").is_err());
    }

    #[test]
    fn test_validate_skill_content_too_long() {
        let long_content = "a".repeat(MAX_SKILL_CONTENT_LEN + 1);
        assert!(validate_skill_content(&long_content).is_err());
    }

    // -- Serialization tests --

    #[test]
    fn test_category_display() {
        assert_eq!(SkillCategory::System.to_string(), "system");
        assert_eq!(SkillCategory::Coding.to_string(), "coding");
        assert_eq!(SkillCategory::Workflow.to_string(), "workflow");
        assert_eq!(SkillCategory::Analysis.to_string(), "analysis");
        assert_eq!(SkillCategory::Custom.to_string(), "custom");
    }

    #[test]
    fn test_category_serde_roundtrip() {
        let cat = SkillCategory::Coding;
        let json = serde_json::to_string(&cat).unwrap();
        assert_eq!(json, "\"coding\"");
        let deserialized: SkillCategory = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, SkillCategory::Coding);
    }

    #[test]
    fn test_skill_create_serialization() {
        let create = SkillCreate {
            name: "test-skill".to_string(),
            description: "A test skill".to_string(),
            category: SkillCategory::Custom,
            content: "# Test\n\nContent here".to_string(),
        };
        let json = serde_json::to_string(&create).unwrap();
        assert!(json.contains("\"name\":\"test-skill\""));
        assert!(json.contains("\"category\":\"custom\""));
    }

    #[test]
    fn test_skill_update_skip_none() {
        let update = SkillUpdate {
            name: Some("new-name".to_string()),
            description: None,
            category: None,
            content: None,
            enabled: None,
        };
        let json = serde_json::to_string(&update).unwrap();
        assert!(json.contains("\"name\":\"new-name\""));
        assert!(!json.contains("description"));
        assert!(!json.contains("category"));
    }
}
