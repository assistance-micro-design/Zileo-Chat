// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Exposes the number of free workflow execution slots to the frontend
//! and to the Kanban scheduler. Source of truth lives in the backend.

use crate::constants::workflow::DEFAULT_MAX_CONCURRENT_WORKFLOWS;
use crate::AppState;
use tauri::State;

/// Returns the number of free slots remaining (max - currently running).
///
/// Saturates to 0 if more than `DEFAULT_MAX_CONCURRENT_WORKFLOWS` happen to
/// be running concurrently.
#[tauri::command]
pub async fn get_workflow_slots_available(state: State<'_, AppState>) -> Result<u8, String> {
    let running = state.streaming_cancellations.lock().await.len();
    let max = DEFAULT_MAX_CONCURRENT_WORKFLOWS;
    let free = max.saturating_sub(running);
    Ok(free as u8)
}
