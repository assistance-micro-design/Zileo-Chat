# Orchestration Intra-Workflow

> **Objectif** : Definir comment l'agent principal determine l'execution parallele ou sequentielle des operations au sein d'un workflow

**Status** : Implementation Complete (Phase 5 + OPT-WF)
**Version** : 2.1 | **Derniere mise a jour** : 2025-12-10

---

## Principes Fondamentaux

### 1. Analyse des Dependances

**L'agent principal evalue chaque operation** :
- Les donnees d'entree necessaires
- Les donnees de sortie produites
- Les relations entre operations

**Decision** :
- **Parallele** : Operations independantes (pas de dependances de donnees)
- **Sequentiel** : Operation B necessite le resultat de A

### 2. Limitation Architecturale

**Regle stricte** : Les sous-agents NE PEUVENT PAS lancer d'autres sous-agents

**Raison** : Reutilisabilite et maintenabilite du code
- Evite recursion complexe
- Garantit controle centralise
- Simplifie debugging et tracabilite

**Seul l'orchestrateur principal** peut spawner et coordonner des sous-agents

---

## Implementation Backend

### Tauri Commands (7 total)

| Command | Signature | File:Line |
|---------|-----------|-----------|
| `create_workflow` | `async fn create_workflow(name: String, agent_id: String, state: State<'_, AppState>) -> Result<String, String>` | commands/workflow.rs:23-62 |
| `execute_workflow` | `async fn execute_workflow(workflow_id: String, message: String, agent_id: String, state: State<'_, AppState>) -> Result<WorkflowResult, String>` | commands/workflow.rs:75-213 |
| `load_workflows` | `async fn load_workflows(state: State<'_, AppState>) -> Result<Vec<Workflow>, String>` | commands/workflow.rs:221-260 |
| `delete_workflow` | `async fn delete_workflow(workflow_id: String, state: State<'_, AppState>) -> Result<(), String>` | commands/workflow.rs:275-389 |
| `load_workflow_full_state` | `async fn load_workflow_full_state(workflow_id: String, state: State<'_, AppState>) -> Result<WorkflowFullState, String>` | commands/workflow.rs:408-602 |
| `execute_workflow_streaming` | `async fn execute_workflow_streaming(window: Window, workflow_id: String, message: String, agent_id: String, locale: String, state: State<'_, AppState>) -> Result<WorkflowResult, String>` | commands/streaming.rs:50-648 |
| `cancel_workflow_streaming` | `async fn cancel_workflow_streaming(workflow_id: String, state: State<'_, AppState>) -> Result<(), String>` | commands/streaming.rs:687-704 |

### Tauri Events (8 total)

| Event Name | Payload Type | Description |
|------------|--------------|-------------|
| `workflow_stream` | `StreamChunk` | Real-time streaming chunks (tokens, tool calls, reasoning) |
| `workflow_complete` | `WorkflowComplete` | Completion event (completed, error, cancelled) |
| `validation_required` | `ValidationRequiredEvent` | Human-in-the-loop validation request |
| `validation_response` | `ValidationResponseEvent` | User's approval/rejection of validation |
| `sub_agent_start` | `StreamChunk` | Sub-agent execution started |
| `sub_agent_progress` | `StreamChunk` | Sub-agent execution progress update |
| `sub_agent_complete` | `StreamChunk` | Sub-agent execution completed with report |
| `sub_agent_error` | `StreamChunk` | Sub-agent execution failed |

### Orchestrator (agents/core/orchestrator.rs)

**Struct**: `AgentOrchestrator`
- Field: `registry: Arc<AgentRegistry>`

**Methods**:

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `pub fn new(registry: Arc<AgentRegistry>) -> Self` | Creates new orchestrator instance |
| `execute` | `pub async fn execute(&self, agent_id: &str, task: Task) -> anyhow::Result<Report>` | Legacy execution (delegates to execute_with_mcp) |
| `execute_with_mcp` | `pub async fn execute_with_mcp(&self, agent_id: &str, task: Task, mcp_manager: Option<Arc<MCPManager>>) -> anyhow::Result<Report>` | Production execution with MCP tool support |
| `execute_parallel` | `pub async fn execute_parallel(&self, tasks: Vec<(String, Task)>) -> Vec<anyhow::Result<Report>>` | Parallel execution via futures::join_all() |

