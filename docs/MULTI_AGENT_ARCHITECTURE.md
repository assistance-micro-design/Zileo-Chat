# Multi-Agent Architecture

> **Stack**: Rust + Tauri 2 + MCP + SurrealDB
> **Goal**: Hierarchical system of reusable agents with standardized communication

## Core Principles

### Dynamic Agent Management

**No default agents** - The user creates all agents via the Settings UI.

| Aspect | Detail |
|--------|--------|
| **CRUD** | UI form with LLM config, tools, MCP servers, skills, system prompt |
| **Persistence** | `agent` table in SurrealDB |
| **Unique names** | Case-insensitive, validated on both frontend and backend |
| **Hybrid resolution** | UUID (fast path) or name via `AgentRegistry.get_by_name()` |
| **Loading** | Automatic at startup via `load_agents_from_db()` |

### Agent Hierarchy

The system uses a two-level hierarchy: a primary agent (orchestrator) coordinates permanent specialized agents (persisted in SurrealDB) and temporary agents (limited lifecycle, auto-destroyed, not persisted).

**Primary Agent**:
- Orchestrates complex tasks
- Delegates to specialized agents via sub-agent tools
- Creates temporary agents
- Aggregates reports
- Manages the lifecycle of temporary agents

**Critical rule**: Sub-agents CANNOT spawn other sub-agents. Only the primary orchestrator can spawn and coordinate sub-agents.

---

## Inter-Agent Communication

### Standard Protocol: Markdown Reports

Each agent produces a structured markdown report containing: agent ID, task description, status (Success/Failed/In Progress), duration, structured results, tools used with call counts, MCP servers called, next actions checklist, and metadata (provider, tokens, cost, tool/MCP call counts).

**Advantages**: Human-readable, machine-parsable, chainable (output becomes input), auditable.

### Transport Layer

Inter-process communication via Rust channels (stdio, optimal performance).

---

## Agent Creation

### Via Settings UI (Primary Method)

1. Settings > Agents > Create Agent
2. Configure: name (1-64 chars), lifecycle, provider, model, temperature, max tokens, max tool iterations (1-200), reasoning effort, tools, skills, MCP servers, folders, system prompt

See `src/lib/stores/agents.ts` for frontend store implementation.

### Backend Interface

**Agent Trait** - Core methods:

| Method | Description |
|--------|-------------|
| `execute(task)` | Basic execution |
| `execute_with_mcp(task, mcp_manager)` | Execution with MCP support (primary method) |
| `capabilities()` | List of capabilities |
| `lifecycle()` | Permanent or Temporary |
| `tools()` / `mcp_servers()` | Configured tools and MCP servers |
| `config()` | Full configuration |

**Input/Output types**:

| Type | Role | Key Fields |
|------|------|------------|
| `Task` | Input | id, description, context (JSON) |
| `Report` | Output | task_id, status (Success/Failed), content (markdown), metrics |
| `ReportMetrics` | Metrics | duration_ms, tokens, tools_used, mcp_calls, tool_executions |

**LLMAgent constructors**: `with_tools` (basic tools), `with_factory` (custom factory), `with_context` (primary agent with sub-agent tools).

See `src-tauri/src/agents/` for implementation.

---

## Sub-Agent Delegation

### Task Scoping

The primary agent decomposes complex tasks and delegates them. It analyzes dependencies to determine execution order: independent tasks run in parallel via `futures::join_all`, while data-dependent tasks run sequentially. Results are aggregated into a unified markdown report, followed by cleanup (temporary agents destroyed, reports persisted, metrics logged).

### Sub-Agent Tools

| Tool | Description | Operations |
|------|-------------|------------|
| **SpawnAgentTool** | Creates and executes a temporary sub-agent | spawn, list_children, terminate |
| **DelegateTaskTool** | Sequential delegation to an existing agent (by ID or name) | delegate, list_agents |
| **ParallelTasksTool** | Parallel execution of multiple tasks (by ID or name) | execute_batch |

**Constraints**:
- Maximum 15 sub-agents per workflow (`MAX_SUB_AGENTS`)
- Only accessible when `is_primary_agent = true`
- "Prompt In, Report Out" pattern (no shared context)
- Sub-agents CANNOT spawn other sub-agents

### Task Bridge (TodoTool Integration)

TodoTool tasks are scoped per agent:
- **Primary agent**: Sees all tasks in the workflow
- **Sub-agent**: Only sees tasks assigned to it via `task_ids` in DelegateTask/ParallelTasks

---

## System Prompt Architecture

### Full Prompt Structure

The complete prompt is assembled from: the agent's system prompt, a tools section (summary for local tools, full description for MCP tools), a skills list (instructions loaded via ReadSkillTool), provider/model and MCP delegation info (only if the agent has delegation tools), conversation history, and the specific task.

### System Prompt Anatomy

| Section | Content |
|---------|---------|
| **Role** | Clear definition: who the agent is, its domain of expertise |
| **Expertise** | Specific technical skills |
| **Tools Usage** | For each tool: when, how, limits |
| **MCP Servers Usage** | Capabilities, patterns, scope |
| **Constraints** | Strict rules (NEVER/ALWAYS) |
| **Response Format** | Report structure, expected metrics |

### Prompt Best Practices

1. **Specificity**: Precise role, not generic
2. **Tools First**: Explain WHEN and HOW to use each tool/MCP
3. **Clear Constraints**: NEVER/ALWAYS for strict rules
4. **Structured Format**: Standardized sections
5. **Validation**: Include validation steps
6. **Metrics**: Request specific metrics in reports

---

## Available Tools

### Basic Tools (accessible by all agents)

