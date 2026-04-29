# Database Schema - SurrealDB

> **Version**: 1.4
> **SurrealDB**: ~2.6 (SCHEMAFULL)
> **Tables**: 19

## Design Notes

- All tables are `SCHEMAFULL` (strict field typing, unknown fields rejected).
- **JSON-string encoding pattern**: Dynamic objects (env vars, tool params, options) are stored as JSON strings (`TYPE string DEFAULT '{}'`) because SurrealDB SCHEMAFULL tables silently drop unknown nested keys on `TYPE object` fields (ERR_SURREAL_001).
- All `id` fields are `TYPE string` (UUID format, managed by application).
- `DEFINE FIELD OVERWRITE` is used everywhere for idempotent schema application.
- See `src-tauri/src/db/` for query implementations.

## Entity Relationship Overview

```
workflow ─────────────┐
                      ├──> message
                      ├──> task
                      ├──> validation_request ──> validation_audit (append-only)
                      ├──> user_question
                      ├──> memory (vector)
                      ├──> tool_execution
                      ├──> thinking_step
                      └──> sub_agent_execution

mcp_server ──────────> mcp_call_log
llm_model ───────────> provider_settings
custom_provider ─────> (linked via provider name)
skill (standalone)
workflow_folder ─────> workflow (grouping)
migration_log (schema versioning)
```

---

## Tables

### workflow

Workflow lifecycle with cumulative token tracking.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| name | string | | User-editable name |
| agent_id | string | | Primary agent |
| status | string ASSERT IN [idle, running, completed, error] | | Workflow state |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |
| completed_at | option\<datetime\> | | |
| total_tokens_input | int | 0 | Cumulative input tokens |
| total_tokens_output | int | 0 | Cumulative output tokens |
| total_cost_usd | float | 0.0 | Cumulative cost (USD) |
| model_id | option\<string\> | | Current model |
| current_context_tokens | int | 0 | Last API call context size |
| sub_agent_tokens_input | int | 0 | Sub-agent input tokens |
| sub_agent_tokens_output | int | 0 | Sub-agent output tokens |
| total_cached_tokens | option\<int\> | 0 | Prompt cache read tokens |
| total_cache_write_tokens | option\<int\> | 0 | Prompt cache write tokens |
| folder_id | option\<string\> | | Reference to workflow_folder |
| pinned | bool | false | Pinned in sidebar |

**Indexes**: (none explicitly defined beyond field-level constraints; queries filter on status, created_at, agent_id)

---

### message

Conversation messages (user, assistant, system) with per-message metrics.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| role | string ASSERT IN [user, assistant, system] | | Message role |
| content | string | | Message body |
| tokens | int | | Total tokens (legacy) |
| tokens_input | option\<int\> | | Input tokens |
| tokens_output | option\<int\> | | Output tokens |
| model | option\<string\> | | Model used |
| provider | option\<string\> | | Provider used |
| cost_usd | option\<float\> | | Cost (USD) |
| duration_ms | option\<int\> | | Response time |
| thinking_tokens | option\<int\> | NONE | Reasoning tokens |
| timestamp | datetime | time::now() | |

**Indexes**: `message_workflow_idx` (workflow_id), `message_timestamp_idx` (timestamp)

---

### memory

Vector storage for RAG and agent context. Supports auto-scoping, importance, and TTL.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| type | string ASSERT IN [user_pref, context, knowledge, decision] | | Memory category |
| content | string | | Indexed text |
| embedding | option\<array\<float\>\> | | Vector (768-3072D) |
| workflow_id | option\<string\> | | Scope (auto-set) |
| metadata | object | | Container for sub-fields |
| metadata.tags | option\<array\<string\>\> | | |
| metadata.priority | option\<float\> | | 0.0-1.0 |
| metadata.agent_source | option\<string\> | | |
| importance | float | 0.5 | Composite scoring weight |
| expires_at | option\<datetime\> | | TTL (auto 7d for context) |
| created_at | datetime | time::now() | |

**Indexes**: `memory_vec_idx` (embedding, HNSW 1024D COSINE), `memory_workflow_idx` (workflow_id), `memory_type_workflow_idx` (type, workflow_id), `memory_type_created_idx` (type, created_at)

---

### validation_request

