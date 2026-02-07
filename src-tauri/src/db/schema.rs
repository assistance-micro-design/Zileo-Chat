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

pub const SCHEMA_SQL: &str = r#"
-- Namespace et Database
DEFINE NAMESPACE zileo;
USE NS zileo;
DEFINE DATABASE chat;
USE DB chat;

-- Table: workflow
-- Extended with cumulative token tracking for Token Display Complet
DEFINE TABLE OVERWRITE workflow SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON workflow TYPE string;
DEFINE FIELD OVERWRITE name ON workflow TYPE string;
DEFINE FIELD OVERWRITE agent_id ON workflow TYPE string;
DEFINE FIELD OVERWRITE status ON workflow TYPE string ASSERT $value IN ['idle', 'running', 'completed', 'error'];
DEFINE FIELD OVERWRITE created_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON workflow TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE completed_at ON workflow TYPE option<datetime>;
-- Cumulative token tracking (Token Display Complet feature)
DEFINE FIELD OVERWRITE total_tokens_input ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_tokens_output ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_cost_usd ON workflow TYPE float DEFAULT 0.0;
DEFINE FIELD OVERWRITE model_id ON workflow TYPE option<string>;
-- Current context size (last API call context window usage)
DEFINE FIELD OVERWRITE current_context_tokens ON workflow TYPE int DEFAULT 0;

-- Table: agent_state
DEFINE TABLE OVERWRITE agent_state SCHEMAFULL;
DEFINE FIELD OVERWRITE agent_id ON agent_state TYPE string;
DEFINE FIELD OVERWRITE lifecycle ON agent_state TYPE string ASSERT $value IN ['permanent', 'temporary'];
DEFINE FIELD OVERWRITE config ON agent_state TYPE object;
DEFINE FIELD OVERWRITE metrics ON agent_state TYPE object;
DEFINE FIELD OVERWRITE last_active ON agent_state TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE unique_agent_id ON agent_state FIELDS agent_id UNIQUE;

-- Table: message
-- Extended with metrics fields for Phase 6 persistence
DEFINE TABLE OVERWRITE message SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON message TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON message TYPE string;
DEFINE FIELD OVERWRITE role ON message TYPE string ASSERT $value IN ['user', 'assistant', 'system'];
DEFINE FIELD OVERWRITE content ON message TYPE string;
DEFINE FIELD OVERWRITE tokens ON message TYPE int;
DEFINE FIELD OVERWRITE tokens_input ON message TYPE option<int>;
DEFINE FIELD OVERWRITE tokens_output ON message TYPE option<int>;
DEFINE FIELD OVERWRITE model ON message TYPE option<string>;
DEFINE FIELD OVERWRITE provider ON message TYPE option<string>;
DEFINE FIELD OVERWRITE cost_usd ON message TYPE option<float>;
DEFINE FIELD OVERWRITE duration_ms ON message TYPE option<int>;
DEFINE FIELD OVERWRITE timestamp ON message TYPE datetime DEFAULT time::now();

-- =============================================
-- Index Review (OPT-DB-11): Write-Heavy Table Analysis
-- =============================================
-- message is a write-heavy table (every LLM response creates a record)
-- Index trade-off: faster reads vs slower writes
-- Keep both indexes as they are actively used:
--   - message_workflow_idx: Required for loading conversation history
--   - message_timestamp_idx: Required for chronological message display in UI
DEFINE INDEX OVERWRITE message_workflow_idx ON message FIELDS workflow_id;
DEFINE INDEX OVERWRITE message_timestamp_idx ON message FIELDS timestamp;

