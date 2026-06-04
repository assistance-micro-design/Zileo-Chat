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
-- UI language the workflow ran in (e.g. "fr", "en"), stamped at execution.
-- Read by the Kanban auto-analyze so the verdict is produced in the same
-- language without a frontend round-trip.
DEFINE FIELD OVERWRITE locale ON workflow TYPE option<string>;
-- Cumulative token tracking (Token Display Complet feature)
DEFINE FIELD OVERWRITE total_tokens_input ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_tokens_output ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_cost_usd ON workflow TYPE float DEFAULT 0.0;
DEFINE FIELD OVERWRITE model_id ON workflow TYPE option<string>;
-- Current context size (last API call context window usage)
DEFINE FIELD OVERWRITE current_context_tokens ON workflow TYPE int DEFAULT 0;
-- Sub-agent token tracking (separate from main agent totals)
DEFINE FIELD OVERWRITE sub_agent_tokens_input ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE sub_agent_tokens_output ON workflow TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_cached_tokens ON workflow TYPE option<int> DEFAULT 0;
DEFINE FIELD OVERWRITE total_cache_write_tokens ON workflow TYPE option<int> DEFAULT 0;
-- Cumulative sub-agent cost (computed per sub-agent with its own pricing).
DEFINE FIELD OVERWRITE sub_agent_cost_usd ON workflow TYPE float DEFAULT 0.0;
-- Hides the workflow from the /agent sidebar listing (SELECT_LIST filters on it).
-- Used by the Kanban card review chat so the per-card conversation never leaks
-- into the workflow picker. Read-only WHERE filter — never projected into the
-- Workflow struct, so no Rust-struct sync surface.
DEFINE FIELD OVERWRITE hidden_from_list ON workflow TYPE bool DEFAULT false;

-- Table: message
-- Extended with metrics fields for persistence
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
DEFINE FIELD OVERWRITE thinking_tokens ON message TYPE option<int> DEFAULT NONE;
DEFINE FIELD OVERWRITE cached_tokens ON message TYPE option<int> DEFAULT NONE;
DEFINE FIELD OVERWRITE cache_write_tokens ON message TYPE option<int> DEFAULT NONE;
-- model_id_used: persist the exact model that produced this response so
-- cross-workflow pricing restoration survives agent reconfiguration.
DEFINE FIELD OVERWRITE model_id_used ON message TYPE option<string> DEFAULT NONE;
-- Optional multimodal attachments (images for vision). Sub-fields are declared
-- explicitly because SCHEMAFULL would otherwise drop dynamic keys (ERR_SURREAL_001).
DEFINE FIELD OVERWRITE attachments ON message TYPE option<array<object>> DEFAULT NONE;
DEFINE FIELD OVERWRITE attachments[*].kind ON message TYPE string;
DEFINE FIELD OVERWRITE attachments[*].mime_type ON message TYPE string;
DEFINE FIELD OVERWRITE attachments[*].data_base64 ON message TYPE string;
DEFINE FIELD OVERWRITE attachments[*].name ON message TYPE option<string>;
DEFINE FIELD OVERWRITE attachments[*].size_bytes ON message TYPE option<int>;
DEFINE FIELD OVERWRITE timestamp ON message TYPE datetime DEFAULT time::now();

-- =============================================
-- Index Review: Write-Heavy Table Analysis
-- =============================================
-- message is a write-heavy table (every LLM response creates a record)
-- Index trade-off: faster reads vs slower writes
-- Keep both indexes as they are actively used:
--   - message_workflow_idx: Required for loading conversation history
--   - message_timestamp_idx: Required for chronological message display in UI
DEFINE INDEX OVERWRITE message_workflow_idx ON message FIELDS workflow_id;
DEFINE INDEX OVERWRITE message_timestamp_idx ON message FIELDS timestamp;

