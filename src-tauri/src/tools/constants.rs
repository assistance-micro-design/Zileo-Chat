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

//! Centralized constants for tools.

// ===== Memory Tool =====
pub mod memory {
    pub const MAX_CONTENT_LENGTH: usize = 50_000;
    pub const DEFAULT_LIMIT: usize = 10;
    pub const MAX_LIMIT: usize = 100;
    pub const DEFAULT_SIMILARITY_THRESHOLD: f64 = 0.7;
    pub const VALID_TYPES: &[&str] = &["user_pref", "context", "knowledge", "decision"];

    // Importance defaults by memory type
    pub const DEFAULT_IMPORTANCE: f64 = 0.5;
    pub const IMPORTANCE_USER_PREF: f64 = 0.8;
    pub const IMPORTANCE_DECISION: f64 = 0.7;
    pub const IMPORTANCE_KNOWLEDGE: f64 = 0.6;
    pub const IMPORTANCE_CONTEXT: f64 = 0.3;

    // Compact list preview
    pub const COMPACT_PREVIEW_LENGTH: usize = 100;

    // TTL
    pub const DEFAULT_CONTEXT_TTL_DAYS: i64 = 7;

    // Scoring weights for composite search ranking
    pub const SCORE_WEIGHT_COSINE: f64 = 0.70;
    pub const SCORE_WEIGHT_IMPORTANCE: f64 = 0.15;
    pub const SCORE_WEIGHT_RECENCY: f64 = 0.15;
    pub const RECENCY_DECAY_DAYS: f64 = 30.0;

    /// Types that are stored as general (cross-workflow) by default
    pub const GENERAL_SCOPE_TYPES: &[&str] = &["user_pref", "knowledge"];
    /// Types that are stored as workflow-scoped by default (complement of GENERAL_SCOPE_TYPES)
    #[allow(dead_code)]
    pub const WORKFLOW_SCOPE_TYPES: &[&str] = &["context", "decision"];
}

// ===== Todo Tool =====
pub mod todo {
    pub const MAX_NAME_LENGTH: usize = 128;
    pub const MAX_DESCRIPTION_LENGTH: usize = 1000;
    pub const PRIORITY_MIN: u8 = 1;
    pub const PRIORITY_MAX: u8 = 5;
    pub const VALID_STATUSES: &[&str] = &["pending", "in_progress", "completed", "blocked"];

    /// Standard SELECT fields for Task queries (OPT-TODO-9).
    /// Use this constant for consistent field selection in get_task() and similar queries.
    pub const TASK_SELECT_FIELDS: &str = "meta::id(id) AS id, workflow_id, name, description, agent_assigned, priority, status, dependencies, duration_ms, created_at, completed_at";
}

// ===== User Question Tool =====
#[allow(dead_code)]
pub mod user_question {
    pub const MAX_QUESTION_LENGTH: usize = 2000;
    pub const MAX_OPTION_ID_LENGTH: usize = 64;
    pub const MAX_OPTION_LABEL_LENGTH: usize = 256;
    pub const MAX_OPTIONS: usize = 20;
    pub const MAX_CONTEXT_LENGTH: usize = 5000;
    pub const MAX_TEXT_RESPONSE_LENGTH: usize = 10000;
    pub const POLL_INTERVALS_MS: &[u64] = &[500, 500, 1000, 1000, 2000, 2000, 5000];
    pub const VALID_TYPES: &[&str] = &["checkbox", "text", "mixed"];
    pub const VALID_STATUSES: &[&str] = &["pending", "answered", "skipped", "timeout"];

    // OPT-UQ-7: Configurable timeout for wait_for_response
    /// Default timeout (seconds) for waiting for user response.
    /// After this duration, the question status is set to "timeout" and an error is returned.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

    // OPT-UQ-12: Circuit Breaker for UserQuestionTool
    /// Number of consecutive timeouts before opening the circuit breaker.
    /// When reached, new questions are rejected until cooldown expires.
    pub const CIRCUIT_FAILURE_THRESHOLD: u32 = 3;

    /// Cooldown period (seconds) before circuit breaker transitions to half-open.
    /// After this period, one question is allowed to test if user is responsive.
    pub const CIRCUIT_COOLDOWN_SECS: u64 = 60;
}

// ===== Sub-Agent Tools =====
#[allow(unused_imports)]
pub mod sub_agent {
    pub use crate::models::sub_agent::constants::MAX_SUB_AGENTS;

