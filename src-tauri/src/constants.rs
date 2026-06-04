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
pub mod workflow {
    /// Maximum number of messages to include in LLM context.
    /// Prevents context overflow while maintaining conversation coherence.
    pub const MESSAGE_HISTORY_LIMIT: usize = 50;

    /// Maximum number of streaming workflows that can run concurrently.
    ///
    /// Backend safety net (frontend also enforces per-mode limits). Prevents
    /// runaway resource use when many workflows are launched in parallel.
    pub const DEFAULT_MAX_CONCURRENT_WORKFLOWS: usize = 3;

    /// Timeout (seconds) for loading workflow full state (multiple parallel queries).
    /// Default: 60 seconds - accounts for multiple parallel queries.
    pub const FULL_STATE_LOAD_TIMEOUT_SECS: u64 = 60;
}

/// Async card-compose constants.
pub mod compose {
    /// Maximum number of detached `start_compose_card` generations that may run
    /// concurrently — a GLOBAL cap (not per-agent), distinct from the worker
    /// scheduler's `DEFAULT_MAX_CONCURRENT_WORKFLOWS` (3).
    ///
    /// Each compose is a full LLM tool-loop, so this bounds the cumulative LLM /
    /// resource load and acts as the anti-DoS gate against spamming the
    /// "Générer l'aperçu" button. The in-memory registry is reset at reboot.
    pub const MAX_CONCURRENT_COMPOSE: usize = 4;

    /// Default hard wall-clock ceiling (seconds) for a single detached compose
    /// run (M-3). A compose stuck on a pathological tool-loop holds its slot for
    /// the whole duration; the timeout frees the slot (via the RAII guard) and
    /// emits `kanban:compose_failed`, bounding cap saturation.
    ///
    /// User-configurable via `settings:kanban` (`compose_timeout_secs`), clamped
    /// to `[COMPOSE_TIMEOUT_MIN_SECS, COMPOSE_TIMEOUT_MAX_SECS]`. The default is
    /// generous (10 min) because reasoning models (xhigh) on large contexts can
    /// legitimately run several minutes before capturing the card.
    pub const COMPOSE_TIMEOUT_DEFAULT_SECS: u64 = 600;

    /// Lower bound for the user-configurable compose timeout (seconds).
    pub const COMPOSE_TIMEOUT_MIN_SECS: u64 = 60;

    /// Upper bound for the user-configurable compose timeout (seconds).
    pub const COMPOSE_TIMEOUT_MAX_SECS: u64 = 1800;
}

/// Validation flow constants.
pub mod validation {
    /// Lower bound for user-configurable validation timeout.
    pub const VALIDATION_TIMEOUT_MIN_SECS: u64 = 5;

    /// Upper bound for user-configurable validation timeout.
    pub const VALIDATION_TIMEOUT_MAX_SECS: u64 = 600;

    /// Maximum number of *Manager content/privilege writes (create/update
    /// prompt or skill, restore, grant/revoke) a single agent run may perform.
    ///
    /// Run-scoped volume cap (counted like `mcp_calls_made`) that bounds the DB
    /// amplification a prompt-injected supervisor could trigger in Auto mode
    /// (gonflement de `prompt_version` / `skill_version`). Sized well above any
    /// legitimate self-improvement run: a supervisor refining a handful of
    /// prompts/skills in one pass stays far under it, while a runaway/adversarial
    /// loop is refused once the cap is reached. Self-grants count toward it.
    pub const MANAGER_MAX_WRITES_PER_RUN: usize = 20;
}

/// Audit log constants.
pub mod audit {
    /// Lower bound (days) for the audit log retention setting.
    pub const RETENTION_MIN_DAYS: i32 = 7;

    /// Upper bound (days) for the audit log retention setting.
    pub const RETENTION_MAX_DAYS: i32 = 90;
}

