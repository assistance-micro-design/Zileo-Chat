# API Reference - Tauri Commands

> Technical reference for Frontend-Backend IPC communication. **150+ commands** across 30+ modules.

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

OpenAI-compatible provider management (OpenRouter, RouterLab, Fireworks, Groq, Together AI, Cerebras, etc.).

| Command | Description |
|---------|-------------|
| `list_providers` | List all providers (builtin + custom). `ProviderInfo` carries the two `supports_*` toggles for custom rows. |
| `create_custom_provider` | Create OpenAI-compatible provider. Accepts trailing optional `supports_cache_control: Option<bool>` + `supports_reasoning_param: Option<bool>` (camelCase from TS). Persisted on the row and applied to the running provider via `set_strict_compat`. |
| `update_custom_provider` | Update custom provider settings (name kept). Same two optional toggles available; canonical DB value is synced back to the runtime after the SET clause runs. |
| `delete_custom_provider` | Delete custom provider and its API key |

**Strict-mode toggles** (ERR_LLM_020, PAT_LLM_005): the two `supports_*` parameters default to `null` from the frontend (mapped to `None` in Rust) which preserves the OpenRouter behaviour on the wire (cache_control + top-level `reasoning` injected). Pass `false` for Fireworks / Groq / Together / Cerebras — Pydantic-strict gateways that reject those extension fields with HTTP 400.

**HTTP `User-Agent`**: every reqwest client built in this codebase starts from `llm::http::default_http_client_builder()` and therefore carries `User-Agent: ZileoChat/<CARGO_PKG_VERSION>` on every outbound request. The version is resolved at compile time, so release tags propagate automatically. New HTTP-touching code must use this builder rather than `reqwest::Client::builder()` directly.

**Reasoning tiers**: `ReasoningEffort` is a 4-variant enum (`low | medium | high | xhigh`). `xhigh` ("Think Max") is forwarded verbatim to OpenAI-compatible providers (16384-token budget, clamped by gateway) and collapses to `"high"` on Mistral. The UI Select only exposes `xhigh` when the model's `api_name` matches the `XHIGH_MODEL_PATTERNS` substring allowlist (`deepseek`, `gpt-5.`, `grok`, `claude-opus`); the normalization helper auto-downgrades stored `xhigh` to `high` when the user switches to a non-matching model so the form state stays consistent.

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
| `reindex_memory_chunks` | Spawn a streaming reindex job (recursive chunker -> `memory_chunk` + embeddings). Emits `reindex-progress` events per processed parent. Returns `ReindexJobStatus { jobId, ... }`. |
| `cancel_reindex_job` | Cancel a running reindex job by `jobId` |
| `get_reindex_job_status` | Read current status for a `jobId` — auto-purges terminal entries on consultation; a background timer also sweeps after 10 minutes |

### Streaming (`commands/streaming/`)

Real-time workflow execution with event streaming.

| Command | Description |
|---------|-------------|
| `execute_workflow_streaming` | Execute workflow with real-time events. Accepts optional `attachments?: MessageAttachment[]` parameter to inject user-provided images into the first multimodal user turn. |
| `cancel_workflow_streaming` | Cancel a running workflow |

### Message (`commands/message.rs`)

Chat message persistence and retrieval.

| Command | Description |
|---------|-------------|
| `save_message` | Persist a message to the database. Accepts optional `attachments?: MessageAttachment[]` (validated for user role, count ≤ 8, MIME whitelist `image/png \| jpeg \| webp \| gif`, base64 payload size cap, name field control-char rejection + 512-byte cap). |
| `load_workflow_messages` | Load all messages for a workflow (SELECT includes `attachments` for multipart replay) |
| `load_workflow_messages_paginated` | Load messages with pagination |
| `delete_message` | Delete a single message |
| `load_workflow_blocks` | Load all structured display blocks for a workflow grouped by message |
| `validate_attachments` | Pre-send validator invoked from `ChatInput.svelte`. Rejects image attachments when the active workflow's agent resolves to a non-vision model (via `resolve_workflow_supports_vision` -> `resolve_agent_supports_vision`, which now reads the nested `llm` object client-side to bypass SCHEMAFULL nested-AS unreliability, and scopes the model lookup by `(api_name, provider)` so duplicate `api_name` rows across custom providers no longer mistrust the wrong row). Fails closed on any DB error. |

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
| `read_image_for_attachment` | Read an image file selected through the Tauri native picker. Validates extension whitelist (`png/jpg/jpeg/webp/gif`) and size (4 MB cap), returns `{ data_base64, mime_type, size_bytes, name }`. Shares the `ALLOWED_IMAGE_EXTENSIONS` + `ext_to_image_mime` helper with the `FileManagerTool.read_image` agent operation. |

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

Secure API key storage (AES-256-GCM via SecureKeyStore). Stored secrets are never returned through IPC; callers can only save, delete, or query key presence.

| Command | Description |
|---------|-------------|
| `save_api_key` | Securely store an API key |
| `delete_api_key` | Remove a stored API key |
| `has_api_key` | Check if an API key exists for a provider without exposing it |
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

### Kanban Card (`commands/kanban_card.rs`)

