// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Snapshot of a prompt taken before any modification (versioning anti-loss).

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A historical snapshot of a prompt taken before an update.
///
/// `edited_by` is either `"user"` or `"agent:<agent_id>"`. When an agent edits,
/// `edit_summary` is required (validated at the tool level).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVersion {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub prompt_id: String,
    pub version: i64,
    pub name: String,
    pub description: String,
    pub category: String,
    pub content: String,
    /// JSON-stringified `Vec<PromptVariable>` (ERR_SURREAL_001).
    #[serde(default = "default_empty_array_json")]
    pub variables_json: String,
    pub edited_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_summary: Option<String>,
    #[serde(default = "Utc::now")]
    pub edited_at: DateTime<Utc>,
}

fn default_empty_array_json() -> String {
    "[]".to_string()
}

/// Lightweight summary for listing versions in the UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptVersionSummary {
    pub id: String,
    pub prompt_id: String,
    pub version: i64,
    pub edited_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_summary: Option<String>,
    pub edited_at: DateTime<Utc>,
}
