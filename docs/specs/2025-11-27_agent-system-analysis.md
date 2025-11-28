# Agent System Analysis - Current State

> **Date**: 2025-11-27
> **Status**: Analysis of existing implementation
> **Purpose**: Document sub-agent creation and execution patterns

---

## Executive Summary

The agent system has foundational infrastructure for multi-agent orchestration, but key features remain unused or unimplemented. The `execute_parallel()` method exists but is marked as dead code. Sub-agent spawning during workflow execution is not yet possible.

---

## 1. Agent Lifecycle Types

### Implementation Status: COMPLETE

**Location**: `src-tauri/src/models/agent.rs`

```rust
pub enum Lifecycle {
    Permanent,  // Persists in DB, survives restarts
    Temporary,  // Memory-only, cleaned after use
}
```

### Lifecycle Behavior

| Type | Storage | Cleanup | Use Case |
|------|---------|---------|----------|
| `Permanent` | SurrealDB `agent` table | Manual delete only | User-created agents via Settings |
| `Temporary` | In-memory registry only | Auto-cleanup via `cleanup_temporary()` | Sub-agents spawned during workflow |

### AgentRegistry Methods

**Location**: `src-tauri/src/agents/core/registry.rs`

| Method | Description | Lifecycle Restriction | Status |
|--------|-------------|----------------------|--------|
| `register(id, agent)` | Adds agent to registry | None | ACTIVE |
| `get(id)` | Retrieves agent by ID | None | ACTIVE |
| `list()` | Lists all agent IDs | None | ACTIVE |
| `unregister(id)` | Removes agent | **Temporary ONLY** | ACTIVE |
| `unregister_any(id)` | Removes any agent | None (for CRUD) | ACTIVE |
| `cleanup_temporary()` | Removes all temporary agents | Temporary only | `#[allow(dead_code)]` |

---

## 2. Agent Execution Patterns

### Implementation Status: PARTIAL

**Location**: `src-tauri/src/agents/core/orchestrator.rs`

### AgentOrchestrator Methods

| Method | Signature | Description | Status |
|--------|-----------|-------------|--------|
| `execute` | `(agent_id, task) -> Report` | Single agent, no MCP | ACTIVE (legacy) |
| `execute_with_mcp` | `(agent_id, task, mcp_manager) -> Report` | Single agent with MCP tools | ACTIVE |
| `execute_parallel` | `(Vec<(agent_id, task)>) -> Vec<Report>` | Multiple agents in parallel | `#[allow(dead_code)]` |

### Parallel Execution Code (Unused)

```rust
pub async fn execute_parallel(
    &self,
    tasks: Vec<(String, Task)>, // Vec<(agent_id, task)>
) -> Vec<anyhow::Result<Report>> {
    use futures::future::join_all;

    let futures = tasks.into_iter().map(|(agent_id, task)| {
        let registry = self.registry.clone();
        async move {
            let agent = registry.get(&agent_id).await?;
            agent.execute(task).await
        }
    });

    join_all(futures).await
}
```

**Note**: This method uses `futures::join_all` for true parallel execution but is never called from any command or workflow.

---

## 3. LLMAgent Tool Loop

### Implementation Status: COMPLETE

**Location**: `src-tauri/src/agents/llm_agent.rs`

### Current Execution Flow

```
User Message
     │
     ▼
┌─────────────────────────────────────────────────────────────┐
│                      LLMAgent                                │
│                                                              │
│  1. Build system prompt with tool definitions                │
│  2. Build user prompt from task + context                    │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐ │
│  │ Tool Loop (max 10 iterations)                          │ │
│  │                                                         │ │
│  │  ┌─────────────┐                                       │ │
│  │  │  LLM Call   │◄────────────────────────────┐        │ │
│  │  └──────┬──────┘                             │        │ │
│  │         │                                     │        │ │
│  │         ▼                                     │        │ │
│  │  ┌─────────────────┐                         │        │ │
│  │  │ Parse Response  │                         │        │ │
│  │  │ for <tool_call> │                         │        │ │
│  │  └────────┬────────┘                         │        │ │
│  │           │                                   │        │ │
│  │     ┌─────┴─────┐                            │        │ │
│  │     │           │                            │        │ │
│  │     ▼           ▼                            │        │ │
│  │  No calls    Tool calls found                │        │ │
│  │     │           │                            │        │ │
│  │     │           ▼                            │        │ │
│  │     │     ┌───────────────┐                  │        │ │
│  │     │     │ Execute Tools │                  │        │ │
│  │     │     │ (Local + MCP) │                  │        │ │
│  │     │     └───────┬───────┘                  │        │ │
│  │     │             │                          │        │ │
│  │     │             ▼                          │        │ │
│  │     │     ┌───────────────┐                  │        │ │
│  │     │     │ Format Results│──────────────────┘        │ │
│  │     │     └───────────────┘                           │ │
│  │     │                                                  │ │
│  │     ▼                                                  │ │
│  │   Exit Loop                                            │ │
│  └────────────────────────────────────────────────────────┘ │
│                                                              │
│  3. Generate final Report with metrics                       │
└─────────────────────────────────────────────────────────────┘
     │
     ▼
  Report
```

