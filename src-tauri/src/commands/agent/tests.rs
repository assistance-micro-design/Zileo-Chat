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

use crate::agents::SimpleAgent;
use crate::db::DBClient;
use crate::models::{AgentConfig, LLMConfig, Lifecycle};
use std::sync::Arc;

async fn setup_test_state() -> (crate::state::AppState, tempfile::TempDir) {
    crate::test_utils::setup_test_state().await
}

/// Creates a test AgentConfig with sensible defaults
fn test_agent_config(id: &str, name: &str) -> AgentConfig {
    AgentConfig {
        id: id.to_string(),
        name: name.to_string(),
        lifecycle: Lifecycle::Permanent,
        llm: LLMConfig {
            provider: "Demo".to_string(),
            model: "test".to_string(),
            temperature: 0.7,
            max_tokens: 1000,
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
    }
}

#[tokio::test]
async fn test_list_agents_empty() {
    let (state, _db_guard) = setup_test_state().await;
    let agents = state.registry.list().await;
    assert!(agents.is_empty(), "New registry should be empty");
}

#[tokio::test]
async fn test_list_agents_with_registered() {
    let (state, _db_guard) = setup_test_state().await;

    let config = test_agent_config("test_agent", "Test Agent");
    let agent = SimpleAgent::new(config);
    state
        .registry
        .register("test_agent".to_string(), Arc::new(agent))
        .await;

    let agents = state.registry.list().await;
    assert_eq!(agents.len(), 1);
    assert!(agents.contains(&"test_agent".to_string()));
}

#[tokio::test]
async fn test_get_agent_config_success() {
    let (state, _db_guard) = setup_test_state().await;

    let config = AgentConfig {
        id: "config_test".to_string(),
        name: "Config Test Agent".to_string(),
        lifecycle: Lifecycle::Temporary,
        llm: LLMConfig {
            provider: "Mistral".to_string(),
            model: "mistral-large".to_string(),
            temperature: 0.5,
            max_tokens: 2000,
            is_reasoning: false,
            context_window: None,
        },
        tools: vec!["tool_a".to_string(), "tool_b".to_string()],
        mcp_servers: vec!["serena".to_string()],
        skills: vec![],
        folders: vec![],
        require_file_confirmation: true,
        system_prompt: "You are a test agent".to_string(),
        max_tool_iterations: 50,
        reasoning_effort: None,
    };

    let agent = SimpleAgent::new(config);
    state
        .registry
        .register("config_test".to_string(), Arc::new(agent))
        .await;

    let retrieved_agent = state.registry.get("config_test").await;
    assert!(retrieved_agent.is_some());

    let retrieved_config = retrieved_agent.unwrap().config().clone();
    assert_eq!(retrieved_config.id, "config_test");
    assert_eq!(retrieved_config.name, "Config Test Agent");
    assert_eq!(retrieved_config.llm.provider, "Mistral");
    assert_eq!(retrieved_config.tools.len(), 2);
}

#[tokio::test]
async fn test_get_agent_config_not_found() {
    let (state, _db_guard) = setup_test_state().await;

    let result = state.registry.get("nonexistent").await;
    assert!(result.is_none(), "Should not find nonexistent agent");
}

#[tokio::test]
async fn test_agent_config_serialization() {
    let config = test_agent_config("serial_test", "Serialization Test");

    let json = serde_json::to_string(&config);
    assert!(json.is_ok(), "AgentConfig should serialize to JSON");

    let json_str = json.unwrap();
    assert!(json_str.contains("\"serial_test\""));
    assert!(json_str.contains("\"permanent\""));
}

#[tokio::test]
async fn test_lifecycle_serialization() {
    assert_eq!(
        serde_json::to_string(&Lifecycle::Permanent).unwrap(),
        "\"permanent\""
    );
    assert_eq!(
        serde_json::to_string(&Lifecycle::Temporary).unwrap(),
        "\"temporary\""
    );
}

#[tokio::test]
async fn test_multiple_agents_listing() {
    let (state, _db_guard) = setup_test_state().await;

    for i in 0..5 {
        let config = AgentConfig {
            lifecycle: Lifecycle::Temporary,
            system_prompt: format!("Agent {} prompt", i),
            ..test_agent_config(&format!("agent_{}", i), &format!("Agent {}", i))
        };

        let agent = SimpleAgent::new(config);
        state
            .registry
            .register(format!("agent_{}", i), Arc::new(agent))
            .await;
    }

    let agents = state.registry.list().await;
    assert_eq!(agents.len(), 5);
}

/// Seeds an agent with a given name in the database, returns its UUID.
/// Note: omit created_at/updated_at - schema defaults to time::now() (ERR_SURREAL_007).
async fn seed_agent_in_db(db: &DBClient, name: &str) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE agent:`{}` SET \
            id = '{}', \
            name = $name, \
            lifecycle = 'permanent', \
            llm = {{ provider: 'mistral', model: 'large', temperature: 0.7, max_tokens: 1000 }}, \
            tools = [], \
            mcp_servers = [], \
            system_prompt = 'Test agent.', \
            max_tool_iterations = 50, \
            reasoning_effort = NONE, \
            created_at = time::now(), \
            updated_at = time::now()",
        id, id
    );
    let response = db
        .db
        .query(&query)
        .bind(("name", name.to_string()))
        .await
        .expect("Query execution failed");
    response.check().expect("CREATE agent failed validation");
    id
}

#[tokio::test]
async fn test_create_agent_rejects_duplicate_name() {
    let (state, _db_guard) = setup_test_state().await;
    seed_agent_in_db(&state.db, "Database Agent").await;

    let result = super::check_agent_name_unique(&state.db, "database agent", None).await;
    assert!(
        result.is_err(),
        "Should reject duplicate name (case-insensitive)"
    );
    let err = result.unwrap_err();
    assert!(
        err.contains("already exists"),
        "Error should mention 'already exists', got: {}",
        err
    );
}

#[tokio::test]
async fn test_update_agent_allows_keeping_own_name() {
    let (state, _db_guard) = setup_test_state().await;
    let agent_id = seed_agent_in_db(&state.db, "My Agent").await;

    let result = super::check_agent_name_unique(&state.db, "My Agent", Some(&agent_id)).await;
    assert!(
        result.is_ok(),
        "Should allow keeping own name, got: {:?}",
        result
    );
}

#[tokio::test]
async fn test_update_agent_rejects_collision_with_other() {
    let (state, _db_guard) = setup_test_state().await;
    let _agent_a = seed_agent_in_db(&state.db, "Agent Alpha").await;
    let agent_b = seed_agent_in_db(&state.db, "Agent Beta").await;

    let result = super::check_agent_name_unique(&state.db, "Agent Alpha", Some(&agent_b)).await;
    assert!(
        result.is_err(),
        "Should reject renaming to existing agent's name"
    );
}
