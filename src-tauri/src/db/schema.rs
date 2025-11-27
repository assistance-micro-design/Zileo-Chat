pub const SCHEMA_SQL: &str = r#"
-- Namespace et Database
DEFINE NAMESPACE zileo;
USE NS zileo;
DEFINE DATABASE chat;
USE DB chat;

-- Table: workflow
DEFINE TABLE workflow SCHEMAFULL;
DEFINE FIELD id ON workflow TYPE string;
DEFINE FIELD name ON workflow TYPE string;
DEFINE FIELD agent_id ON workflow TYPE string;
DEFINE FIELD status ON workflow TYPE string ASSERT $value IN ['idle', 'running', 'completed', 'error'];
DEFINE FIELD created_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD completed_at ON workflow TYPE option<datetime>;

-- Table: agent_state
DEFINE TABLE agent_state SCHEMAFULL;
DEFINE FIELD agent_id ON agent_state TYPE string;
DEFINE FIELD lifecycle ON agent_state TYPE string ASSERT $value IN ['permanent', 'temporary'];
DEFINE FIELD config ON agent_state TYPE object;
DEFINE FIELD metrics ON agent_state TYPE object;
DEFINE FIELD last_active ON agent_state TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_agent_id ON agent_state FIELDS agent_id UNIQUE;

-- Table: message
-- Extended with metrics fields for Phase 6 persistence
DEFINE TABLE message SCHEMAFULL;
DEFINE FIELD id ON message TYPE string;
DEFINE FIELD workflow_id ON message TYPE string;
DEFINE FIELD role ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD content ON message TYPE string;
DEFINE FIELD tokens ON message TYPE int;
DEFINE FIELD tokens_input ON message TYPE option<int>;
DEFINE FIELD tokens_output ON message TYPE option<int>;
DEFINE FIELD model ON message TYPE option<string>;
DEFINE FIELD provider ON message TYPE option<string>;
DEFINE FIELD cost_usd ON message TYPE option<float>;
DEFINE FIELD duration_ms ON message TYPE option<int>;
DEFINE FIELD timestamp ON message TYPE datetime DEFAULT time::now();

-- Index for workflow message queries
DEFINE INDEX message_workflow_idx ON message FIELDS workflow_id;
DEFINE INDEX message_timestamp_idx ON message FIELDS timestamp;

-- Table: memory (vectoriel)
DEFINE TABLE memory SCHEMAFULL;
DEFINE FIELD id ON memory TYPE string;
DEFINE FIELD type ON memory TYPE string ASSERT $value IN ['user_pref', 'context', 'knowledge', 'decision'];
DEFINE FIELD content ON memory TYPE string;
DEFINE FIELD embedding ON memory TYPE option<array<float>>;
DEFINE FIELD workflow_id ON memory TYPE option<string>;
DEFINE FIELD metadata ON memory TYPE object;
DEFINE FIELD created_at ON memory TYPE datetime DEFAULT time::now();

-- Index HNSW pour vector search (1024D Mistral/Ollama embeddings)
DEFINE INDEX memory_vec_idx ON memory FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;
-- Index for workflow scoping
DEFINE INDEX memory_workflow_idx ON memory FIELDS workflow_id;

-- Table: validation_request
DEFINE TABLE validation_request SCHEMAFULL;
DEFINE FIELD id ON validation_request TYPE string;
DEFINE FIELD workflow_id ON validation_request TYPE string;
DEFINE FIELD type ON validation_request TYPE string ASSERT $value IN ['tool', 'sub_agent', 'mcp', 'file_op', 'db_op'];
DEFINE FIELD operation ON validation_request TYPE string;
DEFINE FIELD details ON validation_request TYPE object;
DEFINE FIELD risk_level ON validation_request TYPE string ASSERT $value IN ['low', 'medium', 'high'];
DEFINE FIELD status ON validation_request TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'approved', 'rejected'];
DEFINE FIELD created_at ON validation_request TYPE datetime DEFAULT time::now();

-- Table: task (decomposition workflows with Todo Tool support)
DEFINE TABLE task SCHEMAFULL;
DEFINE FIELD id ON task TYPE string;
DEFINE FIELD workflow_id ON task TYPE string;
DEFINE FIELD name ON task TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 128;
DEFINE FIELD description ON task TYPE string
    ASSERT string::len($value) <= 1000;
DEFINE FIELD agent_assigned ON task TYPE option<string>;
DEFINE FIELD priority ON task TYPE int DEFAULT 3
    ASSERT $value >= 1 AND $value <= 5;
DEFINE FIELD status ON task TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'in_progress', 'completed', 'blocked'];
DEFINE FIELD dependencies ON task TYPE array<string>;
DEFINE FIELD duration_ms ON task TYPE option<int>;
DEFINE FIELD created_at ON task TYPE datetime DEFAULT time::now();
DEFINE FIELD completed_at ON task TYPE option<datetime>;

-- Indexes for task queries
DEFINE INDEX task_workflow_idx ON task FIELDS workflow_id;
DEFINE INDEX task_status_idx ON task FIELDS status;
DEFINE INDEX task_priority_idx ON task FIELDS priority;
DEFINE INDEX task_agent_idx ON task FIELDS agent_assigned;