/// LLM provider HTTP defaults.
pub mod llm_http {
    /// Per-read timeout (seconds) for streaming chat completions.
    ///
    /// `reqwest::ClientBuilder::read_timeout` resets after each successful
    /// read, so this value bounds only the *gap* tolerated between two
    /// consecutive SSE frames — not the total request duration. Raising it
    /// therefore never penalizes fast models; it only widens the silent-stall
    /// window the client will sit through.
    ///
    /// Set to 120s because reasoning models (notably DeepSeek V4 pro/flash)
    /// emit their full thinking trace *before* any answer token, and that
    /// phase — or an intermediate proxy that buffers reasoning chunks — can
    /// stay silent for well over 30s. With a 30s bound the read timed out
    /// mid-thinking and surfaced as `reqwest`'s opaque "error decoding
    /// response body" (see [`crate::llm::sse::collect_sse_to_json`]). 120s
    /// covers a long silent thinking phase while still bounding a genuine
    /// hang. Shared with `mistral.rs` / `openai_compatible.rs` test clients.
    pub const DEFAULT_READ_TIMEOUT_SECS: u64 = 120;
}

/// Default limits for database queries to prevent memory explosion.
pub mod query_limits {
    /// Default limit for list queries (e.g., list_memories, list_tasks)
    pub const DEFAULT_LIST_LIMIT: usize = 1000;
    /// Default limit for model list
    pub const DEFAULT_MODELS_LIMIT: usize = 100;
}

/// Per-run MCP resource budget.
///
/// A single agent run (one tool loop) is bounded independently of the
/// per-iteration cap so a runaway model or a misbehaving / compromised MCP
/// server cannot make unbounded calls or stream unbounded data into the
/// prompt context. Both limits are sized well above any legitimate run and
/// are enforced CHECK-BEFORE-REFUSE (an in-flight result is never truncated:
/// the *next* call is refused once a limit is reached).
pub mod mcp {
    /// Maximum number of MCP tool calls a single agent run may make.
    ///
    /// Grounded well above a legitimate ceiling (the iteration cap clamped to a
    /// few hundred, times a handful of calls each) so it only trips on a
    /// runaway or adversarial loop, never on normal use.
    pub const MCP_MAX_CALLS_PER_RUN: usize = 1000;

    /// Maximum cumulative size, in bytes, of serialized MCP results a single
    /// agent run may accumulate (50 MiB).
    ///
    /// Measured on each MCP result's post-strip sink byte size (serialized
    /// result + error — the bytes that actually reach the LLM, the DB, and the
    /// stream); image sidecar bytes are accounted for separately by the
    /// attachment path. Bounds prompt-context inflation and memory growth from a
    /// server returning very large payloads, whether as a success result or an
    /// error.
    pub const MCP_MAX_RESULT_BYTES_PER_RUN: usize = 50 * 1024 * 1024;

    /// Maximum sink byte size (serialized result + error) of a SINGLE MCP tool
    /// result (10 MiB).
    ///
    /// Closes the [`MCP_MAX_RESULT_BYTES_PER_RUN`] soft-ceiling: the
    /// cumulative budget is a PRE-call gate, so the FIRST oversized result would
    /// otherwise pass through whole (e.g. a compromised server's one-shot giant
    /// payload injected into the prompt once). Any MCP result larger than this is
    /// REPLACED by an error instead of being injected into the run context — never
    /// truncated (truncation corrupts the JSON and hides info). SUCCESS-AGNOSTIC:
    /// a giant ERROR payload (carried in `result.error`) is capped identically to
    /// a giant success payload, since a compromised server controls both. Sized
    /// generously for a large legitimate text result yet well under the 50 MiB
    /// cumulative budget; past a few MiB a single result already overflows any
    /// useful LLM context anyway. MCP-only (local tools are user-trusted).
    pub const MCP_MAX_SINGLE_RESULT_BYTES: usize = 10 * 1024 * 1024;
}

/// Centralized validation constants for Tauri commands.
/// These constants define limits and valid values across the application.
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