-- Table: memory (parent unit, no embedding — embeddings live in memory_chunk)
DEFINE TABLE OVERWRITE memory SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON memory TYPE string;
DEFINE FIELD OVERWRITE type ON memory TYPE string ASSERT $value IN ['user_pref', 'context', 'knowledge', 'decision'];
DEFINE FIELD OVERWRITE content ON memory TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON memory TYPE option<string>;
DEFINE FIELD OVERWRITE metadata ON memory TYPE object;
-- Explicit metadata sub-fields (required for SCHEMAFULL to persist dynamic keys)
DEFINE FIELD OVERWRITE metadata.tags ON memory TYPE option<array<string>>;
DEFINE FIELD OVERWRITE metadata.priority ON memory TYPE option<float>;
DEFINE FIELD OVERWRITE metadata.agent_source ON memory TYPE option<string>;
DEFINE FIELD OVERWRITE importance ON memory TYPE float DEFAULT 0.5;
DEFINE FIELD OVERWRITE expires_at ON memory TYPE option<datetime>;
DEFINE FIELD OVERWRITE created_at ON memory TYPE datetime DEFAULT time::now();

-- Drop legacy embedding column + HNSW index on `memory` (memory_chunk_v1 refactor).
-- Pattern PAT_DB_006: inline REMOVE IF EXISTS is replay-safe on every boot.
REMOVE INDEX IF EXISTS memory_vec_idx ON TABLE memory;
REMOVE FIELD IF EXISTS embedding ON TABLE memory;

-- Index for workflow scoping
DEFINE INDEX OVERWRITE memory_workflow_idx ON memory FIELDS workflow_id;
-- Composite index for search_memories() with type + workflow_id
DEFINE INDEX OVERWRITE memory_type_workflow_idx ON memory FIELDS type, workflow_id;
-- Composite index for TTL cleanup preparation (type + created_at)
DEFINE INDEX OVERWRITE memory_type_created_idx ON memory FIELDS type, created_at;

-- Table: memory_chunk (search-only unit, N chunks per parent memory)
-- Each chunk has its own vector embedding and links back to its parent memory
-- via a typed record link, so traversal `memory_id.field` is native SurrealDB.
DEFINE TABLE OVERWRITE memory_chunk SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON memory_chunk TYPE string;
DEFINE FIELD OVERWRITE memory_id ON memory_chunk TYPE record<memory>;
DEFINE FIELD OVERWRITE chunk_index ON memory_chunk TYPE int ASSERT $value >= 0;
DEFINE FIELD OVERWRITE chunk_count ON memory_chunk TYPE int ASSERT $value >= 1;
DEFINE FIELD OVERWRITE content ON memory_chunk TYPE string;
DEFINE FIELD OVERWRITE embedding ON memory_chunk TYPE option<array<float>>;
DEFINE FIELD OVERWRITE created_at ON memory_chunk TYPE datetime DEFAULT time::now();

-- IF NOT EXISTS (not OVERWRITE): rebuilding this HNSW vector index re-reads every
-- stored chunk embedding and rebuilds the whole graph on each boot (~10s once the
-- store is populated). The embeddings are already persisted, so the index is built
-- once and kept across restarts. To change its definition (dimension / distance),
-- ship a guarded `REMOVE INDEX IF EXISTS ...; DEFINE INDEX ...` migration instead of
-- editing this line, because IF NOT EXISTS will not re-apply a changed body.
DEFINE INDEX IF NOT EXISTS memory_chunk_vec_idx ON memory_chunk FIELDS embedding HNSW DIMENSION 1024 DIST COSINE;
DEFINE INDEX OVERWRITE memory_chunk_parent_idx ON memory_chunk FIELDS memory_id;

-- Table: validation_request
DEFINE TABLE OVERWRITE validation_request SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON validation_request TYPE string;
DEFINE FIELD OVERWRITE workflow_id ON validation_request TYPE string;
DEFINE FIELD OVERWRITE type ON validation_request TYPE string ASSERT $value IN ['tool', 'sub_agent', 'mcp', 'file_op', 'db_op', 'manager_write'];
DEFINE FIELD OVERWRITE operation ON validation_request TYPE string;
DEFINE FIELD OVERWRITE details ON validation_request TYPE string DEFAULT '{}'; -- JSON string (ERR_SURREAL_001: TYPE object drops dynamic keys)
DEFINE FIELD OVERWRITE risk_level ON validation_request TYPE string ASSERT $value IN ['low', 'medium', 'high', 'critical'];
DEFINE FIELD OVERWRITE status ON validation_request TYPE string DEFAULT 'pending' ASSERT $value IN ['pending', 'approved', 'rejected'];
DEFINE FIELD OVERWRITE created_at ON validation_request TYPE datetime DEFAULT time::now();

