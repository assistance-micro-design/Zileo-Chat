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

//! Agent CRUD Tauri commands
//!
//! Provides IPC commands for managing agent configurations with persistence.
//!
//! ## Commands
//!
//! - [`list_agents`] - List all agents (returns AgentSummary[])
//! - [`get_agent_config`] - Get full agent configuration by ID
//! - [`create_agent`] - Create a new agent
//! - [`update_agent`] - Update an existing agent
//! - [`delete_agent`] - Delete an agent

use crate::agents::LLMAgent;
use crate::constants::commands as cmd_const;
use crate::llm::ProviderType;
use crate::models::{
    AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, LLMConfig, Lifecycle,
};
use crate::security::{serialize_for_query, Validator};
use crate::state::AppState;
use crate::tools::context::AgentToolContext;
use crate::tools::registry::TOOL_REGISTRY;
use crate::tools::validation_helper::validate_trimmed_name;
use std::sync::Arc;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Delegates to centralized validate_trimmed_name
fn validate_agent_name(name: &str) -> Result<String, String> {
    validate_trimmed_name(name, "Agent name", cmd_const::MAX_AGENT_NAME_LEN)
}

/// Validates system prompt
fn validate_system_prompt(prompt: &str) -> Result<String, String> {
    let trimmed = prompt.trim();

    if trimmed.is_empty() {
        return Err("System prompt cannot be empty".to_string());
    }

    if trimmed.len() > cmd_const::MAX_SYSTEM_PROMPT_LEN {
        return Err(format!(
            "System prompt exceeds maximum length of {} characters",
            cmd_const::MAX_SYSTEM_PROMPT_LEN
        ));
    }

    Ok(trimmed.to_string())
}

/// Validates LLM configuration
fn validate_llm_config(llm: &LLMConfig) -> Result<LLMConfig, String> {
    // Validate provider (supports builtin + custom providers)
    llm.provider
        .parse::<ProviderType>()
        .map_err(|_| format!("Invalid provider '{}'", llm.provider))?;

    // Validate model name
    let model = llm.model.trim();
    if model.is_empty() {
        return Err("Model name cannot be empty".to_string());
    }
    if model.len() > 128 {
        return Err("Model name exceeds maximum length of 128 characters".to_string());
    }

    // Validate temperature
    if llm.temperature < cmd_const::MIN_TEMPERATURE || llm.temperature > cmd_const::MAX_TEMPERATURE
    {
        return Err(format!(
            "Temperature must be between {} and {}",
            cmd_const::MIN_TEMPERATURE,
            cmd_const::MAX_TEMPERATURE
        ));
    }

    // Validate max_tokens
    if llm.max_tokens < cmd_const::MIN_MAX_TOKENS || llm.max_tokens > cmd_const::MAX_MAX_TOKENS {
        return Err(format!(
            "max_tokens must be between {} and {}",
            cmd_const::MIN_MAX_TOKENS,
            cmd_const::MAX_MAX_TOKENS
        ));
    }

    Ok(LLMConfig {
        provider: llm.provider.clone(),
        model: model.to_string(),
        temperature: llm.temperature,
        max_tokens: llm.max_tokens,
        is_reasoning: llm.is_reasoning,
    })
}

/// Validates tools list
fn validate_tools(tools: &[String]) -> Result<Vec<String>, String> {
    let mut validated = Vec::new();

    for tool in tools {
        let trimmed = tool.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !TOOL_REGISTRY.has_tool(trimmed) {
            return Err(format!(
                "Unknown tool '{}'. Available tools: {:?}",
                trimmed,
                TOOL_REGISTRY.available_tools()
            ));
        }

        validated.push(trimmed.to_string());
    }

    Ok(validated)
}

/// Validates skill names list
///
/// Skill names must match `[a-zA-Z0-9_-]+` (same as skill model validation).
fn validate_skills(skills: &[String]) -> Result<Vec<String>, String> {
    let mut validated = Vec::new();

    for skill in skills {
        let trimmed = skill.trim();
        if trimmed.is_empty() {
            continue;
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!(
                "Invalid skill name '{}'. Only alphanumeric, underscore, and hyphen allowed",
                trimmed
            ));
        }

        if trimmed.len() > 128 {
            return Err(format!(
                "Skill name '{}' exceeds maximum length of 128 characters",
                trimmed
            ));
        }

        validated.push(trimmed.to_string());
    }

    Ok(validated)
}

