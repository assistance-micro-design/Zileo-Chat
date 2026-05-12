# API Reference - Tauri Commands

> Technical reference for Frontend-Backend IPC communication. **143 commands** across 23 modules.

## IPC Architecture

Frontend (`invoke()`) -> Tauri IPC (camelCase to snake_case auto-conversion) ->
Rust commands (`#[tauri::command] async fn -> Result<T, String>`) -> Backend services.

All commands are async on both sides. Frontend calls use `invoke()` from
`@tauri-apps/api/core`.

**IPC naming convention**: TypeScript uses camelCase parameter names, Tauri
automatically converts to snake_case for Rust. Example: `defaultModelId` (TS)
becomes `default_model_id` (Rust).

---

## Command Modules

For complete command signatures, see `src-tauri/src/commands/`.

### Workflow (`commands/workflow.rs`)

Workflow lifecycle management (create, load, rename, delete, batch ops, pinning, folders).

| Command | Description |
|---------|-------------|
| `create_workflow` | Create a new workflow with name and agent |
| `load_workflows` | List workflows with optional status filter |
| `rename_workflow` | Rename an existing workflow |
| `delete_workflow` | Delete workflow and associated data |
| `delete_workflows_batch` | Delete multiple workflows in a single operation |
| `load_workflow_full_state` | Load complete workflow state for recovery |
| `move_workflow_to_folder` | Move a single workflow to a folder |
| `move_workflows_to_folder` | Move multiple workflows to a folder |
| `toggle_workflow_pinned` | Toggle the pinned state of a workflow |

### Agent (`commands/agent/`)

Agent CRUD and configuration management.

| Command | Description |
|---------|-------------|
| `list_agents` | List all agents with summary (no system_prompt) |
| `get_agent_config` | Get full agent configuration |
| `create_agent` | Create agent with LLM, tools, skills, MCP servers config |
| `update_agent` | Partial update of agent configuration |
| `delete_agent` | Delete agent from DB and registry |

### Skill (`commands/skill.rs`)

Skill CRUD for reusable agent instructions.

| Command | Description |
|---------|-------------|
| `list_skills` | List all skills with summary |
| `get_skill` | Get full skill with content |
| `create_skill` | Create a new skill |
| `update_skill` | Partial update of skill |
| `delete_skill` | Delete a skill |

### LLM Models (`commands/llm_models/`)

Model CRUD (builtin + custom) and provider settings.

| Command | Description |
|---------|-------------|
| `list_models` | List models with optional provider filter |
| `get_model` | Get a single model by ID |
| `get_model_by_api_name` | Get a model by API name and provider |
| `create_model` | Create a custom model |
| `update_model` | Update model (builtin: temperature only) |
| `delete_model` | Delete a custom model (builtin protected) |
| `get_provider_settings` | Get provider configuration |
| `update_provider_settings` | Update provider configuration (upsert) |
| `test_provider_connection` | Test provider connectivity (10s timeout) |
| `seed_builtin_models` | Seed DB with builtin models (idempotent) |

### Custom Providers (`commands/custom_provider.rs`)

OpenAI-compatible provider management (OpenRouter, RouterLab, etc.).

| Command | Description |
|---------|-------------|
| `list_providers` | List all providers (builtin + custom) |
| `create_custom_provider` | Create OpenAI-compatible provider |
| `update_custom_provider` | Update custom provider settings |
| `delete_custom_provider` | Delete custom provider and its API key |

### Validation (`commands/validation.rs`)

Human-in-the-loop validation for agent operations.

| Command | Description |
|---------|-------------|
| `create_validation_request` | Create a validation request |
| `list_pending_validations` | List pending validation requests |
| `list_workflow_validations` | List validations for a workflow |
| `approve_validation` | Approve a validation request |
| `reject_validation` | Reject a validation request |
| `delete_validation` | Delete a validation request |
| `get_validation_settings` | Get global validation settings |
| `update_validation_settings` | Update validation mode and thresholds |
| `reset_validation_settings` | Reset validation settings to defaults |
| `list_available_tools` | List local + MCP tools for settings UI |

