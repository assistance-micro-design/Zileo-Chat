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

//! Startup lifecycle commands.

use crate::state::AppState;
use std::sync::atomic::Ordering;
use tauri::State;

/// Returns whether the UI-critical services (providers, embedding) finished
/// initializing.
///
/// The frontend queries this on mount so the startup splash is dismissed even
/// when the `boot_ready` event fired before the listener was attached — the
/// window now comes up before the deferred boot init completes, so that race is
/// real.
///
/// # Returns
/// `true` once the UI is ready to take over from the splash.
#[tauri::command]
pub fn boot_ready_state(state: State<'_, AppState>) -> Result<bool, String> {
    Ok(state.ui_ready.load(Ordering::Acquire))
}
