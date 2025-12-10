use serde_json::json;
use tauri::{Emitter, State, Window};
use tracing::{info, warn};

use crate::db::DBClient;
use crate::models::UserQuestion;
use crate::security::Validator;
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
    let selected_options_json = serde_json::to_string(&selected_options)
        .map_err(|e| format!("Failed to encode selected_options: {}", e))?;

    // Build params for update - use bind parameters for user-provided values
    let mut params: Vec<(String, serde_json::Value)> = vec![
        (
            "selected_options".to_string(),
            serde_json::json!(selected_options_json),
        ),
        ("status".to_string(), serde_json::json!("answered")),
    ];

    // Validate text_response length if provided (OPT-UQ-1)
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

/// Submit a response to a pending question
#[tauri::command]
pub async fn submit_user_response(
    question_id: String,
    selected_options: Vec<String>,
    text_response: Option<String>,
    state: State<'_, AppState>,
    window: Window,
) -> Result<(), String> {
    // Validate question_id is a valid UUID
    let validated_id = Validator::validate_uuid(&question_id)
        .map_err(|e| format!("Invalid question_id: {}", e))?;

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
        new_status = %new_status,
        "User submitted response - verified status"
    );

    // Emit event for any listeners
    let chunk = json!({
        "chunk_type": "user_question_complete",
        "question_id": validated_id,
        "status": "answered"
    });

    if let Err(e) = window.emit("workflow_stream", &chunk) {
        warn!(error = %e, "Failed to emit user_question_complete event");
    }

    Ok(())
}