### Models

#### WorkflowStatus

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowStatus {
    Idle,       // "idle"
    Running,    // "running"
    Completed,  // "completed"
    Error,      // "error"
}
```

#### Workflow

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub id: String,
    pub name: String,
    pub agent_id: String,
    pub status: WorkflowStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub total_tokens_input: u64,
    pub total_tokens_output: u64,
    pub total_cost_usd: f64,
    pub model_id: Option<String>,
    pub current_context_tokens: u64,
}
```

#### WorkflowResult

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowResult {
    pub report: String,                               // Markdown report from agent
    pub metrics: WorkflowMetrics,                     // Execution metrics
    pub tools_used: Vec<String>,                      // Tool names used
    pub mcp_calls: Vec<String>,                       // MCP server calls made
    pub tool_executions: Vec<WorkflowToolExecution>,  // Detailed tool execution data
}
```

#### WorkflowMetrics

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowMetrics {
    pub duration_ms: u64,
    pub tokens_input: usize,
    pub tokens_output: usize,
    pub cost_usd: f64,
    pub provider: String,
    pub model: String,
}
```

#### WorkflowFullState

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowFullState {
    pub workflow: Workflow,
    pub messages: Vec<Message>,
    pub tool_executions: Vec<ToolExecution>,
    pub thinking_steps: Vec<ThinkingStep>,
}
```

#### StreamChunk

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamChunk {
    pub workflow_id: String,
    pub chunk_type: ChunkType,  // token | tool_start | tool_end | reasoning | error | sub_agent_*
    pub content: Option<String>,
    pub tool: Option<String>,
    pub duration: Option<u64>,
    pub sub_agent_id: Option<String>,
    pub sub_agent_name: Option<String>,
    pub parent_agent_id: Option<String>,
    pub metrics: Option<SubAgentStreamMetrics>,
    pub progress: Option<u8>,
    pub task_id: Option<String>,
    pub task_name: Option<String>,
    pub task_status: Option<String>,
    pub task_priority: Option<u8>,
}
```

