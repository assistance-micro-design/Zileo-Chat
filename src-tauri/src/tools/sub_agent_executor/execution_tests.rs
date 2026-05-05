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
            cached_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
            cost_usd: None,
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
            cached_tokens: None,
            cache_write_tokens: None,
            thinking_tokens: None,
            cost_usd: None,
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

// H2 (audit 2026-05-02): SubAgentExecutor::with_parent_message threads the
// spawning agent's assistant message_id through to the records layer so
// `sub_agent_execution.parent_message_id` is set at CREATE time. Without
// it the legacy bulk UPDATE in persistence_step.rs (now removed) used to
// over-attribute nested chains all to the same primary message.

#[tokio::test]
async fn test_with_parent_message_some_sets_field() {
    use std::sync::Arc;

    use crate::agents::core::orchestrator::AgentOrchestrator;
    use crate::agents::core::AgentRegistry;

    let (state, _db_guard) = crate::test_utils::setup_test_state().await;
    let registry = Arc::new(AgentRegistry::new());
    let orchestrator = Arc::new(AgentOrchestrator::new(registry));

    let executor = SubAgentExecutor::with_cancellation(
        state.db.clone(),
        orchestrator,
        None,
        None,
        "wf_a2".to_string(),
        "primary_agent".to_string(),
        None,
    )
    .with_parent_message(Some("msg_primary_001".to_string()));

    assert_eq!(
        executor.parent_message_id,
        Some("msg_primary_001".to_string())
    );
}

#[tokio::test]
async fn test_with_parent_message_none_keeps_default() {
    use std::sync::Arc;

    use crate::agents::core::orchestrator::AgentOrchestrator;
    use crate::agents::core::AgentRegistry;

    let (state, _db_guard) = crate::test_utils::setup_test_state().await;
    let registry = Arc::new(AgentRegistry::new());
    let orchestrator = Arc::new(AgentOrchestrator::new(registry));

    let executor = SubAgentExecutor::with_cancellation(
        state.db.clone(),
        orchestrator,
        None,
        None,
        "wf_a2".to_string(),
        "primary_agent".to_string(),
        None,
    )
    .with_parent_message(None);

    assert!(executor.parent_message_id.is_none());
}

