use serde_json::json;
use tauri::{Emitter, State, Window};
use tracing::{info, warn};

use crate::models::UserQuestion;
use crate::security::Validator;
use crate::state::AppState;

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

    // Validate question exists and is pending using parameterized query
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

    let update_query = if let Some(ref text) = text_response {
        params.push(("text_response".to_string(), serde_json::json!(text)));
        format!(
            "UPDATE user_question:`{}` SET status = $status, selected_options = $selected_options, text_response = $text_response, answered_at = time::now()",
            validated_id
        )
    } else {
        format!(
            "UPDATE user_question:`{}` SET status = $status, selected_options = $selected_options, answered_at = time::now()",
            validated_id
        )
    };

    info!(
        question_id = %validated_id,
        update_query = %update_query,
        "Executing update query"
    );

    state
        .db
        .execute_with_params(&update_query, params)
        .await
        .map_err(|e| format!("Failed to update question: {}", e))?;

    // Verify the update by reading back
    let verify_result: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT status FROM user_question:`{}`",
            validated_id
        ))
        .await
        .map_err(|e| format!("Failed to verify update: {}", e))?;

    let new_status = verify_result
        .first()
        .and_then(|r| r.get("status"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

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
