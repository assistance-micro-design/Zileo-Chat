# Sub-Agent System User Guide

> A comprehensive guide to understanding and using the sub-agent system in Zileo-Chat-3.

## Overview

The sub-agent system enables the primary workflow agent to orchestrate specialized agents for complex, multi-faceted tasks. This hierarchical approach provides:

- **Task Decomposition**: Break complex problems into manageable pieces
- **Parallel Execution**: Run independent analyses simultaneously
- **Specialized Processing**: Route tasks to domain-specific agents
- **Human Oversight**: Validation requests for critical operations

## Architecture

```
Primary Workflow Agent (Orchestrator)
      |
      +-- SpawnAgentTool -----> Temporary Sub-Agents
      |                              |
      +-- DelegateTaskTool ---> Permanent Agents (db_agent, api_agent, etc.)
      |                              |
      +-- ParallelTasksTool --> Multiple Agents in Parallel
```

### Single-Level Hierarchy

Sub-agents operate at exactly one level below the primary agent:

- Primary agent can spawn/delegate/parallelize
- Sub-agents **cannot** spawn their own sub-agents
- This prevents unbounded recursion and ensures predictable behavior

### Maximum Limits

| Constant | Value | Description |
|----------|-------|-------------|
| `MAX_SUB_AGENTS` | 3 | Maximum sub-agents per workflow |
| `MAX_TASK_DESCRIPTION_LEN` | 10000 | Maximum task description length |
| `MAX_RESULT_SUMMARY_LEN` | 5000 | Maximum result summary length |
| `INACTIVITY_TIMEOUT_SECS` | 300 | Timeout after 5 minutes of inactivity |
| `ACTIVITY_CHECK_INTERVAL_SECS` | 30 | Activity monitoring interval |
| `MAX_RETRY_ATTEMPTS` | 2 | Maximum retry attempts (3 total) |
| `INITIAL_RETRY_DELAY_MS` | 500 | Initial delay before retry |
| `CIRCUIT_FAILURE_THRESHOLD` | 3 | Failures before circuit opens |
| `CIRCUIT_COOLDOWN_SECS` | 60 | Cooldown before retry after circuit open |

- **3 tasks per parallel batch** (matches MAX_SUB_AGENTS)
- Sub-agents have restricted tool access by default (sub-agent tools filtered out)

---

## Sub-Agent Tools

### 1. SpawnAgentTool

Creates temporary sub-agents with custom configurations for specialized tasks.

**Operations**:
| Operation | Description |
|-----------|-------------|
| `spawn` | Create and execute a temporary sub-agent |
| `list_children` | List currently spawned sub-agents for this workflow |
| `terminate` | Force-stop a spawned sub-agent |

**When to Use**:
- Need a specialized agent with specific capabilities
- Task requires custom tools or MCP servers
- One-time analysis that doesn't need a permanent agent

**Parameters** (spawn operation):
| Parameter | Required | Default | Description |
|-----------|----------|---------|-------------|
| `name` | Yes | - | Sub-agent name |
| `prompt` | Yes | - | Complete prompt (only input sub-agent receives) |
| `system_prompt` | No | Default sub-agent prompt | Custom system prompt |
| `tools` | No | Parent's tools (minus sub-agent tools) | Tools list |
| `mcp_servers` | No | Parent's MCP servers | MCP servers |
| `provider` | No | Parent's provider | LLM provider |
| `model` | No | Parent's model | Model ID |

**Example Prompt for Primary Agent**:
```
Use the spawn tool to create a CodeAnalyzer agent that:
- Name: "Security Analyzer"
- Task: Analyze the authentication module for vulnerabilities
- Tools: MemoryTool (to store findings), TodoTool (to track issues)
- MCP Servers: serena (for code analysis)
```

**Tool Input**:
```json
{
  "operation": "spawn",
  "name": "Security Analyzer",
  "prompt": "Analyze the authentication module in src/auth/ for security vulnerabilities. Check for: 1) SQL injection, 2) XSS, 3) CSRF. Report findings in markdown format.",
  "tools": ["MemoryTool", "TodoTool"],
  "mcp_servers": ["serena"]
}
```