-- =============================================
-- Table: validation_audit
-- Append-only log of validation decisions for traceability/audit.
-- One row per terminal decision (approve / reject / skip / timeout).
-- Lazy cleanup honors audit.retention_days.
-- =============================================
DEFINE TABLE OVERWRITE validation_audit SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON validation_audit TYPE string;
DEFINE FIELD OVERWRITE validation_id ON validation_audit TYPE string;
DEFINE FIELD OVERWRITE tool_name ON validation_audit TYPE string;
DEFINE FIELD OVERWRITE decision ON validation_audit TYPE string
    ASSERT $value IN ['approved', 'rejected', 'skipped', 'timeout', 'blocked'];
DEFINE FIELD OVERWRITE decided_at ON validation_audit TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE decided_by ON validation_audit TYPE string
    ASSERT $value IN ['user', 'auto', 'timeout', 'policy', 'pre_approved'];
DEFINE FIELD OVERWRITE risk_level ON validation_audit TYPE string
    ASSERT $value IN ['low', 'medium', 'high', 'critical'];
DEFINE FIELD OVERWRITE workflow_id ON validation_audit TYPE option<string>;
DEFINE FIELD OVERWRITE agent_id ON validation_audit TYPE option<string>;
DEFINE FIELD OVERWRITE prompt_preview ON validation_audit TYPE option<string>;
-- Free-form metadata (rejection reason, etc.) stored as JSON string per ERR_SURREAL_001.
DEFINE FIELD OVERWRITE metadata ON validation_audit TYPE string DEFAULT '{}';

DEFINE INDEX OVERWRITE audit_decided_at_idx ON validation_audit FIELDS decided_at;
DEFINE INDEX OVERWRITE audit_validation_id_idx ON validation_audit FIELDS validation_id UNIQUE;
DEFINE INDEX OVERWRITE audit_tool_name_idx ON validation_audit FIELDS tool_name;
DEFINE INDEX OVERWRITE audit_decision_idx ON validation_audit FIELDS decision;

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
-- HTTP authentication (v1.2): metadata only; secrets live in the OS keychain.
-- All three are optional and stored as JSON strings (ERR_SURREAL_001) to keep
-- backward compatibility with existing rows.
DEFINE FIELD OVERWRITE auth_type ON mcp_server TYPE option<string>
    ASSERT $value IS NONE OR $value IN ['none', 'bearer', 'apikey', 'basic'];
DEFINE FIELD OVERWRITE auth_metadata ON mcp_server TYPE option<string>;
DEFINE FIELD OVERWRITE extra_headers ON mcp_server TYPE option<string>;
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
DEFINE FIELD OVERWRITE params ON mcp_call_log TYPE string DEFAULT '{}'; -- JSON string (ERR_SURREAL_001: TYPE object drops dynamic keys)
DEFINE FIELD OVERWRITE result ON mcp_call_log TYPE string DEFAULT '[]'; -- JSON string (was TYPE array | object)
DEFINE FIELD OVERWRITE success ON mcp_call_log TYPE bool;
DEFINE FIELD OVERWRITE duration_ms ON mcp_call_log TYPE int;
DEFINE FIELD OVERWRITE timestamp ON mcp_call_log TYPE datetime DEFAULT time::now();
-- =============================================
-- Index Review: Write-Heavy Table Analysis
-- =============================================
-- mcp_call_log is write-heavy (every MCP tool call creates a record)
-- Index trade-off: faster reads vs slower writes
-- Keep both indexes as they are actively used:
--   - mcp_call_workflow: Required for workflow-scoped MCP call history
--   - mcp_call_server: Required for latency metrics (get_mcp_latency_metrics)
DEFINE INDEX OVERWRITE mcp_call_workflow ON mcp_call_log FIELDS workflow_id;
DEFINE INDEX OVERWRITE mcp_call_server ON mcp_call_log FIELDS server_name;

