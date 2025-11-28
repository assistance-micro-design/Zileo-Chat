# API Reference - Tauri Commands

> Référence technique IPC Frontend ↔ Backend

## Architecture IPC

```
Frontend (TypeScript)
    ↓ invoke()
Tauri Commands (Rust)
    ↓ Business Logic
Backend Services
```

**Pattern** : Async/await both sides, Result<T, String> pour errors

---

## Workflows

### execute_workflow

Execute workflow avec agent spécifique.

**Frontend**
```typescript
const result = await invoke<WorkflowResult>('execute_workflow', {
  workflowId: string,
  message: string,
  agentId: string
});
```

**Backend Signature**
```rust
async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String
) -> Result<WorkflowResult, String>
```

**Returns** : `WorkflowResult`
- `report` : Markdown report agent
- `metrics` : Tokens, duration, cost
- `tools_used` : Liste tools exécutés
- `mcp_calls` : Liste MCP servers appelés

**Errors** : Agent not found, workflow not found, execution failed

---

### create_workflow

Créer nouveau workflow.

**Frontend**
```typescript
const id = await invoke<string>('create_workflow', {
  name: string,
  agentId: string
});
```

**Backend Signature**
```rust
async fn create_workflow(
    name: String,
    agent_id: String
) -> Result<String, String>
```

**Returns** : UUID workflow créé

**Errors** : Agent invalide, validation name failed

---

### save_workflow_state

Persiste état workflow dans DB.

**Frontend**
```typescript
await invoke('save_workflow_state', {
  id: string,
  state: WorkflowState
});
```

**Backend Signature**
```rust
async fn save_workflow_state(
    id: String,
    state: WorkflowState
) -> Result<(), String>
```

**WorkflowState Type**
```rust
struct WorkflowState {
    name: String,
    status: WorkflowStatus,
    messages: Vec<Message>,
    tools: Vec<ToolExecution>,
    metrics: WorkflowMetrics
}
```

**Errors** : DB connection failed, invalid state

---

### load_workflows

Charge workflows (actifs, complétés, ou tous).

**Frontend**
```typescript
const workflows = await invoke<Workflow[]>('load_workflows', {
  filter?: 'active' | 'completed' | 'all'
});
```

**Backend Signature**
```rust
async fn load_workflows(
    filter: Option<String>
) -> Result<Vec<Workflow>, String>
```

**Returns** : Array workflows avec metadata

---

### delete_workflow

Supprime workflow + données associées.

**Frontend**
```typescript
await invoke('delete_workflow', {
  id: string,
  force?: boolean
});
```

**Backend Signature**
```rust
async fn delete_workflow(
    id: String,
    force: Option<bool>
) -> Result<(), String>
```

**force** : Si true, ignore running status
**Errors** : Workflow running (sans force), not found

---

## Agents

### list_agents

Liste tous les agents configurés avec résumé (permanent + temporary).

**Frontend**
```typescript
const agents = await invoke<AgentSummary[]>('list_agents');
```

**Backend Signature**
```rust
async fn list_agents(
    state: State<'_, AppState>
) -> Result<Vec<AgentSummary>, String>
```

**AgentSummary Type**
```typescript
interface AgentSummary {
  id: string;
  name: string;
  lifecycle: 'permanent' | 'temporary';
  provider: string;              // LLM provider (Mistral, Ollama)
  model: string;                 // Model name
  tools_count: number;           // Number of enabled tools
  mcp_servers_count: number;     // Number of MCP servers
}
```

**Returns** : Array de résumés agents (léger, sans system_prompt)

---

### get_agent_config

Récupère configuration complète d'un agent.

**Frontend**
```typescript
const config = await invoke<AgentConfig>('get_agent_config', {
  agentId: string
});
```

**Backend Signature**
```rust
async fn get_agent_config(
    agent_id: String,
    state: State<'_, AppState>
) -> Result<AgentConfig, String>
```

**AgentConfig Type**
```typescript
interface AgentConfig {
  id: string;
  name: string;
  lifecycle: 'permanent' | 'temporary';
  llm: {
    provider: string;
    model: string;
    temperature: number;      // 0.0-2.0
    max_tokens: number;       // 256-128000
  };
  tools: string[];            // ["MemoryTool", "TodoTool"]
  mcp_servers: string[];      // MCP server names
  system_prompt: string;
  created_at?: string;        // ISO 8601
  updated_at?: string;        // ISO 8601
}
```

