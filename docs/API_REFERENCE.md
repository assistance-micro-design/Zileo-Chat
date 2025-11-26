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