Kanban card CRUD and column transitions.

| Command | Description |
|---------|-------------|
| `create_kanban_card` | Create a new card (`title`, `description?`, `kanban_agent_id?`, `target_agent_id`, `prompt_id?` OR `inline_prompt?`, `variables?: HashMap<string,string>`, `target_folder_id?`). Starts in the `todo` column. |
| `get_kanban_card` | Get a single card by id (full state including the linked `workflow_id` if execution started). |
| `list_kanban_cards` | List all cards ordered by `column` then `column_order`. |
| `update_kanban_card` | Partial update of a card. Status / column transitions go through the dedicated `move_kanban_card`. |
| `delete_kanban_card` | Params: `card_id`, `also_delete_schedule?: bool` (default `false`). Delete a card: cascade-removes linked `kanban_card_interaction` rows AND the `review_chat_workflow_id` hidden workflow with all its rows (messages, tool executions, thinking steps); the worker `workflow` row is preserved. `also_delete_schedule=true` also deletes the linked `kanban_schedule` row. Force-delete is allowed even when the card is in `doing` (covers crashed workflows that never emitted `workflow_complete`). |
| `set_kanban_card_workflow_id` | Link the card to an executing workflow. Used by the scheduler when it transitions a card from `ready` to `doing`. |
| `duplicate_kanban_card_as_template` | Clone a card as a recurrence template (target of `create_kanban_schedule`). |
| `move_kanban_card` | Move a card to a different column (`todo / doing / review / done`) and a new `column_order` index. |

### Kanban Schedule (`commands/kanban_schedule.rs`)

Recurrence schedules attached to a card template.

| Command | Description |
|---------|-------------|
| `create_kanban_schedule` | Create a schedule for a card template (`card_template_id`, `days_of_week: u8[]`, `hour: u8`, `minute: u8`, `skip_if_pending: bool`, `enabled: bool`). |
| `get_kanban_schedule` | Get a single schedule by id. |
| `list_kanban_schedules` | List schedules (optional filter on `enabled` or `card_template_id`). |
| `update_kanban_schedule` | Partial update of recurrence fields. Recomputes `next_run_at`. |
| `delete_kanban_schedule` | Delete a schedule. The linked card template is NOT deleted (it stays usable as a manual one-shot). |

### Kanban Compose (`commands/compose_card.rs`)

Auto-compose path driven by a Kanban-kind agent.

| Command | Description |
|---------|-------------|
| `compose_card_from_description` | Params: `kanban_agent_id`, `description`, `locale`. Take a free-text description, dispatch it to the configured Kanban agent with `ListAgentsTool` + `SubmitComposedCardTool` auto-injected, run the tool loop (forced tool call on the opening turn) until `SubmitComposedCardTool` is called, then persist the composed card and return its id. The `locale` is injected into the task context so the card is composed in the UI language (empty → tool-loop default). Uses the Kanban agent's own LLM config (provider, model, reasoning effort). |

### Kanban Analyzer (`commands/kanban_analyzer.rs`)

Card report analysis.

| Command | Description |
|---------|-------------|
| `analyze_card_report` | Analyze the report of a completed card workflow. Dispatches to the configured Kanban agent with `WorkflowManagerTool` + `SubmitAnalysisTool` auto-injected. Runs the tool loop with a forced tool call on the opening turn until `SubmitAnalysisTool` is called. The verdict is produced in the language stamped on the workflow (`workflow.locale`). The full worker report is fed to the analyzer verbatim (never truncated). Returns the verdict (`approve | reject | needs_improvement`), summary, and optional `suggested_prompt_edit`. Triggered manually from the report viewer ("Re-analyze") or automatically by the `workflow_complete` listener when the target agent has `auto_analyze_reports: true`. A boot-time catch-up pass re-runs the analyzer for cards orphaned in `review` (finished, has a workflow, no analysis yet) by an app closed mid-workflow. |

### Kanban Card Chat (`commands/kanban_card_chat.rs`)

Per-card review chat with the supervisor agent.

| Command | Description |
|---------|-------------|
| `open_card_review_chat` | Params: `card_id`, `locale`. Open (or resume) the review chat for a card. On first open, creates a hidden workflow (`workflow.hidden_from_list = true`, filtered out of the `/agent` sidebar), links it to the card via `kanban_card.review_chat_workflow_id`, and seeds it with a structured first assistant message (worker report + last analyze verdict + any `suggested_prompt_edit`, in the requested `locale`). Resume is idempotent — returns the same workflow. Inside this chat the supervisor gains three auto-injected, self-gating tools: `RerunWorkerTool`, `MoveCardTool`, `ScheduleCardTool`. |

### Kanban Interaction (`commands/kanban_interaction.rs`)

Read-only access to the persisted compose / analyze interactions.

| Command | Description |
|---------|-------------|
| `load_card_interactions` | Load all `kanban_card_interaction` rows for a card (compose + analyze, chronological). Each row carries the input task, provider, `model_id_used`, iteration count, final payload summary, response text, token totals, and cost. Rendered inline in the card report viewer. |

### Prompt Versions (`commands/prompt_version.rs`)