**Result**:
```json
{
  "success": true,
  "child_id": "sub_abc123",
  "report": "# Security Analysis Report\n\n## Findings\n...",
  "metrics": {
    "duration_ms": 15000,
    "tokens_input": 2500,
    "tokens_output": 1200
  }
}
```

---

### 2. DelegateTaskTool

Routes tasks to existing permanent agents already configured in the system.

**Operations**:
| Operation | Description |
|-----------|-------------|
| `delegate` | Execute a task via an existing permanent agent |
| `list_agents` | List available agents for delegation (excludes self and temporary agents) |

**When to Use**:
- Task fits a permanent agent's specialty
- Don't need custom configuration
- Want to leverage pre-configured agent capabilities

**Example Prompt for Primary Agent**:
```
Delegate the database optimization task to the db_agent:
- Analyze current query performance
- Identify slow queries
- Suggest index improvements
```

**Tool Input**:
```json
{
  "operation": "delegate",
  "agent_id": "db_agent",
  "prompt": "Analyze the database query performance. Identify the top 5 slowest queries and suggest optimization strategies including potential index additions."
}
```

**Result**:
```json
{
  "success": true,
  "agent_id": "db_agent",
  "report": "# Database Optimization Report\n\n## Slow Queries\n...",
  "metrics": {
    "duration_ms": 8000,
    "tokens_input": 1800,
    "tokens_output": 950
  }
}
```

---

### 3. ParallelTasksTool

Executes multiple independent tasks simultaneously across different agents.

**When to Use**:
- Multiple independent analyses needed
- Time-sensitive operations
- Tasks don't depend on each other's results

**Example Prompt for Primary Agent**:
```
Run a comprehensive code review using parallel analysis:
1. db_agent: Review database migrations
2. api_agent: Check API endpoint security
3. ui_agent: Audit frontend accessibility
```

**Tool Input**:
```json
{
  "operation": "execute_batch",
  "tasks": [
    {
      "agent_id": "db_agent",
      "prompt": "Review database migrations in db/migrations/. Check for: rollback safety, data integrity, performance impact."
    },
    {
      "agent_id": "api_agent",
      "prompt": "Audit API endpoints in src/api/. Check for: authentication, authorization, input validation, rate limiting."
    },
    {
      "agent_id": "ui_agent",
      "prompt": "Check frontend accessibility in src/components/. Verify: ARIA labels, keyboard navigation, color contrast, screen reader support."
    }
  ]
}
```

**Result**:
```json
{
  "success": true,
  "completed": 3,
  "failed": 0,
  "results": [
    {"agent_id": "db_agent", "success": true, "report": "...", "metrics": {...}},
    {"agent_id": "api_agent", "success": true, "report": "...", "metrics": {...}},
    {"agent_id": "ui_agent", "success": true, "report": "...", "metrics": {...}}
  ],
  "aggregated_report": "# Parallel Execution Report\n\n## Agent: db_agent\n..."
}
```

**Performance Benefit**: Total time is approximately equal to the slowest task, not the sum of all tasks.

---

## Human-in-the-Loop Validation

Sub-agent operations trigger validation requests based on risk level:

| Operation | Risk Level | Validation Behavior |
|-----------|------------|---------------------|
| Spawn     | Medium     | Optional validation |
| Delegate  | Medium     | Optional validation |
| Parallel  | High       | Required validation |

### Validation Flow

1. Agent initiates sub-agent operation
2. Backend emits `validation_required` event
3. Frontend displays approval dialog
4. User approves or rejects
5. Operation proceeds or is cancelled

### Validation Event Details