### Validation Audit (`commands/validation_audit.rs`)

Append-only audit log for validation decisions (decided_by user / auto / timeout).

| Command | Description |
|---------|-------------|
| `list_validation_audit` | List audit entries (paginated, filtered by tool, decision, risk, date range) |
| `get_validation_audit_stats` | Summary stats (decision breakdown, top tools, risk distribution) |
| `purge_validation_audit_now` | Manual cleanup honoring `retention_days` setting |
| `export_validation_audit_csv` | Export audit log to a CSV file |

### Memory (`commands/memory.rs`)

Vector memory with semantic search (multi-chunk: 1 parent `memory` row + N indexed `memory_chunk` rows since 2026-05-12).

| Command | Description |
|---------|-------------|
| `add_memory` | Writes 1 parent + N chunks via the UTF-8-safe recursive chunker (FN_RUST_019) |
| `search_memories` | Semantic search over `memory_chunk` with optional `tags_filter` (CONTAINSANY on parent tags). Returns `ChunkSearchResult` carrying both `chunk_id` and `parent_memory_id`. |
| `list_memories` | List memories (parents) with pagination and filters |
| `get_memory` | Get a single memory parent by ID |
| `delete_memory` | Delete a memory entry + cascade-delete its chunks (PAT_DB_007) |
| `clear_memories_by_type` | Clear all memories of a given type + cascade-delete their chunks |
| `purge_expired_memories` | On-demand purge of `context` memories whose `expires_at` is past (parent + chunks). Returns `{ memoriesPurged, chunksPurged }`. The same helper runs best-effort at boot via `AppState::new` (FN_RUST_020). |

### Embedding (`commands/embedding/`)

Embedding configuration, stats, and memory management tools.

| Command | Description |
|---------|-------------|
| `get_embedding_config` | Get current embedding configuration. Returns `Option<EmbeddingConfigSettings>` (`null` when no row) so `configExists` reflects reality. |
| `save_embedding_config` | Save embedding configuration |
| `delete_embedding_config` | Drop the config row and clear the in-memory embedding service |
| `reinit_embedding_service` | Reinitialize the embedding service |
| `test_embedding` | Test embedding generation with a sample |
| `get_memory_stats` | Get memory statistics for dashboard |
| `get_memory_token_stats` | Get token usage statistics for memories |
| `update_memory` | Update an existing memory entry |
| `export_memories` | Export memories to JSON/CSV |
| `import_memories` | Import memories from JSON |
| `reindex_memory_chunks` | Spawn a streaming reindex job (recursive chunker -> `memory_chunk` + embeddings). Emits `reindex-progress` events per processed parent. Optional `force` flag re-chunks everything. Returns `ReindexJobStatus { jobId, ... }`. |
| `cancel_reindex_job` | Cancel a running reindex job by `jobId` |
| `get_reindex_job_status` | Read current status for a `jobId` — auto-purges terminal entries on consultation; a background timer also sweeps after 10 minutes |

### Streaming (`commands/streaming/`)

Real-time workflow execution with event streaming.

| Command | Description |
|---------|-------------|
| `execute_workflow_streaming` | Execute workflow with real-time events |
| `cancel_workflow_streaming` | Cancel a running workflow |

### Message (`commands/message.rs`)

Chat message persistence and retrieval.

| Command | Description |
|---------|-------------|
| `save_message` | Persist a message to the database |
| `load_workflow_messages` | Load all messages for a workflow |
| `load_workflow_messages_paginated` | Load messages with pagination |
| `delete_message` | Delete a single message |
| `load_workflow_blocks` | Load all structured display blocks for a workflow grouped by message |

### Tool Execution (`commands/tool_execution.rs`)

Tool execution logging and retrieval.

| Command | Description |
|---------|-------------|
| `save_tool_execution` | Persist a tool execution log |
| `load_workflow_tool_executions` | Load all tool executions for a workflow |
| `load_message_tool_executions` | Load tool executions for a message |
| `get_tool_execution` | Get a single tool execution by ID |
| `delete_tool_execution` | Delete a single tool execution |
| `clear_workflow_tool_executions` | Delete all tool executions for a workflow |

