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

//! Input validation for streaming workflow execution.
//!
//! Centralizes the validation rules applied before any workflow side effect:
//! UUID/name/message constraints and the concurrent workflow limit.

use crate::security::validate_uuid_field;
use crate::{security::Validator, AppState};
use tauri::State;
use tracing::warn;

/// Validated, owned inputs ready for execution.
pub struct ValidatedInputs {
    pub workflow_id: String,
    pub message: String,
    pub agent_id: String,
}

/// Validate the user-supplied parameters and the concurrency budget.
///
/// Returns the cleaned inputs on success. The concurrency check is a safety
/// net — the frontend already prevents over-spawning, but a backend check
/// closes the race window.
///
/// # Errors
/// Validation errors are returned as user-facing strings. The concurrent
/// limit error is also user-facing (Toast).
pub async fn validate_inputs(
    workflow_id: String,
    message: String,
    agent_id: String,
    state: &State<'_, AppState>,
) -> Result<ValidatedInputs, String> {
    let workflow_id = validate_uuid_field(&workflow_id, "workflow_id")?;

    let message = Validator::validate_message(&message).map_err(|e| {
        warn!(error = %e, "Invalid message");
        format!("Invalid message: {}", e)
    })?;

    let agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    let running_count = state.streaming_cancellations.lock().await.len();
    let max_concurrent = crate::constants::workflow::DEFAULT_MAX_CONCURRENT_WORKFLOWS;
    if running_count >= max_concurrent {
        return Err(format!(
            "Maximum concurrent workflows ({}) reached. Please wait for a workflow to complete.",
            max_concurrent
        ));
    }

    Ok(ValidatedInputs {
        workflow_id,
        message,
        agent_id,
    })
}