/// Validates MCP servers list
fn validate_mcp_servers(servers: &[String]) -> Result<Vec<String>, String> {
    let mut validated = Vec::new();

    for server in servers {
        let trimmed = server.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Basic validation - alphanumeric, underscore, hyphen
        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(format!(
                "Invalid MCP server name '{}'. Only alphanumeric, underscore, and hyphen allowed",
                trimmed
            ));
        }

        validated.push(trimmed.to_string());
    }

    Ok(validated)
}

/// Validates full agent creation config
fn validate_agent_create(config: &AgentConfigCreate) -> Result<AgentConfigCreate, String> {
    Ok(AgentConfigCreate {
        name: validate_agent_name(&config.name)?,
        lifecycle: config.lifecycle.clone(),
        llm: validate_llm_config(&config.llm)?,
        tools: validate_tools(&config.tools)?,
        mcp_servers: validate_mcp_servers(&config.mcp_servers)?,
        skills: validate_skills(&config.skills)?,
        system_prompt: validate_system_prompt(&config.system_prompt)?,
        max_tool_iterations: config.max_tool_iterations.clamp(1, 200),
        enable_thinking: config.enable_thinking,
    })
}

// ============================================================================
// Database Serialization Helpers
// ============================================================================

/// Serialized agent configuration fields for database operations
struct SerializedAgentFields {
    name_json: String,
    llm_json: String,
    tools_json: String,
    mcp_json: String,
    skills_json: String,
    prompt_json: String,
}

/// Serializes agent configuration fields for database storage
fn serialize_agent_fields(config: &AgentConfig) -> Result<SerializedAgentFields, String> {
    let name_json = serialize_for_query(&config.name, "name")?;
    let llm_json = serialize_for_query(&config.llm, "LLM config")?;
    let tools_json = serialize_for_query(&config.tools, "tools")?;
    let mcp_json = serialize_for_query(&config.mcp_servers, "MCP servers")?;
    let skills_json = serialize_for_query(&config.skills, "skills")?;
    let prompt_json = serialize_for_query(&config.system_prompt, "system prompt")?;

    Ok(SerializedAgentFields {
        name_json,
        llm_json,
        tools_json,
        mcp_json,
        skills_json,
        prompt_json,
    })
}

/// Registers an LLMAgent in the registry with proper context
async fn register_agent_runtime(state: &AppState, agent_id: &str, config: AgentConfig) {
    let agent_context = AgentToolContext::from_app_state_full(state);
    let llm_agent = LLMAgent::with_context(
        config,
        state.llm_manager.clone(),
        state.tool_factory.clone(),
        agent_context,
    );
    state
        .registry
        .register(agent_id.to_string(), Arc::new(llm_agent))
        .await;
}

/// Lists all agents with summary information
#[tauri::command]
#[instrument(name = "list_agents", skip(state))]
pub async fn list_agents(state: State<'_, AppState>) -> Result<Vec<AgentSummary>, String> {
    info!("Listing agents");

    let agent_ids = state.registry.list().await;
    let mut summaries = Vec::with_capacity(agent_ids.len());

    for id in agent_ids {
        if let Some(agent) = state.registry.get(&id).await {
            summaries.push(AgentSummary::from(agent.config()));
        }
    }

    info!(count = summaries.len(), "Agents listed");
    Ok(summaries)
}

/// Gets agent configuration by ID
#[tauri::command]
#[instrument(name = "get_agent_config", skip(state), fields(agent_id = %agent_id))]
pub async fn get_agent_config(
    agent_id: String,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    info!("Getting agent configuration");

    // Validate input
    let validated_agent_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    let agent = state
        .registry
        .get(&validated_agent_id)
        .await
        .ok_or_else(|| {
            warn!(agent_id = %validated_agent_id, "Agent not found");
            "Agent not found".to_string()
        })?;

    let config = agent.config().clone();
    info!(
        agent_name = %config.name,
        lifecycle = ?config.lifecycle,
        tools_count = config.tools.len(),
        "Agent configuration retrieved"
    );

    Ok(config)
}

