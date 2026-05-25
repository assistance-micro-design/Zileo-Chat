# Agent Tools Documentation

Technical documentation for the native tools available to agents in the multi-agent system.

---

## Implementation Status

| Tool | Source |
|------|--------|
| **TodoTool** | `src-tauri/src/tools/todo/tool.rs` |
| **MemoryTool** | `src-tauri/src/tools/memory/tool.rs` |
| **CalculatorTool** | `src-tauri/src/tools/calculator/tool.rs` |
| **UserQuestionTool** | `src-tauri/src/tools/user_question/tool.rs` |
| **SpawnAgentTool** | `src-tauri/src/tools/spawn_agent.rs` |
| **DelegateTaskTool** | `src-tauri/src/tools/delegate_task.rs` |
| **ParallelTasksTool** | `src-tauri/src/tools/parallel_tasks.rs` |
| **ReadSkillTool** | `src-tauri/src/tools/read_skill.rs` |
| **FileManagerTool** | `src-tauri/src/tools/file_manager/tool.rs` |
| **PromptManagerTool** | `src-tauri/src/tools/prompt_manager.rs` |
| **SkillManagerTool** | `src-tauri/src/tools/skill_manager/` (folder: `mod.rs`, `crud.rs`, `grant.rs`, `validators.rs`, `versions.rs`, `tests.rs`) |
| **WorkflowManagerTool** | `src-tauri/src/tools/workflow_manager.rs` |
| **ListAgentsTool** | `src-tauri/src/tools/list_agents.rs` |
| **SubmitComposedCardTool** | `src-tauri/src/tools/submit_composed_card.rs` |
| **SubmitAnalysisTool** | `src-tauri/src/tools/submit_analysis.rs` |
| **Tool Execution** | `src-tauri/src/agents/llm_agent.rs` |

**Note**: DB tools (SurrealDBTool, QueryBuilderTool, AnalyticsTool) were removed -- DB access goes through Tauri IPC commands.

### Tool Categories

- **Basic**: MemoryTool, TodoTool, CalculatorTool (no special context required)
- **Interaction**: UserQuestionTool (human-in-the-loop)
- **File**: FileManagerTool (sandboxed filesystem operations)
- **Sub-Agent**: SpawnAgentTool, DelegateTaskTool, ParallelTasksTool (require AgentToolContext)
- **Hidden**: ReadSkillTool (auto-injected when agent has skills, not shown in UI)
- **Kanban Supervisor** (Kanban-kind agents only): PromptManagerTool, SkillManagerTool, WorkflowManagerTool, ListAgentsTool
- **Kanban Private** (auto-injected during compose / analyze, never visible in the UI catalogue): SubmitComposedCardTool, SubmitAnalysisTool

### Sub-Agent Resilience

- Inactivity timeout with heartbeat (300s timeout, 30s check interval)
- Retry with exponential backoff (3 attempts, 500ms-2000ms)
- CancellationToken for graceful shutdown
- Hierarchical correlation IDs for batch tracing

### ToolFactory

Tools are instantiated dynamically via `ToolFactory`. See `src-tauri/src/tools/` for implementation.

---

## 1. TodoTool

**Purpose**: Hierarchical workflow management and agent task orchestration.

### Operations

- `create` -- Create a task (`name` required)
- `get` -- Read task by ID (`task_id`)
- `update_status` -- Update status (`task_id`, `status`)
- `list` -- List workflow tasks (optional `status_filter`). Sub-agents only see their own tasks.
- `complete` -- Mark complete (`task_id`, optional `duration_ms`)
- `delete` -- Delete task (`task_id`)
- `list_agent_tasks` -- List tasks assigned to a specific agent with completion stats (primary agent only)
- `reassign_tasks` -- Reassign tasks to a different agent (primary agent only)

### Task Structure

Fields: `id` (uuid), `workflow_id`, `name` (max 128), `description` (max 1000), `agent_assigned?`, `priority` (1-5), `status` (pending/in_progress/completed/blocked), `dependencies` (uuid[]), `duration_ms?`, `created_at`, `completed_at?`.

### Example