**Errors** : Agent not found

---

### create_agent

Crée un nouvel agent avec configuration complète.

**Frontend**
```typescript
const agentId = await invoke<string>('create_agent', {
  config: {
    name: string,                    // 1-64 chars
    lifecycle: 'permanent' | 'temporary',
    llm: {
      provider: 'Mistral' | 'Ollama',
      model: string,
      temperature: number,           // 0.0-2.0
      max_tokens: number             // 256-128000
    },
    tools: string[],                 // ["MemoryTool", "TodoTool"]
    mcp_servers: string[],           // MCP server names
    system_prompt: string            // 1-10000 chars
  }
});
```

**Backend Signature**
```rust
async fn create_agent(
    config: AgentConfigCreate,
    state: State<'_, AppState>
) -> Result<String, String>
```

**Validation**
- `name`: 1-64 caractères, non vide
- `temperature`: 0.0-2.0
- `max_tokens`: 256-128000
- `tools`: Doit être dans KNOWN_TOOLS ("MemoryTool", "TodoTool")
- `system_prompt`: Non vide

**Returns** : UUID de l'agent créé

**Errors** : Validation failed, database error

---

### update_agent

Met à jour un agent existant (mise à jour partielle).

**Frontend**
```typescript
const updated = await invoke<AgentConfig>('update_agent', {
  agentId: string,
  config: {
    name?: string,
    llm?: LLMConfig,
    tools?: string[],
    mcp_servers?: string[],
    system_prompt?: string
    // Note: lifecycle cannot be changed after creation
  }
});
```

**Backend Signature**
```rust
async fn update_agent(
    agent_id: String,
    config: AgentConfigUpdate,
    state: State<'_, AppState>
) -> Result<AgentConfig, String>
```

**Returns** : Agent mis à jour avec configuration complète

**Errors** : Agent not found, validation failed

---

### delete_agent

Supprime un agent de la base de données et du registry.

**Frontend**
```typescript
await invoke('delete_agent', {
  agentId: string
});
```

**Backend Signature**
```rust
async fn delete_agent(
    agent_id: String,
    state: State<'_, AppState>
) -> Result<(), String>
```

**Effect** : Supprime de SurrealDB et désenregistre du AgentRegistry

**Errors** : Agent not found, database error

---

## Validation (Human-in-the-Loop)

### request_validation

Demande validation utilisateur pour opération.

**Backend → Frontend (Event)**
```typescript
listen<ValidationRequest>('validation_request', (event) => {
  const request = event.payload;
  // Afficher modal validation UI
});
```

**ValidationRequest Type**
```typescript
type ValidationRequest = {
  id: string;
  workflow_id: string;
  type: 'tool' | 'sub_agent' | 'mcp' | 'file_op' | 'db_op';
  operation: string;
  details: Record<string, any>;
  risk_level: 'low' | 'medium' | 'high';
};
```

---

### respond_validation

Envoie réponse validation.

**Frontend**
```typescript
await invoke('respond_validation', {
  requestId: string,
  approved: boolean
});
```

**Backend Signature**
```rust
async fn respond_validation(
    request_id: String,
    approved: bool
) -> Result<(), String>
```

**Effect** : Workflow resume (si approved) ou skip operation (si rejected)

---

## Memory

### add_memory

Ajoute mémoire vectorielle.

**Frontend**
```typescript
await invoke('add_memory', {
  type: 'user_pref' | 'context' | 'knowledge' | 'decision',
  content: string,
  tags?: string[],
  workflowId?: string
});
```

**Backend Signature**
```rust
async fn add_memory(
    type_: MemoryType,
    content: String,
    tags: Option<Vec<String>>,
    workflow_id: Option<String>
) -> Result<String, String>
```

**Returns** : UUID mémoire créée
**Process** : Generate embedding → Store avec vector index

---

### search_memory

Recherche sémantique mémoires.