/// Cancellation regression: when the outer chain that invokes
/// `execute_with_heartbeat_timeout` is dropped (the real-world case is the
/// `tokio::select!` in `commands/streaming/orchestrator_bridge.rs` racing the
/// workflow's `CancellationToken`), the sub-agent's inner task must be
/// aborted too. Before the fix, the inner `tokio::spawn` was a detached task
/// that survived the drop and kept burning the LLM/HTTP call — visible to the
/// user as "cancel does not stop the sub-agent".
#[tokio::test]
async fn test_inner_task_aborted_when_outer_future_dropped() {
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use async_trait::async_trait;

    use crate::agents::core::agent::{
        Agent, Report, ReportMetrics, ReportStatus, Task as AgentTask,
    };
    use crate::agents::core::orchestrator::AgentOrchestrator;
    use crate::agents::core::AgentRegistry;
    use crate::mcp::MCPManager;
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};

    /// Test agent that ticks a shared counter every 10ms forever. If the
    /// agent's task is aborted, the counter stops growing; otherwise it
    /// keeps climbing. This is the discriminating signal — a one-shot
    /// `completed` flag protected by a long sleep cannot tell aborted
    /// apart from still-sleeping.
    struct TickingAgent {
        config: AgentConfig,
        ticks: Arc<AtomicU32>,
    }

    #[async_trait]
    impl Agent for TickingAgent {
        async fn execute(&self, _task: AgentTask) -> anyhow::Result<Report> {
            loop {
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.ticks.fetch_add(1, Ordering::SeqCst);
            }
        }

        async fn execute_with_mcp(
            &self,
            task: AgentTask,
            _mcp_manager: Option<Arc<MCPManager>>,
            _cancellation_token: Option<CancellationToken>,
        ) -> anyhow::Result<Report> {
            // We never return Ok here — the loop in execute() is infinite.
            // execute_with_mcp's signature requires Result<Report,_>, so
            // the unreachable arm uses an empty success report.
            self.execute(task).await?;
            Ok(Report {
                status: ReportStatus::Success,
                content: String::new(),
                response: String::new(),
                metrics: ReportMetrics::empty(0),
            })
        }

        fn capabilities(&self) -> Vec<String> {
            vec!["tick".to_string()]
        }
        fn lifecycle(&self) -> Lifecycle {
            self.config.lifecycle.clone()
        }
        fn tools(&self) -> Vec<String> {
            vec![]
        }
        fn mcp_servers(&self) -> Vec<String> {
            vec![]
        }
        fn system_prompt(&self) -> String {
            self.config.system_prompt.clone()
        }
        fn config(&self) -> &AgentConfig {
            &self.config
        }
    }

    let (state, _db_guard) = crate::test_utils::setup_test_state().await;

    let ticks = Arc::new(AtomicU32::new(0));

    let registry = Arc::new(AgentRegistry::new());
    let agent = Arc::new(TickingAgent {
        config: AgentConfig {
            id: "ticking".to_string(),
            name: "Ticking Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Test".to_string(),
                model: "test-model".to_string(),
                temperature: 0.7,
                max_tokens: 100,
                is_reasoning: false,
                context_window: None,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            folders: vec![],
            require_file_confirmation: true,
            system_prompt: "Test prompt".to_string(),
            max_tool_iterations: 50,
            reasoning_effort: None,
        },
        ticks: ticks.clone(),
    });
    registry.register("ticking".to_string(), agent).await;
    let orchestrator = Arc::new(AgentOrchestrator::new(registry));

    let token = CancellationToken::new();
    let executor = SubAgentExecutor::with_cancellation(
        state.db.clone(),
        orchestrator,
        None,
        None,
        "wf_cancel_drop".to_string(),
        "primary".to_string(),
        Some(token.clone()),
    );

    let task = AgentTask {
        id: "task_drop".to_string(),
        description: "tick forever".to_string(),
        context: serde_json::json!({}),
    };

    // Simulate the bridge `tokio::select!` that drops `execute_with_mcp`'s
    // future when cancellation fires. The sub-executor future is dropped
    // here without ever reaching its internal cancellation branch.
    {
        let exec_future = executor.execute_with_heartbeat_timeout("ticking", task, None);
        tokio::pin!(exec_future);

        tokio::select! {
            _ = &mut exec_future => panic!("ticking agent must not complete in this test"),
            _ = tokio::time::sleep(Duration::from_millis(150)) => {
                // Drop happens at end of select arm — exec_future drops here.
            }
        }
    }

    let ticks_at_drop = ticks.load(Ordering::SeqCst);
    assert!(
        ticks_at_drop > 0,
        "ticking agent must have run before the drop, otherwise the test \
         does not exercise the abort path"
    );

    // After the outer future is dropped, the inner spawned task MUST be
    // aborted. If it is, ticks will not climb any further. If it isn't,
    // ticks will keep growing every ~10ms.
    tokio::time::sleep(Duration::from_millis(300)).await;
    let ticks_after_wait = ticks.load(Ordering::SeqCst);

    assert_eq!(
        ticks_at_drop, ticks_after_wait,
        "inner sub-agent task kept ticking after the outer future was dropped \
         (drop={}, after wait={}). Cancellation does not propagate to the \
         spawned tokio task — drop the bridge select! and the sub-agent \
         survives.",
        ticks_at_drop, ticks_after_wait
    );
}

/// E2E: A→B chain — verifies that a sub-agent execution record is persisted
/// with `parent_message_id` set at CREATE time (replaces the legacy bulk
/// UPDATE in `persistence_step.rs`). The same wiring chains B→C correctly
/// because each spawning agent threads its own `parent_message_id` through
/// `with_parent_message` (defensive — currently single-level enforced).
#[tokio::test]
async fn test_create_execution_record_persists_parent_message_id() {
    use std::sync::Arc;

    use crate::agents::core::orchestrator::AgentOrchestrator;
    use crate::agents::core::AgentRegistry;

    let (state, _db_guard) = crate::test_utils::setup_test_state().await;
    let registry = Arc::new(AgentRegistry::new());
    let orchestrator = Arc::new(AgentOrchestrator::new(registry));

    let workflow_id = uuid::Uuid::new_v4().to_string();
    let primary_message_id = uuid::Uuid::new_v4().to_string();

    let executor = SubAgentExecutor::with_cancellation(
        state.db.clone(),
        orchestrator,
        None,
        None,
        workflow_id.clone(),
        "primary_agent".to_string(),
        None,
    )
    .with_parent_message(Some(primary_message_id.clone()));

    let execution_id = executor
        .create_execution_record("sub_agent_b", "AgentB", "do work")
        .await
        .expect("create_execution_record must succeed");

    let rows: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT meta::id(id) AS id, parent_message_id FROM sub_agent_execution:`{}`",
            execution_id
        ))
        .await
        .expect("query must succeed");

    let row = rows.first().expect("the created execution must exist");
    assert_eq!(
        row.get("parent_message_id").and_then(|v| v.as_str()),
        Some(primary_message_id.as_str()),
        "parent_message_id must be set at CREATE time (H2 audit 2026-05-02)"
    );
}
