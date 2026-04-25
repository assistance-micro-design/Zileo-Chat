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

//! Domain errors for the Tauri command layer.
//!
//! Typed errors that newly written commands can return as
//! `Result<T, CommandError>`. The `From<CommandError> for String` impl
//! preserves the existing `Result<T, String>` Tauri command shape, so
//! adoption is incremental — a handler can switch to `CommandError`
//! internally and `?`-bubble through `.into()` at the boundary.
//!
//! Migration is progressive — existing handlers keep returning
//! `Result<T, String>`; only new code (or refactored modules) needs to
//! adopt `CommandError`.

use crate::agents::error::AgentError;
use crate::llm::LLMError;
use thiserror::Error;

/// Top-level error type for Tauri commands.
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum CommandError {
    /// A SurrealDB query or write failed.
    #[error("Database error: {0}")]
    Db(String),

    /// Input validation rejected (UUID, length, allowed chars, ...).
    #[error("Invalid input: {0}")]
    Validation(String),

    /// The requested resource was not found.
    #[error("{kind} not found: {id}")]
    NotFound { kind: String, id: String },

    /// Forwarded from the agents layer (LLM, tool, sub-agent failure).
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),

    /// Forwarded directly from the LLM layer when the command bypasses
    /// the agents layer (e.g. provider configuration commands).
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),

    /// I/O failure (file system, exports, ...).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON (de)serialization failure.
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),

    /// Catch-all for errors that don't yet have a dedicated variant.
    /// New variants should replace `Other` over time.
    #[error("{0}")]
    Other(String),
}

impl CommandError {
    /// Build a `NotFound` variant without spelling out the struct fields.
    #[allow(dead_code)]
    pub fn not_found(kind: impl Into<String>, id: impl Into<String>) -> Self {
        CommandError::NotFound {
            kind: kind.into(),
            id: id.into(),
        }
    }
}

/// Tauri commands return `Result<T, String>`; converting at the boundary
/// keeps the IPC contract stable while the inside is typed.
impl From<CommandError> for String {
    fn from(err: CommandError) -> Self {
        err.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_error_renders_db_variant() {
        let err = CommandError::Db("timeout".into());
        assert_eq!(err.to_string(), "Database error: timeout");
    }

    #[test]
    fn command_error_not_found_helper() {
        let err = CommandError::not_found("agent", "abc-123");
        assert_eq!(err.to_string(), "agent not found: abc-123");
    }

    #[test]
    fn command_error_converts_to_string_for_tauri() {
        let err: String = CommandError::Validation("uuid malformed".into()).into();
        assert_eq!(err, "Invalid input: uuid malformed");
    }

    #[test]
    fn command_error_wraps_agent_error_via_from() {
        let agent_err = AgentError::Cancelled;
        let cmd_err: CommandError = agent_err.into();
        assert!(cmd_err.to_string().contains("Agent execution cancelled"));
    }

    #[test]
    fn command_error_wraps_llm_error_via_from() {
        let llm_err = LLMError::ConnectionError("ollama down".into());
        let cmd_err: CommandError = llm_err.into();
        assert!(cmd_err.to_string().contains("Connection error"));
    }
}
