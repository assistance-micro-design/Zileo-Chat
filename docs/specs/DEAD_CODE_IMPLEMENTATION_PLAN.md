# Dead Code Implementation Plan

> **Version**: 1.0
> **Date**: 2025-12-11
> **Related Document**: [DEAD_CODE_ANALYSIS.md](./DEAD_CODE_ANALYSIS.md)

## Overview

This document provides detailed implementation steps for activating dead code identified in the analysis. Each task includes:
- Specific file changes required
- Dependencies and prerequisites
- Testing requirements
- Acceptance criteria

---

## Phase 1: Critical Infrastructure

### Task 1.1: Workflow Cancellation System

**Priority**: Critical
**Effort**: 2 days
**Files**:
- `src-tauri/src/state.rs` (lines 150-161)
- `src-tauri/src/commands/workflow.rs`
- `src/lib/stores/workflow.ts`

#### Current Dead Code
```rust
// src-tauri/src/state.rs:150
#[allow(dead_code)]
pub async fn get_cancellation_token(&self, workflow_id: &str) -> Option<CancellationToken>

// src-tauri/src/state.rs:161
#[allow(dead_code)]
pub async fn is_cancelled(&self, workflow_id: &str) -> bool
```

#### Implementation Steps

1. **Create Tauri command for cancellation**
```rust
// src-tauri/src/commands/workflow.rs
#[tauri::command]
pub async fn cancel_workflow(
    state: State<'_, AppState>,
    workflow_id: String,
) -> Result<(), String> {
    if let Some(token) = state.get_cancellation_token(&workflow_id).await {
        token.cancel();
        Ok(())
    } else {
        Err(format!("Workflow {} not found or already completed", workflow_id))
    }
}
```

2. **Integrate token checks in agent execution loop**
```rust
// src-tauri/src/agents/llm_agent.rs - in execute loop
if state.is_cancelled(&workflow_id).await {
    return Err(AgentError::Cancelled);
}
```

3. **Add frontend store method**
```typescript
// src/lib/stores/workflow.ts
async cancelWorkflow(workflowId: string): Promise<void> {
    await invoke('cancel_workflow', { workflowId });
    this.updateWorkflowStatus(workflowId, 'cancelled');
}
```

4. **Add UI button in WorkflowPanel**
```svelte
<!-- src/routes/agent/WorkflowPanel.svelte -->
{#if workflow.status === 'running'}
    <Button variant="destructive" onclick={() => cancelWorkflow(workflow.id)}>
        {$i18n('workflow_cancel')}
    </Button>
{/if}
```

#### Testing
- [ ] Unit test: Token creation and cancellation
- [ ] Integration test: Cancel running workflow
- [ ] UI test: Cancel button visibility and functionality

#### Acceptance Criteria
- [ ] Cancel button appears for running workflows
- [ ] Cancellation stops agent execution within 1 second
- [ ] Workflow status updates to 'cancelled'
- [ ] No orphaned tokens after cancellation

---

### Task 1.2: Tool Factory Activation

**Priority**: Critical
**Effort**: 1 day
**Files**:
- `src-tauri/src/state.rs` (line 40)
- `src-tauri/src/tools/factory.rs`
- `src-tauri/src/agents/llm_agent.rs`

#### Current Dead Code
```rust
// src-tauri/src/state.rs:40
#[allow(dead_code)]
pub tool_factory: Arc<ToolFactory>,
```

#### Implementation Steps

1. **Remove dead_code attribute from state.rs**
```rust
// Remove #[allow(dead_code)] from line 40
pub tool_factory: Arc<ToolFactory>,
```

2. **Initialize ToolFactory in AppState::new()**
```rust
// src-tauri/src/state.rs - in AppState::new()
let tool_factory = Arc::new(ToolFactory::new(db.clone()));
Self {
    tool_factory,
    // ... other fields
}
```

3. **Use factory in agent creation**
```rust
// src-tauri/src/agents/llm_agent.rs
pub fn with_state_factory(state: &AppState) -> Self {
    Self::with_factory(state.tool_factory.clone())
}
```

#### Testing
- [ ] Unit test: Factory creates correct tool instances
- [ ] Integration test: Agent uses factory-created tools

#### Acceptance Criteria
- [ ] Tools created via factory have correct context
- [ ] No regression in existing tool functionality

---

## Phase 2: Embedding & Semantic Search

### Task 2.1: Activate Embedding Module

**Priority**: Critical
**Effort**: 3 days
**Files**:
- `src-tauri/src/llm/embedding.rs`
- `src-tauri/src/state.rs` (line 47)
- `src-tauri/src/commands/settings.rs`
- `src-tauri/src/tools/memory/tool.rs`