-- Relations graph
DEFINE TABLE workflow_agent SCHEMAFULL;
DEFINE FIELD in ON workflow_agent TYPE record<workflow>;
DEFINE FIELD out ON workflow_agent TYPE record<agent_state>;
DEFINE FIELD created_by ON workflow_agent TYPE bool DEFAULT true;

-- Table: mcp_server (MCP server configurations)
DEFINE TABLE mcp_server SCHEMAFULL;
DEFINE FIELD id ON mcp_server TYPE string;
DEFINE FIELD name ON mcp_server TYPE string;
DEFINE FIELD enabled ON mcp_server TYPE bool DEFAULT true;
DEFINE FIELD command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx'];
DEFINE FIELD args ON mcp_server TYPE array<string>;
DEFINE FIELD env ON mcp_server TYPE object;
DEFINE FIELD description ON mcp_server TYPE option<string>;
DEFINE FIELD created_at ON mcp_server TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON mcp_server TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_mcp_id ON mcp_server FIELDS id UNIQUE;
DEFINE INDEX unique_mcp_name ON mcp_server FIELDS name UNIQUE;

-- Table: mcp_call_log (MCP tool call audit log)
DEFINE TABLE mcp_call_log SCHEMAFULL;
DEFINE FIELD id ON mcp_call_log TYPE string;
DEFINE FIELD workflow_id ON mcp_call_log TYPE option<string>;
DEFINE FIELD server_name ON mcp_call_log TYPE string;
DEFINE FIELD tool_name ON mcp_call_log TYPE string;
DEFINE FIELD params ON mcp_call_log TYPE object;
DEFINE FIELD result ON mcp_call_log TYPE object;
DEFINE FIELD success ON mcp_call_log TYPE bool;
DEFINE FIELD duration_ms ON mcp_call_log TYPE int;
DEFINE FIELD timestamp ON mcp_call_log TYPE datetime DEFAULT time::now();
DEFINE INDEX mcp_call_workflow ON mcp_call_log FIELDS workflow_id;
DEFINE INDEX mcp_call_server ON mcp_call_log FIELDS server_name;

-- =============================================
-- Table: llm_model
-- Stores LLM models (builtin + custom)
-- =============================================
DEFINE TABLE llm_model SCHEMAFULL;
DEFINE FIELD id ON llm_model TYPE string;
DEFINE FIELD provider ON llm_model TYPE string
    ASSERT $value IN ['mistral', 'ollama'];
DEFINE FIELD name ON llm_model TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
DEFINE FIELD api_name ON llm_model TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 128;
DEFINE FIELD context_window ON llm_model TYPE int
    ASSERT $value >= 1024 AND $value <= 2000000;
DEFINE FIELD max_output_tokens ON llm_model TYPE int
    ASSERT $value >= 256 AND $value <= 128000;
DEFINE FIELD temperature_default ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 2.0
    DEFAULT 0.7;
DEFINE FIELD is_builtin ON llm_model TYPE bool DEFAULT false;
DEFINE FIELD created_at ON llm_model TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON llm_model TYPE datetime DEFAULT time::now();

DEFINE INDEX unique_model_id ON llm_model FIELDS id UNIQUE;
DEFINE INDEX model_provider_idx ON llm_model FIELDS provider;
DEFINE INDEX model_api_name_idx ON llm_model FIELDS provider, api_name UNIQUE;

-- =============================================
-- Table: provider_settings
-- Configuration per provider
-- =============================================
DEFINE TABLE provider_settings SCHEMAFULL;
DEFINE FIELD provider ON provider_settings TYPE string
    ASSERT $value IN ['mistral', 'ollama'];
DEFINE FIELD enabled ON provider_settings TYPE bool DEFAULT true;
DEFINE FIELD default_model_id ON provider_settings TYPE option<string>;
DEFINE FIELD base_url ON provider_settings TYPE option<string>;
DEFINE FIELD updated_at ON provider_settings TYPE datetime DEFAULT time::now();

DEFINE INDEX unique_provider ON provider_settings FIELDS provider UNIQUE;

-- =============================================
-- Table: agent
-- Stores user-created agent configurations
-- =============================================
DEFINE TABLE agent SCHEMAFULL;
DEFINE FIELD id ON agent TYPE string;
DEFINE FIELD name ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 64;
DEFINE FIELD lifecycle ON agent TYPE string
    ASSERT $value IN ['permanent', 'temporary'];

-- LLM configuration (embedded object)
DEFINE FIELD llm ON agent TYPE object;
DEFINE FIELD llm.provider ON agent TYPE string
    ASSERT $value IN ['Mistral', 'Ollama', 'Demo'];
DEFINE FIELD llm.model ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD llm.temperature ON agent TYPE float
    ASSERT $value >= 0.0 AND $value <= 2.0;
DEFINE FIELD llm.max_tokens ON agent TYPE int
    ASSERT $value >= 256 AND $value <= 128000;

-- Tools and MCP servers
DEFINE FIELD tools ON agent TYPE array<string>;
DEFINE FIELD mcp_servers ON agent TYPE array<string>;

-- System prompt
DEFINE FIELD system_prompt ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 10000;

-- Timestamps
DEFINE FIELD created_at ON agent TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON agent TYPE datetime DEFAULT time::now();

-- Indexes
DEFINE INDEX unique_agent_id ON agent FIELDS id UNIQUE;
DEFINE INDEX agent_name_idx ON agent FIELDS name;
DEFINE INDEX agent_provider_idx ON agent FIELDS llm.provider;
"#;