```typescript
interface ValidationRequiredEvent {
  validation_id: string;
  workflow_id: string;
  operation_type: 'spawn' | 'delegate' | 'parallel_batch';
  operation: string;  // Human-readable description
  risk_level: 'low' | 'medium' | 'high';
  details: {
    sub_agent_name?: string;
    prompt_preview?: string;
    prompt_length?: number;
    tools?: string[];
    mcp_servers?: string[];
    target_agent_id?: string;
    task_count?: number;
    tasks?: Array<{agent_id: string; prompt_preview: string}>;
  };
}
```

---

## Streaming Events

Monitor sub-agent execution in real-time via streaming events:

### Event Types

| Event Type | When Emitted | Payload Contains |
|------------|--------------|------------------|
| `sub_agent_start` | Execution begins | agent ID, name, task description |
| `sub_agent_progress` | Periodic updates | progress %, status message |
| `sub_agent_complete` | Success | report, metrics |
| `sub_agent_error` | Failure | error message, duration |

### Listening for Events

```typescript
import { listen } from '@tauri-apps/api/event';

// Subscribe to workflow stream
const unlisten = await listen('workflow_stream', (event) => {
  const chunk = event.payload;

  switch (chunk.chunk_type) {
    case 'sub_agent_start':
      console.log(`Sub-agent ${chunk.sub_agent_name} started`);
      break;
    case 'sub_agent_progress':
      console.log(`Progress: ${chunk.progress}%`);
      break;
    case 'sub_agent_complete':
      console.log(`Report: ${chunk.content}`);
      console.log(`Duration: ${chunk.metrics.duration_ms}ms`);
      break;
    case 'sub_agent_error':
      console.error(`Error: ${chunk.content}`);
      break;
  }
});
```

---

## Best Practices

### 1. Prompt Engineering for Sub-Agents

Each sub-agent receives **only its prompt** with no shared context:

```
Include in every prompt:
- Complete context needed for the task
- Specific deliverables expected
- Output format requirements (markdown, JSON, etc.)
- Any constraints or boundaries
```

**Example**:
```
You are analyzing the user authentication module.

CONTEXT:
- Framework: SvelteKit 2.x with TypeScript
- Auth library: Lucia Auth
- Database: SurrealDB

TASK:
Analyze src/lib/auth/ for security vulnerabilities.

DELIVERABLES:
1. List of vulnerabilities found (severity: high/medium/low)
2. Code snippets showing the issue
3. Recommended fixes with example code

OUTPUT FORMAT:
Markdown report with headings for each vulnerability.
```

### 2. Tool Selection for Spawned Agents

Be selective with tools to:
- Reduce overhead
- Limit potential impact
- Focus agent capabilities

```json
{
  "tools": ["MemoryTool"],  // Only what's needed
  "mcp_servers": ["serena"] // Only required servers
}
```

### 3. Parallel Task Independence

Ensure parallel tasks don't depend on each other:

```
INDEPENDENT (can run in parallel):
- Analyze database schema
- Review API endpoints
- Check UI accessibility

NOT INDEPENDENT (must be sequential):
- Create database migration
- Apply migration
- Test migration
```

### 4. Error Handling

Always check results for partial failures in parallel execution:

```typescript
const result = await parallelTasksTool.execute({...});

if (!result.success) {
  // Handle partial failures
  for (const taskResult of result.results) {
    if (!taskResult.success) {
      console.error(`${taskResult.agent_id} failed: ${taskResult.error}`);
    }
  }
}
```

---

## TypeScript Types

Import types from `$types/sub-agent`:

```typescript
import type {
  SubAgentStatus,
  SubAgentExecution,
  SubAgentMetrics,
  SubAgentSpawnResult,
  DelegateResult,
  ParallelBatchResult,
  ParallelTaskResult,
  SubAgentEventType,
  SubAgentStreamEvent,
  ValidationRequiredEvent,
  RiskLevel
} from '$types/sub-agent';

// Constants
import { SUB_AGENT_CONSTANTS, SUB_AGENT_EVENTS, VALIDATION_EVENTS } from '$types/sub-agent';

console.log(SUB_AGENT_CONSTANTS.MAX_SUB_AGENTS); // 3
```

