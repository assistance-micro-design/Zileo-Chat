// Copyright 2025 Assistance Micro Design
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for Sub-Agent Tools (Phase F).
//!
//! These tests verify that SpawnAgentTool, DelegateTaskTool, and ParallelTasksTool
//! work correctly with ToolFactory, AgentToolContext, and validation.

use std::sync::Arc;
use tempfile::tempdir;
use tokio::sync::RwLock;

/// Helper to create test database path
fn create_test_db_path() -> (tempfile::TempDir, String) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("sub_agent_test_db");
    let path_str = db_path.to_str().unwrap().to_string();
    (temp_dir, path_str)
}

// ============================================================================
// ToolFactory Sub-Agent Tool Tests
// ============================================================================

#[cfg(test)]
mod tool_factory_sub_agent_tests {
    use super::*;

    #[test]
    fn test_tool_factory_requires_context_check() {
        use zileo_chat::tools::ToolFactory;

        // Sub-agent tools require context
        assert!(ToolFactory::requires_context("SpawnAgentTool"));
        assert!(ToolFactory::requires_context("DelegateTaskTool"));
        assert!(ToolFactory::requires_context("ParallelTasksTool"));

        // Basic tools do not require context
        assert!(!ToolFactory::requires_context("MemoryTool"));
        assert!(!ToolFactory::requires_context("TodoTool"));
    }

    #[test]
    fn test_tool_factory_sub_agent_tools_list() {
        use zileo_chat::tools::ToolFactory;

        let sub_agent_tools = ToolFactory::sub_agent_tools();

        assert!(sub_agent_tools.contains(&"SpawnAgentTool"));
        assert!(sub_agent_tools.contains(&"DelegateTaskTool"));
        assert!(sub_agent_tools.contains(&"ParallelTasksTool"));
        assert_eq!(sub_agent_tools.len(), 3);
    }

    #[test]
    fn test_tool_factory_basic_tools_list() {
        use zileo_chat::tools::ToolFactory;

        let basic_tools = ToolFactory::basic_tools();

        // Basic tools should include MemoryTool and TodoTool
        assert!(basic_tools.contains(&"MemoryTool"));
        assert!(basic_tools.contains(&"TodoTool"));

        // Basic tools should NOT include sub-agent tools
        assert!(!basic_tools.contains(&"SpawnAgentTool"));
        assert!(!basic_tools.contains(&"DelegateTaskTool"));
        assert!(!basic_tools.contains(&"ParallelTasksTool"));
    }

    #[test]
    fn test_tool_factory_is_valid_tool() {
        use zileo_chat::tools::ToolFactory;

        // All known tools should be valid
        assert!(ToolFactory::is_valid_tool("MemoryTool"));
        assert!(ToolFactory::is_valid_tool("TodoTool"));
        assert!(ToolFactory::is_valid_tool("SpawnAgentTool"));
        assert!(ToolFactory::is_valid_tool("DelegateTaskTool"));
        assert!(ToolFactory::is_valid_tool("ParallelTasksTool"));

        // Unknown tools should be invalid
        assert!(!ToolFactory::is_valid_tool("UnknownTool"));
        assert!(!ToolFactory::is_valid_tool(""));
        assert!(!ToolFactory::is_valid_tool("random_string"));
    }

    #[tokio::test]
    async fn test_tool_factory_create_tool_without_context_fails_for_sub_agent_tools() {
        use zileo_chat::db::DBClient;
        use zileo_chat::tools::ToolFactory;

        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let embedding_service = Arc::new(RwLock::new(None));
        let factory = ToolFactory::new(db, embedding_service);

        // Trying to create sub-agent tools without context should fail
        let result = factory
            .create_tool(
                "SpawnAgentTool",
                Some("wf_test".to_string()),
                "test_agent".to_string(),
                None,
            )
            .await;

        assert!(result.is_err());
        // Result is Err but we can't format it because dyn Tool doesn't impl Debug
        // Just verify it's an error

        let result = factory
            .create_tool(
                "DelegateTaskTool",
                Some("wf_test".to_string()),
                "test_agent".to_string(),
                None,
            )
            .await;

        assert!(result.is_err());
    }
}

