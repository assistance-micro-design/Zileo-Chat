# Specification - Removal of Placeholders and Features Inventory

## Metadata
- Date: 2025-11-25
- Stack: Svelte 5.43 + Rust 1.91 + Tauri 2.9 + SurrealDB 2.3
- Complexity: **medium**
- Type: Planning / Inventory (No code implementation)

## Context

**Request**: Planifier de retirer tous les placeholders de l'application. Planifier la mise en place des features de la /docs. (juste lister les features a implementer et leur fichier doc rattache)

**Objective**:
1. Identify and document ALL placeholders in the codebase that need replacement
2. Create a complete inventory of documented features with their implementation status
3. Map each feature to its documentation file

**Scope**:
- Included: Frontend (src/), Backend (src-tauri/), Documentation (docs/)
- Excluded: Test files (mock data in tests is acceptable)

**Success Criteria**:
- [ ] All placeholders identified with file:line references
- [ ] All documented features inventoried
- [ ] Each feature mapped to documentation source
- [ ] Clear implementation priority established

---

## Part 1: Placeholders Analysis

### 1.1 Frontend Placeholders (src/)

The frontend codebase is remarkably clean with **minimal placeholders**.

| Location | Type | Description | Replacement Needed |
|----------|------|-------------|-------------------|
| `src/routes/agent/+page.svelte:25-36` | Hardcoded Data | Single `simple_agent` hardcoded instead of dynamic loading | Replace with `invoke('list_agents')` call |

**Code to replace**:
```typescript
// Current (hardcoded)
let agents = $state<Agent[]>([
    {
        id: 'simple_agent',
        name: 'Simple Agent',
        lifecycle: 'permanent',
        status: 'available',
        capabilities: ['chat', 'analysis'],
        tools: [],
        mcp_servers: []
    }
]);

// Target (dynamic)
let agents = $state<Agent[]>([]);
onMount(async () => {
    agents = await invoke<Agent[]>('list_agents');
});
```

**Frontend Summary**: Only 1 placeholder to fix

---

### 1.2 Backend Placeholders (src-tauri/)

| Location | Type | Severity | Description | Replacement Needed |
|----------|------|----------|-------------|-------------------|
| `src-tauri/src/tools/db/surrealdb_tool.rs:66,88` | **Missing Method** | **CRITICAL** | `query_with_params()` method does not exist | Implement method or use `.bind()` pattern |
| `src-tauri/src/commands/memory.rs:9-10,179-182` | Stub Implementation | INFO | Memory search without vector embeddings | Implement with embedding model (Phase future) |
| `src-tauri/src/commands/streaming.rs:231-253` | Stub Implementation | MEDIUM | `cancel_workflow_streaming` logs warning but doesn't cancel | Implement cooperative cancellation |
| `src-tauri/src/commands/workflow.rs:160` | Hardcoded Value | LOW | `cost_usd: 0.0` always zero | Calculate from provider pricing |
| `src-tauri/src/commands/workflow.rs:161` | Hardcoded Value | LOW | `provider: "Demo".to_string()` | Use actual provider from agent config |

#### Critical: Missing `query_with_params` Method

**File**: `src-tauri/src/tools/db/surrealdb_tool.rs`
**Lines**: 66, 88

```rust
// BROKEN - This method doesn't exist in DBClient
let result: Result<Vec<Value>, _> = self
    .db
    .query_with_params(&query, vec![("data", data.clone())])
    .await;
```

**Fix Required**: Either:
1. Add `query_with_params` to `src-tauri/src/db/client.rs`
2. Or use existing `.bind()` pattern from SurrealDB SDK

#### Prepared Dead Code (Not Placeholders - Intentional)

These are complete implementations marked `#[allow(dead_code)]` for future use:

| Location | Function | Status |
|----------|----------|--------|
| `src-tauri/src/db/client.rs:127-143` | `update<T>()` | Ready for Phase 6+ |
| `src-tauri/src/agents/core/registry.rs:67-84` | `unregister()` | Ready for temporary agents |
| `src-tauri/src/agents/core/registry.rs:89-109` | `cleanup_temporary()` | Ready for lifecycle management |
| `src-tauri/src/agents/core/orchestrator.rs:64-106` | `execute_parallel()` | Ready for multi-agent coordination |

**Backend Summary**: 1 critical fix, 4 improvements needed

---

## Part 2: Features Inventory by Documentation File

### 2.1 API_REFERENCE.md - Tauri Commands

**File**: `docs/API_REFERENCE.md`