---

## Database Persistence

Sub-agent executions are persisted in SurrealDB:

```surql
SELECT * FROM sub_agent_execution
WHERE workflow_id = 'wf_123'
ORDER BY created_at DESC;
```

**Schema**:
```surql
DEFINE TABLE sub_agent_execution SCHEMAFULL;
DEFINE FIELD workflow_id ON sub_agent_execution TYPE string;
DEFINE FIELD parent_agent_id ON sub_agent_execution TYPE string;
DEFINE FIELD sub_agent_id ON sub_agent_execution TYPE string;
DEFINE FIELD sub_agent_name ON sub_agent_execution TYPE string;
DEFINE FIELD task_description ON sub_agent_execution TYPE string;
DEFINE FIELD status ON sub_agent_execution TYPE string;  -- pending | running | completed | error | cancelled
DEFINE FIELD duration_ms ON sub_agent_execution TYPE option<int>;
DEFINE FIELD tokens_input ON sub_agent_execution TYPE option<int>;
DEFINE FIELD tokens_output ON sub_agent_execution TYPE option<int>;
DEFINE FIELD result_summary ON sub_agent_execution TYPE option<string>;
DEFINE FIELD error_message ON sub_agent_execution TYPE option<string>;
DEFINE FIELD created_at ON sub_agent_execution TYPE datetime;
DEFINE FIELD completed_at ON sub_agent_execution TYPE option<datetime>;
```

---

## Tauri Commands

Frontend can interact with sub-agent execution history via Tauri IPC:

### load_workflow_sub_agent_executions

Loads all sub-agent executions for a workflow.

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { SubAgentExecution } from '$types/sub-agent';

const executions = await invoke<SubAgentExecution[]>(
  'load_workflow_sub_agent_executions',
  { workflowId: 'wf_123' }
);
// Returns executions sorted by created_at (oldest first)
```

### clear_workflow_sub_agent_executions

Deletes all sub-agent executions for a workflow.

```typescript
const deletedCount = await invoke<number>(
  'clear_workflow_sub_agent_executions',
  { workflowId: 'wf_123' }
);
console.log(`Deleted ${deletedCount} executions`);
```

---

## Resilience Features

The sub-agent system includes several resilience mechanisms to handle failures gracefully.

### Inactivity Timeout with Heartbeat (OPT-SA-1)

Sub-agents are monitored for activity to detect true hangs without cutting legitimate long-running operations:

- **Activity Check**: Every 30 seconds, the system checks for recent activity
- **Timeout Trigger**: If no activity for 300 seconds (5 minutes), execution is aborted
- **What Counts as Activity**: LLM token received, tool call started/completed, MCP server response

```
Execution starts
    |
    v
[Activity detected?] --YES--> Reset inactivity counter (300s)
    |                              |
    NO                             v
    |                         Continue execution
    v                              |
[Inactivity > 300s?] --YES--> ABORT (truly blocked)
    |
    NO
    v
Continue monitoring...
```

### Retry with Exponential Backoff (OPT-SA-10)

Transient failures are automatically retried with exponential backoff:

- **Max Attempts**: 3 (initial + 2 retries)
- **Backoff Schedule**: 500ms, 1000ms, 2000ms
- **Total Max Delay**: 1500ms

**Retryable Errors** (automatically retried):
- Timeout, connection refused, network errors
- HTTP 502, 503, 429 (rate limit)
- "Service unavailable", "server busy"

**Non-Retryable Errors** (fail immediately):
- Cancelled, permission denied, not found
- Invalid input, authentication errors
- Circuit breaker open

### Circuit Breaker (OPT-SA-8)

The circuit breaker pattern prevents cascade failures:

| State | Description | Behavior |
|-------|-------------|----------|
| **Closed** | Normal operation | Requests allowed |
| **Open** | System unhealthy (3 failures) | Requests rejected for 60s cooldown |
| **HalfOpen** | Testing recovery | Allows one request through |

```
[Closed] --3 failures--> [Open] --60s cooldown--> [HalfOpen]
    ^                                                   |
    |                                                   |
    +------------ success <--- test request ------------+
    |                                                   |
    +------------ [Open] <--- test fails ---------------+