**Frontend**
```typescript
const results = await invoke<Memory[]>('search_memory', {
  query: string,
  topK?: number,
  filters?: MemoryFilters
});
```

**Backend Signature**
```rust
async fn search_memory(
    query: String,
    top_k: Option<usize>,
    filters: Option<MemoryFilters>
) -> Result<Vec<Memory>, String>
```

**Process** : Generate query embedding → KNN search → Return ranked results

---

### list_memories

Liste mémoires avec pagination.

**Frontend**
```typescript
const memories = await invoke<Memory[]>('list_memories', {
  page: number,
  perPage: number,
  filters?: MemoryFilters
});
```

**Backend Signature**
```rust
async fn list_memories(
    page: usize,
    per_page: usize,
    filters: Option<MemoryFilters>
) -> Result<Vec<Memory>, String>
```

---

### delete_memory

Supprime mémoire.

**Frontend**
```typescript
await invoke('delete_memory', {
  id: string
});
```

**Backend Signature**
```rust
async fn delete_memory(
    id: String
) -> Result<(), String>
```

---

## LLM Models CRUD

### list_models

Liste tous les modeles LLM (builtin + custom), avec filtre optionnel par provider.

**Frontend**
```typescript
const models = await invoke<LLMModel[]>('list_models', {
  provider: 'mistral' | 'ollama' | null  // Optional filter
});
```

**Backend Signature**
```rust
async fn list_models(
    provider: Option<String>
) -> Result<Vec<LLMModel>, String>
```

**LLMModel Type**
```typescript
interface LLMModel {
  id: string;                    // UUID for custom, api_name for builtin
  provider: 'mistral' | 'ollama';
  name: string;                  // Human-readable display name
  api_name: string;              // Model identifier for API calls
  context_window: number;        // Max context length (1024 - 2,000,000)
  max_output_tokens: number;     // Max generation length (256 - 128,000)
  temperature_default: number;   // Default temperature (0.0 - 2.0)
  is_builtin: boolean;           // True = system model, cannot delete
  created_at: string;            // ISO 8601 timestamp
  updated_at: string;            // ISO 8601 timestamp
}
```

**Returns** : Array de modeles correspondant au filtre

**Errors** : Invalid provider, database error

---

### get_model

Recupere un modele par ID.

**Frontend**
```typescript
const model = await invoke<LLMModel>('get_model', {
  id: string
});
```

**Backend Signature**
```rust
async fn get_model(
    id: String
) -> Result<LLMModel, String>
```

**Returns** : LLMModel

**Errors** : Model not found, invalid ID

---

### create_model

Cree un nouveau modele custom.

**Frontend**
```typescript
const model = await invoke<LLMModel>('create_model', {
  data: {
    provider: 'mistral' | 'ollama',
    name: string,                  // 1-64 chars
    api_name: string,              // Unique per provider
    context_window: number,        // 1024 - 2,000,000
    max_output_tokens: number,     // 256 - 128,000
    temperature_default?: number   // 0.0 - 2.0, default 0.7
  }
});
```

**Backend Signature**
```rust
async fn create_model(
    data: CreateModelRequest
) -> Result<LLMModel, String>
```

**Returns** : Created LLMModel with generated UUID

**Errors** :
- Validation failed (name, api_name, context_window, etc.)
- Duplicate api_name for provider

---

### update_model

Met a jour un modele existant.

**Frontend**
```typescript
const model = await invoke<LLMModel>('update_model', {
  id: string,
  data: {
    name?: string,
    api_name?: string,
    context_window?: number,
    max_output_tokens?: number,
    temperature_default?: number
  }
});
```

**Backend Signature**
```rust
async fn update_model(
    id: String,
    data: UpdateModelRequest
) -> Result<LLMModel, String>
```

**Restrictions** :
- Builtin models: only `temperature_default` can be modified
- Custom models: all fields modifiable

**Returns** : Updated LLMModel

**Errors** : Model not found, validation failed, builtin restriction

---

### delete_model

Supprime un modele custom.

**Frontend**
```typescript
const success = await invoke<boolean>('delete_model', {
  id: string
});
```

**Backend Signature**
```rust
async fn delete_model(
    id: String
) -> Result<bool, String>
```

**Restrictions** : Builtin models cannot be deleted