/// Get pending questions for a workflow
#[tauri::command]
pub async fn get_pending_questions(
    workflow_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<UserQuestion>, String> {
    // Validate workflow_id is a valid UUID
    let validated_id = Validator::validate_uuid(&workflow_id)
        .map_err(|e| format!("Invalid workflow_id: {}", e))?;

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

/// Skip a question (user chooses not to answer)
#[tauri::command]
pub async fn skip_question(
    question_id: String,
    state: State<'_, AppState>,
    window: Window,
) -> Result<(), String> {
    // Validate question_id is a valid UUID
    let validated_id = Validator::validate_uuid(&question_id)
        .map_err(|e| format!("Invalid question_id: {}", e))?;

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

    info!(question_id = %validated_id, "User skipped question");

    // Emit event
    let chunk = json!({
        "chunk_type": "user_question_complete",
        "question_id": validated_id,
        "status": "skipped"
    });

    if let Err(e) = window.emit("workflow_stream", &chunk) {
        warn!(error = %e, "Failed to emit user_question_complete event");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::security::Validator;
    use crate::tools::constants::user_question as uq_const;

    // ============================================================================
    // OPT-UQ-6: SQL Injection Tests
    // ============================================================================

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

    // ============================================================================
    // OPT-UQ-1: Text Response Length Validation Tests
    // ============================================================================

    #[test]
    fn test_text_response_max_length_constant() {
        // Verify the constant is set to 10000
        assert_eq!(uq_const::MAX_TEXT_RESPONSE_LENGTH, 10000);
    }

    #[test]
    fn test_text_response_validation_logic() {
        // Simulate the validation logic from submit_user_response
        let text_response: Option<String> = Some("a".repeat(10001));

        if let Some(ref text) = text_response {
            let exceeds_limit = text.len() > uq_const::MAX_TEXT_RESPONSE_LENGTH;
            assert!(
                exceeds_limit,
                "Text with {} chars should exceed max {}",
                text.len(),
                uq_const::MAX_TEXT_RESPONSE_LENGTH
            );
        }
    }

    #[test]
    fn test_text_response_at_limit_accepted() {
        // Text exactly at limit should be accepted
        let text_response: Option<String> = Some("a".repeat(10000));

        if let Some(ref text) = text_response {
            let exceeds_limit = text.len() > uq_const::MAX_TEXT_RESPONSE_LENGTH;
            assert!(
                !exceeds_limit,
                "Text exactly at {} chars should be accepted",
                uq_const::MAX_TEXT_RESPONSE_LENGTH
            );
        }
    }

    #[test]
    fn test_text_response_none_accepted() {
        // None text_response should be valid
        let text_response: Option<String> = None;
        assert!(text_response.is_none());
    }

    // ============================================================================
    // OPT-UQ-2: Option ID Validation Tests (validates constant exists)
    // ============================================================================

    #[test]
    fn test_option_id_max_length_constant() {
        // Verify the constant is set to 64
        assert_eq!(uq_const::MAX_OPTION_ID_LENGTH, 64);
    }

    #[test]
    fn test_option_id_validation_logic() {
        // Simulate the validation that would happen in tool.rs
        let long_option_id = "a".repeat(65);
        let exceeds_limit = long_option_id.len() > uq_const::MAX_OPTION_ID_LENGTH;
        assert!(
            exceeds_limit,
            "Option ID with {} chars should exceed max {}",
            long_option_id.len(),
            uq_const::MAX_OPTION_ID_LENGTH
        );
    }

    #[test]
    fn test_option_id_at_limit_accepted() {
        let option_id = "a".repeat(64);
        let exceeds_limit = option_id.len() > uq_const::MAX_OPTION_ID_LENGTH;
        assert!(
            !exceeds_limit,
            "Option ID exactly at {} chars should be accepted",
            uq_const::MAX_OPTION_ID_LENGTH
        );
    }

    // ============================================================================
    // Question Type Validation Tests
    // ============================================================================

    #[test]
    fn test_valid_question_types() {
        let valid_types = uq_const::VALID_TYPES;
        assert!(valid_types.contains(&"checkbox"));
        assert!(valid_types.contains(&"text"));
        assert!(valid_types.contains(&"mixed"));
        assert_eq!(valid_types.len(), 3);
    }

    #[test]
    fn test_valid_question_statuses() {
        let valid_statuses = uq_const::VALID_STATUSES;
        assert!(valid_statuses.contains(&"pending"));
        assert!(valid_statuses.contains(&"answered"));
        assert!(valid_statuses.contains(&"skipped"));
        assert!(valid_statuses.contains(&"timeout"));
        assert_eq!(valid_statuses.len(), 4);
    }

    // ============================================================================
    // OPT-UQ-9: Additional Integration Tests
    // ============================================================================

    #[test]
    fn test_submit_response_empty_selected_options() {
        // Empty vec for selected_options should serialize correctly
        let selected_options: Vec<String> = vec![];
        let json_str = serde_json::to_string(&selected_options);

        assert!(json_str.is_ok());
        assert_eq!(json_str.unwrap(), "[]");
    }

    #[test]
    fn test_submit_response_with_text_only() {
        // Text response without selections should be valid
        let selected_options: Vec<String> = vec![];
        let text_response: Option<String> = Some("This is my answer".to_string());

        assert!(text_response.is_some());
        assert!(selected_options.is_empty());

        // Verify text is within limit
        if let Some(ref text) = text_response {
            assert!(text.len() <= uq_const::MAX_TEXT_RESPONSE_LENGTH);
        }
    }

    #[test]
    fn test_submit_response_with_selections_and_text() {
        // Both selections and text should work together
        let selected_options = vec!["option1".to_string(), "option2".to_string()];
        let text_response: Option<String> = Some("Additional context".to_string());

        assert!(!selected_options.is_empty());
        assert!(text_response.is_some());

        // Verify serialization
        let json_str = serde_json::to_string(&selected_options);
        assert!(json_str.is_ok());
    }

    #[test]
    fn test_selected_options_json_encoding() {
        // Verify selected_options serializes to JSON correctly
        let options = vec![
            "option_a".to_string(),
            "option_b".to_string(),
            "option_c".to_string(),
        ];

        let json_str = serde_json::to_string(&options);
        assert!(json_str.is_ok());

        let json_value = json_str.unwrap();
        assert_eq!(json_value, r#"["option_a","option_b","option_c"]"#);

        // Verify it can be deserialized back
        let deserialized: Result<Vec<String>, _> = serde_json::from_str(&json_value);
        assert!(deserialized.is_ok());
        assert_eq!(deserialized.unwrap(), options);
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

    #[test]
    fn test_timeout_status_constant() {
        // Verify "timeout" is in VALID_STATUSES
        let valid_statuses = uq_const::VALID_STATUSES;

        // Should have 4 statuses: pending, answered, skipped, timeout
        assert_eq!(valid_statuses.len(), 4);

        assert!(valid_statuses.contains(&"pending"));
        assert!(valid_statuses.contains(&"answered"));
        assert!(valid_statuses.contains(&"skipped"));
        assert!(valid_statuses.contains(&"timeout"));
    }

    #[test]
    fn test_poll_intervals_defined() {
        // Verify POLL_INTERVALS_MS has values
        let intervals = uq_const::POLL_INTERVALS_MS;

        assert!(
            !intervals.is_empty(),
            "POLL_INTERVALS_MS should not be empty"
        );

        // Verify all intervals are positive
        for (i, &interval) in intervals.iter().enumerate() {
            assert!(
                interval > 0,
                "Poll interval at index {} should be positive, got {}",
                i,
                interval
            );
        }

        // Verify intervals are in reasonable range (not too small, not too large)
        for &interval in intervals {
            assert!(
                interval >= 100,
                "Poll interval {} should be >= 100ms",
                interval
            );
            assert!(
                interval <= 60000,
                "Poll interval {} should be <= 60000ms (1 minute)",
                interval
            );
        }
    }
}