```json
{ "operation": "create", "name": "Analyze code structure", "priority": 1 }
```

### Use Cases

- Multi-agent orchestration and complex workflow coordination
- Progress tracking for long-running operations (>3 steps)
- Dependency management (sequential or parallel tasks)
- Execution duration metrics for optimization

---

## 2. MemoryTool

**Purpose**: Vector-backed persistent memory in SurrealDB for agent contextual recall.

### Architecture

- **Database**: SurrealDB with HNSW vector indexing (1024D, fixed by schema)
- **Schema**: 1 parent `memory` row + N indexed `memory_chunk` rows linked via `memory_id: record<memory>` (multi-chunk refactor 2026-05-12)
- **Chunker**: `tools/memory/chunker.rs` recursive UTF-8-safe split (paragraph -> line -> sentence -> hard cut on code-point boundary) — FN_RUST_019
- **Search**: composite scoring `cosine_similarity * 0.7 + importance * 0.15 + recency * 0.15`, run against `memory_chunk` with parent traversal via `memory_id.<field>` for filter clauses
- **Embedding providers** (must produce 1024D vectors): Mistral `mistral-embed`, Ollama `mxbai-embed-large` / `bge-large` / any 1024D model. `nomic-embed-text` (768D) is incompatible with the fixed HNSW index and is filtered out of the model picker.

### Operations

- `describe` -- Discovery: memory stats by type/scope
- `add` -- Add memory with auto-scoping (`type`, `content`). Writes 1 parent + N chunks; embeddings are generated per chunk.
- `get` -- Read by ID (`memory_id` — the parent id)
- `list` -- List with filters (mode `compact` or `full`)
- `search` -- Semantic search (`query`, optional `limit`, `threshold`, `tags_filter`). Each hit is a chunk; `chunk_id` is distinct from `parent_memory_id` (call `operation=get` with the parent id for full content).
- `delete` -- Delete by ID (`memory_id`). Cascade-deletes chunks first via SELECT VALUE id subquery (PAT_DB_007 — record-link traversal in DELETE WHERE silently matches zero rows, ERR_SURREAL_013).
- `clear_by_type` -- Bulk delete by type (`type`); same cascade semantics.

### Auto-Scoping

Scope is determined automatically by memory type:
- `user_pref`, `knowledge` -- general scope (cross-workflow)
- `context`, `decision` -- workflow scope (tied to current workflow)
- Override possible via the `scope` parameter

### Example

```json
{ "operation": "search", "query": "vector database indexing", "limit": 5, "tags_filter": ["rag", "indexing"] }
```

### Key Details

- **Default importance**: user_pref=0.8, decision=0.7, knowledge=0.6, context=0.3
- **Auto TTL**: `context` memories expire after 7 days. Expired rows + their chunks are purged at boot (best-effort via `AppState::new`) and on-demand via the `purge_expired_memories` Tauri command surfaced in the Memory Operations card (FN_RUST_020).
- **Tags filter**: `tags_filter` is matched against `metadata.tags` on the parent via CONTAINSANY (FN_RUST_018). Empty/missing filter = no filter.
- **Search result shape**: `chunkId`, `parentMemoryId`, `chunkIndex`, `score`, plus parent fields surfaced via traversal (`content`, `metadata.tags`, `type`, `workflow_id`, `expires_at`). The same content may surface multiple times with different `chunk_id` values — the frontend dashboard dedupes by `parentMemoryId`.
- **Reindex**: backfill / re-chunk via the `reindex_memory_chunks` Tauri command (streaming, cancellable, persisted via LocalStorage). Pre-reindex gap: text search falls back to scanning `memory.content` when the chunk table is globally empty.
- **Security**: all DB queries use bind parameters. See `src-tauri/src/tools/memory/` for implementation.

---

## 3. CalculatorTool

**Purpose**: Mathematical expression evaluation for agents.

### Operations

| Operation | Description | Examples |
|-----------|-------------|---------|
| `unary` | Single-argument functions | sin, cos, tan, sqrt, exp, ln, abs, floor, ceil, round |
| `binary` | Two-argument functions | pow, log, min, max, +, -, *, / |
| `constant` | Mathematical constants | pi, e, tau |

