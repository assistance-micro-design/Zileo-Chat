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
| **Tool Execution** | `src-tauri/src/agents/llm_agent.rs` |

**Note**: DB tools (SurrealDBTool, QueryBuilderTool, AnalyticsTool) were removed -- DB access goes through Tauri IPC commands.

### Tool Categories

- **Basic**: MemoryTool, TodoTool, CalculatorTool (no special context required)
- **Interaction**: UserQuestionTool (human-in-the-loop)
- **File**: FileManagerTool (sandboxed filesystem operations)
- **Sub-Agent**: SpawnAgentTool, DelegateTaskTool, ParallelTasksTool (require AgentToolContext)
- **Hidden**: ReadSkillTool (auto-injected when agent has skills, not shown in UI)

### Sub-Agent Resilience

- Inactivity timeout with heartbeat (300s timeout, 30s check interval)
- Retry with exponential backoff (3 attempts, 500ms-2000ms)
- Circuit breaker (3 failures to open, 60s cooldown)
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
- `list` -- List workflow tasks (optional `status_filter`)
- `complete` -- Mark complete (`task_id`, optional `duration_ms`)
- `delete` -- Delete task (`task_id`)

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

- **Database**: SurrealDB with HNSW vector indexing (1024D)
- **Search**: Composite scoring -- `cosine_similarity * 0.7 + importance * 0.15 + recency * 0.15`
- **Embedding**: Multi-provider abstraction (`src-tauri/src/llm/embedding/`) -- Mistral (1024D), Ollama (768D/1024D)

### Operations

- `describe` -- Discovery: memory stats by type/scope
- `add` -- Add memory with embedding + auto-scoping (`type`, `content`)
- `get` -- Read by ID (`memory_id`)
- `list` -- List with filters (mode `compact` or `full`)
- `search` -- Semantic search (`query`, optional `limit`, `threshold`)
- `delete` -- Delete by ID (`memory_id`)
- `clear_by_type` -- Bulk delete by type (`type`)

### Auto-Scoping

Scope is determined automatically by memory type:
- `user_pref`, `knowledge` -- general scope (cross-workflow)
- `context`, `decision` -- workflow scope (tied to current workflow)
- Override possible via the `scope` parameter

### Example

```json
{ "operation": "search", "query": "vector database indexing", "limit": 5 }
```

### Key Details

- **Default importance**: user_pref=0.8, decision=0.7, knowledge=0.6, context=0.3
- **Auto TTL**: `context` memories expire after 7 days
- **Embedding optional**: If no embedding service, memories are stored without vectors (text search only)
- **Security**: All DB queries use bind parameters. See `src-tauri/src/tools/memory/` for implementation.

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

**Purpose**: Allow agents to read skill documents containing instructions and context.

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

- `tool.rs` -- Struct + Tool trait (10 operations)
- `security.rs` -- Path validation, sandbox enforcement
- `helpers.rs` -- File info formatting, text detection, constants
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

### Security and Safety

- All paths are validated and canonicalized against the agent's authorized folders
- Destructive operations (write overwrite, delete, move, rename) create backups in `.zileo-trash/`
- If `require_file_confirmation` is enabled, destructive operations go through the ValidationHelper system
- Trash cleanup is lazy (triggered on first destructive operation)

### Limits

- Max file read size: 10 MB
- Max list entries: 200
- Max search results: 100
- Trash retention: 7 days, max 500 MB

---

## 7. Tool Execution (LLMAgent)

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

### Constructors

- `LLMAgent::with_factory(config, provider_manager, tool_factory)` -- Local tools only
- `LLMAgent::with_context(config, provider_manager, tool_factory, agent_context)` -- With sub-agent tools

### Key Methods

| Method | File | Description |
|--------|------|-------------|
| `build_tools_section()` | `agents/execution/tools.rs` | Creates tool instances + auto-injects ReadSkillTool |
| `build_system_prompt()` | `agents/prompt.rs` | Injects tool definitions into system prompt |
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
