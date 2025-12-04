// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Scientific Calculator Tool Module
//!
//! Provides mathematical operations for agents:
//! - Arithmetic (add, subtract, multiply, divide, modulo)
//! - Trigonometric (sin, cos, tan, asin, acos, atan)
//! - Hyperbolic (sinh, cosh, tanh)
//! - Exponential (exp, exp2, pow)
//! - Logarithmic (ln, log10, log)
//! - Rounding (floor, ceil, round, trunc)
//! - Utility (abs, sign, degrees, radians)
//! - Mathematical constants (PI, E, TAU, SQRT2, LN2, LN10)

mod tool;

pub use tool::CalculatorTool;
