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

//! Scientific Calculator Tool - struct, constructor, and Tool trait implementation.
//!
//! Operation methods (unary, binary, constant) are in `operations.rs`.

use crate::tools::constants::calculator::{BINARY_OPS, UNARY_OPS};
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
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

USE THIS TOOL WHEN:
- You need to perform arithmetic operations (add, subtract, multiply, divide)
- You need to calculate trigonometric values (sin, cos, tan, asin, acos, atan)
- You need to compute logarithms and exponentials (log, ln, exp, pow)
- You need to access mathematical constants (PI, E, TAU)
- You need to convert between degrees and radians
- You need intermediate calculations for complex workflows

DO NOT USE THIS TOOL WHEN:
- The calculation is trivial mental math (2+2)
- You need to store results persistently (use MemoryTool instead)
- You need multiple chained operations (call tool multiple times)
- Working with complex/imaginary numbers (not supported)

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

BEST PRACTICES:
- Trigonometric functions expect radians, not degrees (use radians operation first)
- Use abs() before sqrt() for potentially negative values
- Check domain constraints before ln/log operations (value must be positive)
- Use nroot for nth roots instead of pow with fractions for better precision

EXAMPLES:
1. Calculate sine: {"operation": "sin", "value": 1.5708}
2. Add numbers: {"operation": "add", "a": 10, "b": 5}
3. Power: {"operation": "pow", "a": 2, "b": 10}
4. Get PI: {"operation": "constant", "name": "pi"}
5. Convert to radians: {"operation": "radians", "value": 180}
6. Hyperbolic sine: {"operation": "sinh", "value": 1.0}
7. Log base 2: {"operation": "log", "a": 8, "b": 2}
8. Cube root: {"operation": "nroot", "a": 27, "b": 3}
9. Modulo: {"operation": "modulo", "a": 17, "b": 5}
10. Minimum: {"operation": "min", "a": 3.5, "b": 2.1}

ERROR HANDLING:
- sqrt(negative): Domain error - use abs() first or check input
- ln/log10(<=0): Domain error - value must be positive
- asin/acos(|x|>1): Domain error - value must be in [-1, 1]
- divide/modulo by zero: Division error
- tan(PI/2): Undefined at odd multiples of PI/2
- log base 1: Invalid base - must be positive and not equal to 1"#
                .to_string(),

            input_schema: json!({
                "type": "object",
                "properties": {
                    "operation": {
                        "type": "string",
                        "description": "Operation: unary (sin/cos/tan/sqrt/exp/ln/abs/floor/ceil/round/degrees/radians), binary (add/subtract/multiply/divide/pow/log/min/max), or 'constant' (pi/e/tau)"
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
}