Supports parentheses, decimals, and negative numbers.

### Example

```json
{ "operation": "binary", "operator": "pow", "a": 2, "b": 10 }
```

---

## 4. UserQuestionTool

**Purpose**: Allow agents to ask interactive questions to users during workflow execution.

### Operations

- `ask` -- Ask a question (`question`, `questionType`)

### Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `question` | string | Yes | Max 2000 chars |
| `questionType` | string | Yes | `checkbox`, `text`, or `mixed` |
| `options` | array | For checkbox/mixed | Max 20 options, each with `id` and `label` |
| `textPlaceholder` | string | No | Placeholder for text field |
| `textRequired` | boolean | No | Whether text is required (for `mixed`) |
| `context` | string | No | Additional context to display (max 5000 chars) |

### Example

```json
{ "operation": "ask", "question": "Which approach?", "questionType": "text" }
```

### Polling and Timeout

Uses progressive polling (500ms to 5s intervals). After 5 minutes without response, the question times out. The circuit breaker tracks consecutive timeouts.

### Circuit Breaker

Prevents question spam when the user is unresponsive:
- **Closed** -- Normal operation
- **Open** -- After 3 consecutive timeouts, questions are rejected
- **HalfOpen** -- After 60s cooldown, one test question is allowed

Transitions: `Closed -[3 timeouts]-> Open -[60s]-> HalfOpen -[success]-> Closed` or `HalfOpen -[timeout]-> Open`.

See `src-tauri/src/tools/user_question/circuit_breaker.rs` for implementation.

### Events

- `user_question_start` -- Question sent, awaiting response
- `user_question_complete` -- Response received, skipped, or timed out

---

## 5. ReadSkillTool

**Purpose**: Let an agent read its OWN assigned skills (its operating procedures). Read-only and scoped to the agent's own allowlist — reading a skill it does not own returns `PermissionDenied`. To inspect or improve a skill the agent does not own, a Kanban agent uses `SkillManagerTool` instead (which reaches any skill in the system). The two tools' descriptions are deliberately disambiguated so the model does not pick `ReadSkillTool` for a non-owned skill and hit the permission gate.

**Hidden**: Auto-injected when the agent has assigned skills; not visible in the frontend UI.

### Operations

- `read` (default) -- Read full skill content (`name` required)
- `list` -- List available skills for the agent

### Access Control

- `list` returns only skills in `agent_skills` AND `enabled = true` in DB
- `read` validates the skill name is assigned to the agent AND enabled
- Returns `PermissionDenied` if not assigned, `NotFound` if absent/disabled

### Auto-Injection

ReadSkillTool is injected automatically in `agents/execution/tools.rs` when the agent has `skills.len() > 0`. An "Available Skills" section is added to the agent's system prompt. Sub-agents inherit their parent's skills.

### Prompt Template Integration

The `{{skill:name}}` syntax in prompt templates is resolved in the streaming pipeline (`commands/streaming/execution.rs`), instructing the LLM to read the skill before proceeding.

---

## 6. FileManagerTool

**Purpose**: Sandboxed filesystem operations within authorized agent folders.

### Architecture

- `tool.rs` -- Struct + Tool trait (11 operations)
- `security.rs` -- Path validation, sandbox enforcement
- `helpers.rs` -- File info formatting, text detection, constants, image extension whitelist
- `trash.rs` -- Trash-based safety (backup, restore, cleanup)

See `src-tauri/src/tools/file_manager/` for implementation.

### Operations

- `list` -- List directory contents (`path`)
- `read` -- Read text file (`path`)
- `write` -- Write/create file with auto-backup (`path`, `content`)
- `replace` -- Regex replacement in file (`path`, `pattern`, `replacement`)
- `create` -- Create directory (`path`)
- `delete` -- Delete to trash (`path`)
- `move` -- Move file/directory (`source`, `destination`)
- `rename` -- Rename file/directory (`path`, `new_name`)
- `search_glob` -- Glob pattern search (`path`, `pattern`)
- `search_content` -- Content search (`path`, `pattern`)
- `read_image` -- Load an image from disk for vision analysis on the next turn (`path`)