-- Table: memory (vectoriel)
DEFINE TABLE OVERWRITE memory SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON memory TYPE string;
DEFINE FIELD OVERWRITE type ON memory TYPE string ASSERT $value IN ['user_pref', 'context', 'knowledge', 'decision'];
DEFINE FIELD OVERWRITE content ON memory TYPE string;
DEFINE FIELD OVERWRITE embedding ON memory TYPE option<array<float>>;
DEFINE FIELD OVERWRITE workflow_id ON memory TYPE option<string>;
DEFINE FIELD OVERWRITE metadata ON memory TYPE object;
-- Explicit metadata sub-fields (required for SCHEMAFULL to persist dynamic keys)
DEFINE FIELD OVERWRITE metadata.tags ON memory TYPE option<array<string>>;
DEFINE FIELD OVERWRITE metadata.priority ON memory TYPE option<float>;
DEFINE FIELD OVERWRITE metadata.agent_source ON memory TYPE option<string>;
DEFINE FIELD OVERWRITE importance ON memory TYPE float DEFAULT 0.5;
DEFINE FIELD OVERWRITE expires_at ON memory TYPE option<datetime>;
DEFINE FIELD OVERWRITE created_at ON memory TYPE datetime DEFAULT time::now();

-- Index HNSW pour vector search (1024D Mistral/Ollama embeddings)
DEFINE INDEX OVERWRITE memory_vec_idx ON memory FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;
-- Index for workflow scoping
DEFINE INDEX OVERWRITE memory_workflow_idx ON memory FIELDS workflow_id;
-- OPT-MEM-4: Composite index for search_memories() with type + workflow_id
DEFINE INDEX OVERWRITE memory_type_workflow_idx ON memory FIELDS type, workflow_id;
-- OPT-MEM-4: Composite index for TTL cleanup preparation (type + created_at)
DEFINE INDEX OVERWRITE memory_type_created_idx ON memory FIELDS type, created_at;

-- Table: validation_request
DEFINE TABLE OVERWRITE validation_request SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON validation_request TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON validation_request TYPE string;
DEFINE FIELD OVERWRITE type ON validation_request TYPE string ASSERT $value IN ['tool', 'sub_agent', 'mcp', 'file_op', 'db_op'];
DEFINE FIELD OVERWRITE operation ON validation_request TYPE string;
DEFINE FIELD OVERWRITE details ON validation_request TYPE object;
DEFINE FIELD OVERWRITE risk_level ON validation_request TYPE string ASSERT $value IN ['low', 'medium', 'high'];
DEFINE FIELD OVERWRITE status ON validation_request TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'approved', 'rejected'];
DEFINE FIELD OVERWRITE created_at ON validation_request TYPE datetime DEFAULT time::now();

-- Table: task (decomposition workflows with Todo Tool support)
DEFINE TABLE OVERWRITE task SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON task TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON task TYPE string;
DEFINE FIELD OVERWRITE name ON task TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE description ON task TYPE string
    ASSERT string::len($value) <= 1000;
DEFINE FIELD OVERWRITE agent_assigned ON task TYPE option<string>;
DEFINE FIELD OVERWRITE priority ON task TYPE int DEFAULT 3
    ASSERT $value >= 1 AND $value <= 5;
DEFINE FIELD OVERWRITE status ON task TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'in_progress', 'completed', 'blocked'];
DEFINE FIELD OVERWRITE dependencies ON task TYPE array<string>;
DEFINE FIELD OVERWRITE duration_ms ON task TYPE option<int>;
DEFINE FIELD OVERWRITE created_at ON task TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE completed_at ON task TYPE option<datetime>;

-- Indexes for task queries
DEFINE INDEX OVERWRITE task_workflow_idx ON task FIELDS workflow_id;
DEFINE INDEX OVERWRITE task_status_idx ON task FIELDS status;
DEFINE INDEX OVERWRITE task_priority_idx ON task FIELDS priority;
DEFINE INDEX OVERWRITE task_agent_idx ON task FIELDS agent_assigned;

-- Relations graph
DEFINE TABLE OVERWRITE workflow_agent SCHEMAFULL;
DEFINE FIELD OVERWRITE in ON workflow_agent TYPE record<workflow>;
DEFINE FIELD OVERWRITE out ON workflow_agent TYPE record<agent_state>;
DEFINE FIELD OVERWRITE created_by ON workflow_agent TYPE bool DEFAULT true;

