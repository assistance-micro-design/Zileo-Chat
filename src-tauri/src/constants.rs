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

//! Application-wide constants.
//!
//! This module contains constants that are shared across multiple modules
//! (commands, workflows, database queries). Tool-specific constants remain
//! in [`crate::tools::constants`].

/// Constants for workflow execution and streaming.
#[allow(dead_code)]
pub mod workflow {
    /// Maximum number of messages to include in LLM context.
    /// Prevents context overflow while maintaining conversation coherence.
    pub const MESSAGE_HISTORY_LIMIT: usize = 50;

    /// Maximum number of streaming workflows that can run concurrently.
    ///
    /// Backend safety net (frontend also enforces per-mode limits). Prevents
    /// runaway resource use when many workflows are launched in parallel.
    pub const DEFAULT_MAX_CONCURRENT_WORKFLOWS: usize = 3;

    // Tokio Timeout Constants
    /// Timeout (seconds) for LLM execution operations.
    /// Default: 5 minutes - generous for complex reasoning tasks.
    pub const LLM_EXECUTION_TIMEOUT_SECS: u64 = 300;

    /// Timeout (seconds) for database operations (queries, updates).
    /// Default: 30 seconds - should be sufficient for most queries.
    pub const DB_OPERATION_TIMEOUT_SECS: u64 = 30;

    /// Timeout (seconds) for loading workflow full state (multiple parallel queries).
    /// Default: 60 seconds - accounts for multiple parallel queries.
    pub const FULL_STATE_LOAD_TIMEOUT_SECS: u64 = 60;
}

/// Validation flow constants.
#[allow(dead_code)]
pub mod validation {
    /// Default timeout (seconds) for validation responses, used as fallback
    /// when user `ValidationSettings.timeout_seconds` cannot be loaded.
    pub const VALIDATION_TIMEOUT_SECS: u64 = 60;

    /// Polling interval (milliseconds) for checking validation status.
    pub const VALIDATION_POLL_MS: u64 = 500;

    /// Lower bound for user-configurable validation timeout.
    pub const VALIDATION_TIMEOUT_MIN_SECS: u64 = 5;

    /// Upper bound for user-configurable validation timeout.
    pub const VALIDATION_TIMEOUT_MAX_SECS: u64 = 600;
}

/// Audit log constants.
#[allow(dead_code)]
pub mod audit {
    /// Lower bound (days) for the audit log retention setting.
    pub const RETENTION_MIN_DAYS: i32 = 7;

    /// Upper bound (days) for the audit log retention setting.
    pub const RETENTION_MAX_DAYS: i32 = 90;
}

/// LLM provider HTTP defaults.
#[allow(dead_code)]
pub mod llm_http {
    /// Default HTTP read timeout (seconds) for non-streaming LLM responses.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 30;
}

/// Default limits for database queries to prevent memory explosion.
#[allow(dead_code)] // Some constants prepared for future use
pub mod query_limits {
    /// Default limit for list queries (e.g., list_memories, list_tasks)
    pub const DEFAULT_LIST_LIMIT: usize = 1000;
    /// Maximum allowed limit for list queries
    pub const MAX_LIST_LIMIT: usize = 10_000;
    /// Default limit for MCP call logs
    pub const DEFAULT_MCP_LOGS_LIMIT: usize = 500;
    /// Default limit for message history
    pub const DEFAULT_MESSAGES_LIMIT: usize = 500;
    /// Default limit for model list
    pub const DEFAULT_MODELS_LIMIT: usize = 100;
}

/// Centralized validation constants for Tauri commands.
/// These constants define limits and valid values across the application.
#[allow(dead_code)]
pub mod commands {
    // ----- Agent -----
    /// Maximum length for agent names
    pub const MAX_AGENT_NAME_LEN: usize = 64;
    /// Maximum length for system prompts
    pub const MAX_SYSTEM_PROMPT_LEN: usize = 10000;
    /// Minimum temperature value for LLM
    pub const MIN_TEMPERATURE: f64 = 0.0;
    /// Maximum temperature value for LLM
    pub const MAX_TEMPERATURE: f64 = 2.0;
    /// Minimum max_tokens value
    pub const MIN_MAX_TOKENS: usize = 256;
    /// Maximum max_tokens value
    pub const MAX_MAX_TOKENS: usize = 128000;
    /// Valid lifecycle values
    pub const VALID_LIFECYCLES: &[&str] = &["permanent", "temporary"];

    // ----- MCP Server -----
    /// Maximum length for MCP server names/IDs
    pub const MAX_MCP_SERVER_NAME_LEN: usize = 64;
    /// Maximum length for MCP server descriptions
    pub const MAX_MCP_DESCRIPTION_LEN: usize = 1024;
    /// Maximum number of command arguments
    pub const MAX_MCP_ARGS_COUNT: usize = 50;
    /// Maximum length for each command argument
    pub const MAX_MCP_ARG_LEN: usize = 512;
    /// Maximum number of environment variables
    pub const MAX_MCP_ENV_COUNT: usize = 50;
    /// Maximum length for environment variable names
    pub const MAX_MCP_ENV_NAME_LEN: usize = 128;
    /// Maximum length for environment variable values
    pub const MAX_MCP_ENV_VALUE_LEN: usize = 4096;

    // ----- Message -----
    /// Maximum length for message content
    pub const MAX_MESSAGE_CONTENT_LEN: usize = 100_000;

    // ----- Tool Execution -----
    /// Maximum length for tool names
    pub const MAX_TOOL_NAME_LEN: usize = 128;
    /// Maximum size for tool parameters (50KB)
    pub const MAX_PARAMS_SIZE: usize = 50 * 1024;

    // ----- Thinking -----
    /// Maximum length for thinking content (50KB)
    pub const MAX_THINKING_CONTENT_LEN: usize = 50 * 1024;

    // ----- Models -----
    /// Maximum length for model IDs
    pub const MAX_MODEL_ID_LEN: usize = 128;
}