### read_image (multimodal)

Loads an image file and surfaces it to the next LLM iteration as a multimodal user turn. Strictly gated on `supports_vision: true`: when the agent's model lacks the flag, `read_image` is omitted from both the tool's `definition()` and its JSON schema (so the model never sees the operation in its tool list) AND `execute()` refuses the call with a clear error if it is somehow forged. This is the tool-side layer of a four-layer defense-in-depth (UI hard-block on paste / picker / drop, UI auto-strip on model switch, IPC `validate_attachments` pre-send rejection, and this tool gating).

- **Extensions whitelist**: `png`, `jpg`, `jpeg`, `webp`, `gif`
- **Size cap**: 8 MB raw bytes
- **Path validation**: same six-layer sandbox check as the other operations
- **Return shape**: `{ path, mime_type, size_bytes, name }` — the lightweight metadata the agent sees in its tool result
- **Side channel**: the raw base64 bytes never reach the tool message, the persisted `tool_execution.output_result` row, or the live `tool_call_complete` stream chunk. The iteration loop strips them off the result and injects a synthetic `role: "user"` multipart turn (`[{type: "text"}, {type: "image_url"}]`) right after the tool message so the model actually sees the picture
- **Per-provider envelope**: handled centrally by `src-tauri/src/llm/image_format.rs` — default OpenAI object shape, normalized to bare string for Mistral, flattened to native `content: string` + sibling `images: [base64]` array for Ollama `/api/chat`

### Security and Safety

- All paths are validated and canonicalized against the agent's authorized folders
- Destructive operations (write overwrite, delete, move, rename) create backups in `.zileo-trash/`
- If `require_file_confirmation` is enabled, destructive operations go through the ValidationHelper system
- Trash cleanup is lazy (triggered on first destructive operation)
- `read_image` shares the `ALLOWED_IMAGE_EXTENSIONS` whitelist + `ext_to_image_mime` helper with the `read_image_for_attachment` Tauri command (frontend picker bridge) — sibling Rust whitelist, mirrored client-side in `ChatInput.svelte`

### Limits

- Max file read size: 10 MB (`read`)
- Max image read size: 8 MB (`read_image`)
- Max list entries: 200
- Max search results: 100
- Trash retention: 7 days, max 500 MB

---

## 7. PromptManagerTool

**Purpose**: Let a Kanban-kind agent manage the prompt template library (read + create + update). No delete.

**Access**: Kanban agents only. Standard agents never see this tool — filtered out at the factory boundary.

### Operations

- `list` -- List all prompt templates (id, name, description, category, variable names)
- `get` -- Get a single prompt by id (full content + variables)
- `create` -- Create a new prompt (`name`, `description`, `category`, `content`, `variables[]`)
- `update` -- Partial update of an existing prompt (`prompt_id`, any subset of fields, optional `edit_summary`). Auto-snapshots a `prompt_version` row before applying.

### Versioning

Every successful `update` writes a `prompt_version` row with the previous content snapshot, the `edit_summary` (validated: trimmed, 256-char cap, control-character rejection via the shared `validate_edit_summary` helper), and an incrementing `version` integer. The "last version" cannot be deleted — the safeguard preserves the audit trail.

---

## 8. SkillManagerTool

**Purpose**: Let a Kanban-kind agent manage the skill library (read + create + update + grant + revoke + version history).

**Access**: Kanban agents only.

### Operations

- `list` -- List all skills (id, name, description, category, enabled)
- `get` -- Get a single skill (full content)
- `create` -- Create a new skill (`name`, `description`, `category`, `content`)
- `update` -- Partial update (`skill_id`, any subset, optional `edit_summary`). Auto-snapshots a `skill_version` row before applying.
- `grant_skill_to_agent` -- Attach an EXISTING skill (`skill_name`) to `target_agent_id`'s allowlist. The skill and agent must exist and share the same kind (a kanban skill only grants to a kanban agent, a standard skill to a standard agent); cross-kind grants are rejected. Idempotent. Distinct from `create_skill`, which auto-grants the freshly-created skill where the same-kind invariant holds by construction.
- `revoke_skill_from_agent` -- Remove a skill from a target agent (`skill_name`, `target_agent_id`)
- `list_versions` -- List version snapshots for a skill (most recent first)
- `restore_version` -- Restore a prior version (writes a new snapshot of the current content before overwriting, so restore is itself versioned)