-- Table: mcp_server (MCP server configurations)
DEFINE TABLE OVERWRITE mcp_server SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON mcp_server TYPE string;
DEFINE FIELD OVERWRITE name ON mcp_server TYPE string;
DEFINE FIELD OVERWRITE enabled ON mcp_server TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx', 'http'];
DEFINE FIELD OVERWRITE args ON mcp_server TYPE array<string>;
-- Store env as JSON string to bypass SurrealDB SCHEMAFULL nested object filtering
DEFINE FIELD OVERWRITE env ON mcp_server TYPE string DEFAULT '{}';
DEFINE FIELD OVERWRITE description ON mcp_server TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON mcp_server TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON mcp_server TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE unique_mcp_id ON mcp_server FIELDS id UNIQUE;
DEFINE INDEX OVERWRITE unique_mcp_name ON mcp_server FIELDS name UNIQUE;

-- Table: mcp_call_log (MCP tool call audit log)
DEFINE TABLE OVERWRITE mcp_call_log SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON mcp_call_log TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON mcp_call_log TYPE option<string>;
DEFINE FIELD OVERWRITE server_name ON mcp_call_log TYPE string;
DEFINE FIELD OVERWRITE tool_name ON mcp_call_log TYPE string;
DEFINE FIELD OVERWRITE params ON mcp_call_log TYPE object;
DEFINE FIELD OVERWRITE result ON mcp_call_log TYPE array | object; -- MCP tool results can be arrays or objects
DEFINE FIELD OVERWRITE success ON mcp_call_log TYPE bool;
DEFINE FIELD OVERWRITE duration_ms ON mcp_call_log TYPE int;
DEFINE FIELD OVERWRITE timestamp ON mcp_call_log TYPE datetime DEFAULT time::now();
-- =============================================
-- Index Review (OPT-DB-11): Write-Heavy Table Analysis
-- =============================================
-- mcp_call_log is write-heavy (every MCP tool call creates a record)
-- Index trade-off: faster reads vs slower writes
-- Keep both indexes as they are actively used:
--   - mcp_call_workflow: Required for workflow-scoped MCP call history
--   - mcp_call_server: Required for Phase 4 latency metrics (get_mcp_latency_metrics)
DEFINE INDEX OVERWRITE mcp_call_workflow ON mcp_call_log FIELDS workflow_id;
DEFINE INDEX OVERWRITE mcp_call_server ON mcp_call_log FIELDS server_name;

-- =============================================
-- Table: llm_model
-- Stores LLM models (builtin + custom)
-- =============================================
DEFINE TABLE OVERWRITE llm_model SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON llm_model TYPE string;
DEFINE FIELD OVERWRITE provider ON llm_model TYPE string
    ASSERT $value IN ['mistral', 'ollama'];
DEFINE FIELD OVERWRITE name ON llm_model TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE api_name ON llm_model TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE context_window ON llm_model TYPE int
    ASSERT $value >= 1024 AND $value <= 2000000;
DEFINE FIELD OVERWRITE max_output_tokens ON llm_model TYPE int
    ASSERT $value >= 256 AND $value <= 128000;
DEFINE FIELD OVERWRITE temperature_default ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 2.0
    DEFAULT 0.7;
DEFINE FIELD OVERWRITE is_builtin ON llm_model TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE is_reasoning ON llm_model TYPE bool DEFAULT false;
-- Pricing per million tokens (USD) - user configurable
DEFINE FIELD OVERWRITE input_price_per_mtok ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 1000.0
    DEFAULT 0.0;
DEFINE FIELD OVERWRITE output_price_per_mtok ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 1000.0
    DEFAULT 0.0;
DEFINE FIELD OVERWRITE created_at ON llm_model TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON llm_model TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE unique_model_id ON llm_model FIELDS id UNIQUE;
DEFINE INDEX OVERWRITE model_provider_idx ON llm_model FIELDS provider;
DEFINE INDEX OVERWRITE model_api_name_idx ON llm_model FIELDS provider, api_name UNIQUE;

-- =============================================
-- Table: provider_settings
-- Configuration per provider
-- =============================================
DEFINE TABLE OVERWRITE provider_settings SCHEMAFULL;
DEFINE FIELD OVERWRITE provider ON provider_settings TYPE string
    ASSERT $value IN ['mistral', 'ollama'];
