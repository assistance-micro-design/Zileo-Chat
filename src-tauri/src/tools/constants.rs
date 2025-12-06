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