### Architecture

Split into a folder for testability and clarity:
- `mod.rs` -- Tool struct, top-level dispatch, ToolDefinition
- `crud.rs` -- `list / get / create / update` against the `skill` table
- `grant.rs` -- `grant / revoke` against `agent.skills`
- `versions.rs` -- `list_versions / restore_version` against `skill_version`
- `validators.rs` -- Shared input validation (name slug, category allowlist, content cap)
- `tests.rs` -- Unit tests (~500 LOC)

---

## 9. WorkflowManagerTool

**Purpose**: Read-only access to historical workflow data so the Kanban analyzer can ground its verdict on real execution artefacts.

**Access**: Kanban agents only.

### Operations

- `list_workflows` -- List workflows (id, name, agent, status, timestamps, token totals)
- `rename_workflow` -- Rename a workflow (`workflow_id`, `name`)
- `folders_create / folders_list / folders_rename / folders_delete` -- Folder CRUD for organisation
- `read_workflow` -- Fetch the full state of a workflow (messages, tool executions, thinking steps, sub-agent reports) for analysis
- `list_workflow_errors` -- Extract just the tool errors and failure events of a workflow (cheaper than `read_workflow` when the analyzer only needs the failure surface)
- `list_workflow_sub_agents` -- List sub-agent executions for a workflow with their final reports

### Usage Pattern

The Kanban analyzer typically calls `read_workflow` once to load the report, optionally `list_workflow_errors` if the verdict trends towards `reject` or `needs_improvement`, and `list_workflow_sub_agents` when the workflow used delegation.

---

## 10. ListAgentsTool (private, Kanban only)

**Purpose**: Discovery of available standard agents during the auto-compose flow. Auto-injected on Kanban agents; not visible in the Settings tool picker.

### Operations

- `list` (single op) -- Returns each standard agent with: `id`, `name`, summary of `system_prompt`, available skills, `folders` (FileManager authorized dirs), `has_file_manager`. Kanban-kind agents are filtered out — a Kanban agent cannot be delegated to.

### Rationale

Cloned from `DelegateTaskTool::list_agents` to keep the supervisor's discovery payload identical to the runtime delegation payload. The compose agent picks its target through this tool, then submits the card via `SubmitComposedCardTool`.

---

## 11. SubmitComposedCardTool (private, Kanban only)

**Purpose**: Finalize an auto-composed Kanban card. Single terminal operation that ends the compose iteration loop.

### Operation

- `submit` -- Inputs: `target_agent_id`, `prompt_id` OR `inline_prompt`, `variables: HashMap<String, String>`, `target_folder_id?`, `title`, `description?`. Persists a `kanban_card` row in the `ready` column.

### Variable Contract Validation

Before persisting, the tool computes the set diff between the prompt template's declared variables (from `prompt.variables[].name`) and the keys supplied in the `variables` payload. Mismatches reject the submission with a structured error fed back to the LLM, so the next iteration corrects the call. `inline_prompt` bypasses this check (no contract to enforce when the prompt is ad-hoc).

### Auto-Injection

The tool is added to the agent's toolkit only during the `compose_card_from_description` Tauri command's tool loop, never during normal Kanban-agent execution. Like the analyze flow, the loop forces a tool call on the opening turn (see section 13, `opening_tool_choice`) so the model engages `SubmitComposedCardTool` instead of finishing without a proposal.

---

## 12. SubmitAnalysisTool (private, Kanban only)

**Purpose**: Finalize an analyzer verdict on a completed card report. Single terminal operation that ends the analyze iteration loop.

### Operation

