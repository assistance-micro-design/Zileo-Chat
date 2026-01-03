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

//! Prompt Library Models
//!
//! Types for managing reusable prompt templates with variable interpolation.
//! Synchronized with src/types/prompt.ts

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// ===== Enums =====

/// Category for organizing prompts
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PromptCategory {
    System,
    User,
    Analysis,
    Generation,
    Coding,
    #[default]
    Custom,
}

impl std::fmt::Display for PromptCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::System => write!(f, "system"),
            Self::User => write!(f, "user"),
            Self::Analysis => write!(f, "analysis"),
            Self::Generation => write!(f, "generation"),
            Self::Coding => write!(f, "coding"),
            Self::Custom => write!(f, "custom"),
        }
    }
}

// ===== Structs =====

/// Variable detected in prompt content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVariable {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "defaultValue"
    )]
    pub default_value: Option<String>,
}

/// Full prompt entity (from database)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Prompt {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: PromptCategory,
    pub content: String,
    #[serde(default)]
    pub variables: Vec<PromptVariable>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Lightweight prompt for list display
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSummary {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: PromptCategory,
    pub variables_count: u32,
    pub updated_at: DateTime<Utc>,
}

/// Data for creating a new prompt
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptCreate {
    pub name: String,
    pub description: String,
    pub category: PromptCategory,
    pub content: String,
}

/// Data for updating an existing prompt (all fields optional)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptUpdate {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category: Option<PromptCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

// ===== Variable Detection and Interpolation =====

impl Prompt {
    /// Extract variables from content using {{variable_name}} pattern
    ///
    /// # Example
    /// ```
    /// use zileo_chat::models::Prompt;
    /// let content = "Hello {{user_name}}, your task is {{task}}";
    /// let vars = Prompt::detect_variables(content);
    /// assert_eq!(vars.len(), 2);
    /// ```
    pub fn detect_variables(content: &str) -> Vec<PromptVariable> {
        let pattern =
            Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}").expect("Invalid regex pattern");

        let mut seen = HashSet::new();
        let mut variables = Vec::new();

        for cap in pattern.captures_iter(content) {
            let name = cap[1].to_string();
            if seen.insert(name.clone()) {
                variables.push(PromptVariable {
                    name,
                    description: None,
                    default_value: None,
                });
            }
        }

        variables
    }

    /// Interpolate variables into content
    ///
    /// Variables not found in the values map are left unchanged.
    ///
    /// # Example
    /// ```
    /// use zileo_chat::models::Prompt;
    /// use std::collections::HashMap;
    /// let content = "Hello {{name}}!";
    /// let mut values = HashMap::new();
    /// values.insert("name".to_string(), "Alice".to_string());
    /// let result = Prompt::interpolate(content, &values);
    /// assert_eq!(result, "Hello Alice!");
    /// ```
    #[allow(dead_code)] // Used in Phase 3 (Frontend)
    pub fn interpolate(content: &str, values: &HashMap<String, String>) -> String {
        let pattern =
            Regex::new(r"\{\{([a-zA-Z_][a-zA-Z0-9_]*)\}\}").expect("Invalid regex pattern");

        pattern
            .replace_all(content, |caps: &regex::Captures| {
                let key = &caps[1];
                values
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| format!("{{{{{}}}}}", key))
            })
            .into_owned()
    }
}

impl From<&Prompt> for PromptSummary {
    fn from(prompt: &Prompt) -> Self {
        Self {
            id: prompt.id.clone(),
            name: prompt.name.clone(),
            description: prompt.description.clone(),
            category: prompt.category.clone(),
            variables_count: prompt.variables.len() as u32,
            updated_at: prompt.updated_at,
        }
    }
}

// ===== Validation Constants =====

pub const MAX_PROMPT_NAME_LEN: usize = 128;
pub const MAX_PROMPT_DESCRIPTION_LEN: usize = 1000;
pub const MAX_PROMPT_CONTENT_LEN: usize = 50000;

// ===== Tests =====

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_variables_single() {
        let content = "Hello {{user_name}}";
        let vars = Prompt::detect_variables(content);
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].name, "user_name");
    }

    #[test]
    fn test_detect_variables_multiple() {
        let content = "Hello {{user}}, your {{task}} is ready";
        let vars = Prompt::detect_variables(content);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "user");
        assert_eq!(vars[1].name, "task");
    }

    #[test]
    fn test_detect_variables_dedup() {
        let content = "{{x}} and {{y}} and {{x}}";
        let vars = Prompt::detect_variables(content);
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_detect_variables_underscore() {
        let content = "{{user_name}} and {{task_description}}";
        let vars = Prompt::detect_variables(content);
        assert_eq!(vars.len(), 2);
        assert_eq!(vars[0].name, "user_name");
        assert_eq!(vars[1].name, "task_description");
    }

    #[test]
    fn test_detect_variables_alphanumeric() {
        let content = "{{var1}} and {{variable2}}";
        let vars = Prompt::detect_variables(content);
        assert_eq!(vars.len(), 2);
    }

    #[test]
    fn test_detect_variables_no_match() {
        let content = "Hello world, no variables here";
        let vars = Prompt::detect_variables(content);
        assert!(vars.is_empty());
    }

    #[test]
    fn test_interpolate_basic() {
        let content = "Hello {{name}}!";
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Alice".to_string());
        let result = Prompt::interpolate(content, &values);
        assert_eq!(result, "Hello Alice!");
    }

    #[test]
    fn test_interpolate_multiple() {
        let content = "{{greeting}} {{name}}, welcome to {{place}}!";
        let mut values = HashMap::new();
        values.insert("greeting".to_string(), "Hello".to_string());
        values.insert("name".to_string(), "Bob".to_string());
        values.insert("place".to_string(), "Paris".to_string());
        let result = Prompt::interpolate(content, &values);
        assert_eq!(result, "Hello Bob, welcome to Paris!");
    }

    #[test]
    fn test_interpolate_missing_var() {
        let content = "Hello {{name}}!";
        let values = HashMap::new();
        let result = Prompt::interpolate(content, &values);
        assert_eq!(result, "Hello {{name}}!");
    }

    #[test]
    fn test_interpolate_partial() {
        let content = "Hello {{name}}, task: {{task}}";
        let mut values = HashMap::new();
        values.insert("name".to_string(), "Charlie".to_string());
        let result = Prompt::interpolate(content, &values);
        assert_eq!(result, "Hello Charlie, task: {{task}}");
    }

    #[test]
    fn test_category_display() {
        assert_eq!(PromptCategory::System.to_string(), "system");
        assert_eq!(PromptCategory::User.to_string(), "user");
        assert_eq!(PromptCategory::Analysis.to_string(), "analysis");
        assert_eq!(PromptCategory::Generation.to_string(), "generation");
        assert_eq!(PromptCategory::Coding.to_string(), "coding");
        assert_eq!(PromptCategory::Custom.to_string(), "custom");
    }

    #[test]
    fn test_prompt_summary_from() {
        let prompt = Prompt {
            id: "test-id".to_string(),
            name: "Test Prompt".to_string(),
            description: "Test description".to_string(),
            category: PromptCategory::System,
            content: "Hello {{name}}".to_string(),
            variables: vec![PromptVariable {
                name: "name".to_string(),
                description: None,
                default_value: None,
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let summary = PromptSummary::from(&prompt);
        assert_eq!(summary.id, "test-id");
        assert_eq!(summary.name, "Test Prompt");
        assert_eq!(summary.variables_count, 1);
    }
}