| Tool | Description | Operations |
|------|-------------|------------|
| **MemoryTool** | Vector persistence | add, get, describe, list, search, delete |
| **TodoTool** | Workflow task management | create, get, update_status, list, complete, delete |
| **CalculatorTool** | Mathematical calculations | unary (sin, cos, tan, sqrt, exp, ln, abs, floor, ceil, round), binary (pow, log, min, max), constant (pi, e, tau) |
| **UserQuestionTool** | Human-in-the-loop questions | ask (checkbox, text, mixed) |
| **FileManagerTool** | Sandboxed file operations | list, read, write, replace, create, delete, move, rename, search_glob, search_content |

### Hidden Tools (auto-injected)

| Tool | Description | Condition |
|------|-------------|-----------|
| **ReadSkillTool** | Reads skill documents | Agent has assigned skills |

### Tool Execution Loop

1. Build system prompt with tool definitions (JSON schema)
2. Call the LLM provider (Mistral/Ollama/Custom)
3. Parse tool calls via `adapter.parse_tool_calls()`
4. Execute: local tools via `ToolFactory`, MCP tools via `MCPManager`
5. Format results via `adapter.format_tool_result()`, feed back to LLM, repeat until `max_tool_iterations` (default: 50) or no more tool calls

### Tool Registry

Global `TOOL_REGISTRY` (`tools/registry.rs`) with 3 categories: Basic, SubAgent, Hidden. Provides validation and tool discovery.

### ToolFactory

Creates tools with context: distinguishes primary agents (with sub-agent tools) from sub-agents (without sub-agent tools) via `is_primary_agent`.

See `src-tauri/src/tools/` for implementation.

---

## Multi-Agent Workflow

### Communication Patterns

| Pattern | Flow |
|---------|------|
| **Request/Response** | Primary -> Task -> Specialized Agent -> Report (MD) -> Primary |
| **Streaming** | Agent -> Stream of chunks -> Primary (SSE via Tauri events) |

### Report Enforcement

The system detects generic reports ("Task completed after N iteration(s)") and forces an additional LLM call with `tools: []` to generate a structured markdown report. Covered by 6 TDD tests.

### Resilience Patterns

| Pattern | Configuration | Description |
|---------|---------------|-------------|
| **Inactivity Timeout** | 300s, heartbeat 30s | Monitoring without cutting legitimate long executions |
| **Retry + Backoff** | 3 attempts, 500ms initial | Retryable errors: timeout, network, rate limit, 502/503/429 |
| **Circuit Breaker** | 3 failures -> 60s cooldown | States: Closed -> Open -> HalfOpen -> Closed |
| **Graceful Cancellation** | CancellationToken propagation | Immediate response, resource cleanup |
| **Hierarchical Tracing** | parent_execution_id | Batch -> task correlation |

### Idempotency and Recovery

- Each subtask has a unique identifier to prevent double execution
- Configurable retry policy (exponential backoff)
- Persistent task journal in SurrealDB

---

## Human-in-the-Loop Validation

| Mode | Behavior |
|------|----------|
| **Auto** | Immediate execution |
| **Manual** | Validation for ALL operations |
| **Selective** | Validation by type (tools, sub_agents, mcp) |

Available overrides: `auto_approve_low` (Manual mode), `always_confirm_high` (Auto mode).

See `src-tauri/src/tools/validation_helper.rs` and `src-tauri/src/commands/validation.rs` for implementation.

---

## State Management

| Type | Storage | Lifecycle |
|------|---------|-----------|
| Permanent Agents | SurrealDB (`agent` table) | Persistent across sessions |
| Temporary Agents | In-memory (Tokio HashMap) | Cleanup on destroy |
| Shared Context Pool | Accessible to all agents | Optimizes token usage |
| Agent-Specific Context | Isolated per agent | Automatic cleanup |

---

## Security

- **Isolation**: Sandboxed tools per agent, permission-based tool access
- **Input Validation**: Strict validation on all inputs
- **Audit Trail**: Agent call chain tracking, request ID propagation
- **Rate Limiting**: Per-agent and per-provider limits

---

## File Structure

| Area | Path |
|------|------|
| Agent core | `src-tauri/src/agents/core/` (agent.rs, registry.rs, orchestrator.rs) |
| LLM agent | `src-tauri/src/agents/llm_agent.rs` |
| Prompt building | `src-tauri/src/agents/prompt.rs` |
| Execution loop | `src-tauri/src/agents/execution/` |
| Tool factory | `src-tauri/src/tools/factory.rs` |
| Tool registry | `src-tauri/src/tools/registry.rs` |
| Basic tools | `src-tauri/src/tools/` (memory, todo, calculator, user_question, file_manager) |
| Hidden tools | `src-tauri/src/tools/read_skill.rs` |
| Sub-agent tools | `src-tauri/src/tools/` (spawn_agent, delegate_task, parallel_tasks, sub_agent_executor, sub_agent_circuit_breaker) |
| Validation | `src-tauri/src/tools/validation_helper.rs` |
| Commands | `src-tauri/src/commands/` (23 modules, 138 commands) |
| Models | `src-tauri/src/models/` |
| LLM providers | `src-tauri/src/llm/` |
| Frontend store | `src/lib/stores/agents.ts` |
| Frontend types | `src/types/agent.ts` |

---

## References

**Protocols**: MCP 2025-06-18, A2A Protocol, JSON-RPC 2.0

**Patterns**: Actor Model, Factory Pattern, Registry Pattern, Strategy Pattern (provider switching), Chain of Responsibility (tool chains)

**Related Docs**: [WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md), [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md), [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)
