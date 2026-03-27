use super::*;
use crate::agents::core::agent::{ReasoningSource, ReasoningStepData, ToolExecutionData};
use crate::tools::constants::sub_agent::{
    ACTIVITY_CHECK_INTERVAL_SECS, INACTIVITY_TIMEOUT_SECS, INITIAL_RETRY_DELAY_MS,
    MAX_RETRY_ATTEMPTS,
};
use tokio_util::sync::CancellationToken;

#[test]
fn test_execution_result_default() {
    let result = ExecutionResult::default();
    assert!(!result.success);
    assert!(result.report.is_empty());
    assert!(result.error_message.is_none());
    assert_eq!(result.metrics.duration_ms, 0);
    assert_eq!(result.metrics.tokens_input, 0);
    assert_eq!(result.metrics.tokens_output, 0);
    assert!(result.tool_executions.is_empty());
    assert!(result.reasoning_steps.is_empty());
}

#[test]
fn test_execution_result_preserves_tool_executions() {
    let tool_exec = ToolExecutionData {
        tool_type: "mcp".to_string(),
        tool_name: "find_symbol".to_string(),
        server_name: Some("serena".to_string()),
        input_params: serde_json::json!({"name": "MyClass"}),
        output_result: serde_json::json!({"found": true}),
        success: true,
        error_message: None,
        duration_ms: 150,
        iteration: 0,
        sequence: 0,
    };
    let result = ExecutionResult {
        success: true,
        report: "Done".to_string(),
        metrics: SubAgentMetrics {
            duration_ms: 1000,
            tokens_input: 500,
            tokens_output: 200,
        },
        error_message: None,
        tool_executions: vec![tool_exec],
        reasoning_steps: Vec::new(),
    };
    assert_eq!(result.tool_executions.len(), 1);
    assert_eq!(result.tool_executions[0].tool_name, "find_symbol");
    assert_eq!(
        result.tool_executions[0].server_name,
        Some("serena".to_string())
    );
}

#[test]
fn test_execution_result_preserves_reasoning_steps() {
    let step = ReasoningStepData {
        content: "Analyzing the codebase structure".to_string(),
        duration_ms: 300,
        sequence: 0,
        source: ReasoningSource::AgentFlow,
    };
    let result = ExecutionResult {
        success: true,
        report: "Done".to_string(),
        metrics: SubAgentMetrics {
            duration_ms: 1000,
            tokens_input: 500,
            tokens_output: 200,
        },
        error_message: None,
        tool_executions: Vec::new(),
        reasoning_steps: vec![step],
    };
    assert_eq!(result.reasoning_steps.len(), 1);
    assert_eq!(
        result.reasoning_steps[0].content,
        "Analyzing the codebase structure"
    );
    assert_eq!(result.reasoning_steps[0].duration_ms, 300);
}

#[test]
fn test_inactivity_timeout_constants() {
    assert_eq!(INACTIVITY_TIMEOUT_SECS, 300);
    assert_eq!(ACTIVITY_CHECK_INTERVAL_SECS, 30);
    const _: () = assert!(
        ACTIVITY_CHECK_INTERVAL_SECS < INACTIVITY_TIMEOUT_SECS / 2,
        "Check interval should be less than half the timeout"
    );
}

#[test]
fn test_cancellation_token_clone_shares_state() {
    let token = CancellationToken::new();
    let token2 = token.clone();
    assert!(!token.is_cancelled());
    assert!(!token2.is_cancelled());

    token.cancel();

    assert!(token.is_cancelled());
    assert!(token2.is_cancelled());
}

#[tokio::test]
async fn test_cancellation_token_immediate_cancellation() {
    let token = CancellationToken::new();
    token.cancel();

    let result =
        tokio::time::timeout(std::time::Duration::from_millis(100), token.cancelled()).await;

    assert!(result.is_ok(), "cancelled() should complete immediately");
}

#[tokio::test]
async fn test_cancellation_token_async_cancellation() {
    let token = CancellationToken::new();
    let token_clone = token.clone();

    let handle = tokio::spawn(async move {
        token_clone.cancelled().await;
        "cancelled"
    });

    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    token.cancel();

    let result = tokio::time::timeout(std::time::Duration::from_millis(100), handle).await;

    assert!(result.is_ok(), "Task should complete after cancellation");
    assert_eq!(result.unwrap().unwrap(), "cancelled");
}

#[test]
fn test_retry_constants() {
    assert_eq!(MAX_RETRY_ATTEMPTS, 2);
    assert_eq!(INITIAL_RETRY_DELAY_MS, 500);
    assert_eq!(SubAgentExecutor::total_retry_delay_ms(), 1500);
}

#[test]
fn test_is_retryable_error_timeout_patterns() {
    assert!(SubAgentExecutor::is_retryable_error("Connection timeout"));
    assert!(SubAgentExecutor::is_retryable_error(
        "Request timed out after 30s"
    ));
    assert!(SubAgentExecutor::is_retryable_error(
        "TIMEOUT waiting for response"
    ));
}