#### WorkflowComplete

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowComplete {
    pub workflow_id: String,
    pub status: CompletionStatus,  // completed | error | cancelled
    pub error: Option<String>,
}
```

---

## Implementation Frontend

### Stores

#### workflowStore (src/lib/stores/workflows.ts)

| Export | Type | Description |
|--------|------|-------------|
| `workflowStore` | Store Object | Main reactive store with CRUD methods |
| `workflows` | Derived Store | List of all workflows |
| `selectedWorkflowId` | Derived Store | Currently selected workflow ID |
| `workflowsLoading` | Derived Store | Loading state indicator |
| `workflowsError` | Derived Store | Error message |
| `workflowSearchFilter` | Derived Store | Search filter text |
| `selectedWorkflow` | Derived Store | Currently selected workflow object |
| `filteredWorkflows` | Derived Store | Workflows filtered by search text |

**Methods**:
```typescript
workflowStore.loadWorkflows(): Promise<void>
workflowStore.select(workflowId: string | null): void
workflowStore.setSearchFilter(filter: string): void
workflowStore.getSelected(): Workflow | undefined
workflowStore.reset(): void
```

#### streamingStore (src/lib/stores/streaming.ts)

| Export | Type | Description |
|--------|------|-------------|
| `streamingStore` | Store Object | Reactive store for streaming state |
| `isStreaming` | Derived Store | Whether streaming is active |
| `streamContent` | Derived Store | Accumulated content from tokens |
| `activeTools` | Derived Store | List of active tools |
| `runningTools` | Derived Store | Tools currently executing |
| `completedTools` | Derived Store | Tools that completed |
| `reasoningSteps` | Derived Store | Reasoning steps captured |
| `streamError` | Derived Store | Error message if streaming failed |
| `isCancelled` | Derived Store | Whether workflow was cancelled |
| `isCompleted` | Derived Store | Whether streaming completed |
| `tokensReceived` | Derived Store | Total tokens received |
| `activeSubAgents` | Derived Store | All active sub-agents |
| `runningSubAgents` | Derived Store | Sub-agents currently running |
| `completedSubAgents` | Derived Store | Sub-agents that completed |
| `activeTasks` | Derived Store | All active tasks |
| `pendingTasks` | Derived Store | Tasks with pending status |
| `runningTasks` | Derived Store | Tasks in progress |
| `completedTasks` | Derived Store | Tasks that completed |

**Methods**:
```typescript
streamingStore.start(workflowId: string): Promise<void>
streamingStore.appendToken(content: string): void
streamingStore.addToolStart(toolName: string): void
streamingStore.completeToolEnd(toolName: string, duration: number): void
streamingStore.failTool(toolName: string, error: string): void
streamingStore.addReasoning(content: string): void
streamingStore.setError(error: string): void
streamingStore.complete(): void
streamingStore.cancel(): void
streamingStore.cleanup(): Promise<void>
streamingStore.reset(): Promise<void>
```

#### activityStore (src/lib/stores/activity.ts)

| Export | Type | Description |
|--------|------|-------------|
| `activityStore` | Store Object | Reactive store for activity events |
| `historicalActivities` | Derived Store | Activities from database |
| `activityFilter` | Derived Store | Current filter ('all', 'tools', 'agents', 'reasoning', 'todos') |
| `allActivities` | Derived Store | Combined historical + streaming activities |
| `filteredActivities` | Derived Store | Activities filtered by current filter |

**Methods**:
```typescript
activityStore.loadHistorical(workflowId: string): Promise<void>
activityStore.setFilter(filter: ActivityFilter): void
activityStore.captureStreamingActivities(): void
activityStore.reset(): void
```

### Types

#### workflow.ts

| Type | Fields |
|------|--------|
| `WorkflowStatus` | `'idle' \| 'running' \| 'completed' \| 'error'` |
| `Workflow` | id, name, agent_id, status, created_at, updated_at, completed_at, total_tokens_input, total_tokens_output, total_cost_usd, model_id, current_context_tokens |
| `WorkflowResult` | report, metrics, tools_used, mcp_calls, tool_executions |
| `WorkflowMetrics` | duration_ms, tokens_input, tokens_output, cost_usd, provider, model |
| `WorkflowFullState` | workflow, messages, tool_executions, thinking_steps |
| `TokenDisplayData` | tokens_input, tokens_output, cumulative_input, cumulative_output, context_max, cost_usd, cumulative_cost_usd, speed_tks, is_streaming |

#### streaming.ts

| Type | Fields |
|------|--------|
| `ChunkType` | `'token' \| 'tool_start' \| 'tool_end' \| 'reasoning' \| 'error' \| 'sub_agent_start' \| 'sub_agent_progress' \| 'sub_agent_complete' \| 'sub_agent_error' \| 'task_create' \| 'task_update' \| 'task_complete'` |
| `SubAgentStreamMetrics` | duration_ms, tokens_input, tokens_output |
| `StreamChunk` | workflow_id, chunk_type, content, tool, duration, sub_agent_id, sub_agent_name, parent_agent_id, metrics, progress, task_id, task_name, task_status, task_priority |
| `WorkflowComplete` | workflow_id, status ('completed' \| 'error' \| 'cancelled'), error |

#### activity.ts

| Type | Fields |
|------|--------|
| `ActivityType` | `'tool_start' \| 'tool_complete' \| 'tool_error' \| 'reasoning' \| 'sub_agent_start' \| 'sub_agent_progress' \| 'sub_agent_complete' \| 'sub_agent_error' \| 'validation' \| 'message' \| 'task_create' \| 'task_update' \| 'task_complete'` |
| `ActivityStatus` | `'pending' \| 'running' \| 'completed' \| 'error'` |
| `ActivityFilter` | `'all' \| 'tools' \| 'agents' \| 'reasoning' \| 'todos'` |
| `WorkflowActivityEvent` | id, timestamp, type, title, description, status, duration, metadata |

### Components

#### Workflow Components (src/lib/components/workflow/)

| Component | Props | File |
|-----------|-------|------|
| `WorkflowItem` | workflow, active, onselect, ondelete, onrename | WorkflowItem.svelte |
| `WorkflowList` | workflows, selectedId, collapsed, onselect, ondelete, onrename | WorkflowList.svelte |
| `WorkflowItemCompact` | workflow, active, onselect | WorkflowItemCompact.svelte |
| `NewWorkflowModal` | open, agents, selectedAgentId, oncreate, onclose | NewWorkflowModal.svelte |
| `ConfirmDeleteModal` | open, workflowName, onconfirm, oncancel | ConfirmDeleteModal.svelte |
| `ActivityFeed` | activities, isStreaming, filter, onFilterChange, collapsed | ActivityFeed.svelte |
| `ActivityItem` | activity | ActivityItem.svelte |
| `ValidationModal` | request, open, onapprove, onreject, onclose | ValidationModal.svelte |
| `TokenDisplay` | data, compact | TokenDisplay.svelte |
| `MetricsBar` | - | MetricsBar.svelte |
| `ReasoningPanel` | - | ReasoningPanel.svelte |
| `ToolExecutionPanel` | - | ToolExecutionPanel.svelte |
| `SubAgentActivity` | - | SubAgentActivity.svelte |

#### Layout Components (src/lib/components/agent/)

| Component | Props | File |
|-----------|-------|------|
| `WorkflowSidebar` | collapsed, workflows, selectedWorkflowId, searchFilter, onsearchchange, onselect, oncreate, ondelete, onrename | WorkflowSidebar.svelte |
| `ActivitySidebar` | collapsed, activities, isStreaming, filter, onfilterchange | ActivitySidebar.svelte |
| `ChatContainer` | messages, messagesLoading, streamContent, isStreaming, disabled, onsend, oncancel | ChatContainer.svelte |
| `AgentHeader` | workflow, agents, selectedAgentId, maxIterations, agentsLoading, messagesLoading, onagentchange, oniterationschange | AgentHeader.svelte |

### Services

#### WorkflowService (src/lib/services/workflow.service.ts)

| Method | Parameters | Returns | Description |
|--------|------------|---------|-------------|
| `loadAll()` | - | `Promise<Workflow[]>` | Load all workflows from database |
| `create(name, agentId)` | name: string, agentId: string | `Promise<string>` | Create new workflow, returns ID |
| `rename(workflowId, name)` | workflowId: string, name: string | `Promise<Workflow>` | Rename workflow |
| `delete(workflowId)` | workflowId: string | `Promise<void>` | Delete workflow |
| `executeStreaming(workflowId, message, agentId, locale)` | workflowId: string, message: string, agentId: string, locale: string | `Promise<WorkflowResult>` | Execute workflow with streaming |
| `cancel(workflowId)` | workflowId: string | `Promise<void>` | Cancel ongoing execution |
| `getFullState(workflowId)` | workflowId: string | `Promise<WorkflowFullState>` | Get complete workflow state |
| `restoreState(workflowId)` | workflowId: string | `Promise<RestorationResult>` | Restore workflow from database |

---

## Execution Flow

### Non-Streaming (execute_workflow)

1. **Validation**: Validate workflow_id, message, agent_id using security::Validator
2. **Load Workflow**: Query SurrealDB with `meta::id()` to avoid Thing enum issues
3. **Create Task**: Generate task_id UUID and build Task struct with description
4. **Execute via Orchestrator**: `orchestrator.execute_with_mcp(agent_id, task, mcp_manager)`
5. **Get Agent Config**: Load provider/model info for accurate metrics
6. **Build WorkflowResult**: Convert tool executions, calculate cost, package report
7. **Return Result**: Send WorkflowResult to frontend

### Streaming (execute_workflow_streaming)

1. **Validation**: Same as non-streaming
2. **Create Cancellation Token**: Enable immediate cancellation via tokio::select!
3. **Load Workflow**: Same SurrealDB query pattern
4. **Generate Message ID**: Early UUID generation for thinking step references
5. **Emit Initial Events**: `StreamChunk::reasoning()` and `StreamChunk::token()`
6. **Persist Initial Thinking Step**: Save to `thinking_step` table
7. **Load Conversation History**: Query last 50 messages for context window
8. **Create Task**: With conversation history in context
9. **Emit Tool Start**: `StreamChunk::tool_start()` for agent execution
10. **Execute with Cancellation**: Race `execute_with_mcp()` vs `cancellation_token.cancelled()`
11. **Save System Prompt** (if first message): Persist to `message` table with role='system'
12. **Emit Completion Reasoning**: `StreamChunk::reasoning()` with duration + tool count
13. **Persist Completion Thinking Step**: Save to database
14. **Stream Response Content**: Split into 50-char chunks with 10ms delay
15. **Load Model for Pricing**: Query `llm_model` by api_name + provider
16. **Calculate Cost**: `calculate_cost(tokens_input, tokens_output, input_price, output_price)`
17. **Update Workflow Cumulative Metrics**: total_tokens_input, total_tokens_output, total_cost_usd
18. **Persist Tool Executions**: Save to `tool_execution` table
19. **Build WorkflowResult**: Same as non-streaming
20. **Emit Completion**: `WorkflowComplete::success()`
21. **Cleanup**: Remove cancellation token from map

### Parallel Execution (orchestrator.execute_parallel)

1. **Prepare Futures**: Map each (agent_id, task) to an async execution
2. **Join All**: `futures::join_all()` executes all tasks concurrently
3. **Return Results**: Vec of Results in same order as input
4. **Total Time**: Approximately equal to slowest individual task

### Cancellation (cancel_workflow_streaming)

1. **Validate Workflow ID**: Security check
2. **Request Cancellation**: Trigger cancellation_token for the workflow
3. **Orchestrator Abort**: tokio::select! detects cancellation
4. **Immediate Stop**: Execution halts mid-LLM-call if needed
5. **Cleanup**: Emit cancelled event, clear token, return error

---

## Types d'Operations Orchestrables

### Sous-Agents
Agents specialises delegues pour taches complexes

**Exemples** :
- DB Agent : Requetes et analytics database
- API Agent : Appels services externes
- Code Agent : Refactoring et analyse code

### Tools (MCP Locaux)
Outils custom exposes via MCP Server interne

**Outils Disponibles** :
- `MemoryTool` : Persistance memoire (add, list, get, delete, search)
- `TodoTool` : Gestion taches (create, list, update, complete, delete)
- `SpawnAgentTool` : Delegation sous-agent (primary agent only)
- `ParallelTasksTool` : Execution parallele (primary agent only)
- `InternalReportTool` : Rapport interne (sub-agent only)

### MCP Servers Externes
Services MCP distants accessibles via MCP Client

**Exemples** :
- `Serena` : Analyse semantique codebase
- `Context7` : Documentation officielle libraries
- `Playwright` : Automation browser
- `Sequential-thinking` : Raisonnement multi-etapes

---

## Matrice de Decision

### Detection Automatique des Dependances

```rust
// Conceptuel - Analyse de dependances
struct Operation {
    id: String,
    inputs: Vec<DataRef>,      // Donnees requises
    outputs: Vec<DataRef>,     // Donnees produites
    operation_type: OpType,    // SubAgent | Tool | MCP
}

