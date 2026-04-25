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

//! Domain errors for the agent execution layer.
//!
//! Provides a typed error vocabulary so callers (orchestrator, tool loop,
//! sub-agents) can match on failure modes instead of relying on `String`
//! payloads. Migration is progressive — newly written code should use
//! `AgentError` directly; existing call sites can adopt it as they are
//! refactored, without any one PR having to convert the entire surface.

use crate::llm::LLMError;
use thiserror::Error;

/// Top-level error type emitted by the agents layer.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum AgentError {
    /// Failure in the underlying LLM call.
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),

    /// A tool invocation failed at execution time.
    #[error("Tool '{name}' failed: {message}")]
    Tool { name: String, message: String },

    /// User validation rejected (or timed out blocking) an operation.
    #[error("Validation rejected: {0}")]
    Validation(String),

    /// The agent configuration is invalid (missing model, bad provider, ...).
    #[error("Invalid agent configuration: {0}")]
    Config(String),

    /// The execution was cancelled by the user (cooperative cancellation,
    /// not a transient failure).
    #[error("Agent execution cancelled")]
    Cancelled,

    /// Catch-all for errors that don't yet have a dedicated variant.
    /// New variants should replace `Other` over time.
    #[error("{0}")]
    Other(String),
}

impl AgentError {
    /// Build a `Tool` variant without having to spell out the struct fields.
    #[allow(dead_code)]
    pub fn tool(name: impl Into<String>, message: impl Into<String>) -> Self {
        AgentError::Tool {
            name: name.into(),
            message: message.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_error_displays_llm_variant() {
        let err = AgentError::from(LLMError::ConnectionError("ollama down".into()));
        let rendered = err.to_string();
        assert!(rendered.contains("LLM error"));
        assert!(rendered.contains("Connection error"));
        assert!(rendered.contains("ollama down"));
    }

    #[test]
    fn agent_error_tool_helper_builds_struct_variant() {
        let err = AgentError::tool("delete_file", "permission denied");
        let rendered = err.to_string();
        assert_eq!(rendered, "Tool 'delete_file' failed: permission denied");
    }

    #[test]
    fn agent_error_cancelled_variant_is_distinct() {
        let err = AgentError::Cancelled;
        assert_eq!(err.to_string(), "Agent execution cancelled");
    }
}