| Feature | Command | Status | Priority |
|---------|---------|--------|----------|
| Create workflow | `create_workflow` | Implemented | - |
| Load workflows | `load_workflows` | Implemented | - |
| Execute workflow | `execute_workflow` | Implemented | - |
| Delete workflow | `delete_workflow` | Implemented | - |
| Save workflow state | `save_workflow_state` | Implemented | - |
| **Export workflow** | `export_workflow` | **Not Implemented** | Medium |
| **Import workflow** | `import_workflow` | **Not Implemented** | Medium |
| List agents | `list_agents` | Implemented | - |
| Get agent config | `get_agent_config` | Implemented | - |
| Update agent config | `update_agent_config` | Implemented | - |
| List pending validations | `list_pending_validations` | Partial | High |
| Approve validation | `approve_validation` | Partial | High |
| Reject validation | `reject_validation` | Partial | High |
| Add memory | `add_memory` | Partial | Medium |
| **Search memory (vector)** | `search_memory` | **Not Implemented** | High |
| List memories | `list_memories` | Partial | - |
| Delete memory | `delete_memory` | Partial | - |
| **List MCP servers** | `list_mcp_servers` | **Not Implemented** | High |
| **Test MCP server** | `test_mcp_server` | **Not Implemented** | High |
| **Update MCP config** | `update_mcp_config` | **Not Implemented** | High |

---

### 2.2 MULTI_AGENT_ARCHITECTURE.md - Agent System

**File**: `docs/MULTI_AGENT_ARCHITECTURE.md`

| Feature | Description | Status | Priority |
|---------|-------------|--------|----------|
| Agent Principal (Orchestrator) | Master agent coordination | Implemented | - |
| Agent Registry | Discovery and lifecycle | Implemented | - | **NE PAS IMPLEMENTER**
| Agent Factory Pattern | AgentBuilder | Implemented | - |  **NE PAS IMPLEMENTER**
| **TOML Configuration** | Config in `agents/config/` | **Not Implemented** | Critical |
| **Database Agent** | SurrealDB operations | **Partial** | High |  **NE PAS IMPLEMENTER**
| **API Agent** | External service integration | **Not Implemented** | High |  **NE PAS IMPLEMENTER**
| **UI Agent** | Component generation | **Not Implemented** | Medium |  **NE PAS IMPLEMENTER**
| **RAG Agent** | Retrieval-augmented generation | **Not Implemented** | High |  **NE PAS IMPLEMENTER**
| **Code Agent** | Code analysis/refactoring | **Not Implemented** | Medium |  **NE PAS IMPLEMENTER**
| **Temporary Agents** | Short-lived task agents | **Not Implemented** | Medium |  **NE PAS IMPLEMENTER**
| Markdown Reports | Standardized output | Implemented | - |
| **Health Checks** | Agent status monitoring | **Not Implemented** | Low |
| **Distributed Tracing** | Request ID propagation | **Not Implemented** | Low |
| **Rate Limiting** | Per-agent limits | **Not Implemented** | Medium |
| **Event-Driven Communication** | Event bus pattern | **Not Implemented** | Medium |
| **Idempotence Guarantee** | Task replay safety | **Not Implemented** | Low |
| **Retry Strategy** | Exponential backoff | **Not Implemented** | Medium |
| **Task Journal** | Execution tracking | **Not Implemented** | Low |

---

### 2.3 MCP_CONFIGURATION_GUIDE.md - MCP Integration

**File**: `docs/MCP_CONFIGURATION_GUIDE.md`

| Feature | Description | Status | Priority |
|---------|-------------|--------|----------|
| MCP Protocol 2025-06-18 | Latest spec | **Not Implemented** | High |
| **NPX Servers** | Node.js MCP servers | **Planned** | High |
| **UVX Servers** | Python MCP servers | **Planned** | High |
| **Docker Servers** | Containerized servers | **Planned** | High |
| JSON-RPC 2.0 | Message format | **Planned** | High |
| **OAuth 2.1** | Authentication | **Not Implemented** | Medium |
| **Streamable HTTP** | Transport | **Not Implemented** | Medium |
| Server Configuration UI | Settings page | **Not Implemented** | High |
| **MCP Inspector** | Debugging tool | **Not Implemented** | Low |
| Environment Variables | Secure secrets | Partial | - |

---

### 2.4 DESIGN_SYSTEM.md - UI Components

**File**: `docs/DESIGN_SYSTEM.md`