History of prompt edits.

| Command | Description |
|---------|-------------|
| `list_prompt_versions` | List version snapshots for a prompt (most recent first). |
| `get_prompt_version` | Get a single version snapshot. |
| `restore_prompt_version` | Restore a prior version. Writes a fresh snapshot of the current content before overwriting, so restore is itself versioned. |
| `delete_prompt_version` | Delete a version snapshot. Refuses to delete the last remaining version (audit-trail safeguard, returns a structured `LastVersionSafeguard` error). |

### Skill Versions (`commands/skill_version.rs`)

History of skill edits. Same contract as prompt versions.

| Command | Description |
|---------|-------------|
| `list_skill_versions` | List version snapshots for a skill. |
| `get_skill_version` | Get a single version snapshot. |
| `restore_skill_version` | Restore a prior version (writes a fresh snapshot first). |
| `delete_skill_version` | Delete a version snapshot. Refuses the last remaining version. |

### Scheduler (`commands/scheduler.rs`)

Background tokio loop driving the Kanban board.

| Command | Description |
|---------|-------------|
| `card_id_for_workflow` | Reverse lookup: given a `workflow_id`, return the linked `kanban_card_id` if any. Used by the `workflow_complete` listener to transition the card from `doing` to `review` and optionally trigger the analyzer. |

The scheduler itself is not a Tauri command — it is a tokio task spawned at app startup that ticks every 60s. Four responsibilities per tick: (1) reclaim orphaned `doing` cards (no `workflow_id` past a grace window) back to `ready` / `todo` so lost concurrency slots are freed (`reclaim_orphaned_doing_cards_core`); (2) pull `ready` cards into `doing` through `WorkflowExecutorService`, gated by `select_cards_to_promote_core` which uses an atomic `WHERE status = 'ready'` flip to prevent double-promotion; (3) evaluate `kanban_schedule` rows whose `next_run_at <= now()` and `enabled = true`, clone the template card into a fresh `ready` card unless `skip_if_pending` is true and a sibling card is already in flight; (4) purge `done` cards older than 3 days that are NOT the template of any enabled schedule, cascading their `kanban_card_interaction` rows but preserving the underlying `workflow`. Emits `kanban:cards_purged` when purges happen.

### Speech-to-Text (`commands/stt.rs` + `commands/settings_stt.rs`)

Push-to-talk voice dictation via Mistral Voxtral. The provider-agnostic core (`llm/stt/transcribe_audio_core`) lets future providers (Ollama Whisper, OpenAI Whisper) plug in without touching the command surface. Settings are persisted as a JSON blob on the `settings` table under key `settings:stt` — no dedicated SurrealDB schema.

| Command | Description |
|---------|-------------|
| `transcribe_audio` | Transcribe a base64 audio blob via the configured STT provider. Validates `MAX_AUDIO_BASE64_LEN` (~25 MiB binary cap × 4/3) before forwarding to the core. Returns the transcript string. |
| `get_stt_settings` | Return the current `STTSettings` (defaults if absent). |
| `update_stt_settings` | Merge an `UpdateSTTSettingsRequest` into the current settings and persist. Validates: enable toggle, model id allowlist, context-bias trim + drop-empties + 10 × 200-char cap + control-char rejection, ISO 639-1 language allowlist. Language is a tri-state `Option<Option<String>>` (absent = keep, `null` = clear to auto, `Some(code)` = set explicit) via the shared `deserialize_explicit_option` helper. |
| `reset_stt_settings` | Replace stored settings with defaults. |

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
| `LLMModel` | `$types/llm` | Model definition (builtin or custom). Carries `supports_vision: boolean` for multimodal capability (manual flag, no auto-detection). |
| `Message` | `$types/message` | Conversation message. Carries optional `attachments?: MessageAttachment[]` for multimodal user turns. |
| `MessageAttachment` | `$types/message` | Multimodal attachment carried by user messages: `{ kind: 'image', mime_type, data_base64, name?, size_bytes? }`. Persisted on `message.attachments[*]` with all sub-fields declared explicitly (SCHEMAFULL would otherwise drop dynamic keys). |
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

### Speech-to-Text Types

| Type | Location | Description |
|------|----------|-------------|
| `STTSettings` | `$types/stt` | Persisted dictation settings (enable, model id, context-bias hints, language override) |
| `UpdateSTTSettingsRequest` | `$types/stt` | Patch payload for `update_stt_settings`; language is `string \| null \| undefined` to carry the tri-state semantic (absent = keep, `null` = clear to auto, `string` = set) |
| `TranscribeAudioRequest` | `$types/stt` | `{mime_type, data_base64}` payload sent to `transcribe_audio` |
| `TranscriptionResponse` | `$types/stt` | `{text}` response from the STT provider |
| `SupportedLanguage` | `$types/stt` | Closed allowlist of ISO 639-1 codes the backend accepts |
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

### `kanban:cards_purged`

Emitted by the Kanban scheduler when one or more stale `done` cards are auto-purged. Payload: `{ purgedCount, cardIds: string[] }`. The `/kanban` page listens to this event and refreshes the board live.

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