### Available Tools

| Tool | Type | Description |
|------|------|-------------|
| `MemoryTool` | Local | Semantic memory with embeddings |
| `TodoTool` | Local | Task management for workflow |
| `*` | MCP | Any tool from configured MCP servers |

### Tool Call Format (XML)

```xml
<tool_call name="MemoryTool">
{"operation": "add", "type": "knowledge", "content": "..."}
</tool_call>

<tool_call name="serena:find_symbol">
{"name_path_pattern": "MyClass", "include_body": true}
</tool_call>
```

---

## 4. Sub-Agent Support

### Implementation Status: NOT IMPLEMENTED

### What Exists (Prepared for Future)

**ValidationType** (`src-tauri/src/models/validation.rs`):

```rust
pub enum ValidationType {
    Tool,
    SubAgent,    // <-- Exists but UNUSED
    MCP,
    FileOp,
    DbOp,
}
```

**Schema Definition** (`src-tauri/src/db/schema.rs`):

```sql
DEFINE FIELD type ON validation_request TYPE string
    ASSERT $value IN ['tool', 'sub_agent', 'mcp', 'file_op', 'db_op'];
```

### What's Missing

| Feature | Description | Required For |
|---------|-------------|--------------|
| `SpawnAgentTool` | Tool to create temporary sub-agent | Dynamic agent creation |
| `DelegateTaskTool` | Tool to delegate task to existing agent | Task distribution |
| Agent communication | Inter-agent message passing | Collaborative workflows |
| Result aggregation | Collect sub-agent reports | Fan-in pattern |

---

## 5. Current Workflow Architecture