### Thinking (`commands/thinking.rs`)

Thinking/reasoning step persistence.

| Command | Description |
|---------|-------------|
| `save_thinking_step` | Persist a thinking/reasoning step |
| `load_workflow_thinking_steps` | Load all thinking steps for a workflow |
| `load_message_thinking_steps` | Load thinking steps for a message |
| `delete_thinking_step` | Delete a single thinking step |
| `clear_workflow_thinking_steps` | Delete all thinking steps for a workflow |

### Task (`commands/task.rs`)

Task management for workflow decomposition (TodoTool).

| Command | Description |
|---------|-------------|
| `create_task` | Create task with priority (1-5) and dependencies |
| `get_task` | Get a single task by ID |
| `list_workflow_tasks` | List all tasks for a workflow |
| `list_tasks_by_status` | Filter tasks by status |
| `update_task` | Partial update of task fields |
| `update_task_status` | Update task status (convenience) |
| `complete_task` | Mark task completed with optional duration |
| `delete_task` | Delete a task |

### Sub-Agent Execution (`commands/sub_agent_execution.rs`)

Sub-agent execution tracking.

| Command | Description |
|---------|-------------|
| `load_workflow_sub_agent_executions` | Load sub-agent executions for a workflow |
| `clear_workflow_sub_agent_executions` | Delete all sub-agent executions for a workflow |

### MCP (`commands/mcp/`)

MCP server management and tool execution.

| Command | Description |
|---------|-------------|
| `list_mcp_servers` | List all configured MCP servers |
| `get_mcp_server` | Get a single MCP server by ID |
| `create_mcp_server` | Create MCP server configuration |
| `update_mcp_server` | Update MCP server configuration |
| `delete_mcp_server` | Delete MCP server configuration |
| `test_mcp_server` | Test MCP server connection |
| `start_mcp_server` | Start an MCP server |
| `stop_mcp_server` | Stop a running MCP server |
| `list_mcp_tools` | List available tools from a server |
| `call_mcp_tool` | Execute a tool on an MCP server |
| `get_mcp_latency_metrics` | Get latency percentiles (p50/p95/p99) |
| `list_mcp_legacy_http_auth` | Detect HTTP servers still using legacy `API_KEY`/`HEADER_*` env vars |

HTTP servers support auth methods Bearer, API Key, and Basic. Secrets are persisted in the OS keychain via `commands/security.rs`; only metadata fields (`auth_type`, `auth_metadata`, `extra_headers`) live in the database. See `src-tauri/src/mcp/http_auth.rs`.

### File Manager (`commands/file_manager.rs`)

Sandboxed filesystem operations and trash management.

| Command | Description |
|---------|-------------|
| `validate_agent_folder` | Validate and canonicalize a folder path |
| `list_trash` | List trash entries for an authorized folder |
| `restore_from_trash_cmd` | Restore a file from trash |

### User Question (`commands/user_question.rs`)

Human-in-the-loop questions from agents during execution.

| Command | Description |
|---------|-------------|
| `submit_user_response` | Submit answer to a pending question |
| `get_pending_questions` | Get pending questions for a workflow |
| `skip_question` | Skip a question (choose not to answer) |

Questions timeout after 5 minutes. Circuit breaker rejects new questions after
3 consecutive timeouts (60s cooldown).

### Prompt (`commands/prompt.rs`)

Prompt template CRUD.

| Command | Description |
|---------|-------------|
| `list_prompts` | List all prompt templates |
| `get_prompt` | Get a single prompt by ID |
| `create_prompt` | Create a new prompt template |
| `update_prompt` | Update an existing prompt |
| `delete_prompt` | Delete a prompt template |
| `search_prompts` | Search prompts by query and/or category |

### Security (`commands/security.rs`)

Secure API key storage (AES-256-GCM via SecureKeyStore).