-- =============================================
-- Table: llm_model
-- Stores LLM models (builtin + custom)
-- =============================================
DEFINE TABLE OVERWRITE llm_model SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON llm_model TYPE string;
DEFINE FIELD OVERWRITE provider ON llm_model TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
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
-- Multimodal vision capability flag (manual user toggle in ModelForm).
DEFINE FIELD OVERWRITE supports_vision ON llm_model TYPE bool DEFAULT false;
-- Whether this model accepts a forced `tool_choice` (`required` / by-name).
-- DEFAULT true = current behaviour (most models accept it). Set false for
-- upstreams that reject it (deepseek-v4 via RouterLab returns HTTP 400). When
-- false, flows that force a tool on the opening turn fall back to Auto.
-- DEFAULT does not backfill existing rows (ERR_SURREAL_011); SELECTs read it
-- via `(supports_forced_tool_choice ?? true)`.
DEFINE FIELD OVERWRITE supports_forced_tool_choice ON llm_model TYPE bool DEFAULT true;
-- Pricing per million tokens (USD) - user configurable
DEFINE FIELD OVERWRITE input_price_per_mtok ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 1000.0
    DEFAULT 0.0;
DEFINE FIELD OVERWRITE output_price_per_mtok ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 1000.0
    DEFAULT 0.0;
DEFINE FIELD OVERWRITE cache_read_price_per_mtok ON llm_model TYPE float
    ASSERT $value >= 0.0 AND $value <= 1000.0
    DEFAULT 0.0;
DEFINE FIELD OVERWRITE cache_write_price_per_mtok ON llm_model TYPE float
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
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE enabled ON provider_settings TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE base_url ON provider_settings TYPE option<string>;
DEFINE FIELD OVERWRITE updated_at ON provider_settings TYPE datetime DEFAULT time::now();
-- Drop legacy decorative column on existing installs (refactor/cleanup-default-model-id).
REMOVE FIELD IF EXISTS default_model_id ON TABLE provider_settings;

DEFINE INDEX OVERWRITE unique_provider ON provider_settings FIELDS provider UNIQUE;

-- =============================================
-- Table: custom_provider
-- Stores user-created OpenAI-compatible providers
-- =============================================
DEFINE TABLE OVERWRITE custom_provider SCHEMAFULL;
DEFINE FIELD OVERWRITE name ON custom_provider TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE display_name ON custom_provider TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE base_url ON custom_provider TYPE string
    ASSERT string::len($value) > 0 AND string::len($value) <= 512;
DEFINE FIELD OVERWRITE enabled ON custom_provider TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE created_at ON custom_provider TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON custom_provider TYPE datetime DEFAULT time::now();
-- Strict-mode toggles for OpenAI-compat providers that reject Anthropic-style
-- `cache_control` (Fireworks, Groq, Together, Cerebras) or the OpenRouter-style
-- top-level `reasoning` object. NONE preserves the OpenRouter/RouterLab default
-- so existing rows continue to behave unchanged (no backfill needed).
DEFINE FIELD OVERWRITE supports_cache_control ON custom_provider TYPE option<bool>;
DEFINE FIELD OVERWRITE supports_reasoning_param ON custom_provider TYPE option<bool>;

DEFINE INDEX OVERWRITE unique_custom_provider_name ON custom_provider FIELDS name UNIQUE;

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
    ASSERT string::len($value) > 0 AND string::len($value) <= 64;
DEFINE FIELD OVERWRITE llm.model ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE llm.temperature ON agent TYPE float
    ASSERT $value >= 0.0 AND $value <= 2.0;
DEFINE FIELD OVERWRITE llm.max_tokens ON agent TYPE int
    ASSERT $value >= 256 AND $value <= 128000;

-- Tools, MCP servers, Skills, and Folders
DEFINE FIELD OVERWRITE tools ON agent TYPE array<string>;
DEFINE FIELD OVERWRITE mcp_servers ON agent TYPE array<string>;
DEFINE FIELD OVERWRITE skills ON agent TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE folders ON agent TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE require_file_confirmation ON agent TYPE bool DEFAULT true;

-- System prompt
DEFINE FIELD OVERWRITE system_prompt ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 10000;

-- Max tool iterations (1-200, default: 50)
DEFINE FIELD OVERWRITE max_tool_iterations ON agent TYPE int
    ASSERT $value >= 1 AND $value <= 200
    DEFAULT 50;