    // OPT-SA-1: Inactivity Timeout with Heartbeat
    /// Timeout (seconds) without any activity before aborting sub-agent execution.
    /// Activity includes: LLM tokens received, tool calls started/completed, MCP responses.
    pub const INACTIVITY_TIMEOUT_SECS: u64 = 300; // 5 minutes

    /// Interval (seconds) between activity checks in the monitoring loop.
    pub const ACTIVITY_CHECK_INTERVAL_SECS: u64 = 30;

    // OPT-SA-3: Centralized Magic Numbers
    /// Maximum characters for result summaries in sub-agent reports.
    /// (Used in OPT-SA-4/5 when event emission is unified)
    #[allow(dead_code)]
    pub const RESULT_SUMMARY_MAX_CHARS: usize = 200;

    /// Maximum characters for task description truncation.
    pub const TASK_DESC_TRUNCATE_CHARS: usize = 100;

    /// Default timeout for validation responses (seconds).
    pub const VALIDATION_TIMEOUT_SECS: u64 = 60;

    /// Polling interval for checking validation status (milliseconds).
    pub const VALIDATION_POLL_MS: u64 = 500;

    // OPT-SA-8: Circuit Breaker for Sub-Agent Execution
    /// Number of consecutive failures before opening the circuit breaker.
    /// When reached, sub-agent executions are rejected until cooldown expires.
    pub const CIRCUIT_FAILURE_THRESHOLD: u32 = 3;

    /// Cooldown period (seconds) before circuit breaker transitions to half-open.
    /// After this period, one execution is allowed to test if the system recovered.
    pub const CIRCUIT_COOLDOWN_SECS: u64 = 60;

    // OPT-SA-10: Retry with Exponential Backoff
    /// Maximum number of retry attempts for transient errors.
    /// Set to 2 for a total of 3 attempts (initial + 2 retries).
    pub const MAX_RETRY_ATTEMPTS: u32 = 2;

    /// Initial delay (milliseconds) before first retry.
    /// Subsequent delays are doubled: 500ms -> 1000ms -> 2000ms.
    pub const INITIAL_RETRY_DELAY_MS: u64 = 500;
}

// ===== Calculator Tool =====
#[allow(dead_code)]
pub mod calculator {
    /// Maximum supported value (to prevent overflow)
    pub const MAX_VALUE: f64 = 1e308;

    /// Minimum positive value (for precision)
    pub const MIN_POSITIVE: f64 = 1e-308;

    /// Valid unary operations
    pub const UNARY_OPS: &[&str] = &[
        "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "sqrt", "cbrt", "exp",
        "exp2", "ln", "log10", "abs", "sign", "floor", "ceil", "round", "trunc", "degrees",
        "radians",
    ];

    /// Valid binary operations
    pub const BINARY_OPS: &[&str] = &[
        "add", "subtract", "multiply", "divide", "modulo", "pow", "log", "min", "max", "atan2",
        "nroot",
    ];

    /// Valid constant names
    pub const VALID_CONSTANTS: &[&str] = &["pi", "e", "tau", "sqrt2", "ln2", "ln10"];
}

// ===== Workflow Constants (OPT-WF-3, OPT-WF-9) =====
/// Constants for workflow execution and streaming.
#[allow(dead_code)]
pub mod workflow {
    /// Maximum number of messages to include in LLM context (OPT-WF-3).
    /// Prevents context overflow while maintaining conversation coherence.
    pub const MESSAGE_HISTORY_LIMIT: usize = 50;

    // OPT-WF-9: Tokio Timeout Constants
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

// ===== Query Limits (OPT-DB-8) =====
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

// ===== Command Validation Constants (OPT-2) =====
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
    pub const MIN_TEMPERATURE: f32 = 0.0;
    /// Maximum temperature value for LLM
    pub const MAX_TEMPERATURE: f32 = 2.0;
    /// Minimum max_tokens value
    pub const MIN_MAX_TOKENS: usize = 256;
    /// Maximum max_tokens value
    pub const MAX_MAX_TOKENS: usize = 128000;
    /// Valid LLM providers
    pub const VALID_PROVIDERS: &[&str] = &["Mistral", "Ollama", "Demo"];
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
    /// Valid model providers (lowercase)
    pub const VALID_MODEL_PROVIDERS: &[&str] = &["mistral", "ollama"];
}
