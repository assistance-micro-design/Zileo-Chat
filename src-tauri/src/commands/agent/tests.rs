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
        kind: None,
        auto_analyze_reports: false,
        mcp_tool_allowlist: Vec::new(),
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
        kind: None,
        auto_analyze_reports: false,
        mcp_tool_allowlist: Vec::new(),
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

/// Seeds an `llm_model` row with the given provider/api_name and reasoning/context flags.
async fn seed_llm_model(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    is_reasoning: bool,
    context_window: u64,
) {
    seed_llm_model_full(
        db,
        provider,
        api_name,
        is_reasoning,
        context_window,
        0.7,
        4096,
    )
    .await;
}

/// Seeds an `llm_model` row with full control over the model defaults so
/// hydrate tests can assert each field independently.
async fn seed_llm_model_full(
    db: &DBClient,
    provider: &str,
    api_name: &str,
    is_reasoning: bool,
    context_window: u64,
    temperature_default: f64,
    max_output_tokens: u64,
) {
    let model_id = uuid::Uuid::new_v4().to_string();
    let data = serde_json::json!({
        "id": model_id,
        "provider": provider,
        "name": api_name,
        "api_name": api_name,
        "context_window": context_window,
        "max_output_tokens": max_output_tokens,
        "temperature_default": temperature_default,
        "is_builtin": false,
        "is_reasoning": is_reasoning,
        "input_price_per_mtok": 0.0,
        "output_price_per_mtok": 0.0,
        "cache_read_price_per_mtok": 0.0,
        "cache_write_price_per_mtok": 0.0,
    });
    db.execute_with_params(
        &format!(
            "CREATE llm_model:`{}` CONTENT $data ; \
             UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
            model_id, model_id
        ),
        vec![("data".to_string(), data)],
    )
    .await
    .expect("Failed to seed llm_model");
}

fn stale_llm_config(provider: &str, model: &str) -> LLMConfig {
    LLMConfig {
        provider: provider.to_string(),
        model: model.to_string(),
        temperature: 0.7,
        max_tokens: 1000,
        is_reasoning: false,
        context_window: None,
    }
}

#[tokio::test]
async fn test_hydrate_llm_from_model_overrides_stale_snapshot() {
    let (state, _db_guard) = setup_test_state().await;
    seed_llm_model_full(
        &state.db,
        "mistral",
        "magistral-medium",
        true,
        128_000,
        0.3,
        8192,
    )
    .await;

    let mut llm = stale_llm_config("Mistral", "magistral-medium");
    super::hydrate_llm_from_model(&state.db, &mut llm)
        .await
        .expect("hydrate should succeed");

    assert!(
        llm.is_reasoning,
        "is_reasoning should be overridden to true"
    );
    assert_eq!(
        llm.context_window,
        Some(128_000),
        "context_window should be overridden from DB"
    );
    // The frontend AgentForm copies these four fields straight from the
    // selected model and offers no per-agent override, so the snapshot is
    // a derived copy. Hydrate must keep them in sync with the model card.
    assert!(
        (llm.temperature - 0.3).abs() < 1e-9,
        "temperature should follow the model's temperature_default"
    );
    assert_eq!(
        llm.max_tokens, 8192,
        "max_tokens should follow the model's max_output_tokens"
    );
}

/// Regression: an existing agent whose model later raised its
/// `temperature_default` / `max_output_tokens` must inherit the new values
/// at next hydrate (typically at app startup), without the user having to
/// re-edit the agent.
#[tokio::test]
async fn test_hydrate_llm_from_model_overrides_temperature_and_max_tokens() {
    let (state, _db_guard) = setup_test_state().await;
    seed_llm_model_full(
        &state.db,
        "routerlab",
        "deepseek-v4-pro",
        true,
        1_000_000,
        0.1,
        65_536,
    )
    .await;

    // Agent saved when the model still defaulted to 0.7 / 4096.
    let mut llm = LLMConfig {
        provider: "routerlab".to_string(),
        model: "deepseek-v4-pro".to_string(),
        temperature: 0.7,
        max_tokens: 4096,
        is_reasoning: false,
        context_window: Some(32_000),
    };
    super::hydrate_llm_from_model(&state.db, &mut llm)
        .await
        .expect("hydrate should succeed");

    assert!((llm.temperature - 0.1).abs() < 1e-9);
    assert_eq!(llm.max_tokens, 65_536);
    assert!(llm.is_reasoning);
    assert_eq!(llm.context_window, Some(1_000_000));
}

