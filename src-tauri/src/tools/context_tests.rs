use super::*;
use crate::db::DBClient;
use crate::state::AppState;
use crate::test_utils::test_tempdir;
use crate::tools::ToolFactory;
use std::sync::Arc;

async fn create_test_state() -> AppState {
    let temp_dir = test_tempdir();
    let db_path = temp_dir.path().join("test_context_db");
    AppState::new(db_path.to_str().unwrap())
        .await
        .expect("Failed to create AppState")
}

#[tokio::test]
async fn test_context_from_app_state() {
    let state = create_test_state().await;
    let context = AgentToolContext::from_app_state_full(&state);

    // Verify all fields are populated
    assert!(context.mcp_manager.is_some());
    // Registry should be the same instance
    assert!(Arc::ptr_eq(&context.registry, &state.registry));
    assert!(Arc::ptr_eq(&context.orchestrator, &state.orchestrator));
    assert!(Arc::ptr_eq(&context.llm_manager, &state.llm_manager));
    assert!(Arc::ptr_eq(&context.tool_factory, &state.tool_factory));
}

#[tokio::test]
async fn test_context_clone() {
    let state = create_test_state().await;
    let context1 = AgentToolContext::from_app_state_full(&state);
    let context2 = context1.clone();

    // Cloned context should share the same Arc instances
    assert!(Arc::ptr_eq(&context1.registry, &context2.registry));
    assert!(Arc::ptr_eq(&context1.orchestrator, &context2.orchestrator));
    assert!(Arc::ptr_eq(&context1.llm_manager, &context2.llm_manager));
}

#[tokio::test]
async fn test_context_without_mcp() {
    let state = create_test_state().await;
    let context = AgentToolContext::from_app_state(&state, None, None);

    // When None is passed, it should still get MCP from state
    assert!(context.mcp_manager.is_some());
    // app_handle should be None
    assert!(context.app_handle.is_none());
}

#[tokio::test]
async fn test_context_new() {
    let temp_dir = test_tempdir();
    let db_path = temp_dir.path().join("test_context_new_db");
    let db = Arc::new(
        DBClient::new(db_path.to_str().unwrap())
            .await
            .expect("Failed to create DB"),
    );
    db.initialize_schema().await.expect("Failed to init schema");

    let registry = Arc::new(AgentRegistry::new());
    let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
    let llm_manager = Arc::new(ProviderManager::new().expect("test provider manager"));
    let embedding_service = Arc::new(tokio::sync::RwLock::new(None));
    let tool_factory = Arc::new(ToolFactory::new(db.clone(), embedding_service));

    let context = AgentToolContext::new(
        registry.clone(),
        orchestrator.clone(),
        llm_manager.clone(),
        None,
        tool_factory.clone(),
        None,
        None, // cancellation_token
    );

    assert!(Arc::ptr_eq(&context.registry, &registry));
    assert!(Arc::ptr_eq(&context.orchestrator, &orchestrator));
    assert!(Arc::ptr_eq(&context.llm_manager, &llm_manager));
    assert!(context.mcp_manager.is_none());
    assert!(Arc::ptr_eq(&context.tool_factory, &tool_factory));
    assert!(context.app_handle.is_none());
    assert!(context.cancellation_token.is_none());
}

#[tokio::test]
async fn test_context_without_cancellation_token() {
    let state = create_test_state().await;

    // from_app_state_full does not include cancellation token
    let context = AgentToolContext::from_app_state_full(&state);
    assert!(context.cancellation_token.is_none());

    // from_app_state does not include cancellation token
    let context2 = AgentToolContext::from_app_state(&state, None, None);
    assert!(context2.cancellation_token.is_none());
}
