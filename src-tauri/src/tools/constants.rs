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
}

// ===== Todo Tool =====
pub mod todo {
    pub const MAX_NAME_LENGTH: usize = 128;
    pub const MAX_DESCRIPTION_LENGTH: usize = 1000;
    pub const PRIORITY_MIN: u8 = 1;
    pub const PRIORITY_MAX: u8 = 5;
    pub const VALID_STATUSES: &[&str] = &["pending", "in_progress", "completed", "blocked"];
}

// ===== User Question Tool =====
#[allow(dead_code)]
pub mod user_question {
    pub const MAX_QUESTION_LENGTH: usize = 2000;
    pub const MAX_OPTION_LABEL_LENGTH: usize = 256;
    pub const MAX_OPTIONS: usize = 20;
    pub const MAX_CONTEXT_LENGTH: usize = 5000;
    pub const MAX_TEXT_RESPONSE_LENGTH: usize = 10000;
    pub const POLL_INTERVALS_MS: &[u64] = &[500, 500, 1000, 1000, 2000, 2000, 5000];
    pub const VALID_TYPES: &[&str] = &["checkbox", "text", "mixed"];
    pub const VALID_STATUSES: &[&str] = &["pending", "answered", "skipped"];
}

// ===== Sub-Agent Tools =====
#[allow(unused_imports)]
pub mod sub_agent {
    pub use crate::models::sub_agent::constants::MAX_SUB_AGENTS;
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
