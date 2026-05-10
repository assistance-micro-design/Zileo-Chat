# Intra-Workflow Orchestration

> **Goal**: How the primary agent determines parallel or sequential execution of operations within a workflow

**Status**: Implementation Complete (Phase 5 + Background Execution + Security Audit)
**Version**: 2.4 | **Last Updated**: 2026-03-26

---

## Core Principles

### Dependency Analysis

The primary agent evaluates each operation based on:
- Required input data
- Produced output data
- Relationships between operations

**Decision**: Independent operations -> Parallel. Operation B requires result from A -> Sequential.

### Architectural Limitation

Sub-agents CANNOT spawn other sub-agents. Only the primary orchestrator can spawn and coordinate sub-agents. Reasons: reusability, centralized control, simpler debugging.

---

## Backend Implementation

### Tauri Commands (11 total)

| Command | Description | File |
|---------|-------------|------|
| `create_workflow` | Create a workflow | commands/workflow.rs |
| `load_workflows` | Load all workflows | commands/workflow.rs |
| `rename_workflow` | Rename a workflow | commands/workflow.rs |
| `delete_workflow` | Delete a workflow | commands/workflow.rs |
| `load_workflow_full_state` | Load full state (messages, tools, thinking) | commands/workflow.rs |
| `delete_workflows_batch` | Batch deletion | commands/workflow.rs |
| `move_workflow_to_folder` | Move to folder | commands/workflow.rs |
| `move_workflows_to_folder` | Batch move to folder | commands/workflow.rs |
| `toggle_workflow_pinned` | Pin/unpin | commands/workflow.rs |
| `execute_workflow_streaming` | Execute with SSE streaming | commands/streaming/execution.rs |
| `cancel_workflow_streaming` | Cancel running execution | commands/streaming/execution.rs |

### Tauri Events (8 total)

| Event | Payload | Description |
|-------|---------|-------------|
| `workflow_stream` | StreamChunk | Real-time streaming (tokens, tool calls, reasoning) |
| `workflow_complete` | WorkflowComplete | Completion (completed, error, cancelled) |
| `validation_required` | ValidationRequiredEvent | Human-in-the-loop request |
| `validation_response` | ValidationResponseEvent | User approval/rejection |
| `sub_agent_start` | StreamChunk | Sub-agent started |
| `sub_agent_progress` | StreamChunk | Sub-agent progress update |
| `sub_agent_complete` | StreamChunk | Sub-agent completed with report |
| `sub_agent_error` | StreamChunk | Sub-agent failed |

### Orchestrator

`AgentOrchestrator` wraps `AgentRegistry` and provides:

| Method | Description |
|--------|-------------|
| `execute(agent_id, task)` | Legacy execution (delegates to execute_with_mcp) |
| `execute_with_mcp(agent_id, task, mcp_manager)` | Production execution with MCP support |
| `execute_parallel(tasks)` | Parallel execution via `futures::join_all()` |

See `src-tauri/src/agents/core/orchestrator.rs` for implementation.

### Key Models

| Model | Key Fields |
|-------|------------|
| `WorkflowStatus` | idle, running, completed, error |
| `Workflow` | id, name, agent_id, status, timestamps, token counts, cost |
| `WorkflowResult` | report (MD), metrics, tools_used, mcp_calls, tool_executions |
| `WorkflowMetrics` | duration_ms, tokens, cost_usd, provider, model |
| `WorkflowFullState` | workflow, messages, tool_executions, thinking_steps |
| `StreamChunk` | workflow_id, chunk_type, content, tool, duration, sub_agent info, task info, tokens |
| `WorkflowComplete` | workflow_id, status (completed/error/cancelled), error |

See `src-tauri/src/models/workflow.rs` and `src-tauri/src/models/streaming.rs` for definitions.

---

## Frontend Implementation

### Stores

| Store | Purpose | Key Exports |
|-------|---------|-------------|
| `workflowStore` | Workflow CRUD + selection | workflows, selectedWorkflow, filteredWorkflows |
| `backgroundWorkflowsStore` | Concurrent background execution (central dispatch, owns per-workflow `WorkflowStreamState`) | runningWorkflows, recentlyCompletedWorkflows, canStartNew, questionPendingIds, getExecution, setViewed |
| `executionBlocksStore` | Per-workflow display blocks (replay buffer for the viewed workflow) | restoreFromChunks |
| `toastStore` | Toast notifications | toasts, visibleToasts, navigationTarget |

See `src/lib/stores/` for store implementations.

### Types