/// Checks that agent name is unique (case-insensitive, trimmed).
///
/// - `exclude_id`: If Some, excludes this agent from the check (for update_agent).
async fn check_agent_name_unique(
    db: &crate::db::DBClient,
    name: &str,
    exclude_id: Option<&str>,
) -> Result<(), String> {
    let trimmed = name.trim();

    let (query, params) = match exclude_id {
        Some(id) => (
            "SELECT meta::id(id) AS id FROM agent WHERE string::lowercase(name) = string::lowercase($name) AND meta::id(id) != $id LIMIT 1",
            vec![
                ("name".to_string(), serde_json::json!(trimmed)),
                ("id".to_string(), serde_json::json!(id)),
            ],
        ),
        None => (
            "SELECT meta::id(id) AS id FROM agent WHERE string::lowercase(name) = string::lowercase($name) LIMIT 1",
            vec![("name".to_string(), serde_json::json!(trimmed))],
        ),
    };

    let results: Vec<serde_json::Value> = db
        .query_with_params(query, params)
        .await
        .map_err(|e| format!("Failed to check agent name uniqueness: {}", e))?;

    if !results.is_empty() {
        return Err(format!("An agent with name '{}' already exists", trimmed));
    }

    Ok(())
}

/// Creates a new agent
///
/// Validates the configuration, persists to database, and registers in memory.
#[tauri::command]
#[instrument(name = "create_agent", skip(state, config), fields(agent_name = %config.name))]
pub async fn create_agent(
    config: AgentConfigCreate,
    state: State<'_, AppState>,
) -> Result<String, String> {
    info!("Creating new agent");

    // Validate input
    let validated = validate_agent_create(&config).map_err(|e| {
        warn!(error = %e, "Agent validation failed");
        e
    })?;

    check_agent_name_unique(&state.db, &validated.name, None)
        .await
        .map_err(|e| {
            warn!(error = %e, "Agent name uniqueness check failed");
            e
        })?;

    // Generate UUID for new agent
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Build full AgentConfig (destructure instead of cloning individual fields)
    let AgentConfigCreate {
        name,
        lifecycle,
        llm,
        tools,
        mcp_servers,
        skills,
        system_prompt,
        max_tool_iterations,
        enable_thinking,
    } = validated;

    // Persist to database - get lifecycle string before moving into AgentConfig
    let lifecycle_str = match lifecycle {
        Lifecycle::Permanent => "permanent",
        Lifecycle::Temporary => "temporary",
    };

    let agent_config = AgentConfig {
        id: agent_id.clone(),
        name,
        lifecycle,
        llm,
        tools,
        mcp_servers,
        skills,
        system_prompt,
        max_tool_iterations,
        enable_thinking,
    };

    // Serialize fields for database
    let fields = serialize_agent_fields(&agent_config)?;

    let query = format!(
        "CREATE agent:`{}` CONTENT {{
            id: '{}',
            name: {},
            lifecycle: '{}',
            llm: {},
            tools: {},
            mcp_servers: {},
            skills: {},
            system_prompt: {},
            max_tool_iterations: {},
            enable_thinking: {},
            created_at: time::now(),
            updated_at: time::now()
        }}",
        agent_id,
        agent_id,
        fields.name_json,
        lifecycle_str,
        fields.llm_json,
        fields.tools_json,
        fields.mcp_json,
        fields.skills_json,
        fields.prompt_json,
        validated.max_tool_iterations,
        validated.enable_thinking
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to persist agent to database");
        format!("Failed to persist agent: {}", e)
    })?;

    // Register agent in runtime
    register_agent_runtime(state.inner(), &agent_id, agent_config).await;

    info!(agent_id = %agent_id, "Agent created successfully");
    Ok(agent_id)
}

/// Merges partial update fields with existing agent config, validating each field.
fn merge_agent_config(
    update: &AgentConfigUpdate,
    existing: &AgentConfig,
) -> Result<AgentConfig, String> {
    let name = match &update.name {
        Some(n) => validate_agent_name(n)?,
        None => existing.name.clone(),
    };
    let llm = match &update.llm {
        Some(l) => validate_llm_config(l)?,
        None => existing.llm.clone(),
    };
    let tools = match &update.tools {
        Some(t) => validate_tools(t)?,
        None => existing.tools.clone(),
    };
    let mcp_servers = match &update.mcp_servers {
        Some(m) => validate_mcp_servers(m)?,
        None => existing.mcp_servers.clone(),
    };
    let skills = match &update.skills {
        Some(s) => validate_skills(s)?,
        None => existing.skills.clone(),
    };
    let system_prompt = match &update.system_prompt {
        Some(p) => validate_system_prompt(p)?,
        None => existing.system_prompt.clone(),
    };
    let max_tool_iterations = update
        .max_tool_iterations
        .map_or(existing.max_tool_iterations, |m| m.clamp(1, 200));
    let enable_thinking = update.enable_thinking.unwrap_or(existing.enable_thinking);

    Ok(AgentConfig {
        id: existing.id.clone(),
        name,
        lifecycle: existing.lifecycle.clone(),
        llm,
        tools,
        mcp_servers,
        skills,
        system_prompt,
        max_tool_iterations,
        enable_thinking,
    })
}

