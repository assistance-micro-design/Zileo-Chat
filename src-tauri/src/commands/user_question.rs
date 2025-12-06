use serde_json::json;
use tauri::{Emitter, State, Window};
use tracing::{info, warn};

use crate::models::UserQuestion;
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
    // Validate question exists and is pending
    let query = format!("SELECT status FROM user_question:`{}`", question_id);
    let result: Vec<serde_json::Value> = state
        .db
        .query_json(&query)
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

    // Update question with response
    let selected_json = serde_json::to_string(&selected_options)
        .map_err(|e| format!("Failed to serialize options: {}", e))?;

    let text_json = text_response
        .as_ref()
        .map(serde_json::to_string)
        .transpose()
        .map_err(|e| format!("Failed to serialize text: {}", e))?;

    let update_query = if let Some(text_str) = text_json {
        format!(
            "UPDATE user_question:`{}` SET status = 'answered', selected_options = '{}', text_response = {}, answered_at = time::now()",
            question_id, selected_json, text_str
        )
    } else {
        format!(
            "UPDATE user_question:`{}` SET status = 'answered', selected_options = '{}', answered_at = time::now()",
            question_id, selected_json
        )
    };

    state
        .db
        .execute(&update_query)
        .await
        .map_err(|e| format!("Failed to update question: {}", e))?;

    info!(question_id = %question_id, "User submitted response");

    // Emit event for any listeners
    let chunk = json!({
        "chunk_type": "user_question_complete",
        "question_id": question_id,
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
    let query = format!(
        "SELECT meta::id(id) AS id, workflow_id, agent_id, question, question_type, \
         options, text_placeholder, text_required, context, status, \
         selected_options, text_response, created_at, answered_at \
         FROM user_question WHERE workflow_id = '{}' AND status = 'pending' \
         ORDER BY created_at ASC",
        workflow_id
    );

    let results: Vec<serde_json::Value> = state
        .db
        .query_json(&query)
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
    // Validate question exists and is pending
    let query = format!("SELECT status FROM user_question:`{}`", question_id);
    let result: Vec<serde_json::Value> = state
        .db
        .query_json(&query)
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

    // Update status to skipped
    let update_query = format!(
        "UPDATE user_question:`{}` SET status = 'skipped', answered_at = time::now()",
        question_id
    );

    state
        .db
        .execute(&update_query)
        .await
        .map_err(|e| format!("Failed to skip question: {}", e))?;

    info!(question_id = %question_id, "User skipped question");

    // Emit event
    let chunk = json!({
        "chunk_type": "user_question_complete",
        "question_id": question_id,
        "status": "skipped"
    });

    if let Err(e) = window.emit("workflow_stream", &chunk) {
        warn!(error = %e, "Failed to emit user_question_complete event");
    }

    Ok(())
}