fn analyze_dependencies(ops: Vec<Operation>) -> ExecutionPlan {
    let mut graph = DependencyGraph::new();

    for op in ops {
        graph.add_node(op.id);
        for input in op.inputs {
            if let Some(producer) = find_producer(&ops, &input) {
                graph.add_edge(producer.id, op.id);
            }
        }
    }

    graph.parallel_batches() // Retourne groupes executables en parallele
}
```

### Exemples de Classification

| Scenario | Type | Raison |
|----------|------|--------|
| Lire 5 fichiers code | **Parallele** | Lectures independantes |
| Analyser puis refactorer | **Sequentiel** | Refactor necessite resultats analyse |
| Query users + Query messages | **Parallele** | Requetes DB independantes |
| Fetch API data puis store DB | **Sequentiel** | Store necessite data fetchee |
| Appel serena + context7 | **Parallele** | MCP servers distincts, pas dependance |
| Search code -> Refactor matches | **Sequentiel** | Refactor depend de search results |

---

## Patterns d'Orchestration

### Pattern 1 : Fan-Out / Fan-In

**Cas d'usage** : Operations paralleles suivies d'agregation

```
Orchestrateur
    |-- [Parallel] Agent DB (query users)
    |-- [Parallel] Agent API (fetch external data)
    +-- [Parallel] MCP serena (search code patterns)
    v