#[tokio::test]
async fn test_hydrate_llm_from_model_no_match_keeps_snapshot() {
    let (state, _db_guard) = setup_test_state().await;
    seed_llm_model(&state.db, "mistral", "some-other-model", true, 64_000).await;

    let mut llm = stale_llm_config("mistral", "unknown-ad-hoc-model");
    let snapshot_before = llm.clone();
    super::hydrate_llm_from_model(&state.db, &mut llm)
        .await
        .expect("hydrate should succeed even without a match");

    assert_eq!(llm.is_reasoning, snapshot_before.is_reasoning);
    assert_eq!(llm.context_window, snapshot_before.context_window);
}

#[tokio::test]
async fn test_hydrate_llm_from_model_empty_api_name_is_noop() {
    let (state, _db_guard) = setup_test_state().await;

    let mut llm = stale_llm_config("mistral", "   ");
    super::hydrate_llm_from_model(&state.db, &mut llm)
        .await
        .expect("empty api_name must not error");

    assert!(!llm.is_reasoning);
    assert!(llm.context_window.is_none());
}

#[tokio::test]
async fn test_hydrate_llm_from_model_provider_match_is_case_insensitive() {
    let (state, _db_guard) = setup_test_state().await;
    seed_llm_model(&state.db, "ollama", "qwen3-thinking", true, 32_768).await;

    let mut llm = stale_llm_config("OLLAMA", "qwen3-thinking");
    super::hydrate_llm_from_model(&state.db, &mut llm)
        .await
        .expect("hydrate should succeed");

    assert!(
        llm.is_reasoning,
        "provider lookup must lowercase before matching"
    );
    assert_eq!(llm.context_window, Some(32_768));
}

/// Seeds an agent row whose `mcp_tool_allowlist` is bound as JSON exactly the
/// way `create_agent` (commands/agent/mod.rs) binds `$mcp_tool_allowlist`. This
/// exercises the real SCHEMAFULL persistence path so a dropped sub-key
/// would surface here.
async fn seed_agent_with_allowlist(
    db: &DBClient,
    allowlist: &[crate::models::agent::McpToolAllowlistEntry],
) -> String {
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        "CREATE agent:`{id}` CONTENT {{ \
            id: '{id}', name: $name, lifecycle: 'permanent', \
            llm: {{ provider: 'mistral', model: 'large', temperature: 0.7, max_tokens: 1000 }}, \
            tools: [], mcp_servers: [], skills: [], folders: [], \
            require_file_confirmation: false, system_prompt: 'p', \
            max_tool_iterations: 50, reasoning_effort: NONE, kind: NONE, \
            auto_analyze_reports: false, mcp_tool_allowlist: $mcp_tool_allowlist, \
            created_at: time::now(), updated_at: time::now() \
        }}"
    );
    db.execute_with_params(
        &query,
        vec![
            ("name".to_string(), serde_json::json!(format!("Agent {id}"))),
            (
                "mcp_tool_allowlist".to_string(),
                serde_json::json!(allowlist),
            ),
        ],
    )
    .await
    .expect("seed agent with allowlist");
    id
}

/// Reads `mcp_tool_allowlist` back from the DB and deserializes the nested
/// array<object> into the typed entries (the canonical reload shape).
async fn load_allowlist(
    db: &DBClient,
    id: &str,
) -> Vec<crate::models::agent::McpToolAllowlistEntry> {
    let rows = db
        .query_json(&format!("SELECT mcp_tool_allowlist FROM agent:`{id}`"))
        .await
        .expect("select allowlist");
    let val = rows
        .into_iter()
        .next()
        .expect("agent row exists")
        .get("mcp_tool_allowlist")
        .cloned()
        .expect("mcp_tool_allowlist field present");
    serde_json::from_value(val).expect("allowlist deserializes into typed entries")
}

/// Persistence: the per-agent `mcp_tool_allowlist`
/// (`array<object>` with `server_id` + `tools` sub-fields) must survive a live
/// SCHEMAFULL write -> read round-trip WITHOUT dropping its sub-keys,
/// and an update must replace it coherently.
#[tokio::test]
async fn mcp_tool_allowlist_survives_db_round_trip_and_update() {
    use crate::models::agent::McpToolAllowlistEntry;
    let (state, _db_guard) = setup_test_state().await;

    let initial = vec![
        McpToolAllowlistEntry {
            server_id: "srv-a".to_string(),
            tools: vec!["read".to_string(), "list".to_string()],
            // Explicitly armed for delegated runs.
            allow_in_delegated_runs: true,
        },
        McpToolAllowlistEntry {
            server_id: "srv-b".to_string(),
            tools: vec!["exec".to_string()],
            allow_in_delegated_runs: false,
        },
    ];
    let id = seed_agent_with_allowlist(&state.db, &initial).await;

    let back = load_allowlist(&state.db, &id).await;
    assert_eq!(
        back.len(),
        2,
        "no allowlist entry may be dropped on a SCHEMAFULL round-trip"
    );
    assert_eq!(back[0].server_id, "srv-a");
    assert_eq!(
        back[0].tools,
        vec!["read", "list"],
        "the `tools` sub-field must not be dropped"
    );
    assert!(
        back[0].allow_in_delegated_runs,
        "the `allow_in_delegated_runs` sub-field must survive the round-trip"
    );
    assert_eq!(back[1].server_id, "srv-b");
    assert_eq!(back[1].tools, vec!["exec"]);
    assert!(
        !back[1].allow_in_delegated_runs,
        "the strict (false) flag must round-trip as false, not silently flip"
    );

    // Update the allowlist -> reload -> coherent (full replace).
    let updated = vec![McpToolAllowlistEntry {
        server_id: "srv-c".to_string(),
        tools: vec!["write".to_string()],
        allow_in_delegated_runs: false,
    }];
    state
        .db
        .execute_with_params(
            &format!("UPDATE agent:`{id}` SET mcp_tool_allowlist = $a"),
            vec![("a".to_string(), serde_json::json!(updated))],
        )
        .await
        .expect("update allowlist");

    let after = load_allowlist(&state.db, &id).await;
    assert_eq!(
        after.len(),
        1,
        "update must replace the allowlist wholesale"
    );
    assert_eq!(after[0].server_id, "srv-c");
    assert_eq!(after[0].tools, vec!["write"]);
}