**Returns** : true if deleted

**Errors** : Model not found, cannot delete builtin model

---

## Provider Settings

### get_provider_settings

Recupere la configuration d'un provider.

**Frontend**
```typescript
const settings = await invoke<ProviderSettings>('get_provider_settings', {
  provider: 'mistral' | 'ollama'
});
```

**Backend Signature**
```rust
async fn get_provider_settings(
    provider: String
) -> Result<ProviderSettings, String>
```

**ProviderSettings Type**
```typescript
interface ProviderSettings {
  provider: 'mistral' | 'ollama';
  enabled: boolean;
  default_model_id: string | null;
  api_key_configured: boolean;  // Never exposes actual key
  base_url: string | null;      // Custom URL (primarily Ollama)
  updated_at: string;           // ISO 8601 timestamp
}
```

**Returns** : ProviderSettings (default values if not yet configured)

---

### update_provider_settings

Met a jour la configuration d'un provider.

**Frontend**
```typescript
// IMPORTANT: Use camelCase for parameter names
const settings = await invoke<ProviderSettings>('update_provider_settings', {
  provider: 'mistral' | 'ollama',
  enabled: boolean | null,
  defaultModelId: string | null,  // camelCase, not default_model_id
  baseUrl: string | null          // camelCase, not base_url
});
```

**Backend Signature**
```rust
async fn update_provider_settings(
    provider: String,
    enabled: Option<bool>,
    default_model_id: Option<String>,
    base_url: Option<String>
) -> Result<ProviderSettings, String>
```

**Upsert Behavior** : Creates settings if not exists, updates otherwise

**Returns** : Updated ProviderSettings

**Errors** : Invalid provider, default_model_id doesn't exist

---

### test_provider_connection

Teste la connexion a un provider.

**Frontend**
```typescript
const result = await invoke<ConnectionTestResult>('test_provider_connection', {
  provider: 'mistral' | 'ollama'
});
```

**Backend Signature**
```rust
async fn test_provider_connection(
    provider: String
) -> Result<ConnectionTestResult, String>
```

**ConnectionTestResult Type**
```typescript
interface ConnectionTestResult {
  provider: 'mistral' | 'ollama';
  success: boolean;
  latency_ms: number | null;     // RTT in milliseconds if successful
  error_message: string | null;  // Error details if failed
  model_tested: string | null;   // Model used for test (if applicable)
}
```

**Test Methods** :
- Mistral: GET /v1/models with API key
- Ollama: GET /api/version

**Timeout** : 10 seconds

**Returns** : ConnectionTestResult (success or failure with details)

---

### seed_builtin_models

Seed la database avec les modeles builtin.

**Frontend**
```typescript
const insertedCount = await invoke<number>('seed_builtin_models');
```

**Backend Signature**
```rust
async fn seed_builtin_models() -> Result<usize, String>
```

**Behavior** :
- Called automatically at app startup if table is empty
- Safe to call multiple times (skips existing models)

**Returns** : Number of models inserted

---

## Task Commands (Todo Tool)

Task management for workflow decomposition. Enables agents to track progress on complex multi-step operations.

### create_task

Creates a new task for a workflow.

**Frontend**
```typescript
const taskId = await invoke<string>('create_task', {
  workflowId: string,           // Associated workflow ID
  name: string,                 // Task name (max 128 chars)
  description: string,          // Task description (max 1000 chars)
  priority?: 1 | 2 | 3 | 4 | 5, // Priority (default: 3)
  agentAssigned?: string,       // Agent ID to assign
  dependencies?: string[]       // Task IDs this depends on
});
```

**Backend Signature**
```rust
async fn create_task(
    workflow_id: String,
    name: String,
    description: String,
    priority: Option<u8>,
    agent_assigned: Option<String>,
    dependencies: Option<Vec<String>>
) -> Result<String, String>
```

**Priority Levels**
- 1: Critical - must be done immediately
- 2: High - should be done soon
- 3: Medium - normal priority (default)
- 4: Low - can wait
- 5: Minimal - do when time permits

**Returns** : UUID of created task

**Errors** : Invalid workflow_id, name too long, invalid priority

---

### get_task

Gets a single task by ID.