| Command | Description |
|---------|-------------|
| `save_api_key` | Securely store an API key |
| `get_api_key` | Retrieve a stored API key |
| `delete_api_key` | Remove a stored API key |
| `has_api_key` | Check if an API key exists for a provider |
| `list_api_key_providers` | List all providers with stored API keys |

### Import/Export (`commands/import_export/`)

Configuration import/export (schema v1.0, v1.1, and v1.2).

| Command | Description |
|---------|-------------|
| `validate_import` | Validate import data and return preview with warnings |
| `execute_import` | Execute import with conflict resolutions |
| `prepare_export_preview` | Prepare export preview with entity selection |
| `generate_export_file` | Generate export JSON from selection |
| `save_export_to_file` | Save export content to a file path |

### Migration (`commands/migration.rs`)

Database schema migrations (idempotent with migration guards).

| Command | Description |
|---------|-------------|
| `migrate_memory_schema` | Migrate memory table for vector search |
| `get_memory_schema_status` | Get memory schema migration status |
| `migrate_mcp_http_schema` | Migrate MCP schema for HTTP support |
| `migrate_mcp_auth_v1` | Migrate MCP schema for HTTP auth fields (auth_type, auth_metadata, extra_headers) |
| `migrate_memory_v2_schema` | Migrate memory table for v2 (importance + TTL) |
| `migrate_reasoning_effort` | Migrate agent enable_thinking to reasoning_effort |
| `migrate_sidebar_features` | Migrate sidebar features (folders, pinning) |
| `migrate_token_cost_accuracy_v1` | Backfill `sub_agent_cost_usd`, `total_cached_tokens`, `total_cache_write_tokens` defaults on legacy workflow rows (auto-applied at startup) |

### Workflow Folder (`commands/workflow_folder.rs`)

Workflow organization into folders with color coding and custom ordering.

| Command | Description |
|---------|-------------|
| `create_workflow_folder` | Create a new workflow folder |
| `list_workflow_folders` | List all workflow folders |
| `rename_workflow_folder` | Rename an existing folder |
| `update_folder_color` | Update a folder's color |
| `delete_workflow_folder` | Delete a folder (workflows moved to root) |
| `reorder_workflow_folders` | Reorder folders by position |

---

## Key Types

All TypeScript types are in `src/types/` (aliased as `$types`).
Rust models are in `src-tauri/src/models/`.
Types are manually synchronized between frontend and backend.

### Core Domain Types

| Type | Location | Description |
|------|----------|-------------|
| `Workflow` | `$types/workflow` | Workflow with status, agent, timestamps |
| `AgentConfig` | `$types/agent` | Full agent config (LLM, tools, skills, MCP, folders) |
| `AgentSummary` | `$types/agent` | Lightweight agent summary (no system_prompt) |
| `Skill` / `SkillSummary` | `$types/skill` | Skill with content / summary without |
| `LLMModel` | `$types/llm` | Model definition (builtin or custom) |
| `Memory` | `$types/memory` | Parent memory entry with type, tags, content (no embedding — moved to MemoryChunk) |
| `ChunkSearchResult` | `$types/memory` | Search result (one row per chunk): `chunkId`, `parentMemoryId`, `chunkIndex`, `score`, plus parent fields surfaced via traversal |
| `ReindexJobStatus` | `$types/embedding` | Streaming reindex job state (`jobId`, `state`, `processed`, `total`, `errorMessage?`) |
| `Task` | `$types/workflow` | Task with priority, status, dependencies |
| `Prompt` | `$types/prompt` | Prompt template with category |

### Provider Types