/// Updates an existing agent
///
/// Validates the configuration, updates database, and re-registers in memory.
#[tauri::command]
#[instrument(name = "update_agent", skip(state, config), fields(agent_id = %agent_id))]
pub async fn update_agent(
    agent_id: String,
    config: AgentConfigUpdate,
    state: State<'_, AppState>,
) -> Result<AgentConfig, String> {
    info!("Updating agent");

    let validated_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    let existing = state.registry.get(&validated_id).await.ok_or_else(|| {
        warn!(agent_id = %validated_id, "Agent not found");
        "Agent not found".to_string()
    })?;

    let mut updated_config = merge_agent_config(&config, existing.config())?;
    updated_config.id = validated_id.clone();

    check_agent_name_unique(&state.db, &updated_config.name, Some(&validated_id))
        .await
        .map_err(|e| {
            warn!(error = %e, "Agent name uniqueness check failed on update");
            e
        })?;

    // Serialize and persist to database
    let fields = serialize_agent_fields(&updated_config)?;
    let query = format!(
        "UPDATE agent:`{}` SET
            name = {},
            llm = {},
            tools = {},
            mcp_servers = {},
            skills = {},
            system_prompt = {},
            max_tool_iterations = {},
            enable_thinking = {},
            updated_at = time::now()",
        validated_id,
        fields.name_json,
        fields.llm_json,
        fields.tools_json,
        fields.mcp_json,
        fields.skills_json,
        fields.prompt_json,
        updated_config.max_tool_iterations,
        updated_config.enable_thinking
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update agent in database");
        format!("Failed to update agent: {}", e)
    })?;

    // Unregister old and register new agent
    state.registry.unregister_any(&validated_id).await;
    register_agent_runtime(state.inner(), &validated_id, updated_config.clone()).await;

    info!(agent_id = %validated_id, "Agent updated successfully");
    Ok(updated_config)
}

