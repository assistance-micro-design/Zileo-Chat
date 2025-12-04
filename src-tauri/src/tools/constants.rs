// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