// ============================================================================
// Sub-Agent Execution Types Tests
// ============================================================================

#[cfg(test)]
mod sub_agent_execution_tests {
    use zileo_chat::models::sub_agent::{
        constants::MAX_SUB_AGENTS, DelegateResult, ParallelBatchResult, ParallelTaskResult,
        SubAgentExecutionComplete, SubAgentExecutionCreate, SubAgentMetrics, SubAgentSpawnResult,
        SubAgentStatus,
    };

    #[test]
    fn test_max_sub_agents_constant() {
        assert_eq!(MAX_SUB_AGENTS, 15);
    }

    #[test]
    fn test_sub_agent_status_serialization() {
        let statuses = [
            SubAgentStatus::Pending,
            SubAgentStatus::Running,
            SubAgentStatus::Completed,
            SubAgentStatus::Error,
            SubAgentStatus::Cancelled,
        ];

        for status in &statuses {
            let json = serde_json::to_string(status).unwrap();
            let deserialized: SubAgentStatus = serde_json::from_str(&json).unwrap();
            assert_eq!(&deserialized.to_string(), &status.to_string());
        }
    }

    #[test]
    fn test_sub_agent_metrics_serialization() {
        let metrics = SubAgentMetrics {
            duration_ms: 1500,
            tokens_input: 100,
            tokens_output: 200,
        };

        let json = serde_json::to_string(&metrics).unwrap();
        assert!(json.contains("\"duration_ms\":1500"));
        assert!(json.contains("\"tokens_input\":100"));
        assert!(json.contains("\"tokens_output\":200"));

        let deserialized: SubAgentMetrics = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.duration_ms, 1500);
    }

    #[test]
    fn test_sub_agent_spawn_result() {
        let result = SubAgentSpawnResult {
            success: true,
            child_id: "sub_123".to_string(),
            report: "# Report\n\nTask completed successfully.".to_string(),
            metrics: SubAgentMetrics {
                duration_ms: 2000,
                tokens_input: 150,
                tokens_output: 300,
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"child_id\":\"sub_123\""));
        assert!(json.contains("Report"));
    }

    #[test]
    fn test_delegate_result() {
        let result = DelegateResult {
            success: true,
            agent_id: "db_agent".to_string(),
            report: "# Analysis Complete".to_string(),
            metrics: SubAgentMetrics {
                duration_ms: 3000,
                tokens_input: 200,
                tokens_output: 400,
            },
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"agent_id\":\"db_agent\""));
    }

    #[test]
    fn test_parallel_batch_result() {
        let result = ParallelBatchResult {
            success: true,
            completed: 2,
            failed: 1,
            results: vec![
                ParallelTaskResult {
                    agent_id: "agent_1".to_string(),
                    success: true,
                    report: Some("Report 1".to_string()),
                    error: None,
                    metrics: Some(SubAgentMetrics {
                        duration_ms: 1000,
                        tokens_input: 50,
                        tokens_output: 100,
                    }),
                },
                ParallelTaskResult {
                    agent_id: "agent_2".to_string(),
                    success: false,
                    report: None,
                    error: Some("Connection timeout".to_string()),
                    metrics: None,
                },
            ],
            aggregated_report: "# Parallel Report".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"completed\":2"));
        assert!(json.contains("\"failed\":1"));
        assert!(json.contains("Connection timeout"));
    }

    #[test]
    fn test_sub_agent_execution_create() {
        let create = SubAgentExecutionCreate::new(
            "wf_123".to_string(),
            "parent_agent".to_string(),
            "sub_456".to_string(),
            "Test Sub-Agent".to_string(),
            "Analyze the code for bugs".to_string(),
        );

        assert_eq!(create.workflow_id, "wf_123");
        assert_eq!(create.parent_agent_id, "parent_agent");
        assert_eq!(create.sub_agent_id, "sub_456");
        assert_eq!(create.sub_agent_name, "Test Sub-Agent");
        assert_eq!(create.status, "pending");
    }

    #[test]
    fn test_sub_agent_execution_complete_success() {
        let complete = SubAgentExecutionComplete::success(
            2500,
            Some(100),
            Some(200),
            "# Success Report".to_string(),
        );

        assert_eq!(complete.status, "completed");
        assert_eq!(complete.duration_ms, 2500);
        assert_eq!(complete.tokens_input, Some(100));
        assert_eq!(complete.tokens_output, Some(200));
        assert!(complete.result_summary.is_some());
        assert!(complete.error_message.is_none());
    }

    #[test]
    fn test_sub_agent_execution_complete_error() {
        let complete = SubAgentExecutionComplete::error(500, "Agent execution failed".to_string());

        assert_eq!(complete.status, "error");
        assert_eq!(complete.duration_ms, 500);
        assert!(complete.error_message.is_some());
        assert_eq!(
            complete.error_message.unwrap(),
            "Agent execution failed".to_string()
        );
    }
}

