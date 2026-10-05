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
//! Initial message assembly for a tool-loop run.
use crate::agents::core::agent::Task;
use crate::agents::prompt;
use tracing::debug;

/// Builds the initial message vector sent to the LLM at the start of a tool loop.
///
/// Two branches:
/// - **Continuation**: `task.context["conversation_messages"]` contains a non-empty
///   array of `{role, content}` entries persisted in the DB. The current user
///   message has already been saved by the frontend before the streaming call,
///   so the array already ends with the latest user turn — we replay it as-is
///   under a freshly regenerated system prompt and do NOT re-append
///   `task.description` (that would duplicate the last user turn).
/// - **First call** (or empty history fallback): build a `[system, user]` pair
///   from the regenerated system prompt and the formatted user prompt
///   (`prompt::build_prompt` may wrap the description with extra context).
///
/// The system prompt is rebuilt every turn because it depends on live agent
/// configuration (tools, MCP servers, locale, current date) that can change
/// between turns. It is therefore never persisted in the DB.
pub(crate) fn build_initial_messages(task: &Task, system_prompt: String) -> Vec<serde_json::Value> {
    let existing = task
        .context
        .get("conversation_messages")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    if !existing.is_empty() {
        let history_count = existing.len();
        let mut msgs = Vec::with_capacity(history_count + 1);
        msgs.push(serde_json::json!({
            "role": "system",
            "content": system_prompt,
        }));
        msgs.extend(existing);
        debug!(
            history_count = history_count,
            "Continuing conversation: regenerated system prompt + replayed history"
        );
        msgs
    } else {
        let base_prompt = prompt::build_prompt(task);
        debug!("First message: building new system prompt with tools");

        // Promote a `pending_attachments` payload (set by `build_task` for
        // streaming workflows) into a multipart user turn. Emits the DEFAULT
        // OpenAI shape; per-provider adapters re-normalize at body-build time.
        let attachments: Vec<crate::models::MessageAttachment> = task
            .context
            .get("pending_attachments")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();

        let user_content = if attachments.is_empty() {
            serde_json::Value::String(base_prompt)
        } else {
            let mut parts = vec![serde_json::json!({
                "type": "text",
                "text": base_prompt,
            })];
            for att in &attachments {
                parts.push(crate::llm::image_format::build_image_content_part_openai(
                    att,
                ));
            }
            serde_json::Value::Array(parts)
        };

        vec![
            serde_json::json!({"role": "system", "content": system_prompt}),
            serde_json::json!({"role": "user", "content": user_content}),
        ]
    }
}