/// Backward-compat: an allowlist entry persisted BEFORE the
/// `allow_in_delegated_runs` field existed (the sub-key is simply absent in the
/// stored object) must reload as `false` (strict). The serde default is the
/// safety net — `DEFINE FIELD ... DEFAULT false` does not backfill existing
/// rows, so a legacy entry's object has no such key.
#[tokio::test]
async fn legacy_allowlist_entry_without_delegation_flag_reloads_as_strict() {
    let (state, _db_guard) = setup_test_state().await;
    let id = uuid::Uuid::new_v4().to_string();
    // Seed an entry object that OMITS `allow_in_delegated_runs` entirely.
    let query = format!(
        "CREATE agent:`{id}` CONTENT {{ \
            id: '{id}', name: 'Legacy', lifecycle: 'permanent', \
            llm: {{ provider: 'mistral', model: 'large', temperature: 0.7, max_tokens: 1000 }}, \
            tools: [], mcp_servers: [], skills: [], folders: [], \
            require_file_confirmation: false, system_prompt: 'p', \
            max_tool_iterations: 50, reasoning_effort: NONE, kind: NONE, \
            auto_analyze_reports: false, mcp_tool_allowlist: $a, \
            created_at: time::now(), updated_at: time::now() \
        }}"
    );
    state
        .db
        .execute_with_params(
            &query,
            vec![(
                "a".to_string(),
                serde_json::json!([{ "server_id": "srv-legacy", "tools": ["read"] }]),
            )],
        )
        .await
        .expect("seed legacy agent");

    let back = load_allowlist(&state.db, &id).await;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].server_id, "srv-legacy");
    assert_eq!(back[0].tools, vec!["read"]);
    assert!(
        !back[0].allow_in_delegated_runs,
        "a legacy entry missing the field must reload as strict (false), not arm delegation"
    );
}

/// REGRESSION (startup agent load): a legacy agent row written BEFORE the
/// `mcp_tool_allowlist` column existed returns the field as explicit `null` on
/// a SCHEMAFULL SELECT (the `DEFAULT []` does not backfill).
/// `#[serde(default)]` alone does NOT intercept an explicit null,
/// so `serde_json::from_value::<AgentConfig>` (main.rs:637)
/// fails and the agent is DROPPED on startup → the entire agent list vanishes
/// from the UI. The null-tolerant deserializer must map null → empty Vec.
#[test]
fn legacy_agent_row_with_null_allowlist_still_loads() {
    let mut row = serde_json::to_value(test_agent_config("a1", "Legacy")).unwrap();
    row["mcp_tool_allowlist"] = serde_json::Value::Null;
    let config: AgentConfig = serde_json::from_value(row)
        .expect("a legacy row with null mcp_tool_allowlist must load with an empty allowlist");
    assert!(config.mcp_tool_allowlist.is_empty());
}

/// Same regression, ABSENT form: an even older row may omit the key entirely.
/// `#[serde(default)]` covers this, but lock it so a future attribute change
/// cannot silently reintroduce the drop.
#[test]
fn legacy_agent_row_with_absent_allowlist_still_loads() {
    let mut row = serde_json::to_value(test_agent_config("a2", "LegacyAbsent")).unwrap();
    row.as_object_mut()
        .expect("row is an object")
        .remove("mcp_tool_allowlist");
    let config: AgentConfig = serde_json::from_value(row)
        .expect("a legacy row without mcp_tool_allowlist must load with an empty allowlist");
    assert!(config.mcp_tool_allowlist.is_empty());
}