DEFINE FIELD OVERWRITE enabled ON provider_settings TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE default_model_id ON provider_settings TYPE option<string>;
DEFINE FIELD OVERWRITE base_url ON provider_settings TYPE option<string>;
DEFINE FIELD OVERWRITE updated_at ON provider_settings TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE unique_provider ON provider_settings FIELDS provider UNIQUE;

-- =============================================
-- Table: agent
-- Stores user-created agent configurations
-- =============================================
DEFINE TABLE OVERWRITE agent SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON agent TYPE string;
DEFINE FIELD OVERWRITE name ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE lifecycle ON agent TYPE string
    ASSERT $value IN ['permanent', 'temporary'];

-- LLM configuration (embedded object)
DEFINE FIELD OVERWRITE llm ON agent TYPE object;
DEFINE FIELD OVERWRITE llm.provider ON agent TYPE string
    ASSERT $value IN ['Mistral', 'Ollama', 'Demo'];
DEFINE FIELD OVERWRITE llm.model ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE llm.temperature ON agent TYPE float
    ASSERT $value >= 0.0 AND $value <= 2.0;
DEFINE FIELD OVERWRITE llm.max_tokens ON agent TYPE int
    ASSERT $value >= 256 AND $value <= 128000;

-- Tools and MCP servers
DEFINE FIELD OVERWRITE tools ON agent TYPE array<string>;
DEFINE FIELD OVERWRITE mcp_servers ON agent TYPE array<string>;

-- System prompt
DEFINE FIELD OVERWRITE system_prompt ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 10000;

-- Max tool iterations (1-200, default: 50)
DEFINE FIELD OVERWRITE max_tool_iterations ON agent TYPE int
    ASSERT $value >= 1 AND $value <= 200
    DEFAULT 50;

-- Enable thinking mode for supported models (default: true)
DEFINE FIELD OVERWRITE enable_thinking ON agent TYPE bool
    DEFAULT true;

-- Timestamps
DEFINE FIELD OVERWRITE created_at ON agent TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON agent TYPE datetime DEFAULT time::now();

-- Indexes
DEFINE INDEX OVERWRITE unique_agent_id ON agent FIELDS id UNIQUE;
DEFINE INDEX OVERWRITE agent_name_idx ON agent FIELDS name;
DEFINE INDEX OVERWRITE agent_provider_idx ON agent FIELDS llm.provider;

-- =============================================
-- Table: tool_execution
-- Logs all tool executions (local + MCP)
-- Phase 3: Tool Execution Persistence
-- =============================================
DEFINE TABLE OVERWRITE tool_execution SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON tool_execution TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON tool_execution TYPE string;
DEFINE FIELD OVERWRITE message_id ON tool_execution TYPE string;
DEFINE FIELD OVERWRITE agent_id ON tool_execution TYPE string;
DEFINE FIELD OVERWRITE tool_type ON tool_execution TYPE string
    ASSERT $value IN ['local', 'mcp'];
DEFINE FIELD OVERWRITE tool_name ON tool_execution TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE server_name ON tool_execution TYPE option<string>;
DEFINE FIELD OVERWRITE input_params ON tool_execution TYPE string;
DEFINE FIELD OVERWRITE output_result ON tool_execution TYPE option<string>;
DEFINE FIELD OVERWRITE success ON tool_execution TYPE bool;
DEFINE FIELD OVERWRITE error_message ON tool_execution TYPE option<string>;
DEFINE FIELD OVERWRITE duration_ms ON tool_execution TYPE int;
DEFINE FIELD OVERWRITE iteration ON tool_execution TYPE int;
DEFINE FIELD OVERWRITE created_at ON tool_execution TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX OVERWRITE tool_exec_workflow_idx ON tool_execution FIELDS workflow_id;
DEFINE INDEX OVERWRITE tool_exec_message_idx ON tool_execution FIELDS message_id;
DEFINE INDEX OVERWRITE tool_exec_agent_idx ON tool_execution FIELDS agent_id;
DEFINE INDEX OVERWRITE tool_exec_type_idx ON tool_execution FIELDS tool_type;

