# Database Schema - SurrealDB

> **Version**: 1.6
> **SurrealDB**: ~2.6 (SCHEMAFULL)
> **Tables**: 25

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
                      ├──> memory ──> memory_chunk (vector, 1 parent : N chunks)
                      ├──> tool_execution
                      ├──> thinking_step
                      └──> sub_agent_execution

mcp_server ──────────> mcp_call_log
llm_model ───────────> provider_settings
custom_provider ─────> (linked via provider name)
skill ──────────────> skill_version (audit trail)
prompt ─────────────> prompt_version (audit trail)
workflow_folder ─────> workflow (grouping)
kanban_card ────────┬──> kanban_schedule (recurrence template)
                    ├──> kanban_card_interaction (compose / analyze)
                    └──> workflow (1:1, linked at "ready -> doing")
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
| locale | option\<string\> | | UI language stamped at execution (e.g. "fr", "en"); read back by the detached Kanban auto-analyze so the verdict is produced in the user's language without a frontend round-trip. Absent on legacy rows → analyzer falls back to its default. |
| current_context_tokens | int | 0 | Last API call context size |
| sub_agent_tokens_input | int | 0 | Sub-agent input tokens |
| sub_agent_tokens_output | int | 0 | Sub-agent output tokens |
| sub_agent_cost_usd | option\<float\> | 0.0 | Aggregated sub-agent cost (USD) |
| total_cached_tokens | option\<int\> | 0 | Prompt cache read tokens |
| total_cache_write_tokens | option\<int\> | 0 | Prompt cache write tokens |
| folder_id | option\<string\> | | Reference to workflow_folder |
| pinned | bool | false | Pinned in sidebar |
| hidden_from_list | bool | false | When true, excluded from the `/agent` sidebar list (`SELECT_LIST` filters `(hidden_from_list ?? false) = false`); still resolvable by id. Used by the per-card review chat workflow. `?? false` coalesce because DEFAULT does not backfill legacy rows (ERR_SURREAL_011) |

**Indexes**: none (queries filter on status, created_at, agent_id via field-level constraints)

---

### message

Conversation messages (user, assistant, system) with per-message metrics.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| role | string ASSERT IN [user, assistant, system] | | Message role |
| content | string | | Message body |
| attachments | option\<array\<object\>\> | NONE | Multimodal attachments (images for vision-capable models). Sub-fields are declared explicitly because SCHEMAFULL otherwise drops dynamic keys |
| attachments[*].kind | string | | Attachment kind (currently always `image`) |
| attachments[*].mime_type | string | | MIME type (`image/png`, `image/jpeg`, `image/webp`, `image/gif`) |
| attachments[*].data_base64 | string | | Raw base64 payload (no `data:` prefix) |
| attachments[*].name | option\<string\> | | Original filename (display-only, control characters rejected at IPC) |
| attachments[*].size_bytes | option\<int\> | | Original byte size |
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

Parent memory record for RAG and agent context. Stores content + metadata only — embeddings live in `memory_chunk` since the multi-chunk refactor (2026-05-12). Supports auto-scoping, importance, and TTL.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| type | string ASSERT IN [user_pref, context, knowledge, decision] | | Memory category |
| content | string | | Indexed text (raw, full content) |
| workflow_id | option\<string\> | | Scope (auto-set) |
| metadata | object | | Container for sub-fields |
| metadata.tags | option\<array\<string\>\> | | Free-text labels (filterable via `tags_filter`) |
| metadata.priority | option\<float\> | | 0.0-1.0 |
| metadata.agent_source | option\<string\> | | |
| importance | float | 0.5 | Composite scoring weight |
| expires_at | option\<datetime\> | | TTL (auto 7d for `context`); purged at boot + on-demand via `purge_expired_memories` |
| created_at | datetime | time::now() | |

**Indexes**: `memory_workflow_idx` (workflow_id), `memory_type_workflow_idx` (type, workflow_id), `memory_type_created_idx` (type, created_at)

**Schema cleanup**: `schema.rs` runs `REMOVE FIELD IF EXISTS embedding ON TABLE memory` and `REMOVE INDEX IF EXISTS memory_vec_idx ON TABLE memory` on boot (PAT_DB_006) — legacy single-vector rows are stripped silently on replay.