[Sequential] Agregation resultats -> Decision
```

### Pattern 2 : Pipeline Sequentiel

**Cas d'usage** : Transformations en chaine

```
Orchestrateur
    v
MCP serena (find symbols) [Sequential]
    v
Tool validate_refactor [Sequential]
    v
Agent Code (apply refactor) [Sequential]
    v
Tool store_memory (save changes) [Sequential]
```

### Pattern 3 : Hybride (Optimise)

**Cas d'usage** : Melange parallele + sequentiel

```
Orchestrateur
    |-- [Parallel] MCP context7 (get docs React)
    +-- [Parallel] MCP context7 (get docs Svelte)
    v
[Sequential] Agent UI (generate component avec docs)
    v
[Parallel] MCP playwright (test accessibility)
[Parallel] Tool validate_a11y (WCAG check)
    v
[Sequential] Agregation validation -> Report
```

---

## Gestion des Erreurs

### Strategies selon Type d'Execution

**Parallele** :
- Echec partiel acceptable si pas critique
- Continue avec resultats disponibles
- Log erreurs pour review

```rust
let results = join_all(parallel_ops).await;
let successful = results.into_iter()
    .filter_map(|r| r.ok())
    .collect();

if successful.is_empty() {
    return Err("All parallel operations failed");
}
```

**Sequentiel** :
- Echec = arret pipeline immediat
- Rollback si necessaire
- Notification utilisateur

```rust
let step1 = operation_a().await?; // ? = fail-fast
let step2 = operation_b(step1).await?;
let step3 = operation_c(step2).await?;
```

### Retry Logic

**Operations idempotentes** : Retry automatique avec backoff

```rust
async fn retry_operation<T>(
    op: impl Future<Output = Result<T>>,
    max_attempts: u32
) -> Result<T> {
    for attempt in 1..=max_attempts {
        match op.await {
            Ok(result) => return Ok(result),
            Err(e) if attempt < max_attempts => {
                sleep(Duration::from_millis(100 * 2u64.pow(attempt))).await;
                continue;
            }
            Err(e) => return Err(e),
        }
    }
}
```

**Operations non-idempotentes** : Pas de retry automatique, validation humaine si critique

---

## Optimisations Performance

### Batch Processing

**Regroupe operations similaires** pour reduire overhead

```rust
// Au lieu de : N appels individuels
for file in files {
    mcp_client.call("serena::read_file", file).await;
}

