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
//! Per-iteration tool-choice policy + provider capability lookup.
use crate::db::DBClient;
use crate::models::function_calling::ToolChoiceMode;
use tracing::warn;

/// Resolves the `tool_choice` to apply for a given iteration of the tool loop.
///
/// `opening_tool_choice` is what the caller requested for the *first* turn.
/// We honour it only on iteration 1 and fall back to [`ToolChoiceMode::Auto`]
/// afterwards. This is what lets a flow force the model to engage its tools on
/// the opening turn (e.g. Kanban analyze / compose, where the model otherwise
/// writes prose and finishes without ever calling its submit tool) while still
/// allowing it to *finish* naturally on a later turn — a plain `Auto` analyze
/// could end with no tool call, and a blanket `Required` would never let the
/// loop terminate (no turn could be tool-free), spinning until max_iterations.
///
/// `model_supports_forced` gates a forced opening choice: some upstreams reject
/// a forced `tool_choice` (`deepseek-v4` via RouterLab returns HTTP 400). When
/// the model's `supports_forced_tool_choice` flag is false we downgrade an
/// opening `Required` to `Auto` so the call succeeds; the empty-capture-slot
/// risk this re-introduces is covered by the boot-time catch-up re-analysis.
pub(crate) fn tool_choice_for_iteration(
    iteration: usize,
    opening_tool_choice: ToolChoiceMode,
    model_supports_forced: bool,
) -> ToolChoiceMode {
    if iteration <= 1 {
        match opening_tool_choice {
            ToolChoiceMode::Required if !model_supports_forced => ToolChoiceMode::Auto,
            other => other,
        }
    } else {
        ToolChoiceMode::Auto
    }
}

/// Reads the `supports_forced_tool_choice` capability for a `(provider, model)`
/// pair from the `llm_model` table. Returns `true` (the historical default)
/// when the model card is absent or the query fails, so a missing row never
/// disables a flow's forced opening turn.
///
/// Provider scoping is mandatory: two custom providers can expose the same
/// `api_name` with different capabilities, so an unscoped lookup would trust
/// the wrong row. Provider strings are compared lowercase to match how the
/// front-end persists them.
pub(crate) async fn load_supports_forced_tool_choice(
    db: &DBClient,
    api_name: &str,
    provider: &str,
) -> bool {
    let query = "SELECT (supports_forced_tool_choice ?? true) AS supports_forced_tool_choice \
         FROM llm_model \
         WHERE api_name = $api_name \
           AND string::lowercase(provider) = string::lowercase($provider) \
         LIMIT 1";
    match db
        .query_json_with_params(
            query,
            vec![
                ("api_name".to_string(), serde_json::json!(api_name)),
                ("provider".to_string(), serde_json::json!(provider)),
            ],
        )
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .next()
            .and_then(|r| r["supports_forced_tool_choice"].as_bool())
            .unwrap_or(true),
        Err(e) => {
            warn!(
                model = %api_name,
                error = %e,
                "Failed to resolve supports_forced_tool_choice, defaulting to true"
            );
            true
        }
    }
}