---

### memory_chunk

Vector chunks linked to a parent `memory` via record link. Created by the recursive UTF-8-safe chunker (`tools/memory/chunker.rs`, FN_RUST_019: paragraph -> line -> sentence -> hard cut on code-point boundary). One HNSW 1024D entry per chunk; parent traversal via `memory_id.<field>` is used for filter clauses (workflow_id, type, expires_at, tags).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| memory_id | record\<memory\> | | Parent link (cascade-delete via children-first SELECT VALUE id IN subquery — PAT_DB_007 / ERR_SURREAL_013) |
| chunk_index | int | | 0-based position inside parent content |
| content | string | | Chunk text |
| embedding | array\<float\> | | Vector (HNSW 1024D, COSINE) |
| created_at | datetime | time::now() | |

**Indexes**: `memory_chunk_vec_idx` (embedding, HNSW 1024D COSINE -- defined with `IF NOT EXISTS` so the graph is built once and kept across restarts; to change its definition, ship a guarded `REMOVE INDEX IF EXISTS ...; DEFINE INDEX ...` migration), `memory_chunk_parent_idx` (memory_id)

**Cascade semantics**: chunks MUST be deleted before their parent. The chunk DELETE WHERE clause must use a subquery (`memory_id IN (SELECT VALUE id FROM memory WHERE ...)`) — a record-link traversal (`memory_id.expires_at < ...`) silently matches zero rows in SurrealDB 2.6 (ERR_SURREAL_013).

---

### validation_request

Human-in-the-loop validation requests.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| workflow_id | string | | Parent workflow |
| type | string ASSERT IN [tool, sub_agent, mcp, file_op, db_op, manager_write] | | Operation type (`manager_write` = a prompt/skill/workflow `*Manager` self-improvement write) |
| operation | string | | Operation description |
| details | string | '{}' | JSON string (dynamic params) |
| risk_level | string ASSERT IN [low, medium, high, critical] | | Risk assessment |
| status | string ASSERT IN [pending, approved, rejected] | 'pending' | |
| created_at | datetime | time::now() | |

**Indexes**: (none explicitly defined)

---

### validation_audit