Human-in-the-loop validation requests.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| type | string ASSERT IN [tool, sub_agent, mcp, file_op, db_op] | | Operation type |
| operation | string | | Operation description |
| details | string | '{}' | JSON string (dynamic params) |
| risk_level | string ASSERT IN [low, medium, high, critical] | | Risk assessment |
| status | string ASSERT IN [pending, approved, rejected] | 'pending' | |
| created_at | datetime | time::now() | |

**Indexes**: (none explicitly defined)

---

### validation_audit

Append-only audit log of validation decisions (user / auto / timeout). Resilient: write failures never block the validation flow. Retention is user-configurable (7-90 days, see `RETENTION_MIN_DAYS` / `RETENTION_MAX_DAYS` in `constants.rs`).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| validation_id | string | | Source `validation_request` id |
| tool_name | string | | Tool / operation name |
| decision | string ASSERT IN [approved, rejected, timeout] | | Final decision |
| decided_by | string ASSERT IN [user, auto, timeout] | | Decision source |
| decided_at | datetime | time::now() | Decision timestamp |
| risk_level | string ASSERT IN [low, medium, high, critical] | | Risk at decision time |
| workflow_id | option\<string\> | | Parent workflow |
| agent_id | option\<string\> | | Requesting agent |
| prompt_preview | option\<string\> | | Truncated request preview |
| metadata | string | '{}' | JSON string (extra context) |

**Indexes**: `audit_decided_at_idx` (decided_at), `audit_validation_id_idx` (validation_id), `audit_tool_name_idx` (tool_name), `audit_decision_idx` (decision)

---

### task

Decomposed workflow tasks with Todo Tool support.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| name | string (1-128 chars) | | Task name |
| description | string (max 1000 chars) | | Task details |
| agent_assigned | option\<string\> | | Responsible agent |
| priority | int (1-5) | 3 | 1=critical |
| status | string ASSERT IN [pending, in_progress, completed, blocked] | 'pending' | |
| dependencies | array\<string\> | | Task IDs (string, not UUID) |
| duration_ms | option\<int\> | | Elapsed time if completed |
| created_at | datetime | time::now() | |
| completed_at | option\<datetime\> | | |

**Indexes**: `task_workflow_idx` (workflow_id), `task_status_idx` (status), `task_priority_idx` (priority), `task_agent_idx` (agent_assigned)

---

### agent

User-created agent configurations.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| name | string (1-64 chars) | | Unique name |
| lifecycle | string ASSERT IN [permanent, temporary] | | |
| llm | object | | LLM configuration |
| llm.provider | string (1-64 chars) | | Provider name |
| llm.model | string (1-128 chars) | | Model identifier |
| llm.temperature | float (0.0-2.0) | | |
| llm.max_tokens | int (256-128000) | | |
| tools | array\<string\> | | Enabled tool names |
| mcp_servers | array\<string\> | | MCP server names |
| skills | array\<string\> | [] | Skill names |
| folders | array\<string\> | [] | FileManager authorized dirs |
| require_file_confirmation | bool | true | Confirm destructive file ops |
| system_prompt | string (1-10000 chars) | | |
| max_tool_iterations | int (1-200) | 50 | Tool loop limit |
| reasoning_effort | option\<string\> ASSERT IN [low, medium, high] | NONE | Thinking model effort |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_agent_id` (id, UNIQUE), `agent_name_idx` (name, UNIQUE), `agent_provider_idx` (llm.provider)

---

### skill

Reusable markdown instruction documents assignable to agents.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| name | string (1-128 chars, `^[a-zA-Z0-9_-]+$`) | | Unique slug |
| description | string (1-500 chars) | | Short description |
| category | string ASSERT IN [system, coding, workflow, analysis, custom] | | |
| content | string (1-50000 chars) | | Markdown instructions |
| enabled | bool | true | |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_skill_id` (id, UNIQUE), `unique_skill_name` (name, UNIQUE), `skill_category_idx` (category), `skill_enabled_idx` (enabled)

---

### mcp_server