**Frontend**
```typescript
const task = await invoke<Task>('get_task', {
  taskId: string
});
```

**Backend Signature**
```rust
async fn get_task(
    task_id: String
) -> Result<Task, String>
```

**Task Type**
```typescript
interface Task {
  id: string;
  workflow_id: string;
  name: string;
  description: string;
  agent_assigned?: string;
  priority: 1 | 2 | 3 | 4 | 5;
  status: 'pending' | 'in_progress' | 'completed' | 'blocked';
  dependencies: string[];
  duration_ms?: number;
  created_at: string;
  completed_at?: string;
}
```

**Errors** : Task not found, invalid ID

---

### list_workflow_tasks

Lists all tasks for a workflow.

**Frontend**
```typescript
const tasks = await invoke<Task[]>('list_workflow_tasks', {
  workflowId: string
});
```

**Backend Signature**
```rust
async fn list_workflow_tasks(
    workflow_id: String
) -> Result<Vec<Task>, String>
```

**Returns** : Tasks sorted by priority (asc), then created_at (asc)

---

### list_tasks_by_status

Lists tasks filtered by status.

**Frontend**
```typescript
const tasks = await invoke<Task[]>('list_tasks_by_status', {
  status: 'pending' | 'in_progress' | 'completed' | 'blocked',
  workflowId?: string  // Optional filter
});
```

**Backend Signature**
```rust
async fn list_tasks_by_status(
    status: String,
    workflow_id: Option<String>
) -> Result<Vec<Task>, String>
```

**Returns** : Tasks matching status, sorted by priority and created_at

**Errors** : Invalid status value

---

### update_task

Updates task fields (partial update).

**Frontend**
```typescript
const updated = await invoke<Task>('update_task', {
  taskId: string,
  updates: {
    name?: string,
    description?: string,
    agentAssigned?: string,
    priority?: 1 | 2 | 3 | 4 | 5,
    status?: 'pending' | 'in_progress' | 'completed' | 'blocked',
    dependencies?: string[],
    durationMs?: number
  }
});
```

**Backend Signature**
```rust
async fn update_task(
    task_id: String,
    updates: TaskUpdate
) -> Result<Task, String>
```

**Returns** : Updated task

**Errors** : No fields to update, validation failed, task not found

---

### update_task_status

Updates task status specifically (convenience command).

**Frontend**
```typescript
const updated = await invoke<Task>('update_task_status', {
  taskId: string,
  status: 'pending' | 'in_progress' | 'completed' | 'blocked'
});
```

**Backend Signature**
```rust
async fn update_task_status(
    task_id: String,
    status: String
) -> Result<Task, String>
```

**Returns** : Updated task

**Errors** : Invalid status, task not found

---

### complete_task

Marks task as completed with optional duration.

**Frontend**
```typescript
const completed = await invoke<Task>('complete_task', {
  taskId: string,
  durationMs?: number  // Execution duration in milliseconds
});
```

**Backend Signature**
```rust
async fn complete_task(
    task_id: String,
    duration_ms: Option<u64>
) -> Result<Task, String>
```

**Effect** : Sets status to 'completed', records completed_at timestamp

**Returns** : Completed task with metrics

---

### delete_task

Deletes a task.

**Frontend**
```typescript
await invoke('delete_task', {
  taskId: string
});
```

**Backend Signature**
```rust
async fn delete_task(
    task_id: String
) -> Result<(), String>
```

**Errors** : Task not found, database error

---

## MCP Servers

### list_mcp_servers

Liste MCP servers configurés.

**Frontend**
```typescript
const servers = await invoke<MCPServer[]>('list_mcp_servers');
```

**MCPServer Type**
```typescript
type MCPServer = {
  name: string;
  status: 'online' | 'offline' | 'error';
  capabilities: string[];
  latency_avg_ms?: number;
  error_count: number;
};
```

---

### test_mcp_server

Test connexion MCP server.

**Frontend**
```typescript
const result = await invoke<ConnectionTest>('test_mcp_server', {
  serverName: string
});
```

**Effect** : Execute simple discovery (list_tools) pour vérifier

---

### update_mcp_config

Met à jour configuration MCP server.