-- Reasoning effort for thinking models (null = disabled)
DEFINE FIELD OVERWRITE reasoning_effort ON agent TYPE option<string>
    ASSERT $value IS NONE OR $value IN ['low', 'medium', 'high', 'xhigh']
    DEFAULT NONE;

-- Timestamps
DEFINE FIELD OVERWRITE created_at ON agent TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON agent TYPE datetime DEFAULT time::now();

-- Indexes
DEFINE INDEX OVERWRITE unique_agent_id ON agent FIELDS id UNIQUE;
DEFINE INDEX OVERWRITE agent_name_idx ON agent FIELDS name UNIQUE;
DEFINE INDEX OVERWRITE agent_provider_idx ON agent FIELDS llm.provider;

-- =============================================
-- Table: tool_execution
-- Logs all tool executions (local + MCP)
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
DEFINE FIELD OVERWRITE sequence ON tool_execution TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE created_at ON tool_execution TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX OVERWRITE tool_exec_workflow_idx ON tool_execution FIELDS workflow_id;
DEFINE INDEX OVERWRITE tool_exec_message_idx ON tool_execution FIELDS message_id;
DEFINE INDEX OVERWRITE tool_exec_agent_idx ON tool_execution FIELDS agent_id;
DEFINE INDEX OVERWRITE tool_exec_type_idx ON tool_execution FIELDS tool_type;

-- =============================================
-- Table: thinking_step
-- Captures agent reasoning/thinking steps
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
DEFINE FIELD OVERWRITE sequence ON thinking_step TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE source ON thinking_step TYPE string DEFAULT 'agent_flow'
    ASSERT $value IN ['agent_flow', 'model_thinking'];
DEFINE FIELD OVERWRITE created_at ON thinking_step TYPE datetime DEFAULT time::now();

-- Indexes for efficient querying
DEFINE INDEX OVERWRITE thinking_workflow_idx ON thinking_step FIELDS workflow_id;
DEFINE INDEX OVERWRITE thinking_message_idx ON thinking_step FIELDS message_id;
DEFINE INDEX OVERWRITE thinking_agent_idx ON thinking_step FIELDS agent_id;

-- =============================================
-- Table: sub_agent_execution
-- Tracks sub-agent spawn/delegate operations
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
-- Per-sub-agent cost computed with the sub-agent's OWN pricing, not the
-- parent's. Aggregated into workflow.sub_agent_cost_usd.
DEFINE FIELD OVERWRITE cost_usd ON sub_agent_execution TYPE option<float>;
DEFINE FIELD OVERWRITE cached_tokens ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE cache_write_tokens ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE thinking_tokens ON sub_agent_execution TYPE option<int>;
DEFINE FIELD OVERWRITE result_summary ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE error_message ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE parent_execution_id ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE parent_message_id ON sub_agent_execution TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON sub_agent_execution TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE completed_at ON sub_agent_execution TYPE option<datetime>;

-- Indexes for sub_agent_execution queries
DEFINE INDEX OVERWRITE sub_agent_workflow_idx ON sub_agent_execution FIELDS workflow_id;
DEFINE INDEX OVERWRITE sub_agent_parent_idx ON sub_agent_execution FIELDS parent_agent_id;
DEFINE INDEX OVERWRITE sub_agent_status_idx ON sub_agent_execution FIELDS status;
DEFINE INDEX OVERWRITE sub_agent_message_idx ON sub_agent_execution FIELDS parent_message_id;

-- =============================================
-- Table: user_question
-- Stores user interaction questions for agent clarification
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

-- =============================================
-- Table: skill
-- Stores reusable skill definitions (markdown instructions for agents)
-- =============================================
DEFINE TABLE OVERWRITE skill SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON skill TYPE string;
DEFINE FIELD OVERWRITE name ON skill TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128
    AND $value = /^[a-zA-Z0-9_-]+$/;
DEFINE FIELD OVERWRITE description ON skill TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 500;
DEFINE FIELD OVERWRITE category ON skill TYPE string
    ASSERT $value IN ['system', 'coding', 'workflow', 'analysis', 'custom'];