MCP server configurations.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | Unique identifier |
| name | string | | User-friendly name |
| enabled | bool | true | |
| command | string ASSERT IN [docker, npx, uvx, http] | | Transport type |
| args | array\<string\> | | Command arguments |
| env | string | '{}' | JSON-encoded env vars |
| description | option\<string\> | | |
| auth_type | option\<string\> ASSERT IN [none, bearer, apikey, basic] | | HTTP auth method (HTTP transport only) |
| auth_metadata | option\<string\> | | JSON-encoded non-sensitive auth metadata (header name, username) |
| extra_headers | option\<string\> | | JSON-encoded additional HTTP headers |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_mcp_id` (id, UNIQUE), `unique_mcp_name` (name, UNIQUE)

**Secrets**: Bearer tokens, API key values, and Basic passwords are stored in the OS keychain under `mcp_auth_<server_id>`, never in the database. See `src-tauri/src/mcp/secrets.rs`.

---

### mcp_call_log

Audit log for MCP tool calls.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | option\<string\> | | Parent workflow |
| server_name | string | | MCP server name |
| tool_name | string | | Tool called |
| params | string | '{}' | JSON string (call params) |
| result | string | '[]' | JSON string (call result) |
| success | bool | | |
| duration_ms | int | | Response time |
| timestamp | datetime | time::now() | |

**Indexes**: `mcp_call_workflow` (workflow_id), `mcp_call_server` (server_name)

---

### llm_model

LLM model registry (builtin + custom).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID (custom) or api_name (builtin) |
| provider | string (1-64 chars) | | Provider name |
| name | string (1-64 chars) | | Human-readable name |
| api_name | string (1-128 chars) | | API model identifier |
| context_window | int (1024-2000000) | | Context size |
| max_output_tokens | int (256-128000) | | Max output |
| temperature_default | float (0.0-2.0) | 0.7 | |
| is_builtin | bool | false | |
| is_reasoning | bool | false | Thinking model |
| input_price_per_mtok | float (0.0-1000.0) | 0.0 | USD per million input tokens |
| output_price_per_mtok | float (0.0-1000.0) | 0.0 | USD per million output tokens |
| cache_read_price_per_mtok | float (0.0-1000.0) | 0.0 | USD per million cache-read tokens |
| cache_write_price_per_mtok | float (0.0-1000.0) | 0.0 | USD per million cache-write tokens |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_model_id` (id, UNIQUE), `model_provider_idx` (provider), `model_api_name_idx` (provider + api_name, UNIQUE)

---

### provider_settings

Per-provider LLM configuration.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| provider | string (1-64 chars) | | Provider name (UNIQUE key) |
| enabled | bool | true | |
| default_model_id | option\<string\> | | Default model |
| base_url | option\<string\> | | Custom API endpoint |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_provider` (provider, UNIQUE)

---

### custom_provider

User-created OpenAI-compatible provider metadata.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| name | string (1-64 chars) | | URL-safe slug (UNIQUE key) |
| display_name | string (1-128 chars) | | Human-readable name |
| base_url | string (1-512 chars) | | API endpoint |
| enabled | bool | true | |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_custom_provider_name` (name, UNIQUE)

API keys are stored in SecureKeyStore (OS keyring), never in the database.

---

### tool_execution

Persisted tool execution log (local + MCP).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| message_id | string | | Parent message |
| agent_id | string | | Executing agent |
| tool_type | string ASSERT IN [local, mcp] | | Tool origin |
| tool_name | string (1-128 chars) | | Tool name |
| server_name | option\<string\> | | MCP server (if mcp) |
| input_params | string | | JSON string (tool input) |
| output_result | option\<string\> | | JSON string (tool output) |
| success | bool | | |
| error_message | option\<string\> | | Error details |
| duration_ms | int | | Execution time |
| iteration | int | | Tool loop iteration |
| sequence | int | 0 | Order within iteration |
| created_at | datetime | time::now() | |

**Indexes**: `tool_exec_workflow_idx` (workflow_id), `tool_exec_message_idx` (message_id), `tool_exec_agent_idx` (agent_id), `tool_exec_type_idx` (tool_type)

---

### thinking_step

Agent reasoning/thinking steps (chain-of-thought).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| message_id | string | | Parent message |
| agent_id | string | | Thinking agent |
| step_number | int (>= 0) | | Step index |
| content | string (1-50000 chars) | | Thinking content |
| duration_ms | option\<int\> | | Step duration |
| tokens | option\<int\> | | Token count |
| sequence | int | 0 | Order within message |
| source | string ASSERT IN [agent_flow, model_thinking] | 'agent_flow' | Origin of thinking |
| created_at | datetime | time::now() | |