| File | Key Types |
|------|-----------|
| `workflow.ts` | WorkflowStatus, Workflow, WorkflowResult, WorkflowMetrics, WorkflowFullState, TokenDisplayData |
| `streaming.ts` | ChunkType (16 variants), StreamChunk, WorkflowComplete, SubAgentStreamMetrics |
| `background-workflow.ts` | WorkflowStreamState, Toast, BackgroundWorkflowStatus |

See `src/types/` for type definitions.

### Components

| Area | Components |
|------|------------|
| **Workflow** | WorkflowItem, WorkflowList, WorkflowItemCompact, NewWorkflowModal, ValidationModal, TokenDisplay, MetricsBar, AgentSelector, FolderItem, StatusFilters, UserQuestionModal |
| **Layout** | WorkflowSidebar, ChatContainer, AgentHeader |
| **UI** | ToastContainer (global overlay), ToastItem |

Layout: 2 columns (WorkflowSidebar + Chat/Agent Interface). Activity info displayed inline via the block-by-block system.

### Services

| Service | Key Methods |
|---------|-------------|
| `WorkflowService` | loadAll, create, rename, delete, executeStreaming, cancel, getFullState, restoreState |
| `WorkflowExecutorService` | execute (with canStart check, background registration, isStillViewed guard) |

---

## Execution Flows

### Streaming Execution

The streaming execution flow proceeds through these steps: validate inputs (workflow_id, message, agent_id), check concurrent limit (max 3), create cancellation token, load workflow from SurrealDB, generate message ID, emit initial events, persist initial thinking step, load conversation history (last 50 messages, filtered to `user`/`assistant` roles only — `system` rows are frontend error notifications and must never be replayed to the LLM), create Task with history context. Then execute with cancellation: race `execute_with_mcp()` vs cancellation token. On cancel: emit cancelled event, cleanup, return. On success: emit completion reasoning, stream response content (50-char chunks, 10ms delay), load model for pricing, calculate cost, update workflow cumulative metrics, persist tool executions, emit `WorkflowComplete::success()`, cleanup cancellation token.

The system prompt is **rebuilt every turn** by `build_system_prompt_with_tools` (it depends on live agent config — tools, MCP servers, locale, current date) and is therefore never persisted. In continuation mode, `build_initial_messages` replays the persisted history as-is under the regenerated system prompt, without re-appending `task.description` (the frontend already saved the current user turn before streaming).

See `src-tauri/src/commands/streaming/execution.rs`, `src-tauri/src/commands/streaming/helpers.rs::load_conversation_history`, and `src-tauri/src/agents/execution/tool_loop.rs::build_initial_messages` for the full implementation.

### Parallel Execution

`Orchestrator.execute_parallel(tasks)` maps each (agent_id, task) pair to an async execution, runs all concurrently via `futures::join_all()`, and returns `Vec<Result<Report>>` in input order. Total time approximates the slowest individual task.

### Cancellation

Cancellation validates the workflow_id, triggers the cancellation token, which `tokio::select!` detects in the execute loop. Execution halts immediately (mid-LLM-call if needed), emits a cancelled event, and clears the token.

---

## Human-in-the-Loop Validation

### Modes

| Mode | Behavior |
|------|----------|
| **Auto** | Immediate execution |
| **Manual** | Validation for all operations |
| **Selective** | Per type: tools, sub_agents, mcp (individually configurable) |

### Risk Overrides

| Option | Effect |
|--------|--------|
| `auto_approve_low` | In Manual mode, auto-approves Low risk operations |
| `always_confirm_high` | In Auto mode, forces validation for High risk operations |

### Validation Flow

When an agent detects an operation requiring validation, the ValidationHelper checks settings. If not needed, it executes immediately. If needed, it creates a ValidationRequest in the DB (pending), emits a `validation_required` event to the frontend, and the UI displays a ValidationModal. The user approves or rejects. The frontend calls `respond_to_validation`, updates the status in DB. If approved, execution continues. If rejected, an error is returned and the workflow stops.

See `src-tauri/src/tools/validation_helper.rs` for implementation.

---

## Orchestration Patterns

### Pattern 1: Fan-Out / Fan-In

Parallel operations followed by aggregation. The orchestrator dispatches independent tasks to multiple agents (e.g., DB agent, API agent, MCP server) in parallel, then sequentially aggregates results for decision-making.

### Pattern 2: Sequential Pipeline

Chained transformations where each step feeds into the next. Example: find symbols (MCP) -> validate refactor (tool) -> apply refactor (agent) -> save changes (tool).

### Pattern 3: Hybrid