// ============================================================================
// Streaming Events Tests
// ============================================================================

#[cfg(test)]
mod streaming_events_tests {
    use zileo_chat::models::streaming::{events, ChunkType, StreamChunk, SubAgentStreamMetrics};

    #[test]
    fn test_stream_chunk_sub_agent_start() {
        let chunk = StreamChunk::sub_agent_start(
            "wf_123".to_string(),
            "sub_456".to_string(),
            "Analysis Agent".to_string(),
            "parent_789".to_string(),
            "Analyze the database schema".to_string(),
        );

        assert_eq!(chunk.workflow_id, "wf_123");
        assert_eq!(chunk.chunk_type, ChunkType::SubAgentStart);
        assert_eq!(chunk.sub_agent_id, Some("sub_456".to_string()));
        assert_eq!(chunk.sub_agent_name, Some("Analysis Agent".to_string()));
        assert_eq!(chunk.parent_agent_id, Some("parent_789".to_string()));
    }

    #[test]
    fn test_stream_chunk_sub_agent_complete() {
        let metrics = SubAgentStreamMetrics {
            duration_ms: 2000,
            tokens_input: 100,
            tokens_output: 200,
        };

        let chunk = StreamChunk::sub_agent_complete(
            "wf_123".to_string(),
            "sub_456".to_string(),
            "Analysis Agent".to_string(),
            "parent_789".to_string(),
            "# Report\n\nAnalysis completed.".to_string(),
            metrics,
        );

        assert_eq!(chunk.chunk_type, ChunkType::SubAgentComplete);
        assert!(chunk.content.is_some());
        assert!(chunk.metrics.is_some());

        let returned_metrics = chunk.metrics.unwrap();
        assert_eq!(returned_metrics.duration_ms, 2000);
    }

    #[test]
    fn test_stream_chunk_sub_agent_error() {
        let chunk = StreamChunk::sub_agent_error(
            "wf_123".to_string(),
            "sub_456".to_string(),
            "Analysis Agent".to_string(),
            "parent_789".to_string(),
            "Connection timeout".to_string(),
            500,
        );

        assert_eq!(chunk.chunk_type, ChunkType::SubAgentError);
        assert_eq!(chunk.content, Some("Connection timeout".to_string()));
        assert_eq!(chunk.duration, Some(500));
    }

    #[test]
    fn test_event_names() {
        assert_eq!(events::WORKFLOW_STREAM, "workflow_stream");
        assert_eq!(events::WORKFLOW_COMPLETE, "workflow_complete");
    }
}

// ============================================================================
// Validation Helper Tests
// ============================================================================

#[cfg(test)]
mod validation_helper_tests {
    use zileo_chat::models::streaming::SubAgentOperationType;
    use zileo_chat::models::validation::RiskLevel;
    use zileo_chat::tools::validation_helper::ValidationHelper;

    #[test]
    fn test_risk_level_for_spawn() {
        let risk = ValidationHelper::determine_risk_level(&SubAgentOperationType::Spawn);
        assert_eq!(risk, RiskLevel::Medium);
    }

    #[test]
    fn test_risk_level_for_delegate() {
        let risk = ValidationHelper::determine_risk_level(&SubAgentOperationType::Delegate);
        assert_eq!(risk, RiskLevel::Medium);
    }