- `submit` -- Inputs: `verdict: "approve" | "reject" | "needs_improvement"`, `summary` (markdown, surfaced in the card report viewer), optional `suggested_prompt_edit` (consumed by the "Improve prompt" modal).

### Auto-Injection

Injected during the `analyze_card_report` command's tool loop. The analyzer is wired to `WorkflowManagerTool` for evidence retrieval and `SubmitAnalysisTool` for the verdict; standard skills/tools are not in scope. The loop runs with a forced tool call on the opening turn (see section 13, `opening_tool_choice`), so a model that would otherwise reply in prose is compelled to submit a verdict rather than leaving the capture slot empty.

### `auto_analyze_reports`

When the target agent on a card has `auto_analyze_reports: true`, the `workflow_complete` listener fires the analyzer automatically after the workflow finishes and transitions the card from `doing` to `review` with the verdict pre-loaded.

---

## 13. Tool Execution (LLMAgent)

**Purpose**: Autonomous tool execution loop for agents.

### Architecture

- `src-tauri/src/agents/llm_agent.rs` -- Struct, constructors
- `src-tauri/src/agents/execution/tools.rs` -- Tool setup, auto-injection
- `src-tauri/src/agents/execution/tool_loop.rs` -- Execution loop
- `src-tauri/src/agents/prompt.rs` -- System prompt construction

### Execution Flow

1. Build system prompt with tool definitions
2. Call LLM provider (Mistral/Ollama/OpenAI-compatible)
3. Parse tool calls from response via `ToolAdapter`
4. Execute tools (local via ToolFactory, MCP via MCPManager)
5. Format results and feed back to LLM
6. Repeat until no tool calls or max iterations reached (default: 50)

### `opening_tool_choice`

`execute_with_tools` takes an `opening_tool_choice` applied to the first iteration only (`tool_choice_for_iteration` reverts to `Auto` afterwards). The standard workflow path passes `Auto` end-to-end. The Kanban analyze and compose flows pass `Required` so the model must emit their mandatory capture-slot tool call (`SubmitAnalysisTool` / `SubmitComposedCardTool`) on the opening turn — a blanket `Auto` could finish with an empty slot, while a blanket `Required` would never let the loop terminate. See `src-tauri/src/agents/execution/tool_loop.rs`.

### Constructor

`LLMAgent::with_context(config, provider_manager, tool_factory, agent_context)` -- Single constructor. The `AgentToolContext` carries shared dependencies (registry, orchestrator, llm_manager, mcp_manager, tool_factory, app_handle, cancellation_token) down to the tools created on each turn. Sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool) are only created when the task context does NOT set `is_sub_agent: true`, enforcing the single-level hierarchy.

### Key Methods

| Method | File | Description |
|--------|------|-------------|
| `create_local_tools()` | `agents/execution/tools.rs` | Creates tool instances + auto-injects ReadSkillTool |
| `collect_tool_definitions()` | `agents/execution/tools.rs` | Collects local + MCP tool definitions for the system prompt |
| `build_system_prompt_with_tools()` | `agents/prompt.rs` | Injects tool definitions into system prompt (rebuilt per turn) |
| `build_initial_messages()` | `agents/execution/tool_loop.rs` | Builds the first message vector: [system, user] (first call) or [system, ...history] (continuation) |
| `adapter.parse_tool_calls()` | `llm/tool_adapter.rs` | Parses tool_calls JSON from LLM response |
| `adapter.format_tool_result()` | `llm/tool_adapter.rs` | Formats results as JSON for LLM |

---

## Orchestration Workflow

Typical agent workflow sequence:

1. **Init**: Agent activates workflow
2. **Plan**: Create tasks with TodoTool
3. **Context**: Load relevant memories via MemoryTool search
4. **Execute**: Progress tasks + write intermediate memories
5. **Communicate**: Generate reports for handoff (multi-agent)
6. **Finalize**: Validate completion, clean up temporary data

---

## References

- [SurrealDB Vector Search](https://surrealdb.com/docs/surrealdb/reference-guide/vector-search)
- [Tauri File System Plugin](https://v2.tauri.app/plugin/file-system/)

---

**Version**: 2.6