#[test]
fn test_is_retryable_error_network_patterns() {
    assert!(SubAgentExecutor::is_retryable_error("Connection refused"));
    assert!(SubAgentExecutor::is_retryable_error(
        "Network error: unreachable"
    ));
    assert!(SubAgentExecutor::is_retryable_error(
        "Connection reset by peer"
    ));
}

#[test]
fn test_is_retryable_error_http_status_codes() {
    assert!(SubAgentExecutor::is_retryable_error(
        "HTTP 503 Service Unavailable"
    ));
    assert!(SubAgentExecutor::is_retryable_error(
        "Error 502 Bad Gateway"
    ));
    assert!(SubAgentExecutor::is_retryable_error(
        "429 Too Many Requests"
    ));
}

#[test]
fn test_is_retryable_error_rate_limit_patterns() {
    assert!(SubAgentExecutor::is_retryable_error("Rate limit exceeded"));
    assert!(SubAgentExecutor::is_retryable_error("rate_limit_error"));
    assert!(SubAgentExecutor::is_retryable_error(
        "Too many requests, try again"
    ));
}

#[test]
fn test_is_retryable_error_service_patterns() {
    assert!(SubAgentExecutor::is_retryable_error(
        "Service temporarily unavailable"
    ));
    assert!(SubAgentExecutor::is_retryable_error(
        "Temporary failure, retry later"
    ));
    assert!(SubAgentExecutor::is_retryable_error("Server is overloaded"));
    assert!(SubAgentExecutor::is_retryable_error(
        "Server busy, please retry"
    ));
}

#[test]
fn test_is_retryable_error_non_retryable_patterns() {
    assert!(!SubAgentExecutor::is_retryable_error(
        "Execution cancelled by user"
    ));
    assert!(!SubAgentExecutor::is_retryable_error("Permission denied"));
    assert!(!SubAgentExecutor::is_retryable_error("Resource not found"));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Invalid configuration"
    ));
    assert!(!SubAgentExecutor::is_retryable_error("Unauthorized access"));
    assert!(!SubAgentExecutor::is_retryable_error("Bad request format"));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Circuit breaker is open"
    ));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Validation failed for input"
    ));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Authentication required"
    ));
    assert!(!SubAgentExecutor::is_retryable_error("403 Forbidden"));
}

#[test]
fn test_is_retryable_error_non_retryable_takes_precedence() {
    assert!(!SubAgentExecutor::is_retryable_error(
        "Operation cancelled due to timeout validation failed"
    ));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Invalid request, do not retry"
    ));
}

#[test]
fn test_is_retryable_error_case_insensitive() {
    assert!(SubAgentExecutor::is_retryable_error("TIMEOUT"));
    assert!(SubAgentExecutor::is_retryable_error("TimeOut"));
    assert!(SubAgentExecutor::is_retryable_error("CONNECTION REFUSED"));
    assert!(!SubAgentExecutor::is_retryable_error("CANCELLED"));
    assert!(!SubAgentExecutor::is_retryable_error("Invalid"));
}

#[test]
fn test_is_retryable_error_unknown_errors() {
    assert!(!SubAgentExecutor::is_retryable_error(
        "Something went wrong"
    ));
    assert!(!SubAgentExecutor::is_retryable_error(
        "Unknown error occurred"
    ));
    assert!(!SubAgentExecutor::is_retryable_error(""));
}

#[test]
fn test_create_execution_record_with_parent_default_none() {
    use crate::models::sub_agent::SubAgentExecutionCreate;

    let create = SubAgentExecutionCreate::new(
        "wf_001".to_string(),
        "parent".to_string(),
        "child".to_string(),
        "name".to_string(),
        "prompt".to_string(),
    );
    assert!(create.parent_execution_id.is_none());
}

#[test]
fn test_create_execution_record_with_parent_some() {
    use crate::models::sub_agent::SubAgentExecutionCreate;

    let parent_id = "parent_exec_123".to_string();
    let create = SubAgentExecutionCreate::with_parent(
        "wf_001".to_string(),
        "parent".to_string(),
        "child".to_string(),
        "name".to_string(),
        "prompt".to_string(),
        Some(parent_id.clone()),
    );
    assert_eq!(create.parent_execution_id, Some(parent_id));
}

#[test]
fn test_correlation_id_serialization_with_parent() {
    use crate::models::sub_agent::SubAgentExecutionCreate;

    let create = SubAgentExecutionCreate::with_parent(
        "wf".to_string(),
        "parent".to_string(),
        "child".to_string(),
        "name".to_string(),
        "prompt".to_string(),
        Some("batch_123".to_string()),
    );

    let json = serde_json::to_string(&create).unwrap();
    assert!(json.contains("\"parent_execution_id\":\"batch_123\""));
}

#[test]
fn test_correlation_id_serialization_without_parent() {
    use crate::models::sub_agent::SubAgentExecutionCreate;

    let create = SubAgentExecutionCreate::new(
        "wf".to_string(),
        "parent".to_string(),
        "child".to_string(),
        "name".to_string(),
        "prompt".to_string(),
    );

    let json = serde_json::to_string(&create).unwrap();
    assert!(!json.contains("parent_execution_id"));
}
