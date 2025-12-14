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

//! Unit tests for UserQuestionTool.
//!
//! These tests verify the synchronous validation and definition methods
//! without requiring database access or async operations.

use super::UserQuestionTool;
use crate::db::DBClient;
use crate::models::QuestionOption;
use crate::tools::constants::user_question as uq_const;
use crate::tools::Tool;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

/// Helper to run async DB creation for sync tests
fn create_test_tool_sync() -> UserQuestionTool {
    // Use tokio runtime to create DB in test context
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        // Create a temporary in-memory database for tests
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_user_question_{}.db", Uuid::new_v4()));
        let db = Arc::new(DBClient::new(db_path.to_str().unwrap()).await.unwrap());
        UserQuestionTool::new(
            db,
            "test_workflow".to_string(),
            "test_agent".to_string(),
            None,
        )
    })
}

/// Creates a minimal UserQuestionTool for testing synchronous methods.
fn create_test_tool() -> UserQuestionTool {
    create_test_tool_sync()
}

// ===== Definition Tests =====

#[test]
fn test_definition_has_correct_id() {
    let tool = create_test_tool();
    let def = tool.definition();
    assert_eq!(def.id, "UserQuestionTool");
}

#[test]
fn test_definition_has_correct_name() {
    let tool = create_test_tool();
    let def = tool.definition();
    assert_eq!(def.name, "User Question Tool");
}

#[test]
fn test_definition_has_correct_schema() {
    let tool = create_test_tool();
    let def = tool.definition();

    // Verify input schema structure
    let schema = &def.input_schema;
    assert_eq!(schema["type"], "object");

    // Verify required fields
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("operation")));
    assert!(required.contains(&json!("question")));
    assert!(required.contains(&json!("questionType")));

    // Verify properties exist
    let properties = &schema["properties"];
    assert!(properties.get("operation").is_some());
    assert!(properties.get("question").is_some());
    assert!(properties.get("questionType").is_some());
    assert!(properties.get("options").is_some());
    assert!(properties.get("textPlaceholder").is_some());
    assert!(properties.get("textRequired").is_some());
    assert!(properties.get("context").is_some());

    // Verify operation enum
    let operation_enum = properties["operation"]["enum"].as_array().unwrap();
    assert_eq!(operation_enum.len(), 1);
    assert_eq!(operation_enum[0], "ask");

    // Verify questionType enum
    let type_enum = properties["questionType"]["enum"].as_array().unwrap();
    assert_eq!(type_enum.len(), 3);
    assert!(type_enum.contains(&json!("checkbox")));
    assert!(type_enum.contains(&json!("text")));
    assert!(type_enum.contains(&json!("mixed")));
}

#[test]
fn test_definition_has_correct_output_schema() {
    let tool = create_test_tool();
    let def = tool.definition();

    let output = &def.output_schema;
    assert_eq!(output["type"], "object");

    let properties = &output["properties"];
    assert!(properties.get("success").is_some());
    assert!(properties.get("selectedOptions").is_some());
    assert!(properties.get("textResponse").is_some());
    assert!(properties.get("message").is_some());
}

#[test]
fn test_definition_requires_confirmation_is_false() {
    let tool = create_test_tool();
    let def = tool.definition();
    assert!(!def.requires_confirmation);
}

// ===== validate_input Tests =====

#[test]
fn test_validate_input_missing_operation() {
    let tool = create_test_tool();
    let input = json!({
        "question": "Test question?",
        "questionType": "text"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("operation"));
}

#[test]
fn test_validate_input_missing_question() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "questionType": "text"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("question"));
}

#[test]
fn test_validate_input_missing_question_type() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "question": "Test question?"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("questionType"));
}

#[test]
fn test_validate_input_valid() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "question": "Test question?",
        "questionType": "text"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_valid_with_all_fields() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "question": "Test question?",
        "questionType": "checkbox",
        "options": [
            {"id": "opt1", "label": "Option 1"},
            {"id": "opt2", "label": "Option 2"}
        ],
        "textPlaceholder": "Enter text here",
        "textRequired": true,
        "context": "Some additional context"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_not_object() {
    let tool = create_test_tool();
    let input = json!("not an object");

    let result = tool.validate_input(&input);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("must be an object"));
}