```

### Graceful Cancellation (OPT-SA-7)

Sub-agent executions can be cancelled gracefully via CancellationToken:

- User cancels workflow in UI
- CancellationToken propagates to all active sub-agents
- Sub-agents respond immediately instead of waiting for timeout
- Clean shutdown with proper resource cleanup

### Hierarchical Tracing (OPT-SA-11)

Parallel batch executions use correlation IDs for tracing:

- **batch_id**: Unique ID for the parallel batch operation
- **parent_execution_id**: Links individual tasks to their batch
- Enables hierarchical log aggregation and debugging

```typescript
// Logs show correlation
[batch_id: "batch_abc123"]
  └── [execution_id: "exec_001", parent: "batch_abc123"] Task 1
  └── [execution_id: "exec_002", parent: "batch_abc123"] Task 2
  └── [execution_id: "exec_003", parent: "batch_abc123"] Task 3
```

---

## Troubleshooting

### "Only the primary workflow agent can spawn sub-agents"

**Cause**: A sub-agent attempted to spawn another sub-agent.

**Solution**: Ensure only the primary agent uses SpawnAgentTool/DelegateTaskTool/ParallelTasksTool.

### "Maximum 3 sub-agents exceeded"

**Cause**: Workflow already has 3 active sub-agents.

**Solution**: Wait for current sub-agents to complete or terminate one before spawning more.

### "Cannot delegate to self"

**Cause**: Agent tried to delegate to its own ID.

**Solution**: Use a different agent ID or spawn a new sub-agent instead.

### "Agent not found in registry"

**Cause**: Target agent ID doesn't exist.

**Solution**: Use `list_agents` operation to see available agents.

### "Sub-agent inactive for X seconds"

**Cause**: Sub-agent produced no activity (tokens, tool calls) for the inactivity timeout period.

**Solution**:
- Check if the LLM provider is responding
- Verify MCP servers are accessible
- Consider increasing complexity of task to avoid long idle periods

### "Circuit breaker open"

**Cause**: The sub-agent execution system has detected 3 consecutive failures and entered protective mode.

**Solution**:
- Wait 60 seconds for automatic recovery
- Check underlying services (LLM provider, MCP servers)
- Review error logs for root cause

### "Execution cancelled"

**Cause**: User or system cancelled the workflow while sub-agent was executing.

**Solution**: This is expected behavior when cancellation is requested. Re-run the workflow if needed.

---

## Examples

### Full Codebase Audit

```
Primary Agent Prompt:
"Perform a comprehensive audit of the codebase using parallel analysis."

Internal tool calls:
1. ParallelTasksTool.execute_batch([
     {agent: "security_agent", prompt: "Check for OWASP Top 10..."},
     {agent: "performance_agent", prompt: "Profile hot paths..."},
     {agent: "accessibility_agent", prompt: "WCAG 2.1 compliance..."}
   ])

2. SpawnAgentTool.spawn({
     name: "Report Synthesizer",
     prompt: "Combine all audit reports into executive summary..."
   })
```

### Sequential Pipeline with Delegation

```
Primary Agent Flow:
1. DelegateTaskTool.delegate("db_agent", "Export schema...")
2. Use result to inform next step
3. DelegateTaskTool.delegate("api_agent", "Generate API from schema...")
4. Compile final documentation
```

---

## Related Documentation

- [API Reference](./API_REFERENCE.md) - Complete tool specifications
- [Multi-Agent Architecture](./MULTI_AGENT_ARCHITECTURE.md) - System design
- [Agent Tools Documentation](./AGENT_TOOLS_DOCUMENTATION.md) - All available tools
- [Testing Strategy](./TESTING_STRATEGY.md) - Testing sub-agent workflows
