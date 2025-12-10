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
use crate::models::{
    AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, LLMConfig, Lifecycle,
};
use crate::security::Validator;
use crate::state::AppState;
use crate::tools::constants::commands as cmd_const;
use crate::tools::context::AgentToolContext;
use crate::tools::registry::TOOL_REGISTRY;
use std::sync::Arc;
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Validates agent name
fn validate_agent_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Agent name cannot be empty".to_string());
    }

    if trimmed.len() > cmd_const::MAX_AGENT_NAME_LEN {
        return Err(format!(
            "Agent name exceeds maximum length of {} characters",
            cmd_const::MAX_AGENT_NAME_LEN
        ));
    }

    if trimmed.chars().any(|c| c.is_control() && c != '\n') {
        return Err("Agent name cannot contain control characters".to_string());
    }

    Ok(trimmed.to_string())
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
    // Validate provider
    if !cmd_const::VALID_PROVIDERS.contains(&llm.provider.as_str()) {
        return Err(format!(
            "Invalid provider '{}'. Valid providers: {:?}",
            llm.provider,
            cmd_const::VALID_PROVIDERS
        ));
    }

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
        system_prompt: validate_system_prompt(&config.system_prompt)?,
        max_tool_iterations: config.max_tool_iterations.clamp(1, 200),
        enable_thinking: config.enable_thinking,
    })
}

// ============================================================================
// Database Serialization Helpers (OPT-5)
// ============================================================================

/// Serialized agent configuration fields for database operations
struct SerializedAgentFields {
    name_json: String,
    llm_json: String,
    tools_json: String,
    mcp_json: String,
    prompt_json: String,
}