// ===== Question Validation Tests (via execute - would need async) =====
// Note: These tests demonstrate what WOULD be tested in integration tests.
// For unit tests, we focus on synchronous validation in validate_input().

#[test]
fn test_constants_exist() {
    // Verify all constants are defined and have expected values
    assert_eq!(uq_const::MAX_QUESTION_LENGTH, 2000);
    assert_eq!(uq_const::MAX_OPTION_ID_LENGTH, 64);
    assert_eq!(uq_const::MAX_OPTION_LABEL_LENGTH, 256);
    assert_eq!(uq_const::MAX_OPTIONS, 20);
    assert_eq!(uq_const::MAX_CONTEXT_LENGTH, 5000);
    assert_eq!(uq_const::DEFAULT_TIMEOUT_SECS, 300);
}

#[test]
fn test_valid_types_constant() {
    assert_eq!(uq_const::VALID_TYPES, &["checkbox", "text", "mixed"]);
}

#[test]
fn test_poll_intervals_constant() {
    assert_eq!(
        uq_const::POLL_INTERVALS_MS,
        &[500, 500, 1000, 1000, 2000, 2000, 5000]
    );
}

#[test]
fn test_valid_statuses_constant() {
    assert_eq!(
        uq_const::VALID_STATUSES,
        &["pending", "answered", "skipped", "timeout"]
    );
}

// ===== Helper Tests for QuestionOption =====

#[test]
fn test_question_option_serialization() {
    let option = QuestionOption {
        id: "opt1".to_string(),
        label: "Option 1".to_string(),
    };

    let json = serde_json::to_value(&option).unwrap();
    assert_eq!(json["id"], "opt1");
    assert_eq!(json["label"], "Option 1");
}

#[test]
fn test_question_option_deserialization() {
    let json = json!({
        "id": "opt1",
        "label": "Option 1"
    });

    let option: QuestionOption = serde_json::from_value(json).unwrap();
    assert_eq!(option.id, "opt1");
    assert_eq!(option.label, "Option 1");
}

// ===== Edge Case Tests =====

#[test]
fn test_validate_input_empty_strings() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "",
        "question": "",
        "questionType": ""
    });

    // validate_input only checks field presence, not content
    // Content validation happens in execute/ask_question
    let result = tool.validate_input(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_null_values() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "question": "Test?",
        "questionType": "text",
        "options": null,
        "textPlaceholder": null,
        "textRequired": null,
        "context": null
    });

    let result = tool.validate_input(&input);
    assert!(result.is_ok());
}

#[test]
fn test_validate_input_extra_fields() {
    let tool = create_test_tool();
    let input = json!({
        "operation": "ask",
        "question": "Test?",
        "questionType": "text",
        "extraField": "should be ignored"
    });

    let result = tool.validate_input(&input);
    assert!(result.is_ok());
}

// ===== Boundary Tests for Constants =====

#[test]
fn test_max_question_length_boundary() {
    // This would be tested in integration tests with execute()
    // Here we just verify the constant exists and is reasonable
    let max_len = uq_const::MAX_QUESTION_LENGTH;
    assert!(max_len > 0, "MAX_QUESTION_LENGTH should be positive");
    assert!(max_len <= 10000, "MAX_QUESTION_LENGTH should be reasonable");
}

#[test]
fn test_max_options_boundary() {
    let max_opts = uq_const::MAX_OPTIONS;
    assert!(max_opts > 0, "MAX_OPTIONS should be positive");
    assert!(max_opts <= 100, "MAX_OPTIONS should be reasonable");
}

#[test]
fn test_timeout_is_reasonable() {
    let timeout = uq_const::DEFAULT_TIMEOUT_SECS;
    assert!(timeout >= 60, "Timeout should be at least 1 minute");
    assert!(timeout <= 600, "Timeout should be at most 10 minutes");
}

// ===== Documentation Tests =====

#[test]
fn test_definition_description_mentions_timeout() {
    let tool = create_test_tool();
    let def = tool.definition();
    // Description should mention the timeout
    assert!(def.description.contains("5 minutes") || def.description.contains("Timeout"));
}

#[test]
fn test_definition_description_mentions_question_types() {
    let tool = create_test_tool();
    let def = tool.definition();
    assert!(def.description.contains("checkbox"));
    assert!(def.description.contains("text"));
    assert!(def.description.contains("mixed"));
}