**Indexes**: `thinking_workflow_idx` (workflow_id), `thinking_message_idx` (message_id), `thinking_agent_idx` (agent_id)

---

### sub_agent_execution

Sub-agent spawn/delegate execution history.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| parent_agent_id | string | | Delegating agent |
| sub_agent_id | string | | Spawned agent |
| sub_agent_name | string (1-128 chars) | | Agent display name |
| task_description | string (1-10000 chars) | | Delegated task |
| status | string ASSERT IN [pending, running, completed, error, cancelled] | | |
| duration_ms | option\<int\> | | Execution time |
| tokens_input | option\<int\> | | Input tokens used |
| tokens_output | option\<int\> | | Output tokens used |
| result_summary | option\<string\> | | Completion summary |
| error_message | option\<string\> | | Error details |
| parent_execution_id | option\<string\> | | Parent execution (nesting) |
| created_at | datetime | time::now() | |
| completed_at | option\<datetime\> | | |

**Indexes**: `sub_agent_workflow_idx` (workflow_id), `sub_agent_parent_idx` (parent_agent_id), `sub_agent_status_idx` (status)

---

### user_question

Agent-to-user interactive questions (human-in-the-loop).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| agent_id | string | | Asking agent |
| question | string (1-1000 chars) | | Question text |
| question_type | string ASSERT IN [checkbox, text, mixed] | | Input mode |
| options | string | '[]' | JSON string (checkbox options) |
| text_placeholder | option\<string\> | | Placeholder for text input |
| text_required | bool | false | |
| context | option\<string\> | | Additional context |
| status | string ASSERT IN [pending, answered, skipped] | 'pending' | |
| selected_options | string | '[]' | JSON string (selected IDs) |
| text_response | option\<string\> | | User text answer |
| created_at | datetime | time::now() | |
| answered_at | option\<datetime\> | | |

**Indexes**: `user_question_workflow_idx` (workflow_id), `user_question_status_idx` (status), `user_question_workflow_status_idx` (workflow_id + status)

---

### workflow_folder

Sidebar folder grouping for workflows.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| name | string (1-128 chars) | | Folder name |
| color | string (`^#[0-9a-fA-F]{6}$`) | | Hex color |
| sort_order | int | 0 | Display order |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_folder_id` (id, UNIQUE)

---

### migration_log

Schema migration guard (prevents re-execution of destructive migrations).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| name | string | | Migration identifier (e.g. "001_embedding_migration") |
| applied_at | datetime | time::now() | When migration was applied |

**Indexes**: `unique_migration_name` (name, UNIQUE)

---

## SCHEMALESS Tables

The `prompt` table (prompt library) is created on demand via `CREATE prompt:...` and is intentionally SCHEMALESS. Persisted fields are described by the `Prompt` model (`src-tauri/src/models/prompt.rs`): `id`, `name`, `description`, `category` (system/user/analysis/generation/coding/custom), `content`, `variables[]`, `created_at`, `updated_at`. Validation lives in `commands/prompt.rs`.

---

## Vector Search (HNSW)

| Property | Value |
|----------|-------|
| Table | memory |
| Field | embedding |
| Algorithm | HNSW (Hierarchical Navigable Small World) |
| Distance | Cosine similarity |
| Dimensions | 1024 (configurable via migration) |

Supports KNN search returning top_k results with cosine similarity score.

Embedding dimensions by provider: OpenAI 1536D/3072D, Mistral 1024D, Ollama 768D/1024D.

---

## Security

- **Agent scoping**: Queries scoped by `agent_id` / `workflow_id`
- **API keys**: Never stored in DB (OS keyring via SecureKeyStore)
- **Input validation**: All user input validated and parameterized (no `format!()` injection)
- **External data**: Sanitized via `sanitize_for_surrealdb()` before insertion
- **Audit trail**: `validation_request` + `validation_audit` + `mcp_call_log` + `tool_execution`

---

## Source of Truth

- Schema definition: `src-tauri/src/db/schema.rs`
- Query implementations: `src-tauri/src/db/queries.rs`
- Persistence layer: `src-tauri/src/db/persistence.rs`
- Migrations: `src-tauri/src/commands/migration.rs`
- Security helpers: `src-tauri/src/security/validation.rs`
