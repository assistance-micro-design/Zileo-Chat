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

use crate::models::serde_utils::deserialize_thing_id;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionType {
    Checkbox,
    Text,
    Mixed,
}

impl Default for QuestionType {
    fn default() -> Self {
        Self::Text
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestionStatus {
    Pending,
    Answered,
    Skipped,
}

impl Default for QuestionStatus {
    fn default() -> Self {
        Self::Pending
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestionOption {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuestion {
    #[serde(deserialize_with = "deserialize_thing_id", default)]
    pub id: String,
    pub workflow_id: String,
    pub agent_id: String,
    pub question: String,
    #[serde(default)]
    pub question_type: QuestionType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<QuestionOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_placeholder: Option<String>,
    #[serde(default)]
    pub text_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
    #[serde(default)]
    pub status: QuestionStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_response: Option<String>,
    pub created_at: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub answered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserQuestionCreate {
    pub workflow_id: String,
    pub agent_id: String,
    pub question: String,
    pub question_type: String,
    pub options: String,
    pub text_placeholder: Option<String>,
    pub text_required: bool,
    pub context: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserQuestionStreamPayload {
    pub question_id: String,
    pub question: String,
    pub question_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<QuestionOption>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_placeholder: Option<String>,
    #[serde(default)]
    pub text_required: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<String>,
}

/// Response from user answering a question
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields read via serde deserialization, used in Tauri commands
pub struct UserQuestionResponse {
    pub question_id: String,
    pub selected_options: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_response: Option<String>,
}