#### Current Dead Code
```rust
// src-tauri/src/llm/embedding.rs:44
#[allow(dead_code)] // Module-level - entire module

// src-tauri/src/state.rs:47
#[allow(dead_code)]
pub embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>>,
```

#### Implementation Steps

1. **Remove module-level dead_code from embedding.rs**
```rust
// Remove line 44: #[allow(dead_code)]
// Keep field-level annotations for serde fields
```

2. **Add embedding configuration command**
```rust
// src-tauri/src/commands/settings.rs
#[tauri::command]
pub async fn configure_embedding_service(
    state: State<'_, AppState>,
    provider: String,
    model: String,
    api_key: Option<String>,
) -> Result<(), String> {
    let service = match provider.as_str() {
        "mistral" => EmbeddingService::new_mistral(model, api_key),
        "openai" => EmbeddingService::new_openai(model, api_key),
        _ => return Err("Unknown provider".to_string()),
    }?;

    state.set_embedding_service(Arc::new(service)).await;
    Ok(())
}
```

3. **Integrate with MemoryTool**
```rust
// src-tauri/src/tools/memory/tool.rs
async fn create_memory_with_embedding(
    &self,
    content: &str,
) -> Result<Memory, ToolError> {
    let embedding = if let Some(service) = self.state.get_embedding_service().await {
        Some(service.embed(content).await?)
    } else {
        None
    };

    let create = MemoryCreateWithEmbedding {
        content: content.to_string(),
        embedding,
        // ... other fields
    };

    self.db.create_memory_with_embedding(create).await
}
```

4. **Add vector search query**
```rust
// src-tauri/src/db/memory.rs
pub async fn search_by_embedding(
    &self,
    query_embedding: Vec<f32>,
    limit: usize,
) -> Result<Vec<Memory>, DbError> {
    // SurrealDB vector search using knn
    let query = format!(
        "SELECT * FROM memory WHERE embedding <|{},{}|> $embedding LIMIT {}",
        query_embedding.len(),
        10, // knn parameter
        limit
    );
    // ...
}
```

#### Testing
- [ ] Unit test: Embedding generation with mock provider
- [ ] Integration test: Memory creation with embedding
- [ ] Integration test: Semantic search returns relevant results

#### Acceptance Criteria
- [ ] Embedding configuration persisted in settings
- [ ] Memories can be created with embeddings
- [ ] Semantic search returns top-k similar memories
- [ ] Graceful fallback when embedding service unavailable

---

### Task 2.2: Memory Model Builders

**Priority**: Medium
**Effort**: 1 day
**Files**:
- `src-tauri/src/models/memory.rs` (lines 90, 101, 135)

#### Current Dead Code
```rust
// src-tauri/src/models/memory.rs:90
#[allow(dead_code)]
pub fn new(content: String, memory_type: MemoryType) -> Self

// src-tauri/src/models/memory.rs:101
#[allow(dead_code)]
pub fn with_workflow(content: String, memory_type: MemoryType, workflow_id: String) -> Self

// src-tauri/src/models/memory.rs:135
#[allow(dead_code)]
pub struct MemoryCreateWithEmbedding
```

#### Implementation Steps

1. **Remove dead_code attributes**
2. **Use builders in MemoryTool operations**
```rust
// src-tauri/src/tools/memory/operations.rs
let create = MemoryCreate::with_workflow(
    content.clone(),
    MemoryType::Knowledge,
    self.context.workflow_id.clone(),
);
```

#### Acceptance Criteria
- [ ] Memories link to workflow context
- [ ] Workflow-scoped memory queries work

---

## Phase 3: Resilience Layer

### Task 3.1: LLM Circuit Breaker

**Priority**: High
**Effort**: 2 days
**Files**:
- `src-tauri/src/llm/circuit_breaker.rs`
- `src-tauri/src/llm/manager.rs`
- `src-tauri/src/llm/mod.rs`

#### Current Dead Code
```rust
// src-tauri/src/llm/circuit_breaker.rs:69
#[allow(dead_code)]
impl CircuitBreakerConfig

// src-tauri/src/llm/circuit_breaker.rs:139
#[allow(dead_code)]
pub struct CircuitBreaker
```

#### Implementation Steps