**Frontend**
```typescript
await invoke('update_mcp_config', {
  serverName: string,
  config: MCPConfig
});
```

**MCPConfig**
```typescript
type MCPConfig = {
  command: 'docker' | 'npx' | 'uvx';
  args: string[];
  env?: Record<string, string>;
};
```

**Restart Required** : Oui (v1), config chargée au startup

---

## Utilities

### count_tokens

Compte tokens texte selon provider.

**Frontend**
```typescript
const count = await invoke<number>('count_tokens', {
  text: string,
  provider: string
});
```

**Backend Signature**
```rust
async fn count_tokens(
    text: String,
    provider: String
) -> Result<usize, String>
```

**Uses** : Provider-specific tokenizer

---

### export_workflow

Exporte workflow en JSON/Markdown.

**Frontend**
```typescript
const data = await invoke<string>('export_workflow', {
  id: string,
  format: 'json' | 'markdown'
});
```

**Backend Signature**
```rust
async fn export_workflow(
    id: String,
    format: String
) -> Result<String, String>
```

**Returns** : Serialized data

---

### import_workflow

Importe workflow depuis JSON.

**Frontend**
```typescript
const id = await invoke<string>('import_workflow', {
  data: string
});
```

**Backend Signature**
```rust
async fn import_workflow(
    data: String
) -> Result<String, String>
```

**Validation** : Schema validation avant import

---

## Sub-Agent Tools

The sub-agent system allows the primary workflow agent to spawn, delegate to, and execute tasks in parallel across specialized agents.

### SpawnAgentTool

Internal tool used by agents to spawn temporary sub-agents for specialized tasks.

**Tool Definition**
```json
{
  "id": "SpawnAgentTool",
  "name": "Spawn Agent",
  "operations": ["spawn", "list", "terminate"]
}
```

**Operations**

**spawn**: Create and execute a temporary sub-agent
```json
{
  "operation": "spawn",
  "name": "CodeAnalyzer",
  "prompt": "Analyze the codebase for security vulnerabilities...",
  "tools": ["MemoryTool", "TodoTool"],
  "mcp_servers": ["serena"]
}
```

**list**: List active sub-agents for current workflow
```json
{
  "operation": "list"
}
```

**terminate**: Terminate a running sub-agent
```json
{
  "operation": "terminate",
  "child_id": "sub_agent_uuid"
}
```

**Result Type**
```typescript
interface SubAgentSpawnResult {
  success: boolean;
  child_id: string;
  report: string;  // Markdown report from sub-agent
  metrics: {
    duration_ms: number;
    tokens_input: number;
    tokens_output: number;
  };
}
```

**Constraints**
- Maximum 3 sub-agents per workflow
- Only primary agent can spawn sub-agents
- Sub-agents cannot spawn their own sub-agents (single level hierarchy)

---

### DelegateTaskTool

Internal tool for delegating tasks to existing permanent agents.

**Tool Definition**
```json
{
  "id": "DelegateTaskTool",
  "name": "Delegate Task",
  "operations": ["delegate", "list_agents"]
}
```

**Operations**

**delegate**: Delegate a task to a permanent agent
```json
{
  "operation": "delegate",
  "agent_id": "db_agent",
  "prompt": "Analyze the database schema for performance issues..."
}
```

**list_agents**: List available permanent agents
```json
{
  "operation": "list_agents"
}
```

**Result Type**
```typescript
interface DelegateResult {
  success: boolean;
  agent_id: string;
  report: string;
  metrics: {
    duration_ms: number;
    tokens_input: number;
    tokens_output: number;
  };
}
```

**Constraints**
- Only primary agent can delegate tasks
- Cannot delegate to self
- Target agent must exist in registry

---

### ParallelTasksTool

Internal tool for executing multiple tasks in parallel across different agents.

**Tool Definition**
```json
{
  "id": "ParallelTasksTool",
  "name": "Parallel Tasks",
  "operations": ["execute_batch"]
}
```

**Operations**

**execute_batch**: Execute multiple tasks in parallel
```json
{
  "operation": "execute_batch",
  "tasks": [
    {"agent_id": "db_agent", "prompt": "Analyze database schema..."},
    {"agent_id": "api_agent", "prompt": "Review API security..."},
    {"agent_id": "ui_agent", "prompt": "Check accessibility..."}
  ]
}
```

