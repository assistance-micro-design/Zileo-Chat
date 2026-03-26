use super::*;
use tempfile::tempdir;

async fn create_test_factory() -> ToolFactory {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");
    let db = Arc::new(
        DBClient::new(db_path.to_str().unwrap())
            .await
            .expect("Failed to create DB"),
    );
    db.initialize_schema().await.expect("Failed to init schema");
    let embedding_service = Arc::new(tokio::sync::RwLock::new(None));
    ToolFactory::new(db, embedding_service)
}

#[test]
fn test_available_tools() {
    let tools = ToolFactory::available_tools();
    assert!(tools.contains(&"MemoryTool"));
    assert!(tools.contains(&"TodoTool"));
    assert!(tools.contains(&"CalculatorTool"));
    assert!(tools.contains(&"UserQuestionTool"));
    assert!(tools.contains(&"ReadSkillTool"));
    assert!(tools.contains(&"FileManagerTool"));
    assert!(tools.contains(&"SpawnAgentTool"));
    assert!(tools.contains(&"DelegateTaskTool"));
    assert!(tools.contains(&"ParallelTasksTool"));
    assert_eq!(tools.len(), 9); // 5 basic + 1 hidden + 3 sub-agent
}

#[test]
fn test_basic_tools() {
    let tools = ToolFactory::basic_tools();
    assert!(tools.contains(&"MemoryTool"));
    assert!(tools.contains(&"TodoTool"));
    assert!(tools.contains(&"CalculatorTool"));
    assert!(tools.contains(&"UserQuestionTool"));
    assert!(!tools.contains(&"SpawnAgentTool"));
    assert_eq!(tools.len(), 5);
}

#[test]
fn test_sub_agent_tools() {
    let tools = ToolFactory::sub_agent_tools();
    assert!(tools.contains(&"SpawnAgentTool"));
    assert!(tools.contains(&"DelegateTaskTool"));
    assert!(tools.contains(&"ParallelTasksTool"));
    assert!(!tools.contains(&"MemoryTool"));
    assert_eq!(tools.len(), 3);
}

#[test]
fn test_is_valid_tool() {
    assert!(ToolFactory::is_valid_tool("MemoryTool"));
    assert!(ToolFactory::is_valid_tool("TodoTool"));
    assert!(ToolFactory::is_valid_tool("CalculatorTool"));
    assert!(ToolFactory::is_valid_tool("UserQuestionTool"));
    assert!(ToolFactory::is_valid_tool("ReadSkillTool"));
    assert!(ToolFactory::is_valid_tool("FileManagerTool"));
    assert!(ToolFactory::is_valid_tool("SpawnAgentTool"));
    assert!(!ToolFactory::is_valid_tool("InvalidTool"));
    assert!(!ToolFactory::is_valid_tool("memory_tool"));
}

#[test]
fn test_requires_context() {
    assert!(!ToolFactory::requires_context("MemoryTool"));
    assert!(!ToolFactory::requires_context("TodoTool"));
    assert!(!ToolFactory::requires_context("CalculatorTool"));
    assert!(!ToolFactory::requires_context("UserQuestionTool"));
    assert!(!ToolFactory::requires_context("FileManagerTool"));
    assert!(ToolFactory::requires_context("SpawnAgentTool"));
    assert!(ToolFactory::requires_context("DelegateTaskTool"));
    assert!(ToolFactory::requires_context("ParallelTasksTool"));
}

#[tokio::test]
async fn test_create_memory_tool() {
    let factory = create_test_factory().await;

    let result = factory
        .create_tool(
            "MemoryTool",
            Some("wf_test".to_string()),
            "test_agent".to_string(),
            None,
        )
        .await;

    assert!(result.is_ok());
    let tool = result.unwrap();
    assert_eq!(tool.definition().id, "MemoryTool");
}

#[tokio::test]
async fn test_create_todo_tool() {
    let factory = create_test_factory().await;

    let result = factory
        .create_tool(
            "TodoTool",
            Some("wf_test".to_string()),
            "test_agent".to_string(),
            None,
        )
        .await;

    assert!(result.is_ok());
    let tool = result.unwrap();
    assert_eq!(tool.definition().id, "TodoTool");
}

#[tokio::test]
async fn test_create_calculator_tool() {
    let factory = create_test_factory().await;

    let result = factory
        .create_tool(
            "CalculatorTool",
            None, // CalculatorTool doesn't need workflow_id
            "test_agent".to_string(),
            None,
        )
        .await;

    assert!(result.is_ok());
    let tool = result.unwrap();
    assert_eq!(tool.definition().id, "CalculatorTool");
}

#[tokio::test]
async fn test_create_read_skill_tool() {
    let factory = create_test_factory().await;

    let result = factory
        .create_tool("ReadSkillTool", None, "test_agent".to_string(), None)
        .await;

    assert!(result.is_ok());
    let tool = result.unwrap();
    assert_eq!(tool.definition().id, "ReadSkillTool");
}

#[tokio::test]
async fn test_create_unknown_tool() {
    let factory = create_test_factory().await;

    let result = factory
        .create_tool("UnknownTool", None, "test_agent".to_string(), None)
        .await;

    assert!(result.is_err());
    match result {
        Err(msg) => assert!(msg.contains("Unknown tool")),
        Ok(_) => panic!("Expected error for unknown tool"),
    }
}

#[tokio::test]
async fn test_create_tools_batch() {
    let factory = create_test_factory().await;

    let tool_names = vec![
        "MemoryTool".to_string(),
        "TodoTool".to_string(),
        "InvalidTool".to_string(), // Should be skipped
    ];

    let tools = factory
        .create_tools(
            &tool_names,
            Some("wf_batch".to_string()),
            "batch_agent".to_string(),
            None,
        )
        .await;

    // Should create 2 valid tools, skip 1 invalid
    assert_eq!(tools.len(), 2);
}

#[tokio::test]
async fn test_factory_without_embedding() {
    let factory = create_test_factory().await;

    // MemoryTool should still work without embedding service
    let result = factory
        .create_tool("MemoryTool", None, "test_agent".to_string(), None)
        .await;
    assert!(result.is_ok());
}