1. **Create per-provider circuit breakers**
```rust
// src-tauri/src/llm/manager.rs
pub struct ProviderManager {
    providers: HashMap<String, Box<dyn LLMProvider>>,
    circuit_breakers: HashMap<String, CircuitBreaker>,
}

impl ProviderManager {
    pub async fn call_with_resilience(
        &self,
        provider_id: &str,
        request: ChatRequest,
    ) -> Result<ChatResponse, LLMError> {
        let breaker = self.circuit_breakers.get(provider_id)
            .ok_or(LLMError::ProviderNotFound)?;

        breaker.call(|| async {
            let provider = self.providers.get(provider_id)?;
            provider.chat(request).await
        }).await
    }
}
```

2. **Configure failure thresholds**
```rust
// Default configuration
let config = CircuitBreakerConfig {
    failure_threshold: 5,
    success_threshold: 2,
    timeout: Duration::from_secs(30),
    half_open_max_calls: 3,
};
```

3. **Expose circuit state in API**
```rust
#[tauri::command]
pub async fn get_provider_health(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<ProviderHealth, String> {
    let manager = state.provider_manager.read().await;
    manager.get_circuit_state(&provider_id)
}
```

#### Testing
- [ ] Unit test: State transitions (Closed -> Open -> HalfOpen)
- [ ] Unit test: Failure counting and threshold
- [ ] Integration test: Provider failure triggers circuit open

#### Acceptance Criteria
- [ ] Circuit opens after 5 consecutive failures
- [ ] Circuit half-opens after 30 second timeout
- [ ] 2 successes in half-open state close circuit
- [ ] UI shows provider health status

---

### Task 3.2: Sub-Agent Resilience

**Priority**: High
**Effort**: 1 day
**Files**:
- `src-tauri/src/tools/sub_agent_executor.rs` (lines 260, 349)

#### Current Dead Code
```rust
// src-tauri/src/tools/sub_agent_executor.rs:349
#[allow(dead_code)] // Will be used when tools are updated to use resilience
pub fn with_resilience(/* ... */) -> Self
```

#### Implementation Steps

1. **Update SpawnAgentTool to use resilient executor**
```rust
// src-tauri/src/tools/spawn_agent.rs
let executor = SubAgentExecutor::with_resilience(
    workflow_id,
    parent_agent_id,
    db.clone(),
    orchestrator.clone(),
    mcp_manager.clone(),
    CircuitBreakerConfig::default(),
);
```

2. **Wire circuit breaker to sub-agent failures**
```rust
// Track sub-agent failures per workflow
if execution_result.is_err() {
    self.circuit_breaker.record_failure();
}
```

#### Acceptance Criteria
- [ ] Sub-agent spawn failures trigger circuit breaker
- [ ] Cascading failures prevented
- [ ] Metrics visible in debugging

---

## Phase 4: Pricing & Monitoring

### Task 4.1: Activate Pricing Module

**Priority**: High
**Effort**: 2 days
**Files**:
- `src-tauri/src/llm/pricing.rs`
- `src-tauri/src/models/streaming.rs`
- `src-tauri/src/commands/workflow.rs`
- `src/lib/stores/workflow.ts`

#### Current Dead Code
```rust
// src-tauri/src/llm/pricing.rs:16
#[allow(dead_code)] // Module-level
```

#### Implementation Steps

1. **Hook pricing into token streaming**
```rust
// src-tauri/src/models/streaming.rs
impl StreamingEvent {
    pub fn token_with_cost(
        token: String,
        input_tokens: usize,
        output_tokens: usize,
        model: &str,
    ) -> Self {
        let cost = pricing::calculate_cost(model, input_tokens, output_tokens);
        Self::TokenChunk {
            token,
            input_tokens: Some(input_tokens),
            output_tokens: Some(output_tokens),
            cost: Some(cost),
        }
    }
}
```

2. **Add cost tracking to workflow**
```rust
// src-tauri/src/models/workflow.rs
pub struct WorkflowMetrics {
    pub total_input_tokens: usize,
    pub total_output_tokens: usize,
    pub total_cost_usd: f64,
}
```

3. **Display in UI**
```svelte
<!-- WorkflowMetrics.svelte -->
<div class="workflow-metrics">
    <span>{formatTokens(metrics.totalInputTokens)} input</span>
    <span>{formatTokens(metrics.totalOutputTokens)} output</span>
    <span>${metrics.totalCostUsd.toFixed(4)}</span>
</div>
```

#### Testing
- [ ] Unit test: Cost calculation accuracy
- [ ] Integration test: Costs accumulate correctly

#### Acceptance Criteria
- [ ] Per-workflow cost displayed in UI
- [ ] Costs match provider pricing
- [ ] Historical cost tracking

---

### Task 4.2: Query Statistics

**Priority**: Low
**Effort**: 1 day
**Files**:
- `src-tauri/src/db/client.rs` (lines 30, 474)