**Result Type**
```typescript
interface ParallelBatchResult {
  success: boolean;
  completed: number;
  failed: number;
  results: Array<{
    agent_id: string;
    success: boolean;
    report?: string;
    error?: string;
    metrics?: {
      duration_ms: number;
      tokens_input: number;
      tokens_output: number;
    };
  }>;
  aggregated_report: string;  // Combined markdown report
}
```

**Constraints**
- Maximum 3 tasks per batch
- Only primary agent can execute parallel tasks
- All tasks execute concurrently (total time ~= slowest task)
- Each agent only receives its prompt (no shared context)

---

### Sub-Agent Validation Events

Human-in-the-loop validation for sub-agent operations.

**validation_required Event**
```typescript
listen<ValidationRequiredEvent>('validation_required', (event) => {
  const {
    validation_id,
    workflow_id,
    operation_type,  // 'spawn' | 'delegate' | 'parallel_batch'
    operation,
    risk_level,      // 'low' | 'medium' | 'high'
    details
  } = event.payload;
});
```

**Approve/Reject Commands**
```typescript
await invoke('approve_validation', { validationId: string });
await invoke('reject_validation', { validationId: string, reason?: string });
```

---

### Sub-Agent Streaming Events

Real-time events for sub-agent execution monitoring.

**Stream Chunk Types**
```typescript
type SubAgentChunkType =
  | 'sub_agent_start'     // Sub-agent execution started
  | 'sub_agent_progress'  // Progress update (0-100%)
  | 'sub_agent_complete'  // Sub-agent finished successfully
  | 'sub_agent_error';    // Sub-agent encountered error
```

**Listen for Sub-Agent Events**
```typescript
listen<StreamChunk>('workflow_stream', (event) => {
  const chunk = event.payload;
  if (chunk.chunk_type.startsWith('sub_agent_')) {
    console.log('Sub-agent event:', {
      subAgentId: chunk.sub_agent_id,
      subAgentName: chunk.sub_agent_name,
      parentAgentId: chunk.parent_agent_id,
      content: chunk.content,
      metrics: chunk.metrics
    });
  }
});
```

---

## Events (Backend → Frontend)

### workflow_stream

Streaming tokens/status workflow.

**Écoute**
```typescript
const unlisten = await listen<StreamChunk>('workflow_stream', (event) => {
  const chunk = event.payload;
  // chunk.type: 'token' | 'tool_start' | 'tool_end' | 'reasoning'
});
```

**StreamChunk Type**
```typescript
type StreamChunk = {
  workflow_id: string;
  type: 'token' | 'tool_start' | 'tool_end' | 'reasoning' | 'error';
  content?: string;
  tool?: string;
  duration?: number;
};
```

---

### validation_request

Demande validation (cf section Validation)

---

### workflow_complete

Notification workflow terminé.

**Écoute**
```typescript
await listen<WorkflowComplete>('workflow_complete', (event) => {
  const { workflow_id, status } = event.payload;
});
```

---

### agent_status_update

Mise à jour status agent.

**Écoute**
```typescript
await listen<AgentStatus>('agent_status_update', (event) => {
  const { agent_id, status } = event.payload;
  // status: 'available' | 'busy'
});
```

---

## Error Handling

### Error Format Standard

```typescript
try {
  await invoke('command', { params });
} catch (error) {
  // error: string (user-friendly message)
  console.error(error);
}
```

**Backend Pattern**
```rust
.map_err(|e| format!("Operation failed: {}", e))
```

**User Messages** : Pas stack traces, actions correctives suggérées

---

## Types TypeScript (Frontend)

Types générés depuis Rust avec `ts-rs` ou manuellement synchronisés.

**Localisation** : `src/lib/types/api.ts`

**Synchronisation** : Valider types après changements backend

---

## Références

**Tauri IPC** : https://v2.tauri.app/develop/calling-rust/
**Tauri Events** : https://v2.tauri.app/develop/inter-process-communication/
**Error Handling** : Voir ARCHITECTURE_DECISIONS.md (anyhow + thiserror)