DEFINE FIELD OVERWRITE content ON skill TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 50000;
DEFINE FIELD OVERWRITE enabled ON skill TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE kind ON skill TYPE option<string>
    ASSERT $value = NONE OR $value IN ['kanban'];
DEFINE FIELD OVERWRITE created_at ON skill TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON skill TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE unique_skill_id ON skill FIELDS id UNIQUE;
DEFINE INDEX OVERWRITE unique_skill_name ON skill FIELDS name UNIQUE;
DEFINE INDEX OVERWRITE skill_category_idx ON skill FIELDS category;
DEFINE INDEX OVERWRITE skill_enabled_idx ON skill FIELDS enabled;
DEFINE INDEX OVERWRITE skill_kind_idx ON skill FIELDS kind;

-- =============================================
-- Table: migration_log
-- Tracks applied database migrations to prevent re-execution
-- Migration guard for embedding-destructive operations
-- =============================================
DEFINE TABLE OVERWRITE migration_log SCHEMAFULL;
DEFINE FIELD OVERWRITE name ON migration_log TYPE string;
DEFINE FIELD OVERWRITE applied_at ON migration_log TYPE datetime DEFAULT time::now();
DEFINE INDEX OVERWRITE unique_migration_name ON migration_log FIELDS name UNIQUE;

-- =============================================
-- Table: workflow_folder
-- Organizes workflows into named folders
-- =============================================
DEFINE TABLE OVERWRITE workflow_folder SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON workflow_folder TYPE string;
DEFINE FIELD OVERWRITE name ON workflow_folder TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 128;
DEFINE FIELD OVERWRITE color ON workflow_folder TYPE string
    ASSERT $value = /^#[0-9a-fA-F]{6}$/;
DEFINE FIELD OVERWRITE sort_order ON workflow_folder TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE created_at ON workflow_folder TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON workflow_folder TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE unique_folder_id ON workflow_folder FIELDS id UNIQUE;

-- Workflow extensions for folders and pinning
DEFINE FIELD OVERWRITE folder_id ON workflow TYPE option<string>;
DEFINE FIELD OVERWRITE pinned ON workflow TYPE bool DEFAULT false;

-- Backfill pinned field on existing workflows (DEFAULT only applies to new records)
UPDATE workflow SET pinned = false WHERE pinned IS NONE;

-- =============================================
-- Agent extensions for Kanban feature (additive, backward-compatible)
-- =============================================
DEFINE FIELD OVERWRITE kind ON agent TYPE option<string>
    ASSERT $value IS NONE OR $value IN ['kanban'];
DEFINE FIELD OVERWRITE auto_analyze_reports ON agent TYPE bool DEFAULT false;

-- Per-agent MCP tool allowlist for unattended (detached) runs.
-- Nested object sub-keys are defined explicitly (SCHEMAFULL
-- drops dynamic sub-keys otherwise), mirroring the multimodal attachments[*].* pattern.
DEFINE FIELD OVERWRITE mcp_tool_allowlist ON agent TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE mcp_tool_allowlist[*].server_id ON agent TYPE string;
DEFINE FIELD OVERWRITE mcp_tool_allowlist[*].tools ON agent TYPE array<string> DEFAULT [];
DEFINE FIELD OVERWRITE mcp_tool_allowlist[*].allow_in_delegated_runs ON agent TYPE bool DEFAULT false;

-- Backfill mcp_tool_allowlist on existing agents (DEFAULT only applies to new
-- records). Without this, agents created before this column
-- existed keep it NONE: the SCHEMAFULL SELECT then returns `null` and the
-- startup load (main.rs) fails to deserialize AgentConfig and DROPS the agent,
-- so the whole agent list vanishes from the UI. Idempotent: re-running the DDL
-- matches nothing once the column is set. (No backfill needed on the [*].*
-- sub-fields — they live on array entries, materialised by the parent.)
UPDATE agent SET mcp_tool_allowlist = [] WHERE mcp_tool_allowlist IS NONE;

-- =============================================
-- Table: kanban_card
-- Cards representing a task to delegate to an agent workflow.
-- =============================================
DEFINE TABLE OVERWRITE kanban_card SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON kanban_card TYPE string;
DEFINE FIELD OVERWRITE title ON kanban_card TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 200;
DEFINE FIELD OVERWRITE description ON kanban_card TYPE string
    ASSERT string::len($value) <= 5000;