-- =============================================
-- Table: thinking_step
-- Captures agent reasoning/thinking steps
-- Phase 4: Thinking Steps Persistence
-- =============================================
DEFINE TABLE OVERWRITE thinking_step SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON thinking_step TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON thinking_step TYPE string;
DEFINE FIELD OVERWRITE message_id ON thinking_step TYPE string;
DEFINE FIELD OVERWRITE agent_id ON thinking_step TYPE string;
DEFINE FIELD OVERWRITE step_number ON thinking_step TYPE int
    ASSERT $value >= 0;
DEFINE FIELD OVERWRITE content ON thinking_step TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 50000;
DEFINE FIELD OVERWRITE duration_ms ON thinking_step TYPE option<int>;
DEFINE FIELD OVERWRITE tokens ON thinking_step TYPE option<int>;
DEFINE FIELD OVERWRITE created_at ON thinking_step TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX OVERWRITE thinking_workflow_idx ON thinking_step FIELDS workflow_id;
DEFINE INDEX OVERWRITE thinking_message_idx ON thinking_step FIELDS message_id;
DEFINE INDEX OVERWRITE thinking_agent_idx ON thinking_step FIELDS agent_id;

-- =============================================
-- Table: sub_agent_execution
-- Tracks sub-agent spawn/delegate operations
-- Phase 6A: Sub-Agent System Infrastructure
-- =============================================
DEFINE TABLE OVERWRITE sub_agent_execution SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON sub_agent_execution TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON sub_agent_execution TYPE string;
DEFINE FIELD OVERWRITE parent_agent_id ON sub_agent_execution TYPE string;
DEFINE FIELD OVERWRITE sub_agent_id ON sub_agent_execution TYPE string;
DEFINE FIELD OVERWRITE sub_agent_name ON sub_agent_execution TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE task_description ON sub_agent_execution TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 10000;
DEFINE FIELD OVERWRITE status ON sub_agent_execution TYPE string
    ASSERT $value IN ['pending', 'running', 'completed', 'error', 'cancelled'];
DEFINE FIELD OVERWRITE duration_ms ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE tokens_input ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE tokens_output ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE result_summary ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE error_message ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON sub_agent_execution TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE completed_at ON sub_agent_execution TYPE option<datetime>;

-- Indexes for sub_agent_execution queries
DEFINE INDEX OVERWRITE sub_agent_workflow_idx ON sub_agent_execution FIELDS workflow_id;
DEFINE INDEX OVERWRITE sub_agent_parent_idx ON sub_agent_execution FIELDS parent_agent_id;
DEFINE INDEX OVERWRITE sub_agent_status_idx ON sub_agent_execution FIELDS status;

-- =============================================
-- Table: user_question
-- Stores user interaction questions for agent clarification
-- Phase 4: UserQuestionTool - Database Schema Setup
-- =============================================
DEFINE TABLE OVERWRITE user_question SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON user_question TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON user_question TYPE string;
DEFINE FIELD OVERWRITE agent_id ON user_question TYPE string;
DEFINE FIELD OVERWRITE question ON user_question TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 1000;
DEFINE FIELD OVERWRITE question_type ON user_question TYPE string
    ASSERT $value IN ['checkbox', 'text', 'mixed'];
DEFINE FIELD OVERWRITE options ON user_question TYPE string DEFAULT '[]';
DEFINE FIELD OVERWRITE text_placeholder ON user_question TYPE option<string>;
DEFINE FIELD OVERWRITE text_required ON user_question TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE context ON user_question TYPE option<string>;
DEFINE FIELD OVERWRITE status ON user_question TYPE string DEFAULT 'pending'
    ASSERT $value IN ['pending', 'answered', 'skipped'];
DEFINE FIELD OVERWRITE selected_options ON user_question TYPE string DEFAULT '[]';
DEFINE FIELD OVERWRITE text_response ON user_question TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON user_question TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE answered_at ON user_question TYPE option<datetime>;

-- Indexes for efficient querying
DEFINE INDEX OVERWRITE user_question_workflow_idx ON user_question FIELDS workflow_id;
DEFINE INDEX OVERWRITE user_question_status_idx ON user_question FIELDS status;
DEFINE INDEX OVERWRITE user_question_workflow_status_idx ON user_question FIELDS workflow_id, status;
"#;