#### Implementation Steps

1. **Enable query_with_stats() for slow query logging**
```rust
// src-tauri/src/db/client.rs
pub async fn query_with_stats<T>(&self, query: &str) -> Result<(Vec<T>, QueryStats), DbError> {
    let start = Instant::now();
    let result = self.query(query).await;
    let duration = start.elapsed();

    if duration > Duration::from_millis(100) {
        warn!("Slow query ({}ms): {}", duration.as_millis(), query);
    }

    Ok((result?, QueryStats { duration, rows_affected: result.len() }))
}
```

2. **Add metrics endpoint**
```rust
#[tauri::command]
pub async fn get_db_metrics(state: State<'_, AppState>) -> Result<DbMetrics, String> {
    state.db.get_metrics().await
}
```

#### Acceptance Criteria
- [ ] Slow queries logged with threshold
- [ ] Metrics available for debugging

---

## Phase 5: UX Enhancements

### Task 5.1: Streaming Event Factories

**Priority**: Medium
**Effort**: 1 day
**Files**:
- `src-tauri/src/models/streaming.rs` (lines 287, 381, 411)

#### Current Dead Code
```rust
// Line 287
pub fn sub_agent_progress(agent_id: &str, status: &str, progress: f32) -> Self

// Line 381
pub fn task_create(task: &Task) -> Self

// Line 411
pub fn task_update(task_id: &str, status: TaskStatus) -> Self
```

#### Implementation Steps

1. **Emit progress during sub-agent execution**
```rust
// src-tauri/src/tools/sub_agent_executor.rs
for (i, step) in execution_steps.iter().enumerate() {
    let progress = (i as f32) / (execution_steps.len() as f32);
    self.emit_event(StreamingEvent::sub_agent_progress(
        &agent_id,
        "executing",
        progress,
    )).await;
    // ... execute step
}
```

2. **Handle in frontend**
```typescript
// src/lib/stores/streaming.ts
case 'SubAgentProgress':
    subAgentStore.updateProgress(event.agentId, event.progress);
    break;
```

#### Acceptance Criteria
- [ ] Sub-agent progress visible in UI
- [ ] Task updates reflected in real-time

---

### Task 5.2: Query Builders (Security)

**Priority**: Medium
**Effort**: 1 day
**Files**:
- `src-tauri/src/tools/utils.rs` (lines 130, 203)

#### Implementation Steps

1. **Replace string concatenation with QueryBuilder**
```rust
// Before
let query = format!("SELECT * FROM memory WHERE type = '{}'", memory_type);

// After
let query = QueryBuilder::new()
    .select("*")
    .from("memory")
    .where_eq("type", memory_type)
    .build();
```

2. **Use ParamQueryBuilder for user input**
```rust
let (query, params) = ParamQueryBuilder::new()
    .select("*")
    .from("memory")
    .where_param("content", "CONTAINS", search_term)
    .build();

db.query_with_params(&query, params).await
```

#### Acceptance Criteria
- [ ] No raw string interpolation with user input
- [ ] SQL injection tests pass

---

## Cleanup Tasks (Post v2.0)

### Cleanup 1: Remove Backward Compatibility Code

**Condition**: 6 months after v2.0 stable
**Files**:
- `src-tauri/src/tools/sub_agent_executor.rs:533` (execute_with_metrics)

### Cleanup 2: Audit Unused Builder Methods

**Condition**: No usage in 6 months
**Files**:
- `src-tauri/src/models/function_calling.rs`
- `src-tauri/src/models/streaming.rs`

### Cleanup 3: Update Documentation

After each phase, update:
- `CLAUDE.md` - Remove phase references for completed items
- `TOOLS_REFERENCE.md` - Add newly activated features
- `API_REFERENCE.md` - Document new commands

---

## Progress Tracking

| Phase | Task | Status | PR |
|-------|------|--------|-----|
| 1 | 1.1 Cancellation | Not Started | - |
| 1 | 1.2 Tool Factory | Not Started | - |
| 2 | 2.1 Embeddings | Not Started | - |
| 2 | 2.2 Memory Builders | Not Started | - |
| 3 | 3.1 Circuit Breaker | Not Started | - |
| 3 | 3.2 Sub-Agent Resilience | Not Started | - |
| 4 | 4.1 Pricing | Not Started | - |
| 4 | 4.2 Query Stats | Not Started | - |
| 5 | 5.1 Streaming Events | Not Started | - |
| 5 | 5.2 Query Builders | Not Started | - |

---

*Document generated: 2025-12-11*
*Owner: Development Team*
