// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Snapshot of a skill taken before any modification (versioning anti-loss).

use super::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersion {
    #[serde(deserialize_with = "deserialize_thing_id")]
    pub id: String,
    pub skill_id: String,
    pub version: i64,
    pub name: String,
    pub description: String,
    pub category: String,
    pub content: String,
    pub edited_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_summary: Option<String>,
    #[serde(default = "Utc::now")]
    pub edited_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillVersionSummary {
    pub id: String,
    pub skill_id: String,
    pub version: i64,
    pub edited_by: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_summary: Option<String>,
    pub edited_at: DateTime<Utc>,
}