// Preferer : 1 appel batch
mcp_client.call("serena::read_files_batch", files).await;
```

### Caching Intelligent

**Evite recalculs** pour operations deterministes

```rust
let cache_key = format!("context7::docs::{}", library);
if let Some(cached) = cache.get(&cache_key) {
    return cached;
}
let result = mcp_client.call("context7::get_library_docs", library).await?;
cache.insert(cache_key, result.clone(), Duration::from_secs(3600));
```

### Timeouts (OPT-WF-9)

**Timeouts configures** via constantes dans `tools/constants.rs`:

```rust
// src-tauri/src/tools/constants.rs
pub mod workflow {
    pub const LLM_EXECUTION_TIMEOUT_SECS: u64 = 300;       // 5 minutes pour execution LLM
    pub const DB_OPERATION_TIMEOUT_SECS: u64 = 30;         // 30 secondes pour DB ops
    pub const FULL_STATE_LOAD_TIMEOUT_SECS: u64 = 60;      // 60 secondes pour chargement etat complet
    pub const MESSAGE_HISTORY_LIMIT: usize = 50;           // Max messages dans contexte
}
```

**Utilisation**:
```rust
use tokio::time::timeout;
use crate::tools::constants::workflow as wf_const;

// execute_workflow() - timeout sur execution LLM
let report = timeout(
    Duration::from_secs(wf_const::LLM_EXECUTION_TIMEOUT_SECS),
    execution_future,
).await.map_err(|_| "Workflow execution timed out")?;

