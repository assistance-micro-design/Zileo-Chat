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

//! Reasoning emission helpers for the agent tool loop.
//!
//! Centralizes the bridge between the in-memory reasoning trace
//! ([`ReasoningStepData`]) and Tauri streaming events ([`StreamChunk`]),
//! plus the small format helpers that prepare LLM errors for display.

use crate::agents::core::agent::{ReasoningSource, ReasoningStepData};
use crate::llm::LLMError;
use crate::models::agent::ReasoningEffort;
use crate::models::streaming::{events, StreamChunk};
use crate::models::AgentConfig;
use crate::tools::context::AgentToolContext;
use tauri::Emitter;
use tracing::warn;

/// Formats an LLMError into a user-friendly error message.
pub(crate) fn format_llm_error(error: &LLMError) -> String {
    match error {
        LLMError::ConnectionError(msg) => {
            format!(
                "Connection error: {}\n\nMake sure the LLM service is running and accessible.",
                msg
            )
        }
        LLMError::ModelNotFound(msg) => format!("Model not found: {}", msg),
        LLMError::MissingApiKey(provider) => {
            format!(
                "API key missing for {}. Please configure it in Settings.",
                provider
            )
        }
        LLMError::RequestFailed(msg) => format!("Request failed: {}", msg),
        _ => error.to_string(),
    }
}

/// Returns the effective reasoning effort based on model capability.
///
/// When `is_reasoning` is true but no explicit effort is set, defaults to Medium.
/// This ensures reasoning models always use the thinking path (important for Ollama
/// where the `think` parameter controls separate thinking extraction).
pub(crate) fn effective_reasoning_effort(config: &AgentConfig) -> Option<ReasoningEffort> {
    if config.llm.is_reasoning {
        Some(
            config
                .reasoning_effort
                .clone()
                .unwrap_or(ReasoningEffort::Medium),
        )
    } else {
        None
    }
}

/// Emits a streaming event to the frontend via Tauri.
pub(crate) fn emit_progress(agent_context: Option<&AgentToolContext>, chunk: StreamChunk) {
    if let Some(context) = agent_context {
        if let Some(ref handle) = context.app_handle {
            if let Err(e) = handle.emit(events::WORKFLOW_STREAM, &chunk) {
                warn!(error = %e, "Failed to emit LLM agent progress event");
            }
        }
    }
}

/// Emits a reasoning step and records it in the in-memory trace.
///
/// `agent_id` / `agent_name` / `is_sub_agent` are propagated to the
/// `StreamChunk` so the frontend can apply the sub-agent visual treatment
/// when the emitting agent is a delegated one rather than the orchestrator.
#[allow(clippy::too_many_arguments)]
pub(crate) fn emit_reasoning(
    agent_context: Option<&AgentToolContext>,
    event_workflow_id: &str,
    content: String,
    elapsed_ms: u64,
    sequence: u32,
    source: ReasoningSource,
    steps: &mut Vec<ReasoningStepData>,
    agent_id: Option<String>,
    agent_name: Option<String>,
    is_sub_agent: bool,
) {
    emit_progress(
        agent_context,
        StreamChunk::reasoning(
            event_workflow_id.to_string(),
            content.clone(),
            agent_id,
            agent_name,
            is_sub_agent,
        ),
    );
    steps.push(ReasoningStepData {
        content,
        duration_ms: elapsed_ms,
        sequence,
        source,
    });
}