### Single-Agent Execution (Current)

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend                              │
│  invoke('execute_workflow_streaming', {                      │
│      workflowId, message, agentId                           │
│  })                                                          │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Tauri Command                             │
│              commands/streaming.rs                           │
│                                                              │
│  1. Load agent config from DB                               │
│  2. Create LLMAgent with ToolFactory                        │
│  3. Build Task with context                                 │
│  4. Call orchestrator.execute_with_mcp()                    │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   AgentOrchestrator                          │
│    execute_with_mcp(agent_id, task, mcp_manager)            │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      LLMAgent                                │
│                   (Tool Loop)                                │
└─────────────────────────────────────────────────────────────┘
```

### Multi-Agent Execution (Target Architecture)

```
┌─────────────────────────────────────────────────────────────┐
│                        Frontend                              │
│  invoke('execute_workflow_streaming', {                      │
│      workflowId, message, agentId                           │
│  })                                                          │
└─────────────────────────────┬───────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                   Primary Agent                              │
│                                                              │
│  Tool Loop Iteration N:                                     │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ <tool_call name="SpawnAgentTool">                      │  │
│  │ {                                                      │  │
│  │   "name": "Research Sub-Agent",                        │  │
│  │   "lifecycle": "temporary",                            │  │
│  │   "task": "Find all usages of function X",            │  │
│  │   "tools": ["serena:find_referencing_symbols"]        │  │
│  │ }                                                      │  │
│  │ </tool_call>                                           │  │
│  └───────────────────────────────────────────────────────┘  │
│                              │                               │
│                              ▼                               │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              SpawnAgentTool Handler                    │  │
│  │                                                        │  │
│  │  1. Create AgentConfig with Lifecycle::Temporary       │  │
│  │  2. Register in AgentRegistry                          │  │
│  │  3. Call orchestrator.execute_with_mcp()              │  │
│  │  4. Return sub-agent Report to primary agent          │  │
│  │  5. Unregister temporary agent                        │  │
│  └───────────────────────────────────────────────────────┘  │
│                              │                               │
│                              ▼                               │
│  Continue tool loop with sub-agent results...               │
└─────────────────────────────────────────────────────────────┘
```

---

## 6. Orchestration Patterns (Documented but Not Implemented)

**Reference**: `docs/WORKFLOW_ORCHESTRATION.md`

### Pattern Comparison

| Pattern | Description | Code Status | Usage Status |
|---------|-------------|-------------|--------------|
| **Sequential** | A -> B -> C | Not implemented | - |
| **Parallel (Fan-Out)** | A -> [B, C, D] simultaneously | `execute_parallel()` exists | UNUSED |
| **Fan-In** | [B, C, D] -> E (aggregation) | Not implemented | - |
| **Pipeline** | Transform chain | Not implemented | - |
| **Hybrid** | Mix of patterns | Not implemented | - |

### Sequential vs Parallel Decision

```
Task Analysis
     │
     ├─── Independent sub-tasks? ─────► Parallel Execution
     │         │                            │
     │         │                            ▼
     │         │                    ┌───────────────────┐
     │         │                    │ execute_parallel  │
     │         │                    │ (Vec of tasks)    │
     │         │                    └───────────────────┘
     │         │
     │         └─── Dependent sub-tasks? ───► Sequential Execution
     │                                            │
     │                                            ▼
     │                                   ┌───────────────────┐
     │                                   │ execute_with_mcp  │
     │                                   │ (one at a time)   │
     │                                   └───────────────────┘
     │
     └─── Single task ──────────────────► Direct Execution
                                              │
                                              ▼
                                     ┌───────────────────┐
                                     │ execute_with_mcp  │
                                     │ (current agent)   │
                                     └───────────────────┘
```

---

## 7. Implementation Roadmap for Sub-Agents

### Phase A: SpawnAgentTool (Priority: HIGH)

**Estimated Effort**: 6-8 hours

```rust
// New tool: src-tauri/src/tools/spawn_agent.rs

pub struct SpawnAgentTool {
    orchestrator: Arc<AgentOrchestrator>,
    registry: Arc<AgentRegistry>,
    provider_manager: Arc<ProviderManager>,
    tool_factory: Arc<ToolFactory>,
}

impl Tool for SpawnAgentTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            id: "SpawnAgentTool".to_string(),
            name: "Spawn Sub-Agent".to_string(),
            description: "Creates and executes a temporary sub-agent for a specific task".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "name": { "type": "string" },
                    "task": { "type": "string" },
                    "system_prompt": { "type": "string" },
                    "tools": { "type": "array", "items": { "type": "string" } },
                    "mcp_servers": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["name", "task"]
            }),
        }
    }

    async fn execute(&self, args: Value) -> Result<Value> {
        // 1. Create temporary AgentConfig
        // 2. Register in registry
        // 3. Execute via orchestrator
        // 4. Cleanup (unregister)
        // 5. Return sub-agent report
    }
}
```

### Phase B: DelegateTaskTool (Priority: MEDIUM)

**Estimated Effort**: 4 hours

```rust
// Delegate to existing agent without creating new one

pub struct DelegateTaskTool {
    orchestrator: Arc<AgentOrchestrator>,
}

impl Tool for DelegateTaskTool {
    async fn execute(&self, args: Value) -> Result<Value> {
        let agent_id = args["agent_id"].as_str()?;
        let task_description = args["task"].as_str()?;

        let task = Task {
            id: uuid::Uuid::new_v4().to_string(),
            description: task_description.to_string(),
            context: args["context"].clone(),
        };

        self.orchestrator.execute_with_mcp(agent_id, task, mcp).await
    }
}
```

### Phase C: Parallel Sub-Agents (Priority: MEDIUM)

**Estimated Effort**: 6 hours

```rust
// New tool for parallel execution

pub struct ParallelTasksTool {
    orchestrator: Arc<AgentOrchestrator>,
}

