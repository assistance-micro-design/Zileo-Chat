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

Liste agents disponibles (permanents + temporaires actifs).

**Frontend**
```typescript
const agents = await invoke<Agent[]>('list_agents');
```

**Backend Signature**
```rust
async fn list_agents() -> Result<Vec<Agent>, String>
```

**Agent Type**
```typescript
type Agent = {
  id: string;
  name: string;
  lifecycle: 'permanent' | 'temporary';
  capabilities: string[];
  tools: string[];
  mcp_servers: string[];
  status: 'available' | 'busy';
};
```

---

### get_agent_config

Récupère configuration agent.

**Frontend**
```typescript
const config = await invoke<AgentConfig>('get_agent_config', {
  agentId: string
});
```

**Backend Signature**
```rust
async fn get_agent_config(
    agent_id: String
) -> Result<AgentConfig, String>
```

**AgentConfig** : Configuration TOML parsée

---

### update_agent_config

Met à jour configuration agent.

**Frontend**
```typescript
await invoke('update_agent_config', {
  agentId: string,
  config: AgentConfig
});
```

**Backend Signature**
```rust
async fn update_agent_config(
    agent_id: String,
    config: AgentConfig
) -> Result<(), String>
```

**Errors** : Invalid config, agent not found

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

## Providers

### list_providers

Liste providers LLM configurés.

**Frontend**
```typescript
const providers = await invoke<Provider[]>('list_providers');
```

**Provider Type**
```typescript
type Provider = {
  id: string;
  name: 'Mistral' | 'Ollama' | 'Claude' | 'GPT-4' | 'Gemini'; // Phase 1: Mistral + Ollama
  status: 'configured' | 'unconfigured' | 'error';
  models: string[];
};
```

---

### test_provider_connection

Test connexion provider.

**Frontend**
```typescript
const result = await invoke<ConnectionTest>('test_provider_connection', {
  providerId: string
});
```

**ConnectionTest Type**
```typescript
type ConnectionTest = {
  success: boolean;
  latency_ms?: number;
  error?: string;
};
```

---

### update_provider_config

Met à jour configuration provider.

**Frontend**
```typescript
await invoke('update_provider_config', {
  providerId: string,
  config: ProviderConfig
});
```

**ProviderConfig**
```typescript
type ProviderConfig = {
  api_key?: string;
  endpoint?: string;
  model_default?: string;
  rate_limit?: number;
  timeout?: number;
};
```

**Security** : API keys encryptées avant storage

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