| Type | Location | Description |
|------|----------|-------------|
| `ProviderSettings` | `$types/llm` | Provider config (enabled, base URL — `default_model_id` removed in PR #145) |
| `ProviderInfo` | `$types/custom-provider` | Unified provider info (builtin + custom) |
| `ConnectionTestResult` | `$types/llm` | Provider connectivity test result |

### Streaming and Events

| Type | Location | Description |
|------|----------|-------------|
| `StreamChunk` | `$types/streaming` | Real-time streaming event payload |
| `ChatBlock` | `$types/chat-block` | Structured display block (tool, thinking, sub-agent, task) |
| `UserQuestion` | `$types/user-question` | Agent question to user with options |
| `ValidationRequest` | `$types/validation` | Human-in-the-loop validation request |

### MCP Types

| Type | Location | Description |
|------|----------|-------------|
| `MCPServer` | `$types/mcp` | MCP server config and status |
| `MCPServerConfig` | `$types/mcp` | Server config (transport + HTTP auth metadata) |
| `MCPServerConfigWithSecret` | `$types/mcp` | Config payload that carries `authSecret` for create/update only |
| `MCPAuthType` | `$types/mcp` | Union: `'none' \| 'bearer' \| 'apikey' \| 'basic'` |
| `MCPAuthMetadata` | `$types/mcp` | Non-sensitive auth metadata (header name, username) |
| `MCPAuthSecret` | `$types/mcp` | Secret payload (token/value/password); never returned by read commands |
| `LegacyHttpAuthWarning` | `$types/mcp` | HTTP servers still using legacy env vars |
| `MCPLatencyMetrics` | `$types/mcp` | Latency percentiles (p50/p95/p99) |
| `AvailableToolInfo` | `$types/tool` | Tool info (local or MCP source) |

### Import/Export Types

| Type | Location | Description |
|------|----------|-------------|
| `ExportConfig` | `$types/import-export` | Exported configuration (schema v1.0/v1.1/v1.2) |
| `ImportResult` | `$types/import-export` | Import result with warnings and post-actions |
| `ImportWarning` | `$types/import-export` | Structured warning (type, severity, entity, action) |

---

## Events (Backend to Frontend)

Events are emitted via Tauri's event system. Listen with `listen()` from
`@tauri-apps/api/event`.

### `workflow_stream`

Real-time streaming during workflow execution. Chunk types (see
`ChunkType` enum in `src-tauri/src/models/streaming.rs`):
`tool_start`, `tool_end`, `tool_call_complete`, `reasoning`,
`thinking_block`, `response_block`, `sub_agent_start`,
`sub_agent_progress`, `sub_agent_complete`, `sub_agent_error`,
`task_create`, `task_update`, `task_complete`, `user_question_start`,
`user_question_complete`, `error`.

### `workflow_complete`

Emitted when workflow execution finishes. Payload: `{ workflow_id, status }`.

### `agent_status_update`

Agent availability changes. Payload: `{ agent_id, status }` where status is
`'available'` or `'busy'`.

### `validation_required`

Human-in-the-loop validation request for sub-agent operations. Payload includes
`validation_id`, `operation_type`, `risk_level`, and `details`.

### `reindex-progress`

Per-parent progress for the streaming `reindex_memory_chunks` job. Payload:
`{ jobId, state: 'running' | 'done' | 'cancelled' | 'error', processed, total, errorMessage? }`. Filter listeners by `jobId` — the frontend stores the running `jobId` in `LocalStorage` so a navigation/reload can resume the progress UI and surface a retroactive toast on remount.

---

## Error Handling

### Frontend Pattern

See `$lib/utils/error.ts` for `getErrorMessage()`. All `invoke()` calls should
be wrapped in try/catch, extracting user-friendly messages via `getErrorMessage(e)`.

### Backend Pattern

All Tauri commands return `Result<T, String>`. Errors are formatted as
user-friendly messages with `.map_err(|e| format!("Failed to ...: {}", e))?`.

### Input Validation

All commands validate inputs using `crate::security::Validator` before
processing. UUID fields are validated with `validate_uuid_field()`, user text
with `Validator::validate_workflow_name()` / `Validator::validate_message()`.

---

## References

- **Tauri IPC**: https://v2.tauri.app/develop/calling-rust/
- **Tauri Events**: https://v2.tauri.app/develop/inter-process-communication/
- **Command source**: `src-tauri/src/commands/`
- **TypeScript types**: `src/types/` (alias `$types`)
- **Rust models**: `src-tauri/src/models/`
- **Error handling**: See `ARCHITECTURE_DECISIONS.md`