// load_workflow_full_state() - timeout sur chargement parallele
let results = timeout(
    Duration::from_secs(wf_const::FULL_STATE_LOAD_TIMEOUT_SECS),
    parallel_queries,
).await?;
```

---

## Monitoring et Observabilite

### Metriques par Workflow

```rust
struct WorkflowMetrics {
    total_duration: Duration,
    parallel_batches: Vec<BatchMetrics>,
    sequential_steps: Vec<StepMetrics>,

    parallelization_ratio: f32,  // Ops paralleles / Total ops
    speedup_factor: f32,         // Temps theorique seq / Temps reel
}
```

### Visualisation Execution

**Gantt Chart** pour analyser bottlenecks

```
Time ->
0ms     500ms    1000ms   1500ms   2000ms
|--------|--------|--------|--------|--------|
[DB Query                        ] 1800ms <- Bottleneck
[API Call 1     ]
[API Call 2       ]
[MCP serena  ]
                 [Aggregate       ] 400ms
```

---

## Best Practices

### DO

- **Analyser dependances** avant execution
- **Maximiser parallelisme** quand pas de dependances
- **Batch similaires operations** pour overhead reduit
- **Timeout adaptatifs** selon historique performance
- **Cache resultats** deterministes
- **Log detaille** pour debugging et optimisation
- **Fail-fast** sur erreurs critiques en sequentiel

### DON'T

- **Sous-agents lancent sous-agents** : Violation regle architecture
- **Paralleliser avec dependances** : Resultats incorrects
- **Ignorer erreurs paralleles** : Valider resultats partiels acceptables
- **Timeout uniformes** : Ajuster selon type operation
- **Surcharge parallelisme** : Limite selon ressources disponibles (CPU, memoire)
- **Nesting excessif** : Max 3 niveaux orchestration

---

## References

**Architecture** : [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
**Tools Agents** : [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)
**MCP Integration** : [MCP_ARCHITECTURE_DECISION.md](MCP_ARCHITECTURE_DECISION.md)
**API Reference** : [API_REFERENCE.md](API_REFERENCE.md)
**Database Schema** : [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)

---

**File Locations**:
- Backend Commands: `src-tauri/src/commands/workflow.rs`, `src-tauri/src/commands/streaming.rs`
- Orchestrator: `src-tauri/src/agents/core/orchestrator.rs`
- Query Constants: `src-tauri/src/db/queries.rs` (OPT-WF-1: centralized queries + cascade module)
- Timeout Constants: `src-tauri/src/tools/constants.rs` (workflow module, OPT-WF-3/9)
- Models: `src-tauri/src/models/workflow.rs`, `src-tauri/src/models/streaming.rs`
- Frontend Stores: `src/lib/stores/workflows.ts`, `src/lib/stores/streaming.ts`, `src/lib/stores/activity.ts`
- Frontend Types: `src/types/workflow.ts`, `src/types/streaming.ts`, `src/types/activity.ts`
- Frontend Services: `src/lib/services/workflow.service.ts`
- Components: `src/lib/components/workflow/`, `src/lib/components/agent/`
