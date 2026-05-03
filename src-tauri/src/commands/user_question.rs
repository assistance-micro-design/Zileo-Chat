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

use tauri::{Emitter, State, Window};
use tracing::{info, instrument, warn};

use crate::db::DBClient;
use crate::models::streaming::{events, StreamChunk};
use crate::models::UserQuestion;
use crate::security::{serialize_for_query, validate_uuid_field};
use crate::state::AppState;

/// Validate that a question exists and has status "pending"
async fn validate_question_pending(db: &DBClient, question_id: &str) -> Result<(), String> {
    // Query question status (question_id is a validated UUID)
    let result: Vec<serde_json::Value> = db
        .query_json(&format!(
            "SELECT status FROM user_question:`{}`",
            question_id
        ))
        .await
        .map_err(|e| format!("Failed to query question: {}", e))?;

    let record = result
        .first()
        .ok_or_else(|| format!("Question not found: {}", question_id))?;

    let status = record
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if status != "pending" {
        return Err(format!("Question is not pending (status: {})", status));
    }

    Ok(())
}

/// Update a question to "answered" status with the provided response
async fn update_question_answered(
    db: &DBClient,
    question_id: &str,
    selected_options: &[String],
    text_response: Option<&str>,
) -> Result<(), String> {
    // Encode selected_options as JSON string (matching the CREATE pattern)
    let selected_options_json = serialize_for_query(&selected_options, "selected_options")?;

    // Build params for update - use bind parameters for user-provided values
    let mut params: Vec<(String, serde_json::Value)> = vec![
        (
            "selected_options".to_string(),
            serde_json::json!(selected_options_json),
        ),
        ("status".to_string(), serde_json::json!("answered")),
    ];

    // Validate text_response length if provided
    if let Some(text) = text_response {
        if text.len() > crate::tools::constants::user_question::MAX_TEXT_RESPONSE_LENGTH {
            return Err(format!(
                "Text response too long: {} chars (max {})",
                text.len(),
                crate::tools::constants::user_question::MAX_TEXT_RESPONSE_LENGTH
            ));
        }
    }

    let update_query = if text_response.is_some() {
        params.push((
            "text_response".to_string(),
            serde_json::json!(text_response),
        ));
        format!(
            "UPDATE user_question:`{}` SET status = $status, selected_options = $selected_options, text_response = $text_response, answered_at = time::now()",
            question_id
        )
    } else {
        format!(
            "UPDATE user_question:`{}` SET status = $status, selected_options = $selected_options, answered_at = time::now()",
            question_id
        )
    };

    info!(
        question_id = %question_id,
        update_query = %update_query,
        "Executing update query"
    );

    db.execute_with_params(&update_query, params)
        .await
        .map_err(|e| format!("Failed to update question: {}", e))?;

    Ok(())
}

/// Verify that the update was successful by reading back the status
async fn verify_update_success(db: &DBClient, question_id: &str) -> Result<String, String> {
    let verify_result: Vec<serde_json::Value> = db
        .query_json(&format!(
            "SELECT status FROM user_question:`{}`",
            question_id
        ))
        .await
        .map_err(|e| format!("Failed to verify update: {}", e))?;

    let new_status = verify_result
        .first()
        .and_then(|r| r.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    Ok(new_status.to_string())
}

/// Submit a response to a pending question.
///
/// `workflow_id` is required so the emitted `user_question_complete` chunk
/// carries the correct workflow context — the frontend dispatcher routes by
/// `workflow_id` and silently drops chunks with an empty value (H1 audit
/// 2026-05-02).
#[tauri::command]
#[instrument(name = "submit_user_response", skip(state, window))]
pub async fn submit_user_response(
    question_id: String,
    workflow_id: String,
    selected_options: Vec<String>,
    text_response: Option<String>,
    state: State<'_, AppState>,
    window: Window,
) -> Result<(), String> {
    let validated_id = validate_uuid_field(&question_id, "question_id")?;
    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    validate_question_pending(&state.db, &validated_id).await?;
    update_question_answered(
        &state.db,
        &validated_id,
        &selected_options,
        text_response.as_deref(),
    )
    .await?;
    let new_status = verify_update_success(&state.db, &validated_id).await?;

    info!(
        question_id = %validated_id,
        workflow_id = %validated_workflow_id,
        new_status = %new_status,
        "User submitted response - verified status"
    );

    // Emit typed event so the frontend can clear `hasPendingQuestion` for
    // this workflow (background-workflows.ts routes by workflow_id).
    let chunk = StreamChunk::user_question_complete(
        validated_workflow_id.to_string(),
        validated_id.to_string(),
    );

    if let Err(e) = window.emit(events::WORKFLOW_STREAM, &chunk) {
        warn!(error = %e, "Failed to emit user_question_complete event");
    }

    Ok(())
}

/// Get pending questions for a workflow
#[tauri::command]
#[instrument(name = "get_pending_questions", skip(state))]
pub async fn get_pending_questions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<UserQuestion>, String> {
    let validated_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Use parameterized query to prevent injection
    let query = "SELECT meta::id(id) AS id, workflow_id, agent_id, question, question_type, \
         options, text_placeholder, text_required, context, status, \
         selected_options, text_response, created_at, answered_at \
         FROM user_question WHERE workflow_id = $workflow_id AND status = 'pending' \
         ORDER BY created_at ASC";

    let results: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            query,
            vec![("workflow_id".to_string(), serde_json::json!(validated_id))],
        )
        .await
        .map_err(|e| format!("Failed to query questions: {}", e))?;

    let questions: Vec<UserQuestion> = results
        .into_iter()
        .filter_map(|v| {
            // Parse options from JSON string
            let mut question: UserQuestion = serde_json::from_value(v.clone()).ok()?;

            // Options might be stored as JSON string, parse if needed
            if let Some(opts_str) = v.get("options").and_then(|o| o.as_str()) {
                question.options = serde_json::from_str(opts_str).ok();
            }

            Some(question)
        })
        .collect();

    Ok(questions)
}