Mix of parallel and sequential. Example: fetch documentation from multiple sources in parallel, then sequentially generate a component using those docs, then run accessibility checks in parallel, then aggregate validation results into a final report.

---

## Error Handling

### By Execution Type

**Parallel**: Partial failure is acceptable if not critical. Continues with available results. Logs errors for review. Fails only if ALL results fail.

**Sequential**: Failure triggers immediate pipeline halt (fail-fast). Rollback if necessary. User notification.

### Retry Logic

- **Idempotent operations**: Automatic retry with exponential backoff
- **Non-idempotent operations**: No automatic retry, human validation if critical

---

## Performance

### Timeouts

| Constant | Value | Usage |
|----------|-------|-------|
| `LLM_EXECUTION_TIMEOUT_SECS` | 300 (5 min) | LLM execution |
| `DB_OPERATION_TIMEOUT_SECS` | 30 | Database operations |
| `FULL_STATE_LOAD_TIMEOUT_SECS` | 60 | Full state loading |
| `MESSAGE_HISTORY_LIMIT` | 50 | Max messages in context |

See `src-tauri/src/constants.rs` for all constants.

---

## Background Workflow Execution

### Concurrency Limits

| Validation Mode | Max Concurrent |
|----------------|----------------|
| Auto | 3 |
| Manual | 1 |
| Selective | 1 |

Enforced at two levels: frontend (`backgroundWorkflowsStore.canStart()`) and backend safety net.

### Architecture

The background execution architecture centers on `backgroundWorkflowsStore` as the central dispatch. It owns global Tauri event listeners, maintains a `Map<workflowId, WorkflowStreamState>`, routes chunks to the viewed workflow via callbacks, fires toasts for non-viewed workflow events, and enforces concurrency limits. The `ToastContainer` is mounted globally in `+layout.svelte`. The agent page initializes the store and sets forward callbacks (onChunk, onComplete, onUserQuestion). `WorkflowSidebar` receives running, recently completed, and question-pending workflow IDs as props.

The `toastStore` provides `addWorkflowComplete()` (success/error toast with 5s auto-dismiss), `addUserQuestion()` (persistent toast), and `requestNavigation()` (sets navigationTarget for page reaction).

### Visual Indicators

| Indicator | Appearance |
|-----------|------------|
| Running workflow | Green pulsing dot (8px, 2s animation) |
| Running (compact) | Pulsing box-shadow ring (green glow) |
| Pending question | Small orange dot (6px) top-right corner |
| Sidebar sections | "Running" (green) and "Recently Completed" (gray) headers |

### Workflow Switching

1. `setViewed(workflowId)` updates view tracking
2. `getExecution(workflowId)` retrieves WorkflowStreamState
3. `executionBlocksStore.restoreFromChunks(workflowId, chunkHistory)` rebuilds the block UI from the replay buffer
4. If `hasPendingQuestion`, opens UserQuestionModal

### Cleanup

Completed executions auto-removed after 10 minutes (`CLEANUP_INTERVAL_MS = 600000`).

---

## Best Practices

### DO

- Analyze dependencies before execution
- Maximize parallelism when there are no dependencies
- Batch similar operations
- Use adaptive timeouts based on performance history
- Cache deterministic results
- Use detailed logging for debugging
- Fail-fast on critical errors in sequential flows

### DON'T

- Have sub-agents spawn sub-agents (architecture rule violation)
- Parallelize with dependencies (incorrect results)
- Ignore parallel errors (validate partial results)
- Use uniform timeouts (adjust based on operation type)
- Overload parallelism (limit based on resources)
- Nest excessively (max 3 levels)

---

## File Locations

| Area | Path |
|------|------|
| Backend Commands | `src-tauri/src/commands/workflow.rs`, `commands/streaming/` |
| Orchestrator | `src-tauri/src/agents/core/orchestrator.rs` |
| Constants | `src-tauri/src/constants.rs` (workflow module) |
| Models | `src-tauri/src/models/workflow.rs`, `models/streaming.rs` |
| Frontend Stores | `src/lib/stores/workflows.ts`, `background-workflows.ts`, `execution-blocks.ts`, `toast.ts` |
| Frontend Types | `src/types/workflow.ts`, `streaming.ts`, `background-workflow.ts` |
| Frontend Services | `src/lib/services/workflow.service.ts`, `workflowExecutor.service.ts` |
| Components | `src/lib/components/workflow/`, `components/agent/`, `components/ui/` |

---

## References

- [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
- [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)
- [API_REFERENCE.md](API_REFERENCE.md)
- [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)