DEFINE FIELD OVERWRITE kanban_agent_id ON kanban_card TYPE string;
DEFINE FIELD OVERWRITE target_agent_id ON kanban_card TYPE string;
DEFINE FIELD OVERWRITE prompt_id ON kanban_card TYPE option<string>;
DEFINE FIELD OVERWRITE inline_prompt ON kanban_card TYPE option<string>;
-- Variables stored as JSON string (ERR_SURREAL_001 dynamic keys droppees sous SCHEMAFULL)
DEFINE FIELD OVERWRITE variables ON kanban_card TYPE string DEFAULT '{}';
DEFINE FIELD OVERWRITE target_folder_id ON kanban_card TYPE option<string>;
DEFINE FIELD OVERWRITE status ON kanban_card TYPE string
    ASSERT $value IN ['todo', 'ready', 'doing', 'review', 'done', 'failed', 'proposed'];
DEFINE FIELD OVERWRITE column ON kanban_card TYPE string
    ASSERT $value IN ['todo', 'doing', 'review', 'done'];
DEFINE FIELD OVERWRITE column_order ON kanban_card TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE workflow_id ON kanban_card TYPE option<string>;
DEFINE FIELD OVERWRITE error_summary ON kanban_card TYPE option<string>;
-- Workflow backing the in-place "review chat" with the Kanban agent. Distinct
-- from `workflow_id` (the worker run). Created hidden (workflow.hidden_from_list)
-- so the conversation never surfaces in the /agent sidebar.
DEFINE FIELD OVERWRITE review_chat_workflow_id ON kanban_card TYPE option<string>;
DEFINE FIELD OVERWRITE created_at ON kanban_card TYPE datetime DEFAULT time::now();
DEFINE FIELD OVERWRITE updated_at ON kanban_card TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE kanban_card_status_idx ON kanban_card FIELDS status;
DEFINE INDEX OVERWRITE kanban_card_column_idx ON kanban_card FIELDS column, column_order;
DEFINE INDEX OVERWRITE kanban_card_workflow_idx ON kanban_card FIELDS workflow_id;
DEFINE INDEX OVERWRITE kanban_card_review_chat_idx ON kanban_card FIELDS review_chat_workflow_id;
DEFINE INDEX OVERWRITE kanban_card_kanban_agent_idx ON kanban_card FIELDS kanban_agent_id;

-- =============================================
-- Table: kanban_schedule
-- Recurrence rules for kanban_card templates.
-- =============================================
DEFINE TABLE OVERWRITE kanban_schedule SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON kanban_schedule TYPE string;
DEFINE FIELD OVERWRITE card_template_id ON kanban_schedule TYPE string;
DEFINE FIELD OVERWRITE days_of_week ON kanban_schedule TYPE array<int>
    ASSERT array::all($value, |$v| $v >= 0 AND $v <= 6);
DEFINE FIELD OVERWRITE hour ON kanban_schedule TYPE int
    ASSERT $value >= 0 AND $value <= 23;
DEFINE FIELD OVERWRITE minute ON kanban_schedule TYPE int
    ASSERT $value >= 0 AND $value <= 59;
DEFINE FIELD OVERWRITE next_run_at ON kanban_schedule TYPE datetime;
DEFINE FIELD OVERWRITE last_run_at ON kanban_schedule TYPE option<datetime>;
DEFINE FIELD OVERWRITE enabled ON kanban_schedule TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE skip_if_pending ON kanban_schedule TYPE bool DEFAULT false;
DEFINE FIELD OVERWRITE created_at ON kanban_schedule TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE kanban_schedule_next_run_idx ON kanban_schedule FIELDS next_run_at, enabled;
DEFINE INDEX OVERWRITE kanban_schedule_card_idx ON kanban_schedule FIELDS card_template_id;

-- =============================================
-- Table: kanban_card_interaction
-- Persistance des appels LLM meta de l'agent Kanban (compose / analyze)
-- pour affichage historique dans KanbanCardReportViewer.
-- Une interaction = un tool_loop complet (N iterations + tool calls).
-- =============================================
DEFINE TABLE OVERWRITE kanban_card_interaction SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE card_id ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE kind ON kanban_card_interaction TYPE string
    ASSERT $value IN ['compose', 'analyze'];
