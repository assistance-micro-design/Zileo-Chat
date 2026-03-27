use super::tool::CalculatorTool;
use crate::tools::{Tool, ToolError};
use serde_json::json;

fn calculator() -> CalculatorTool {
    CalculatorTool::new()
}

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
    let result = tool
        .execute(json!({"operation": "nroot", "a": -4.0, "b": 2.0}))
        .await;
    assert!(result.is_err());
}

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
fn test_default_impl() {
    let tool = CalculatorTool;
    let def = tool.definition();
    assert_eq!(def.id, "CalculatorTool");
}