/// Skip a question (user chooses not to answer).
///
/// `workflow_id` is required so the emitted `user_question_complete` chunk
/// carries the correct workflow context — the frontend dispatcher routes by
/// `workflow_id` and silently drops chunks with an empty value (H1 audit
/// 2026-05-02).
#[tauri::command]
#[instrument(name = "skip_question", skip(state, window))]
pub async fn skip_question(
    question_id: String,
    workflow_id: String,
    state: State<'_, AppState>,
    window: Window,
) -> Result<(), String> {
    let validated_id = validate_uuid_field(&question_id, "question_id")?;
    let validated_workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    // Validate question exists and is pending (validated_id is safe UUID)
    let result: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT status FROM user_question:`{}`",
            validated_id
        ))
        .await
        .map_err(|e| format!("Failed to query question: {}", e))?;

    let record = result
        .first()
        .ok_or_else(|| format!("Question not found: {}", validated_id))?;

    let status = record
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    if status != "pending" {
        return Err(format!("Question is not pending (status: {})", status));
    }

    // Update status to skipped (validated_id is safe UUID)
    let update_query = format!(
        "UPDATE user_question:`{}` SET status = 'skipped', answered_at = time::now()",
        validated_id
    );

    state
        .db
        .execute(&update_query)
        .await
        .map_err(|e| format!("Failed to skip question: {}", e))?;

    info!(
        question_id = %validated_id,
        workflow_id = %validated_workflow_id,
        "User skipped question"
    );

    // Emit typed event so the frontend can clear `hasPendingQuestion` for
    // this workflow (background-workflows.ts routes by workflow_id).
    let chunk = StreamChunk::user_question_complete(
        validated_workflow_id.to_string(),
        validated_id.to_string(),
    );

    if let Err(e) = window.emit(events::WORKFLOW_STREAM, &chunk) {
        warn!(error = %e, "Failed to emit user_question_complete event");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::security::Validator;

    #[test]
    fn test_sql_injection_question_id_rejected() {
        // Attempt SQL injection via question_id
        let malicious_id = "'; DROP TABLE user_question; --";
        let result = Validator::validate_uuid(malicious_id);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("Invalid UUID"),
            "Should reject SQL injection attempt: {}",
            err
        );
    }

    #[test]
    fn test_sql_injection_workflow_id_rejected() {
        // Attempt SQL injection via workflow_id
        let malicious_id = "' OR '1'='1";
        let result = Validator::validate_uuid(malicious_id);

        assert!(result.is_err());
    }

    #[test]
    fn test_sql_injection_union_attack_rejected() {
        // Attempt UNION-based injection
        let malicious_id = "1' UNION SELECT * FROM agent --";
        let result = Validator::validate_uuid(malicious_id);

        assert!(result.is_err());
    }

    #[test]
    fn test_valid_uuid_accepted() {
        // Valid UUID should pass
        let valid_id = "550e8400-e29b-41d4-a716-446655440000";
        let result = Validator::validate_uuid(valid_id);

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), valid_id);
    }

    #[test]
    fn test_skip_question_uuid_validation() {
        // Invalid UUID should be rejected in skip_question
        let invalid_ids = vec![
            "not-a-uuid",
            "12345",
            "'; DROP TABLE user_question; --",
            "550e8400-e29b-41d4-a716",                    // Incomplete UUID
            "550e8400-e29b-41d4-a716-446655440000-extra", // Too long
        ];

        for invalid_id in invalid_ids {
            let result = Validator::validate_uuid(invalid_id);
            assert!(
                result.is_err(),
                "Invalid UUID '{}' should be rejected",
                invalid_id
            );
        }
    }

    #[test]
    fn test_get_pending_questions_uuid_validation() {
        // Invalid workflow_id should be rejected in get_pending_questions
        let invalid_ids = vec![
            "not-a-uuid",
            "' OR '1'='1",
            "1' UNION SELECT * FROM workflow --",
            "",
        ];

        for invalid_id in invalid_ids {
            let result = Validator::validate_uuid(invalid_id);
            assert!(
                result.is_err(),
                "Invalid workflow_id '{}' should be rejected",
                invalid_id
            );
        }
    }
}