DEFINE FIELD OVERWRITE kanban_agent_id ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE provider ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE model_id_used ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE task_input ON kanban_card_interaction TYPE string;
-- iterations : array d'objets (ERR_SURREAL_001 sous-champs explicites)
DEFINE FIELD OVERWRITE iterations ON kanban_card_interaction TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE iterations[*].iteration_index ON kanban_card_interaction TYPE int;
DEFINE FIELD OVERWRITE iterations[*].reasoning ON kanban_card_interaction TYPE option<string>;
DEFINE FIELD OVERWRITE iterations[*].response_content ON kanban_card_interaction TYPE option<string>;
DEFINE FIELD OVERWRITE iterations[*].tool_calls ON kanban_card_interaction TYPE array<object> DEFAULT [];
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].tool_name ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].mcp_server ON kanban_card_interaction TYPE option<string>;
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].input_json ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].output_json ON kanban_card_interaction TYPE string;
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].duration_ms ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE iterations[*].tool_calls[*].success ON kanban_card_interaction TYPE bool DEFAULT true;
DEFINE FIELD OVERWRITE iterations[*].tokens_input ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE iterations[*].tokens_output ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE iterations[*].cached_tokens ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE iterations[*].cost_usd ON kanban_card_interaction TYPE float DEFAULT 0.0;
DEFINE FIELD OVERWRITE iterations[*].duration_ms ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE final_payload_summary ON kanban_card_interaction TYPE option<string>;
DEFINE FIELD OVERWRITE final_response_text ON kanban_card_interaction TYPE option<string>;
DEFINE FIELD OVERWRITE total_tokens_input ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_tokens_output ON kanban_card_interaction TYPE int DEFAULT 0;
DEFINE FIELD OVERWRITE total_cost_usd ON kanban_card_interaction TYPE float DEFAULT 0.0;
DEFINE FIELD OVERWRITE created_at ON kanban_card_interaction TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE kanban_card_interaction_card_idx ON kanban_card_interaction FIELDS card_id;

-- =============================================
-- Table: prompt_version
-- Snapshot of a prompt taken AVANT toute modification (versionning anti-perte).
-- =============================================
DEFINE TABLE OVERWRITE prompt_version SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE prompt_id ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE version ON prompt_version TYPE int;
DEFINE FIELD OVERWRITE name ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE description ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE category ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE content ON prompt_version TYPE string;
-- Variables as JSON string (ERR_SURREAL_001 sur objets imbriques dans array)
DEFINE FIELD OVERWRITE variables_json ON prompt_version TYPE string DEFAULT '[]';
DEFINE FIELD OVERWRITE edited_by ON prompt_version TYPE string;
DEFINE FIELD OVERWRITE edit_summary ON prompt_version TYPE option<string>;
DEFINE FIELD OVERWRITE edited_at ON prompt_version TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE prompt_version_prompt_idx ON prompt_version FIELDS prompt_id, version;

-- =============================================
-- Table: skill_version
-- Snapshot of a skill taken AVANT toute modification.
-- =============================================
DEFINE TABLE OVERWRITE skill_version SCHEMAFULL;
DEFINE FIELD OVERWRITE id ON skill_version TYPE string;
DEFINE FIELD OVERWRITE skill_id ON skill_version TYPE string;
DEFINE FIELD OVERWRITE version ON skill_version TYPE int;
DEFINE FIELD OVERWRITE name ON skill_version TYPE string;
DEFINE FIELD OVERWRITE description ON skill_version TYPE string;
DEFINE FIELD OVERWRITE category ON skill_version TYPE string;
DEFINE FIELD OVERWRITE content ON skill_version TYPE string;
DEFINE FIELD OVERWRITE edited_by ON skill_version TYPE string;
DEFINE FIELD OVERWRITE edit_summary ON skill_version TYPE option<string>;
DEFINE FIELD OVERWRITE edited_at ON skill_version TYPE datetime DEFAULT time::now();

DEFINE INDEX OVERWRITE skill_version_skill_idx ON skill_version FIELDS skill_id, version;
"#;
