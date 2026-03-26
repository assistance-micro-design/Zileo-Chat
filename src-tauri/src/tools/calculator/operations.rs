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

//! Calculator operation methods (unary, binary, constant).

use super::tool::CalculatorTool;
use crate::tools::constants::calculator::VALID_CONSTANTS;
use crate::tools::response::ResponseBuilder;
use crate::tools::{ToolError, ToolResult};
use serde_json::{json, Value};
use std::f64::consts::{E, FRAC_1_SQRT_2, LN_10, LN_2, PI, SQRT_2, TAU};

impl CalculatorTool {
    /// Executes a unary operation.
    pub(crate) fn execute_unary(&self, operation: &str, value: f64) -> ToolResult<Value> {
        let result = match operation {
            // Trigonometric
            "sin" => value.sin(),
            "cos" => value.cos(),
            "tan" => {
                // Check for near-90-degree angles where tan is undefined
                let normalized = (value / std::f64::consts::FRAC_PI_2).round();
                if (value - normalized * std::f64::consts::FRAC_PI_2).abs() < 1e-10
                    && normalized as i64 % 2 != 0
                {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Tangent undefined at value {} (near odd multiple of PI/2)",
                        value
                    )));
                }
                value.tan()
            }
            "asin" => {
                if !(-1.0..=1.0).contains(&value) {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Arc sine domain error: value {} not in [-1, 1]",
                        value
                    )));
                }
                value.asin()
            }
            "acos" => {
                if !(-1.0..=1.0).contains(&value) {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Arc cosine domain error: value {} not in [-1, 1]",
                        value
                    )));
                }
                value.acos()
            }
            "atan" => value.atan(),

            // Hyperbolic
            "sinh" => value.sinh(),
            "cosh" => value.cosh(),
            "tanh" => value.tanh(),

            // Roots
            "sqrt" => {
                if value < 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot compute square root of negative number ({}). Use abs() first or check input value.",
                        value
                    )));
                }
                value.sqrt()
            }
            "cbrt" => value.cbrt(),

            // Exponential
            "exp" => value.exp(),
            "exp2" => value.exp2(),

            // Logarithmic
            "ln" => {
                if value <= 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Natural logarithm domain error: value {} must be positive",
                        value
                    )));
                }
                value.ln()
            }
            "log10" => {
                if value <= 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Log10 domain error: value {} must be positive",
                        value
                    )));
                }
                value.log10()
            }

            // Rounding
            "floor" => value.floor(),
            "ceil" => value.ceil(),
            "round" => value.round(),
            "trunc" => value.trunc(),

            // Utility
            "abs" => value.abs(),
            "sign" => value.signum(),
            "degrees" => value.to_degrees(),
            "radians" => value.to_radians(),

            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown unary operation: '{}'",
                    operation
                )));
            }
        };

        // Check for NaN or Infinity results
        if result.is_nan() {
            return Err(ToolError::ExecutionFailed(format!(
                "Operation '{}' resulted in NaN for value {}",
                operation, value
            )));
        }
        if result.is_infinite() {
            return Err(ToolError::ExecutionFailed(format!(
                "Operation '{}' resulted in infinity for value {}",
                operation, value
            )));
        }

        Ok(ResponseBuilder::new()
            .success(true)
            .field("operation", json!(operation))
            .field("value", json!(value))
            .field("result", json!(result))
            .message("Calculation completed successfully")
            .build())
    }

    /// Executes a binary operation.
    pub(crate) fn execute_binary(&self, operation: &str, a: f64, b: f64) -> ToolResult<Value> {
        let result = match operation {
            // Arithmetic
            "add" => a + b,
            "subtract" => a - b,
            "multiply" => a * b,
            "divide" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Division by zero. Cannot divide {} by 0.",
                        a
                    )));
                }
                a / b
            }
            "modulo" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Modulo by zero. Cannot compute {} % 0.",
                        a
                    )));
                }
                a % b
            }

            // Power
            "pow" => {
                let result = a.powf(b);
                if result.is_nan() && a < 0.0 && b.fract() != 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot raise negative number {} to non-integer power {}",
                        a, b
                    )));
                }
                result
            }

            // Logarithm with custom base
            "log" => {
                if a <= 0.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Logarithm domain error: value {} must be positive",
                        a
                    )));
                }
                if b <= 0.0 || b == 1.0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Invalid logarithm base: {}. Base must be positive and not equal to 1.",
                        b
                    )));
                }
                a.log(b)
            }

            // Min/Max
            "min" => a.min(b),
            "max" => a.max(b),

            // Two-argument arctangent
            "atan2" => a.atan2(b),

            // Nth root
            "nroot" => {
                if b == 0.0 {
                    return Err(ToolError::ExecutionFailed(
                        "Cannot compute 0th root".to_string(),
                    ));
                }
                if a < 0.0 && (b as i64) % 2 == 0 {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Cannot compute even root ({}) of negative number ({})",
                        b, a
                    )));
                }
                if a < 0.0 {
                    // Odd root of negative number
                    -(-a).powf(1.0 / b)
                } else {
                    a.powf(1.0 / b)
                }
            }

            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown binary operation: '{}'",
                    operation
                )));
            }
        };

        // Check for NaN or Infinity results
        if result.is_nan() {
            return Err(ToolError::ExecutionFailed(format!(
                "Operation '{}({}, {})' resulted in NaN",
                operation, a, b
            )));
        }
        if result.is_infinite() {
            return Err(ToolError::ExecutionFailed(format!(
                "Operation '{}({}, {})' resulted in infinity",
                operation, a, b
            )));
        }

        Ok(ResponseBuilder::new()
            .success(true)
            .field("operation", json!(operation))
            .field("a", json!(a))
            .field("b", json!(b))
            .field("result", json!(result))
            .message("Calculation completed successfully")
            .build())
    }

    /// Retrieves a mathematical constant.
    pub(crate) fn get_constant(&self, name: &str) -> ToolResult<Value> {
        let (value, description) = match name.to_lowercase().as_str() {
            "pi" => (PI, "Circle constant (ratio of circumference to diameter)"),
            "e" => (E, "Euler's number (base of natural logarithm)"),
            "tau" => (TAU, "Circle constant (2 * PI)"),
            "sqrt2" => (SQRT_2, "Square root of 2"),
            "ln2" => (LN_2, "Natural logarithm of 2"),
            "ln10" => (LN_10, "Natural logarithm of 10"),
            "sqrt1_2" | "frac_1_sqrt_2" => (FRAC_1_SQRT_2, "1 / sqrt(2)"),
            _ => {
                return Err(ToolError::InvalidInput(format!(
                    "Unknown constant: '{}'. Valid constants: {:?}",
                    name, VALID_CONSTANTS
                )));
            }
        };

        Ok(ResponseBuilder::new()
            .success(true)
            .field("operation", json!("constant"))
            .field("name", json!(name))
            .field("result", json!(value))
            .message(description)
            .build())
    }
}