    #[test]
    fn test_risk_level_for_parallel_batch() {
        let risk = ValidationHelper::determine_risk_level(&SubAgentOperationType::ParallelBatch);
        assert_eq!(risk, RiskLevel::High);
    }

    #[test]
    fn test_spawn_details_format() {
        let details = ValidationHelper::spawn_details(
            "CodeAnalyzer",
            "Analyze the code for bugs and security issues",
            &["MemoryTool".to_string(), "TodoTool".to_string()],
            &["serena".to_string()],
        );

        // Should contain sub-agent name
        assert!(details.get("sub_agent_name").is_some());
        assert_eq!(details["sub_agent_name"].as_str().unwrap(), "CodeAnalyzer");

        // Should contain tools
        let tools = details["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);

        // Should contain MCP servers
        let mcp = details["mcp_servers"].as_array().unwrap();
        assert_eq!(mcp.len(), 1);
    }

    #[test]
    fn test_delegate_details_format() {
        let details = ValidationHelper::delegate_details(
            "db_agent",
            "Database Agent",
            "Analyze the database schema for performance issues",
        );

        assert_eq!(details["target_agent_id"].as_str().unwrap(), "db_agent");
        assert_eq!(
            details["target_agent_name"].as_str().unwrap(),
            "Database Agent"
        );
        assert!(details.get("prompt_preview").is_some());
    }

    #[test]
    fn test_parallel_details_format() {
        let tasks = vec![
            ("agent_1".to_string(), "Task 1".to_string()),
            ("agent_2".to_string(), "Task 2".to_string()),
            ("agent_3".to_string(), "Task 3".to_string()),
        ];

        let details = ValidationHelper::parallel_details(&tasks);

        assert_eq!(details["task_count"].as_u64().unwrap(), 3);

        let task_details = details["tasks"].as_array().unwrap();
        assert_eq!(task_details.len(), 3);
    }

    #[test]
    fn test_spawn_details_long_prompt_truncation() {
        let long_prompt = "x".repeat(300);

        let details = ValidationHelper::spawn_details("LongPromptAgent", &long_prompt, &[], &[]);

        // Prompt preview should be truncated
        let preview = details["prompt_preview"].as_str().unwrap();
        assert!(preview.len() <= 203); // 200 + "..."

        // Full prompt length should be preserved
        assert_eq!(details["prompt_length"].as_u64().unwrap(), 300);
    }
}

// ============================================================================
// Context Tests
// ============================================================================

#[cfg(test)]
mod agent_tool_context_tests {
    use super::*;
    use zileo_chat::agents::core::{AgentOrchestrator, AgentRegistry};
    use zileo_chat::db::DBClient;
    use zileo_chat::llm::ProviderManager;
    use zileo_chat::tools::context::AgentToolContext;
    use zileo_chat::tools::ToolFactory;

    #[tokio::test]
    async fn test_agent_tool_context_creation() {
        let (_temp, db_path) = create_test_db_path();
        let db = Arc::new(DBClient::new(&db_path).await.expect("DB init failed"));
        db.initialize_schema().await.expect("Schema init failed");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager = Arc::new(ProviderManager::new());
        let embedding_service = Arc::new(RwLock::new(None));
        let tool_factory = Arc::new(ToolFactory::new(db.clone(), embedding_service));

        let context = AgentToolContext::new(
            registry.clone(),
            orchestrator.clone(),
            llm_manager.clone(),
            None, // No MCP manager
            tool_factory.clone(),
            None, // No app handle
            None, // No cancellation token
        );

        // Context should have all required components
        assert!(Arc::ptr_eq(&context.registry, &registry));
        assert!(Arc::ptr_eq(&context.orchestrator, &orchestrator));
        assert!(Arc::ptr_eq(&context.llm_manager, &llm_manager));
        assert!(Arc::ptr_eq(&context.tool_factory, &tool_factory));
        assert!(context.mcp_manager.is_none());
        assert!(context.app_handle.is_none());
    }
}