/// Serializes agent configuration fields for database storage
fn serialize_agent_fields(config: &AgentConfig) -> Result<SerializedAgentFields, String> {
    let name_json = serde_json::to_string(&config.name).map_err(|e| {
        error!(error = %e, "Failed to serialize name");
        format!("Failed to serialize name: {}", e)
    })?;

    let llm_json = serde_json::to_string(&config.llm).map_err(|e| {
        error!(error = %e, "Failed to serialize LLM config");
        format!("Failed to serialize LLM config: {}", e)
    })?;

    let tools_json = serde_json::to_string(&config.tools).map_err(|e| {
        error!(error = %e, "Failed to serialize tools");
        format!("Failed to serialize tools: {}", e)
    })?;

    let mcp_json = serde_json::to_string(&config.mcp_servers).map_err(|e| {
        error!(error = %e, "Failed to serialize MCP servers");
        format!("Failed to serialize MCP servers: {}", e)
    })?;

    let prompt_json = serde_json::to_string(&config.system_prompt).map_err(|e| {
        error!(error = %e, "Failed to serialize system prompt");
        format!("Failed to serialize system prompt: {}", e)
    })?;

    Ok(SerializedAgentFields {
        name_json,
        llm_json,
        tools_json,
        mcp_json,
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

    // Generate UUID for new agent
    let agent_id = uuid::Uuid::new_v4().to_string();

    // Build full AgentConfig (OPT-7: destructure instead of cloning individual fields)
    let AgentConfigCreate {
        name,
        lifecycle,
        llm,
        tools,
        mcp_servers,
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
        system_prompt,
        max_tool_iterations,
        enable_thinking,
    };

    // Serialize fields for database (OPT-5 refactoring)
    let fields = serialize_agent_fields(&agent_config)?;

    let query = format!(
        "CREATE agent:`{}` CONTENT {{
            id: '{}',
            name: {},
            lifecycle: '{}',
            llm: {},
            tools: {},
            mcp_servers: {},
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
        fields.prompt_json,
        validated.max_tool_iterations,
        validated.enable_thinking
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to persist agent to database");
        format!("Failed to persist agent: {}", e)
    })?;

    // Register agent in runtime (OPT-5 refactoring)
    register_agent_runtime(state.inner(), &agent_id, agent_config).await;

    info!(agent_id = %agent_id, "Agent created successfully");
    Ok(agent_id)
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

    // Validate agent ID
    let validated_id = Validator::validate_agent_id(&agent_id).map_err(|e| {
        warn!(error = %e, "Invalid agent_id");
        format!("Invalid agent_id: {}", e)
    })?;

    // Get existing agent
    let existing = state.registry.get(&validated_id).await.ok_or_else(|| {
        warn!(agent_id = %validated_id, "Agent not found");
        "Agent not found".to_string()
    })?;

    let existing_config = existing.config().clone();

    // Build updated config (merge with existing)
    let new_name = match &config.name {
        Some(n) => validate_agent_name(n)?,
        None => existing_config.name.clone(),
    };

    let new_llm = match &config.llm {
        Some(l) => validate_llm_config(l)?,
        None => existing_config.llm.clone(),
    };

    let new_tools = match &config.tools {
        Some(t) => validate_tools(t)?,
        None => existing_config.tools.clone(),
    };

    let new_mcp = match &config.mcp_servers {
        Some(m) => validate_mcp_servers(m)?,
        None => existing_config.mcp_servers.clone(),
    };

    let new_prompt = match &config.system_prompt {
        Some(p) => validate_system_prompt(p)?,
        None => existing_config.system_prompt.clone(),
    };

    let new_max_iterations = match config.max_tool_iterations {
        Some(m) => m.clamp(1, 200),
        None => existing_config.max_tool_iterations,
    };

    let new_enable_thinking = match config.enable_thinking {
        Some(e) => e,
        None => existing_config.enable_thinking,
    };

    let updated_config = AgentConfig {
        id: validated_id.clone(),
        name: new_name,
        lifecycle: existing_config.lifecycle.clone(), // Cannot change lifecycle
        llm: new_llm,
        tools: new_tools,
        mcp_servers: new_mcp,
        system_prompt: new_prompt,
        max_tool_iterations: new_max_iterations,
        enable_thinking: new_enable_thinking,
    };

    // Serialize fields for database (OPT-5 refactoring)
    let fields = serialize_agent_fields(&updated_config)?;

    let query = format!(
        "UPDATE agent:`{}` SET
            name = {},
            llm = {},
            tools = {},
            mcp_servers = {},
            system_prompt = {},
            max_tool_iterations = {},
            enable_thinking = {},
            updated_at = time::now()",
        validated_id,
        fields.name_json,
        fields.llm_json,
        fields.tools_json,
        fields.mcp_json,
        fields.prompt_json,
        new_max_iterations,
        new_enable_thinking
    );

    state.db.execute(&query).await.map_err(|e| {
        error!(error = %e, "Failed to update agent in database");
        format!("Failed to update agent: {}", e)
    })?;

    // Unregister old and register new agent (OPT-5 refactoring)
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

/// Loads all agents from database and registers them in memory
///
/// Note: This function is no longer called directly. Agent loading is now
/// done inline in the setup hook in main.rs to ensure app_handle is available.
#[allow(dead_code)]
pub async fn load_agents_from_db(state: &AppState) -> Result<usize, String> {
    info!("Loading agents from database");

    // Query all agents from database
    let query = "SELECT meta::id(id) AS id, name, lifecycle, llm, tools, mcp_servers, system_prompt FROM agent";

    let results: Vec<serde_json::Value> = state
        .db
        .db
        .query(query)
        .await
        .map(|mut r| r.take(0).unwrap_or_default())
        .map_err(|e| {
            error!(error = %e, "Failed to query agents from database");
            format!("Failed to query agents: {}", e)
        })?;

    let mut loaded = 0;

    for row in results {
        // Parse agent config from row
        let id = row["id"].as_str().unwrap_or("").to_string();
        if id.is_empty() {
            warn!("Skipping agent with empty ID");
            continue;
        }

        let name = row["name"].as_str().unwrap_or("Unknown").to_string();

        let lifecycle_str = row["lifecycle"].as_str().unwrap_or("permanent");
        let lifecycle = match lifecycle_str {
            "temporary" => Lifecycle::Temporary,
            _ => Lifecycle::Permanent,
        };

        // Parse LLM config
        let llm_value = &row["llm"];
        let llm = LLMConfig {
            provider: llm_value["provider"]
                .as_str()
                .unwrap_or("Mistral")
                .to_string(),
            model: llm_value["model"]
                .as_str()
                .unwrap_or("mistral-large-latest")
                .to_string(),
            temperature: llm_value["temperature"].as_f64().unwrap_or(0.7) as f32,
            max_tokens: llm_value["max_tokens"].as_u64().unwrap_or(4096) as usize,
        };

        // Parse tools
        let tools: Vec<String> = row["tools"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Parse MCP servers
        let mcp_servers: Vec<String> = row["mcp_servers"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        let system_prompt = row["system_prompt"]
            .as_str()
            .unwrap_or("You are a helpful assistant.")
            .to_string();

        let max_tool_iterations = row["max_tool_iterations"]
            .as_u64()
            .map(|v| v as usize)
            .unwrap_or(50)
            .clamp(1, 200);

        let enable_thinking = row["enable_thinking"].as_bool().unwrap_or(true);

        let config = AgentConfig {
            id: id.clone(),
            name,
            lifecycle,
            llm,
            tools,
            mcp_servers,
            system_prompt,
            max_tool_iterations,
            enable_thinking,
        };

        // Create LLMAgent with AgentToolContext for sub-agent operations
        let agent_context = AgentToolContext::from_app_state_full(state);
        let llm_agent = LLMAgent::with_context(
            config,
            state.llm_manager.clone(),
            state.tool_factory.clone(),
            agent_context,
        );
        state.registry.register(id, Arc::new(llm_agent)).await;

        loaded += 1;
    }

    info!(count = loaded, "Agents loaded from database");
    Ok(loaded)
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
        let llm_manager = Arc::new(crate::llm::ProviderManager::new());
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
            },
            tools: vec!["tool1".to_string()],
            mcp_servers: vec![],
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
            },
            tools: vec!["tool_a".to_string(), "tool_b".to_string()],
            mcp_servers: vec!["serena".to_string()],
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
            },
            tools: vec![],
            mcp_servers: vec![],
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
                },
                tools: vec![],
                mcp_servers: vec![],
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
}