Append-only audit log of validation decisions (user / auto / timeout). Write failures never block the validation flow. Retention is user-configurable (7-90 days, see `RETENTION_MIN_DAYS` / `RETENTION_MAX_DAYS` in `constants.rs`).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| validation_id | string | | Source `validation_request` id |
| tool_name | string | | Tool / operation name |
| decision | string ASSERT IN [approved, rejected, skipped, timeout, blocked] | | Final decision (`blocked` = a fail-closed policy refusal, e.g. an unarmed MCP tool in a detached run) |
| decided_by | string ASSERT IN [user, auto, timeout, policy, pre_approved] | | Decision source (`policy` = the detached allowlist gate; `pre_approved` = a `*Manager` write executed under Auto without confirmation) |
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
| kind | option\<string\> ASSERT IN [standard, kanban] | NONE | Agent kind. `kanban` agents only see the supervisor toolkit (PromptManagerTool, SkillManagerTool, WorkflowManagerTool, ListAgentsTool) and cannot be delegated to. `NONE` is treated as `standard` for backward compatibility. |
| auto_analyze_reports | bool | false | When true, the `workflow_complete` listener auto-triggers `analyze_card_report` for any Kanban card linked to the completing workflow |
| require_file_confirmation | bool | true | Confirm destructive file ops |
| mcp_tool_allowlist | array\<object\> | [] | Per-agent MCP tool allowlist gating **unattended** (detached) runs. An MCP tool auto-called in a detached run executes only if armed here. Sub-fields declared explicitly. Backfilled to `[]` on legacy rows (`WHERE mcp_tool_allowlist IS NONE`) and read via a null-tolerant deserializer (`#[serde(default)]` does not intercept explicit `null`) |
| mcp_tool_allowlist[*].server_id | string | | MCP server id the entry arms |
| mcp_tool_allowlist[*].tools | array\<string\> | [] | Armed tool names on that server |
| mcp_tool_allowlist[*].allow_in_delegated_runs | bool | false | When false (default = strict), the entry is honoured only in the agent's own detached runs, not when reached as a Delegate/Parallel callee (closes the cross-agent confused-deputy) |
| system_prompt | string (1-10000 chars) | | |
| max_tool_iterations | int (1-200) | 50 | Tool loop limit |
| reasoning_effort | option\<string\> | NONE | Thinking model effort. No DB-level `ASSERT`; valid values `low \| medium \| high \| xhigh` are enforced by the `ReasoningEffort` enum (backend) and the provider-aware UI selector. `xhigh` ("Think Max") collapses to `high` on Mistral. |
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
| supports_vision | bool | false | Multimodal vision capability (manual user toggle in ModelForm — no auto-detection). Drives the soft warning in ChatInput when an image is attached on a non-vision model. |
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
| base_url | option\<string\> | | Custom API endpoint (read by `load_ollama_base_url` for embedding init — FN_RUST_017) |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_provider` (provider, UNIQUE)

**Schema cleanup**: `schema.rs` runs `REMOVE FIELD IF EXISTS default_model_id ON TABLE provider_settings` on boot (PR #145, decorative field removal — PAT_DB_006).

---

### custom_provider

User-created OpenAI-compatible provider metadata.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| name | string (1-64 chars) | | URL-safe slug (UNIQUE key) |
| display_name | string (1-128 chars) | | Human-readable name |
| base_url | string (1-512 chars) | | API endpoint |
| enabled | bool | true | |
| supports_cache_control | option\<bool\> | NONE | Strict-mode toggle: when `false`, skip Anthropic-style `cache_control` content parts (Fireworks, Groq, Together, Cerebras reject them with HTTP 400). `NONE` / `true` preserves OpenRouter Anthropic behaviour. |
| supports_reasoning_param | option\<bool\> | NONE | Strict-mode toggle: when `false`, clear the OpenRouter-style top-level `reasoning: {effort, max_tokens}` object AND strip `reasoning` / `reasoning_content` / `reasoning_details` / `provider_specific_fields` from echoed assistant messages on multi-turn tool loops. `NONE` / `true` preserves OpenRouter and RouterLab behaviour. |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `unique_custom_provider_name` (name, UNIQUE)

**Strict-mode defaults**: both `supports_*` columns are defined as `option<bool>` **without** `DEFAULT`. Legacy rows (created before 2026-05-17) keep `NONE` semantics, which the wire path treats as the OpenRouter-preserving default — no backfill required (ERR_LLM_020, PAT_LLM_005).

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

### kanban_card

Kanban board card. One row per work item. Lifecycle: `todo -> ready -> doing -> review -> done` (async auto-composed cards enter as `proposed` and become `ready` only once the user validates them).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| title | string (1-200 chars) | | Card title |
| description | string | | Free-text body (max 5000 chars) |
| kanban_agent_id | string | | Kanban-kind agent that composed the card |
| target_agent_id | string | | Standard agent that will execute the card |
| prompt_id | option\<string\> | | Reference to a stored prompt template |
| inline_prompt | option\<string\> | | Ad-hoc prompt body when `prompt_id` is not set (mutually exclusive) |
| variables | string (JSON) | '{}' | Prompt variables as `HashMap<string,string>`, JSON-string-encoded (ERR_SURREAL_001) |
| target_folder_id | option\<string\> | | Optional FileManager folder constraint for the run |
| status | string ASSERT IN [todo, ready, doing, review, done, failed, proposed] | todo | Logical state. `proposed` = a card generated by the async auto-compose flow, awaiting validation (approve → `ready`, or reject) before the scheduler runs it |
| column | string ASSERT IN [todo, doing, review, done] | todo | Board column (mirror of status, drag-free) |
| column_order | int | 0 | Sort index within the column |
| workflow_id | option\<string\> | | Set when the scheduler transitions the card to `doing` |
| review_chat_workflow_id | option\<string\> | | Hidden workflow backing the per-card review chat; resolves the card via back-reference for the card-chat tools |
| error_summary | option\<string\> | | Short failure description if the execution errored |
| created_at | datetime | time::now() | |
| updated_at | datetime | time::now() | |

**Indexes**: `kanban_card_status_idx` (status), `kanban_card_column_idx` (column, column_order), `kanban_card_workflow_idx` (workflow_id), `kanban_card_review_chat_idx` (review_chat_workflow_id), `kanban_card_kanban_agent_idx` (kanban_agent_id)

---

### kanban_schedule

Recurrence schedule attached to a card template (the card row is duplicated on each tick).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| card_template_id | string | | Reference to the `kanban_card` row used as a blueprint |
| days_of_week | array\<int\> (0–6) | | Weekday codes: 0=Monday … 6=Sunday (chrono `num_days_from_monday`) |
| hour | int (0-23) | | |
| minute | int (0-59) | | |
| next_run_at | datetime | | Next scheduled tick; recomputed on every fire and on update |
| last_run_at | option\<datetime\> | | Last successful fire |
| enabled | bool | true | When false, the scheduler skips this row entirely |
| skip_if_pending | bool | false | When true, do not fire if a sibling card created from this template is still in flight (`todo / ready / doing / review`) |
| created_at | datetime | time::now() | |

**Indexes**: `kanban_schedule_card_idx` (card_template_id), `kanban_schedule_next_run_idx` (next_run_at, enabled)

---

### kanban_card_interaction

Persisted record of each Kanban agent interaction with a card (compose + analyze). Two interactions per card maximum on the happy path: one `compose`, one `analyze` (when `auto_analyze_reports` or manual trigger).

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| card_id | string | | Parent card |
| kind | string ASSERT IN [compose, analyze] | | Interaction kind |
| kanban_agent_id | string | | Kanban agent that ran the interaction |
| provider | string | | LLM provider used |
| model_id_used | string | | Model id resolved at execution time |
| task_input | string | | The free-text description (compose) or the workflow report (analyze) |
| iterations | array\<object\> | [] | Each element is one tool-loop cycle. Sub-fields per element: `iteration_index int`, `reasoning option<string>`, `response_content option<string>`, `tool_calls array<object>` (each: `tool_name string`, `mcp_server option<string>`, `input_json string`, `output_json string`, `duration_ms int`, `success bool`), `tokens_input int`, `tokens_output int`, `cached_tokens int`, `cost_usd float`, `duration_ms int`. All sub-fields declared explicitly with `DEFINE FIELD OVERWRITE` (ERR_SURREAL_001). |
| final_payload_summary | option\<string\> | | Summary of the submitted payload (composed card or verdict + summary) |
| final_response_text | option\<string\> | | Final assistant response text |
| total_tokens_input | int | 0 | |
| total_tokens_output | int | 0 | |
| total_cost_usd | option\<float\> | 0.0 | Aggregated cost across iterations |
| created_at | datetime | time::now() | |

**Indexes**: `kanban_card_interaction_card_idx` (card_id)

---

### prompt_version

Append-only audit trail of prompt edits. Written on every `update_prompt` and `restore_prompt_version`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| prompt_id | string | | Parent prompt |
| version | int | | Monotonic per-prompt counter (1-based) |
| name | string | | Snapshot of name |
| description | string | | Snapshot of description |
| category | string | | Snapshot of category |
| content | string | | Snapshot of full content |
| variables_json | string (JSON) | '[]' | Snapshot of variables array, JSON-string-encoded |
| edited_by | string | | "user" or the Kanban agent id when edited via `PromptManagerTool.update` |
| edit_summary | option\<string\> (1-256 chars) | | Short user-supplied edit message (trimmed, control-character rejected) |
| edited_at | datetime | time::now() | |

**Indexes**: `prompt_version_prompt_idx` (prompt_id, version)

---

### skill_version

Append-only audit trail of skill edits. Same contract as `prompt_version`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| id | string | | UUID |
| skill_id | string | | Parent skill |
| version | int | | Monotonic per-skill counter (1-based) |
| name | string | | Snapshot of name |
| description | string | | Snapshot of description |
| category | string | | Snapshot of category |
| content | string | | Snapshot of full content |
| edited_by | string | | "user" or the Kanban agent id when edited via `SkillManagerTool.update` |
| edit_summary | option\<string\> (1-256 chars) | | Short user-supplied edit message |
| edited_at | datetime | time::now() | |

**Indexes**: `skill_version_skill_idx` (skill_id, version)

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

## Single-Row Settings (JSON blobs)

Some settings live as a single JSON blob rather than as a structured table — used when the surface is bound to one user-level config object and the field set evolves frequently. The row id is the table name (e.g. `settings:stt`), the payload is a `config` JSON string.

| Key | Source of truth | Description |
|-----|-----------------|-------------|
| `settings:stt` | `STTSettings` (`src-tauri/src/models/stt.rs`) | Push-to-talk voice dictation: enable toggle, Voxtral model id, context-bias hints, optional ISO 639-1 language override. Persisted as JSON to avoid a migration on every additive field; validation lives in `commands/settings_stt.rs::apply_update`. |
| `settings:kanban` | `KanbanSettings` (`src-tauri/src/commands/settings_kanban.rs`) | Kanban tuning. Currently `compose_timeout_secs` (default 600, clamp 60-1800), the wall-clock ceiling for a detached card compose run. Clamped on both write and read. |
| `settings:mcp_network` | `McpNetworkSettings` (`src-tauri/src/mcp/network_settings.rs`) | MCP HTTP connectivity. Currently `allow_private_network` (default `false`): opt-in to reach MCP HTTP servers on private / LAN ranges. Seeds a process-global fail-secure snapshot at boot; cloud-metadata / link-local / reserved targets stay blocked regardless. |

---

## Vector Search (HNSW)

| Property | Value |
|----------|-------|
| Table | memory_chunk |
| Field | embedding |
| Algorithm | HNSW (Hierarchical Navigable Small World) |
| Distance | Cosine similarity |
| Dimensions | 1024 (fixed by schema; non-configurable since multi-chunk refactor) |

Supports KNN search returning top_k chunks with cosine similarity score. The search result type (`ChunkSearchResult`) exposes BOTH `chunk_id` (the row matched) and `parent_memory_id` (used by the agent's `operation=get` to retrieve the full parent content).

**Compatible embedding models (1024D required)**: Mistral `mistral-embed`, Ollama `mxbai-embed-large` / `bge-large` / any 1024D model. `nomic-embed-text` (768D) is incompatible with the fixed HNSW index and has been removed from the model picker.

**Search filter clauses** (parent-side, via `memory_id.<field>` traversal in SELECT only — NOT in DELETE WHERE, see ERR_SURREAL_013):
- `tags_filter` (CONTAINSANY) — optional list of tags the parent must carry (FN_RUST_018)
- `workflow_id`, `type`, `expires_at` — standard scope/TTL gates

---

## Security

- **Agent scoping**: Queries scoped by `agent_id` / `workflow_id`
- **API keys**: Never stored in DB (OS keyring via SecureKeyStore)
- **Input validation**: All user input validated and parameterized (no `format!()` injection)
- **External data**: Sanitized via `sanitize_for_surrealdb()` before insertion
- **Audit trail**: `validation_request` + `validation_audit` + `mcp_call_log` + `tool_execution`

---

## Schema Initialization at Boot

`initialize_schema` in `src-tauri/src/db/client.rs` applies `SCHEMA_SQL` (defined in `src-tauri/src/db/schema.rs`) with a fingerprint gate to avoid paying the full re-apply cost on every launch:

1. A stable text fingerprint of `SCHEMA_SQL` is computed at boot and compared against a hash stored in a `schema_meta:current` record.
2. If the fingerprint matches the stored value the schema is skipped entirely -- this is the normal path on a populated database.
3. If the fingerprint differs (a schema edit was shipped) the full `SCHEMA_SQL` is applied statement by statement. Errors are surfaced per-statement; if any statement fails the fingerprint is not stored, so the schema re-applies on the next boot instead of being frozen in a half-applied state.
4. After a clean apply the new fingerprint is written to `schema_meta:current`.

The HNSW index on `memory_chunk` is declared `IF NOT EXISTS` (not `OVERWRITE`) because rebuilding the vector graph from stored embeddings takes roughly 10 seconds once the table is populated. B-tree indexes under `OVERWRITE` rebuild in a few seconds. Both costs were previously paid unconditionally on every startup. The fingerprint gate eliminates them entirely on unchanged schema runs.

## Source of Truth

Schema in `src-tauri/src/db/schema.rs`; queries in `db/queries.rs`; persistence in `db/persistence.rs`; migrations in `commands/migration.rs`; security helpers in `security/validation.rs`.
