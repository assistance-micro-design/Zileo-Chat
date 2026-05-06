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

//! Tool-specific constants.
//!
//! Application-wide constants (workflow, query_limits, commands) are in
//! [`crate::constants`].

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
}

pub mod todo {
    pub const MAX_NAME_LENGTH: usize = 128;
    pub const MAX_DESCRIPTION_LENGTH: usize = 1000;
    pub const PRIORITY_MIN: u8 = 1;
    pub const PRIORITY_MAX: u8 = 5;
    pub const VALID_STATUSES: &[&str] = &["pending", "in_progress", "completed", "blocked"];

    /// Standard SELECT fields for Task queries.
    /// Use this constant for consistent field selection in get_task() and similar queries.
    pub const TASK_SELECT_FIELDS: &str = "meta::id(id) AS id, workflow_id, name, description, agent_assigned, priority, status, dependencies, duration_ms, created_at, completed_at";
}

pub mod user_question {
    pub const MAX_QUESTION_LENGTH: usize = 2000;
    pub const MAX_OPTION_ID_LENGTH: usize = 64;
    pub const MAX_OPTION_LABEL_LENGTH: usize = 256;
    pub const MAX_OPTIONS: usize = 20;
    pub const MAX_CONTEXT_LENGTH: usize = 5000;
    pub const MAX_TEXT_RESPONSE_LENGTH: usize = 10000;
    pub const POLL_INTERVALS_MS: &[u64] = &[500, 500, 1000, 1000, 2000, 2000, 5000];
    pub const VALID_TYPES: &[&str] = &["checkbox", "text", "mixed"];

    // Configurable timeout for wait_for_response
    /// Default timeout (seconds) for waiting for user response.
    /// After this duration, the question status is set to "timeout" and an error is returned.
    pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

    // Circuit Breaker for UserQuestionTool
    /// Number of consecutive timeouts before opening the circuit breaker.
    /// When reached, new questions are rejected until cooldown expires.
    pub const CIRCUIT_FAILURE_THRESHOLD: u32 = 3;

    /// Cooldown period (seconds) before circuit breaker transitions to half-open.
    /// After this period, one question is allowed to test if user is responsive.
    pub const CIRCUIT_COOLDOWN_SECS: u64 = 60;
}

#[allow(unused_imports)]
pub mod sub_agent {
    pub use crate::models::sub_agent::constants::{MAX_PARALLEL_TASKS_PER_BATCH, MAX_SUB_AGENTS};

    // Inactivity Timeout with Heartbeat
    /// Timeout (seconds) without any activity before aborting sub-agent execution.
    /// Activity includes: LLM tokens received, tool calls started/completed, MCP responses.
    pub const INACTIVITY_TIMEOUT_SECS: u64 = 300; // 5 minutes

    /// Interval (seconds) between activity checks in the monitoring loop.
    pub const ACTIVITY_CHECK_INTERVAL_SECS: u64 = 30;

    // Centralized Magic Numbers
    /// Maximum characters for task description truncation.
    pub const TASK_DESC_TRUNCATE_CHARS: usize = 100;

    /// Default timeout for validation responses (seconds).
    pub const VALIDATION_TIMEOUT_SECS: u64 = 60;

    /// Polling interval for checking validation status (milliseconds).
    pub const VALIDATION_POLL_MS: u64 = 500;

    // Circuit Breaker for Sub-Agent Execution
    /// Number of consecutive failures before opening the circuit breaker.
    /// When reached, sub-agent executions are rejected until cooldown expires.
    pub const CIRCUIT_FAILURE_THRESHOLD: u32 = 3;

    /// Cooldown period (seconds) before circuit breaker transitions to half-open.
    /// After this period, one execution is allowed to test if the system recovered.
    pub const CIRCUIT_COOLDOWN_SECS: u64 = 60;

    // Retry with Exponential Backoff
    /// Maximum number of retry attempts for transient errors.
    /// Set to 2 for a total of 3 attempts (initial + 2 retries).
    pub const MAX_RETRY_ATTEMPTS: u32 = 2;

    /// Initial delay (milliseconds) before first retry.
    /// Subsequent delays are doubled: 500ms -> 1000ms -> 2000ms.
    pub const INITIAL_RETRY_DELAY_MS: u64 = 500;
}

pub mod calculator {
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