/// Deletes an agent
///
/// Removes from database and unregisters from memory.
#[tauri::command]
#[instrument(name = "delete_agent", skip(state), fields(agent_id = %agent_id))]
pub async fn delete_agent(agent_id: String, state: State<'_, AppState>) -> Result<(), String> {
    info!("Deleting agent");

    // Validate agent ID
    let validated_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // Check agent exists
    if state.registry.get(&validated_id).await.is_none() {
        warn!(agent_id = %validated_id, "Agent not found");
        return Err("Agent not found".to_string());
    }

    // Delete from database
    let query = format!("DELETE agent:`{}`", validated_id);
    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to delete agent from database");
        format!("Failed to delete agent: {}", e)
    })?;

    // Unregister from memory
    state.registry.unregister_any(&validated_id).await;

    info!(agent_id = %validated_id, "Agent deleted successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::agents::core::{AgentOrchestrator, AgentRegistry};
    use crate::agents::SimpleAgent;
    use crate::db::DBClient;
    use crate::models::{AgentConfig, LLMConfig, Lifecycle};
    use crate::state::AppState;
    use std::sync::Arc;
    use tempfile::tempdir;

    /// Helper to create test AppState with registry
    async fn setup_test_state() -> AppState {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_db");
        let db_path_str = db_path.to_str().unwrap();

        let db = Arc::new(
            DBClient::new(db_path_str)
                .await
                .expect("Failed to create test DB"),
        );
        db.initialize_schema()
            .await
            .expect("Failed to initialize schema");

        let registry = Arc::new(AgentRegistry::new());
        let orchestrator = Arc::new(AgentOrchestrator::new(registry.clone()));
        let llm_manager =
            Arc::new(crate::llm::ProviderManager::new().expect("test provider manager"));
        let mcp_manager = Arc::new(
            crate::mcp::MCPManager::new(db.clone())
                .await
                .expect("Failed to create MCP manager"),
        );

        // Leak temp_dir to keep it alive during test
        std::mem::forget(temp_dir);

        // Create shared embedding service reference
        let embedding_service = Arc::new(tokio::sync::RwLock::new(None));

        AppState {
            db: db.clone(),
            registry,
            orchestrator,
            llm_manager,
            mcp_manager,
            tool_factory: Arc::new(crate::tools::ToolFactory::new(
                db,
                embedding_service.clone(),
            )),
            embedding_service,
            streaming_cancellations: Arc::new(tokio::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
            app_handle: Arc::new(std::sync::RwLock::new(None)),
        }
    }

    #[tokio::test]
    async fn test_list_agents_empty() {
        let state = setup_test_state().await;
        let agents = state.registry.list().await;
        assert!(agents.is_empty(), "New registry should be empty");
    }

    #[tokio::test]
    async fn test_list_agents_with_registered() {
        let state = setup_test_state().await;

        // Register agent
        let config = AgentConfig {
            id: "test_agent".to_string(),
            name: "Test Agent".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Demo".to_string(),
                model: "test".to_string(),
                temperature: 0.7,
                max_tokens: 1000,
                is_reasoning: false,
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
            skills: vec![],
            system_prompt: "Test".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

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
        let state = setup_test_state().await;

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
            },
            tools: vec!["tool_a".to_string(), "tool_b".to_string()],
            mcp_servers: vec!["serena".to_string()],
            skills: vec![],
            system_prompt: "You are a test agent".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

        let agent = SimpleAgent::new(config.clone());
        state
            .registry
            .register("config_test".to_string(), Arc::new(agent))
            .await;

        // Get config
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
        let state = setup_test_state().await;

        let result = state.registry.get("nonexistent").await;
        assert!(result.is_none(), "Should not find nonexistent agent");
    }

    #[tokio::test]
    async fn test_agent_config_serialization() {
        let config = AgentConfig {
            id: "serial_test".to_string(),
            name: "Serialization Test".to_string(),
            lifecycle: Lifecycle::Permanent,
            llm: LLMConfig {
                provider: "Ollama".to_string(),
                model: "llama3".to_string(),
                temperature: 0.8,
                max_tokens: 4096,
                is_reasoning: false,
            },
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
            system_prompt: "Test prompt".to_string(),
            max_tool_iterations: 50,
            enable_thinking: true,
        };

        // Verify JSON serialization
        let json = serde_json::to_string(&config);
        assert!(json.is_ok(), "AgentConfig should serialize to JSON");

        let json_str = json.unwrap();
        assert!(json_str.contains("\"serial_test\""));
        assert!(json_str.contains("\"permanent\""));
        assert!(json_str.contains("\"Ollama\""));
    }

    #[tokio::test]
    async fn test_lifecycle_serialization() {
        // Test Lifecycle enum serialization
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
        let state = setup_test_state().await;

        // Register multiple agents
        for i in 0..5 {
            let config = AgentConfig {
                id: format!("agent_{}", i),
                name: format!("Agent {}", i),
                lifecycle: Lifecycle::Temporary,
                llm: LLMConfig {
                    provider: "Demo".to_string(),
                    model: "test".to_string(),
                    temperature: 0.7,
                    max_tokens: 1000,
                    is_reasoning: false,
                },
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
                system_prompt: format!("Agent {} prompt", i),
                max_tool_iterations: 50,
                enable_thinking: true,
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

    // ========================================================================
    // SA-020/P1: Agent name uniqueness tests
    // ========================================================================

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
                enable_thinking = false, \
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
        let state = setup_test_state().await;
        // Seed an agent named "Database Agent"
        seed_agent_in_db(&state.db, "Database Agent").await;

        // Attempt to check same name (case-insensitive) => should fail
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
        let state = setup_test_state().await;
        // Seed agent
        let agent_id = seed_agent_in_db(&state.db, "My Agent").await;

        // Check uniqueness excluding self => should pass
        let result = super::check_agent_name_unique(&state.db, "My Agent", Some(&agent_id)).await;
        assert!(
            result.is_ok(),
            "Should allow keeping own name, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_update_agent_rejects_collision_with_other() {
        let state = setup_test_state().await;
        // Seed two agents
        let _agent_a = seed_agent_in_db(&state.db, "Agent Alpha").await;
        let agent_b = seed_agent_in_db(&state.db, "Agent Beta").await;

        // Try to rename Agent Beta to "Agent Alpha" => should fail
        let result = super::check_agent_name_unique(&state.db, "Agent Alpha", Some(&agent_b)).await;
        assert!(
            result.is_err(),
            "Should reject renaming to existing agent's name"
        );
    }
}
