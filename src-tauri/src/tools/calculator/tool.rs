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

//! Scientific Calculator Tool Implementation
//!
//! Stateless tool providing mathematical operations for agents.

use crate::tools::constants::calculator::{BINARY_OPS, UNARY_OPS, VALID_CONSTANTS};
use crate::tools::response::ResponseBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::f64::consts::{E, FRAC_1_SQRT_2, LN_10, LN_2, PI, SQRT_2, TAU};
use tracing::debug;

/// Scientific calculator tool for agents.
///
/// This tool provides mathematical operations:
/// - Basic arithmetic (add, subtract, multiply, divide)
/// - Trigonometric functions (sin, cos, tan, etc.)
/// - Logarithmic functions (log, ln, log10)
/// - Exponential functions (exp, pow, sqrt)
/// - Mathematical constants (PI, E, TAU)
///
/// # Stateless Design
///
/// Unlike MemoryTool or TodoTool, CalculatorTool is completely stateless.
/// It does not require database access or workflow scoping.
pub struct CalculatorTool;

impl CalculatorTool {
    /// Creates a new CalculatorTool instance.
    pub fn new() -> Self {
        debug!("CalculatorTool instance created");
        Self
    }

    /// Executes a unary operation.
    fn execute_unary(&self, operation: &str, value: f64) -> ToolResult<Value> {
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
    fn execute_binary(&self, operation: &str, a: f64, b: f64) -> ToolResult<Value> {
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
    fn get_constant(&self, name: &str) -> ToolResult<Value> {
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

impl Default for CalculatorTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for CalculatorTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "CalculatorTool".to_string(),
            name: "Scientific Calculator".to_string(),
            description: r#"Performs mathematical calculations for agents.

USE THIS TOOL TO:
- Perform arithmetic operations (add, subtract, multiply, divide)
- Calculate trigonometric values (sin, cos, tan, asin, acos, atan)
- Compute logarithms and exponentials (log, ln, exp, pow)
- Access mathematical constants (PI, E, TAU)
- Convert between degrees and radians

OPERATIONS:

**Unary Operations** (require "value"):
- sin, cos, tan: Trigonometric (value in radians)
- asin, acos, atan: Inverse trigonometric
- sinh, cosh, tanh: Hyperbolic
- sqrt, cbrt: Square/cube root
- exp, exp2: Exponential (e^x, 2^x)
- ln, log10: Natural/base-10 logarithm
- abs, sign: Absolute value, sign
- floor, ceil, round, trunc: Rounding
- degrees, radians: Angle conversion

**Binary Operations** (require "a" and "b"):
- add, subtract, multiply, divide, modulo: Arithmetic
- pow: Power (a^b)
- log: Logarithm base b of a
- min, max: Minimum/maximum
- atan2: Two-argument arctangent
- nroot: nth root (b-th root of a)

**Constants** (require "name"):
- pi, e, tau, sqrt2, ln2, ln10

EXAMPLES:
1. Calculate sine: {"operation": "sin", "value": 1.5708}
2. Add numbers: {"operation": "add", "a": 10, "b": 5}
3. Power: {"operation": "pow", "a": 2, "b": 10}
4. Get PI: {"operation": "constant", "name": "pi"}
5. Convert to radians: {"operation": "radians", "value": 180}"#
                .to_string(),

            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "The operation to perform"
                    },
                    "value": {
                        "type": "number",
                        "description": "Input value for unary operations"
                    },
                    "a": {
                        "type": "number",
                        "description": "First operand for binary operations"
                    },
                    "b": {
                        "type": "number",
                        "description": "Second operand for binary operations"
                    },
                    "name": {
                        "type": "string",
                        "enum": ["pi", "e", "tau", "sqrt2", "ln2", "ln10"],
                        "description": "Constant name (for 'constant' operation)"
                    }
                },
                "required": ["operation"]
            }),

            output_schema: json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"},
                    "operation": {"type": "string"},
                    "result": {"type": "number"},
                    "message": {"type": "string"},
                    "a": {"type": "number"},
                    "b": {"type": "number"},
                    "value": {"type": "number"},
                    "name": {"type": "string"}
                }
            }),

            requires_confirmation: false,
        }
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing 'operation' field".to_string()))?;

        debug!(operation = %operation, "Executing calculator operation");

        // Dispatch based on operation type
        if UNARY_OPS.contains(&operation) {
            let value = input["value"].as_f64().ok_or_else(|| {
                ToolError::InvalidInput(format!(
                    "Unary operation '{}' requires 'value' field (number)",
                    operation
                ))
            })?;
            self.execute_unary(operation, value)
        } else if BINARY_OPS.contains(&operation) {
            let a = input["a"].as_f64().ok_or_else(|| {
                ToolError::InvalidInput(format!(
                    "Binary operation '{}' requires 'a' field (number)",
                    operation
                ))
            })?;
            let b = input["b"].as_f64().ok_or_else(|| {
                ToolError::InvalidInput(format!(
                    "Binary operation '{}' requires 'b' field (number)",
                    operation
                ))
            })?;
            self.execute_binary(operation, a, b)
        } else if operation == "constant" {
            let name = input["name"].as_str().ok_or_else(|| {
                ToolError::InvalidInput(
                    "Constant operation requires 'name' field (string)".to_string(),
                )
            })?;
            self.get_constant(name)
        } else {
            Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: unary={:?}, binary={:?}, constant",
                operation, UNARY_OPS, BINARY_OPS
            )))
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        // Check operation field exists
        let operation = input["operation"].as_str().ok_or_else(|| {
            ToolError::InvalidInput(
                "Missing required field 'operation'. Specify operation type.".to_string(),
            )
        })?;

        // Validate operation is known
        let is_unary = UNARY_OPS.contains(&operation);
        let is_binary = BINARY_OPS.contains(&operation);
        let is_constant = operation == "constant";

        if !is_unary && !is_binary && !is_constant {
            return Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: {:?} (unary), {:?} (binary), 'constant'",
                operation, UNARY_OPS, BINARY_OPS
            )));
        }

        // Validate required parameters based on operation type
        if is_unary && input["value"].as_f64().is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Unary operation '{}' requires 'value' field (number)",
                operation
            )));
        }

        if is_binary {
            if input["a"].as_f64().is_none() {
                return Err(ToolError::InvalidInput(format!(
                    "Binary operation '{}' requires 'a' field (number)",
                    operation
                )));
            }
            if input["b"].as_f64().is_none() {
                return Err(ToolError::InvalidInput(format!(
                    "Binary operation '{}' requires 'b' field (number)",
                    operation
                )));
            }
        }

        if is_constant && input["name"].as_str().is_none() {
            return Err(ToolError::InvalidInput(
                "Constant operation requires 'name' field. Valid names: pi, e, tau, sqrt2, ln2, ln10".to_string(),
            ));
        }

        Ok(())
    }

    fn requires_confirmation(&self) -> bool {
        false // Calculator operations are safe, no side effects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to create tool
    fn calculator() -> CalculatorTool {
        CalculatorTool::new()
    }

    // ==================== Unary Operations ====================

    #[tokio::test]
    async fn test_sin_zero() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sin", "value": 0.0}))
            .await
            .unwrap();
        assert!(result["success"].as_bool().unwrap());
        let res = result["result"].as_f64().unwrap();
        assert!((res - 0.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sin_pi_half() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sin", "value": std::f64::consts::FRAC_PI_2}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_cos_zero() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "cos", "value": 0.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_tan_pi_quarter() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "tan", "value": std::f64::consts::FRAC_PI_4}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sqrt_positive() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sqrt", "value": 4.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 2.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sqrt_negative_error() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sqrt", "value": -4.0}))
            .await;
        assert!(result.is_err());
        match result {
            Err(ToolError::ExecutionFailed(msg)) => {
                assert!(msg.contains("negative"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    #[tokio::test]
    async fn test_ln_positive() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "ln", "value": std::f64::consts::E}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_ln_non_positive_error() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "ln", "value": -1.0}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_abs_negative() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "abs", "value": -5.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 5.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_degrees_pi() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "degrees", "value": std::f64::consts::PI}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 180.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_radians_180() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "radians", "value": 180.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::PI).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_floor() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "floor", "value": 3.7}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 3.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_ceil() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "ceil", "value": 3.2}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 4.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_round() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "round", "value": 3.5}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 4.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_cbrt() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "cbrt", "value": 8.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 2.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_exp() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "exp", "value": 1.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::E).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_exp2() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "exp2", "value": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 8.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_log10() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "log10", "value": 100.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 2.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sinh() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sinh", "value": 0.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 0.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_cosh() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "cosh", "value": 0.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_tanh() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "tanh", "value": 0.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 0.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_asin() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "asin", "value": 1.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_asin_domain_error() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "asin", "value": 2.0}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_acos() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "acos", "value": 0.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::FRAC_PI_2).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_atan() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "atan", "value": 1.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sign_negative() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sign", "value": -5.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - (-1.0)).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_sign_positive() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "sign", "value": 5.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_trunc() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "trunc", "value": -3.7}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - (-3.0)).abs() < 1e-10);
    }

    // ==================== Binary Operations ====================

    #[tokio::test]
    async fn test_add() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "add", "a": 10.0, "b": 5.0}))
            .await
            .unwrap();
        assert!(result["success"].as_bool().unwrap());
        let res = result["result"].as_f64().unwrap();
        assert!((res - 15.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_subtract() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "subtract", "a": 10.0, "b": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 7.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_multiply() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "multiply", "a": 4.0, "b": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 12.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_divide() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "divide", "a": 10.0, "b": 2.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 5.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_divide_by_zero() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "divide", "a": 10.0, "b": 0.0}))
            .await;
        assert!(result.is_err());
        match result {
            Err(ToolError::ExecutionFailed(msg)) => {
                assert!(msg.contains("Division by zero"));
            }
            _ => panic!("Expected ExecutionFailed error"),
        }
    }

    #[tokio::test]
    async fn test_modulo() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "modulo", "a": 10.0, "b": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_modulo_by_zero() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "modulo", "a": 10.0, "b": 0.0}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_pow() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "pow", "a": 2.0, "b": 10.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 1024.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_log_base() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "log", "a": 8.0, "b": 2.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 3.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_log_invalid_base() {
        let tool = calculator();
        // Base 1 is invalid
        let result = tool
            .execute(json!({"operation": "log", "a": 8.0, "b": 1.0}))
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_min() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "min", "a": 3.0, "b": 5.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 3.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_max() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "max", "a": 3.0, "b": 5.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 5.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_atan2() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "atan2", "a": 1.0, "b": 1.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::FRAC_PI_4).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_nroot_cube() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "nroot", "a": 8.0, "b": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - 2.0).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_nroot_negative_odd() {
        let tool = calculator();
        // Cube root of -8 should be -2
        let result = tool
            .execute(json!({"operation": "nroot", "a": -8.0, "b": 3.0}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - (-2.0)).abs() < 1e-10);
    }

    #[tokio::test]
    async fn test_nroot_negative_even_error() {
        let tool = calculator();
        // Even root of negative number should error
        let result = tool
            .execute(json!({"operation": "nroot", "a": -4.0, "b": 2.0}))
            .await;
        assert!(result.is_err());
    }

    // ==================== Constants ====================

    #[tokio::test]
    async fn test_constant_pi() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "pi"}))
            .await
            .unwrap();
        assert!(result["success"].as_bool().unwrap());
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::PI).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_e() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "e"}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::E).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_tau() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "tau"}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::TAU).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_sqrt2() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "sqrt2"}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::SQRT_2).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_ln2() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "ln2"}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::LN_2).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_ln10() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "ln10"}))
            .await
            .unwrap();
        let res = result["result"].as_f64().unwrap();
        assert!((res - std::f64::consts::LN_10).abs() < 1e-15);
    }

    #[tokio::test]
    async fn test_constant_unknown() {
        let tool = calculator();
        let result = tool
            .execute(json!({"operation": "constant", "name": "unknown"}))
            .await;
        assert!(result.is_err());
    }

    // ==================== Validation ====================

    #[test]
    fn test_validate_missing_operation() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"value": 5.0}));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("operation"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_unknown_operation() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "unknown"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_unary_missing_value() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "sin"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_binary_missing_a() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "add", "b": 5.0}));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_binary_missing_b() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "add", "a": 5.0}));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_constant_missing_name() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "constant"}));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_valid_unary() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "sin", "value": 0.0}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_valid_binary() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "add", "a": 1.0, "b": 2.0}));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_valid_constant() {
        let tool = calculator();
        let result = tool.validate_input(&json!({"operation": "constant", "name": "pi"}));
        assert!(result.is_ok());
    }

    // ==================== Tool Trait ====================

    #[test]
    fn test_definition() {
        let tool = calculator();
        let def = tool.definition();
        assert_eq!(def.id, "CalculatorTool");
        assert_eq!(def.name, "Scientific Calculator");
        assert!(!def.requires_confirmation);
        assert!(def.description.contains("sin"));
        assert!(def.description.contains("add"));
        assert!(def.description.contains("pi"));
    }

    #[test]
    fn test_requires_confirmation() {
        let tool = calculator();
        assert!(!tool.requires_confirmation());
    }

    #[test]
    fn test_default_impl() {
        let tool = CalculatorTool::default();
        let def = tool.definition();
        assert_eq!(def.id, "CalculatorTool");
    }
}