| Component | Status | Priority |
|-----------|--------|----------|
| Light/Dark Theme | Implemented | - |
| Theme Store | Implemented | - |
| CSS Variables | Implemented | - |
| Button (4 variants) | Implemented | - |
| Card | Implemented | - |
| Badge | Implemented | - |
| Modal | Implemented | - |
| Input | Implemented | - |
| Select | Implemented | - |
| Textarea | Implemented | - |
| StatusIndicator | Implemented | - |
| Spinner | Implemented | - |
| ProgressBar | Implemented | - |
| AppContainer | Implemented | - |
| FloatingMenu | Implemented | - |
| Sidebar | Implemented | - |
| NavItem | Implemented | - |
| WorkflowItem | Implemented | - |
| MessageBubble | Implemented | - |
| ChatInput | Implemented | - |
| ValidationModal | Implemented | - |
| **Agent Settings Editor** | **Not Implemented** | Medium |
| **Memory Management UI** | **Not Implemented** | Medium |
| **MCP Server Config UI** | **Not Implemented** | High |

---

### 2.5 DATABASE_SCHEMA.md - SurrealDB Schema

**File**: `docs/DATABASE_SCHEMA.md`

| Feature | Status | Priority |
|---------|--------|----------|
| workflow table | Implemented | - |
| agent_state table | Implemented | - |
| message table | Implemented | - |
| task table | Implemented | - |
| validation_request table | Partial | High |
| memory table | Partial | - |
| **Vector Indexing (HNSW)** | **Not Implemented** | High |
| **KNN Search** | **Not Implemented** | High |
| **Embeddings Storage** | **Not Implemented** | High |
| **Audit Trail** | **Not Implemented** | Medium |

---

### 2.6 MULTI_PROVIDER_SPECIFICATIONS.md - LLM Providers

**File**: `docs/MULTI_PROVIDER_SPECIFICATIONS.md`

| Provider | Status | Priority |
|----------|--------|----------|
| Mistral | Implemented | - |
| Ollama | Implemented | - |
| **Claude** | **Not Implemented** | High | **NE PAS IMPLEMENTER**
| **GPT/OpenAI** | **Not Implemented** | High | **NE PAS IMPLEMENTER**
| **Gemini** | **Not Implemented** | Medium | **NE PAS IMPLEMENTER**
| **DeepSeek** | **Not Implemented** | Low | **NE PAS IMPLEMENTER**
| Streaming (all providers) | Partial | High |
| Token Counting | Implemented | - |

---

### 2.7 TESTING_STRATEGY.md - Testing

**File**: `docs/TESTING_STRATEGY.md`

| Feature | Status | Priority |
|---------|--------|----------|
| Unit Tests (Rust) | Partial | Medium |
| Integration Tests | Partial | Medium |
| E2E Tests (Playwright) | Implemented | - |
| **Database Tests** | **Not Implemented** | Medium |
| **Tool Execution Tests** | **Not Implemented** | Medium |
| **MCP Communication Tests** | **Not Implemented** | Medium |

---

### 2.8 AGENT_TOOLS_DOCUMENTATION.md - Agent Tools

**File**: `docs/AGENT_TOOLS_DOCUMENTATION.md`

| Tool | Status | Priority |
|------|--------|----------|
| **Todo Tool** | **Not Implemented** | Medium |
| **Memory Tool** | **Partial** | High |
| **Database Tool** | **Partial** | High |
| **HTTP Client Tool** | **Not Implemented** | High |
| **Component Generator** | **Not Implemented** | Low |
| **A11y Validator** | **Not Implemented** | Low |
| **Refactor Tool** | **Not Implemented** | Low |

---

### 2.9 WORKFLOW_ORCHESTRATION.md - Orchestration

**File**: `docs/WORKFLOW_ORCHESTRATION.md`

| Feature | Status | Priority |
|---------|--------|----------|
| Task Decomposition | Implemented | - |
| Agent Assignment | Implemented | - |
| Parallel Execution | Implemented | - |
| Sequential Execution | Implemented | - |
| **Dependency Analysis** | **Partial** | Medium |
| Report Aggregation | Implemented | - |
| State Management | Implemented | - |

---

## Part 3: Implementation Priorities

### Priority 1: Critical Fixes (Must Fix)

| Item | Location | Doc Reference |
|------|----------|---------------|
| Fix `query_with_params` missing method | `src-tauri/src/tools/db/surrealdb_tool.rs:66,88` | - |
| Replace hardcoded agent in frontend | `src/routes/agent/+page.svelte:25-36` | - |

### Priority 2: High Priority Features

| Feature | Documentation File | Estimated Effort |
|---------|-------------------|------------------|
| MCP Server Integration | `MCP_CONFIGURATION_GUIDE.md` | Large |
| Vector Search / RAG | `DATABASE_SCHEMA.md`, `API_REFERENCE.md` | Large |
| TOML Agent Configuration | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| Complete Validation Flow | `API_REFERENCE.md` | Medium |
| Database Agent (complete) | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| API Agent | `MULTI_AGENT_ARCHITECTURE.md` | Large |

### Priority 3: Medium Priority Features