impl Tool for ParallelTasksTool {
    async fn execute(&self, args: Value) -> Result<Value> {
        let tasks: Vec<(String, Task)> = // parse from args

        // Use existing execute_parallel
        let results = self.orchestrator.execute_parallel(tasks).await;

        // Aggregate results
        json!({
            "completed": results.iter().filter(|r| r.is_ok()).count(),
            "failed": results.iter().filter(|r| r.is_err()).count(),
            "reports": // collect reports
        })
    }
}
```

### Phase D: Validation Integration (Priority: HIGH)

**Estimated Effort**: 3 hours

```rust
// Before spawning sub-agent, require validation

async fn execute(&self, args: Value) -> Result<Value> {
    // Create validation request
    let validation = create_validation_request(
        workflow_id,
        ValidationType::SubAgent,
        format!("Spawn sub-agent: {}", args["name"]),
        json!({
            "agent_name": args["name"],
            "task": args["task"],
            "tools": args["tools"]
        }),
        RiskLevel::Medium,
    ).await?;

    // Wait for approval (or auto-approve based on settings)
    if !validation.approved {
        return Err("Sub-agent creation rejected by user");
    }

    // Proceed with spawn...
}
```

---

## 8. Dependencies and Prerequisites

### For Sub-Agent Implementation

| Dependency | Current Status | Required Action |
|------------|---------------|-----------------|
| `AgentRegistry` | Complete | None |
| `AgentOrchestrator` | Complete | Expose `execute_parallel` |
| `ToolFactory` | Complete | Add SpawnAgentTool |
| `ValidationType::SubAgent` | Defined | Implement validation flow |
| Validation UI | Not implemented | Create modal for sub-agent approval |

### Tool Factory Update Required

```rust
// src-tauri/src/tools/factory.rs

pub const KNOWN_TOOLS: [&str; 4] = [
    "MemoryTool",
    "TodoTool",
    "SpawnAgentTool",     // NEW
    "DelegateTaskTool",   // NEW
];

fn create_tool(&self, name: &str, ...) -> Option<Arc<dyn Tool>> {
    match name {
        "MemoryTool" => Some(Arc::new(MemoryTool::new(...))),
        "TodoTool" => Some(Arc::new(TodoTool::new(...))),
        "SpawnAgentTool" => Some(Arc::new(SpawnAgentTool::new(...))),  // NEW
        "DelegateTaskTool" => Some(Arc::new(DelegateTaskTool::new(...))),  // NEW
        _ => None,
    }
}
```

---

## 9. Summary

### What's Working

| Component | Status | Notes |
|-----------|--------|-------|
| Agent Lifecycle (Permanent/Temporary) | COMPLETE | Full CRUD support |
| AgentRegistry | COMPLETE | All methods functional |
| Single-Agent Execution | COMPLETE | With MCP and local tools |
| Tool Loop | COMPLETE | XML parsing, 10 max iterations |
| MemoryTool | COMPLETE | Semantic search ready |
| TodoTool | COMPLETE | Task management |

### What's Prepared but Unused

| Component | Status | Location |
|-----------|--------|----------|
| `execute_parallel()` | `#[allow(dead_code)]` | orchestrator.rs:99-141 |
| `cleanup_temporary()` | `#[allow(dead_code)]` | registry.rs:101-124 |
| `ValidationType::SubAgent` | Defined, unused | validation.rs:22 |

### What's Missing

| Feature | Priority | Effort | Blocking |
|---------|----------|--------|----------|
| SpawnAgentTool | HIGH | 6-8h | Sub-agent creation |
| DelegateTaskTool | MEDIUM | 4h | Task delegation |
| ParallelTasksTool | MEDIUM | 6h | Parallel execution |
| Validation UI for SubAgent | HIGH | 3h | User approval |
| Inter-agent communication | LOW | 8h | Complex workflows |

---

## 10. Recommended Next Steps

1. **Implement SpawnAgentTool** - Enable dynamic sub-agent creation
2. **Wire up execute_parallel** - Remove dead_code, expose via tool
3. **Add Validation UI** - Modal for sub-agent approval
4. **Test with simple scenarios** - Single sub-agent, then parallel
5. **Document patterns** - Update WORKFLOW_ORCHESTRATION.md with examples

---

**Version**: 1.0
**Author**: Claude Code Analysis
**Last Updated**: 2025-11-27