| Feature | Documentation File | Estimated Effort |
|---------|-------------------|------------------|
| Export/Import Workflows | `API_REFERENCE.md` | Small |
| Streaming Cancellation | `API_REFERENCE.md` | Medium |
| Cost Calculation | `API_REFERENCE.md` | Small |
| RAG Agent | `MULTI_AGENT_ARCHITECTURE.md` | Large | **NE PAS IMPLEMENTER**
| UI Agent | `MULTI_AGENT_ARCHITECTURE.md` | Large | **NE PAS IMPLEMENTER**
| Code Agent | `MULTI_AGENT_ARCHITECTURE.md` | Large | **NE PAS IMPLEMENTER**
| Temporary Agents | `MULTI_AGENT_ARCHITECTURE.md` | Medium | **NE PAS IMPLEMENTER**
| Rate Limiting | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| Audit Trail | `DATABASE_SCHEMA.md` | Medium |
| Agent Settings Editor UI | `DESIGN_SYSTEM.md` | Medium |
| Memory Management UI | `DESIGN_SYSTEM.md` | Medium |

### Priority 4: Low Priority Features

| Feature | Documentation File | Estimated Effort |
|---------|-------------------|------------------|
| Health Checks | `MULTI_AGENT_ARCHITECTURE.md` | Small |
| Distributed Tracing | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| Idempotence Guarantee | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| Task Journal | `MULTI_AGENT_ARCHITECTURE.md` | Medium |
| Gemini Provider | `MULTI_PROVIDER_SPECIFICATIONS.md` | Medium | **NE PAS IMPLEMENTER**
| DeepSeek Provider | `MULTI_PROVIDER_SPECIFICATIONS.md` | Small | **NE PAS IMPLEMENTER**
| MCP Inspector | `MCP_CONFIGURATION_GUIDE.md` | Medium |
| Component Generator Tool | `AGENT_TOOLS_DOCUMENTATION.md` | Large |
| A11y Validator Tool | `AGENT_TOOLS_DOCUMENTATION.md` | Medium |
| Refactor Tool | `AGENT_TOOLS_DOCUMENTATION.md` | Large |

---

## Part 4: Summary Statistics

### Placeholders Count

| Category | Count | Severity |
|----------|-------|----------|
| Frontend Placeholders | 1 | Medium |
| Backend Critical | 1 | Critical |
| Backend Stubs | 4 | Low-Medium |
| **Total Placeholders** | **6** | - |

### Features Status

| Status | Count |
|--------|-------|
| Fully Implemented | ~34 |
| Partially Implemented | ~22 |
| Not Implemented | ~24 |
| **Total Documented Features** | **~80** |

### Implementation Coverage

```
Implemented:     ████████████████░░░░░░░░░  42%
Partial:         ██████████░░░░░░░░░░░░░░░  28%
Not Implemented: ███████░░░░░░░░░░░░░░░░░░  30%
```

---

## Part 5: Documentation File Index

| Documentation File | Features Documented | Implementation Rate |
|-------------------|--------------------|--------------------|
| `docs/API_REFERENCE.md` | 20 commands | ~70% |
| `docs/MULTI_AGENT_ARCHITECTURE.md` | 30+ features | ~40% |
| `docs/MCP_CONFIGURATION_GUIDE.md` | 10+ features | ~10% |
| `docs/DESIGN_SYSTEM.md` | 20+ components | ~85% |
| `docs/DATABASE_SCHEMA.md` | 10+ features | ~60% |
| `docs/MULTI_PROVIDER_SPECIFICATIONS.md` | 6 providers | ~33% |
| `docs/TESTING_STRATEGY.md` | 6 test types | ~40% |
| `docs/AGENT_TOOLS_DOCUMENTATION.md` | 7 tools | ~20% |
| `docs/WORKFLOW_ORCHESTRATION.md` | 8 features | ~80% |
| `docs/FRONTEND_SPECIFICATIONS.md` | 15+ features | ~80% |

---

## Next Steps

### Immediate Actions (This Sprint)
1. Fix `query_with_params` critical bug
2. Replace hardcoded agent with dynamic loading
3. Complete validation flow backend

### Short-Term (Next 2 Sprints)
1. MCP Server Integration
2. Vector Search / RAG implementation
3. Additional LLM providers (Claude, GPT)

### Medium-Term (Next Month)
1. Specialized Agents (API, UI, RAG, Code)
2. Agent TOML Configuration System
3. Export/Import Workflows

---

## References

- Project Guidelines: `CLAUDE.md`
- Implementation Plan: `docs/specs/2025-11-25_spec-complete-implementation-plan.md`
- Project Status: `docs/specs/2025-11-25_consolidated-project-status.md`
- All documentation: `docs/` directory
