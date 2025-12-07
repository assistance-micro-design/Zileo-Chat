# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Zileo-Chat-3** is a desktop multi-agent application with a conversational interface, built using a modern stack:
- **Frontend**: SvelteKit 2.49.0 + Svelte 5.43.14 + Vite 5.4.0
- **Backend**: Rust 1.91.1 + Tauri 2.9.4
- **Database**: SurrealDB 2.4.0 (embedded RocksDB for desktop)
- **LLM Framework**: Rig.rs 0.24.0 (multi-provider abstraction)
- **Protocol**: MCP 2025-06-18 (official Anthropic SDK)
- **Additional**: async-trait 0.1, futures 0.3 (multi-agent async patterns)

**Current Status**: Phase 5 Backend Features complete. Functional Agent System operational. Phase 6 Integration pending.

## Implementation Progress

| Phase | Status | Description |
|-------|--------|-------------|
| Phase 0 | Complete | Base implementation (agents, LLM, DB, security, 19 Tauri commands) |
| Phase 1 | Complete | Design System Foundation (theme, 12 UI components) |
| Phase 2 | Complete | Layout Components (AppContainer, Sidebar, FloatingMenu, NavItem) |
| Phase 3 | Complete | Chat & Workflow Components (MessageBubble, ChatInput, WorkflowItem) |
| Phase 4 | Complete | Pages Refactoring (agent page, settings page with new components) |
| **Phase 5** | **Complete** | Backend Features (validation, memory, streaming, agent CRUD - 99 Tauri commands) |
| Phase 6 | Pending | Integration & Polish (E2E tests, accessibility audit) |

### Functional Agent System (Complete)

**Phases Implemented**:
1. **Phase 1**: Backend Agent Persistence (SurrealDB table, CRUD commands)
2. **Phase 2**: Frontend Store & Types (agentStore, AgentSummary, AgentConfigCreate)
3. **Phase 3**: Agent Settings UI (AgentSettings, AgentList, AgentForm components)
4. **Phase 4**: Agent Selector Integration (store-based loading, empty state handling)
5. **Phase 5**: Tool Execution Integration (XML tool calls, ToolFactory, MCPManager loop)

**Key Features**:
- Zero agents at startup (user creates all agents via Settings)
- Full CRUD via UI (create, read, update, delete)
- Agents persist in SurrealDB (table `agent`)
- Tool execution loop with MemoryTool, TodoTool, UserQuestionTool
- Human-in-the-loop via UserQuestionTool (interactive prompts)
- MCP server integration per agent

### Phase 1 Deliverables (Complete)

**Theme System** (`src/lib/stores/theme.ts`):
- Light/dark mode toggle with localStorage persistence
- System preference detection on init

**UI Components** (`src/lib/components/ui/`):
| Component | Description |
|-----------|-------------|
| `Button` | 4 variants (primary, secondary, ghost, danger), 4 sizes |
| `Badge` | Semantic variants (primary, success, warning, error) |
| `StatusIndicator` | Animated status dots (idle, running, completed, error) |
| `Spinner` | Loading spinner with configurable size |
| `ProgressBar` | Progress bar with optional percentage |
| `Card` | Container with header/body/footer snippets |
| `Modal` | Accessible dialog (Escape key, backdrop click) |
| `Input` | Text input with label and help text |
| `Select` | Dropdown with typed options |
| `Textarea` | Multi-line input |
| `Skeleton` | Loading placeholder with text, circular, rectangular variants |
| `LanguageSelector` | i18n language picker with flag display |

**Usage**:
```typescript
import { Button, Card, Modal } from '$lib/components/ui';
import { theme } from '$lib/stores/theme';
```

## Essential Commands

### Development

```bash
# Start dev mode (frontend + backend with HMR)
npm run tauri dev

# Frontend only (UI development)
npm run dev

# Backend only (Rust command testing)
cd src-tauri && cargo run
```

### Quality & Validation

```bash
# Frontend validation
npm run lint              # ESLint
npm run check             # svelte-check + TypeScript strict mode
npm run test              # Vitest unit tests
npm run build             # Production build test

# Backend validation
cd src-tauri
cargo fmt --check         # Format verification
cargo clippy -- -D warnings  # Linting with warnings as errors
cargo test                # Unit tests
cargo test --lib          # Library tests only
cargo build --release     # Release build
```

### Testing

```bash
# Frontend tests
npm run test              # Vitest
npm run test:e2e          # Playwright E2E tests

# Backend tests
cd src-tauri
cargo test                # All tests
cargo test -- --nocapture # With debug output
```

## Architecture Overview

### Multi-Agent System Hierarchy

```
Agent Principal (Orchestrator)
├─ Agents Spécialisés (permanent) - DB, API, Analytics, UI
└─ Agents Temporaires (lifecycle limité)
```

**Communication Protocol**: Markdown reports (standardized, human-readable, machine-parsable)

### Project Structure

```
zileo-chat-3/
├─ src/                     # Frontend (SvelteKit)
│  ├─ routes/               # File-based routing
│  │  ├─ settings/          # Settings page
│  │  └─ agent/             # Agent interaction page
│  ├─ lib/
│  │  ├─ components/        # Reusable Svelte components
│  │  │  └─ ui/             # Atomic UI components (Button, Card, Modal, etc.)
│  │  └─ stores/            # Svelte state management (theme, workflows, agents)
│  ├─ styles/               # Global CSS (variables, reset, utilities)
│  └─ types/                # TypeScript interfaces (alias: $types)
│
├─ src-tauri/               # Backend (Rust)
│  ├─ src/
│  │  ├─ main.rs            # Entry point
│  │  ├─ commands/          # Tauri IPC commands
│  │  ├─ agents/            # Multi-agent system
│  │  │  ├─ core/           # Orchestrator, registry, agent trait
│  │  │  ├─ llm_agent.rs    # LLM-backed agent implementation
│  │  │  └─ simple_agent.rs # Basic agent implementation
│  │  ├─ llm/               # Rig.rs integration, providers
│  │  ├─ mcp/               # MCP client/server
│  │  ├─ tools/             # Custom MCP tools
│  │  ├─ db/                # SurrealDB client
│  │  └─ models/            # Rust structs (sync with TS types)
│  ├─ Cargo.toml
│  └─ tauri.conf.json       # Tauri configuration
│
└─ docs/                    # Comprehensive documentation
```

### IPC Communication Pattern

Frontend → Backend communication uses Tauri's `invoke()`:

```typescript
// Frontend (TypeScript)
const result = await invoke<WorkflowResult>('execute_workflow', {
  workflowId: string,
  message: string,
  agentId: string
});
```

```rust
// Backend (Rust)
#[tauri::command]
async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String
) -> Result<WorkflowResult, String> {
    // Implementation
}
```

**Critical**: All Tauri commands must be registered in `src-tauri/src/main.rs` using `tauri::generate_handler![]`.

### Tauri Parameter Naming Convention

**IMPORTANT**: Tauri automatically converts parameter names between Rust (`snake_case`) and JavaScript (`camelCase`).

| Rust Parameter | JavaScript Parameter |
|----------------|---------------------|
| `workflow_id`  | `workflowId`        |
| `agent_id`     | `agentId`           |
| `api_key`      | `apiKey`            |
| `default_model_id` | `defaultModelId` |
| `base_url`     | `baseUrl`           |
| `server_name`  | `serverName`        |
| `type_filter`  | `typeFilter`        |
| `memory_type`  | `memoryType`        |

**Rules**:
- Rust commands use `snake_case` for parameters
- TypeScript `invoke()` calls use `camelCase` for parameters
- Tauri handles the conversion automatically
- Single-word parameters (e.g., `id`, `name`, `provider`) remain unchanged

```typescript
// CORRECT - camelCase in TypeScript
await invoke('update_provider_settings', {
  provider: 'mistral',
  defaultModelId: 'mistral-small-latest',  // NOT default_model_id
  baseUrl: null                             // NOT base_url
});

// INCORRECT - snake_case will NOT work
await invoke('update_provider_settings', {
  provider: 'mistral',
  default_model_id: 'mistral-small-latest', // WRONG!
  base_url: null                             // WRONG!
});
```

### Phase 5 Commands (Validation, Memory, Streaming)

**Validation** (`src-tauri/src/commands/validation.rs`):
```typescript
// Human-in-the-loop validation for critical operations
await invoke('create_validation_request', { workflowId, validationType, operation, details, riskLevel });
await invoke<ValidationRequest[]>('list_pending_validations');
await invoke<ValidationRequest[]>('list_workflow_validations', { workflowId });
await invoke('approve_validation', { validationId });
await invoke('reject_validation', { validationId, reason });
await invoke('delete_validation', { validationId });
```

**Memory** (`src-tauri/src/commands/memory.rs`):
```typescript
// RAG and context persistence (stub without vector embeddings)
await invoke<string>('add_memory', { memoryType, content, metadata });
await invoke<Memory[]>('list_memories', { typeFilter? });
await invoke<Memory>('get_memory', { memoryId });
await invoke('delete_memory', { memoryId });
await invoke<MemorySearchResult[]>('search_memories', { query, limit?, typeFilter? });
await invoke<number>('clear_memories_by_type', { memoryType });
```

**Streaming** (`src-tauri/src/commands/streaming.rs`):
```typescript
// Real-time workflow execution with Tauri events
import { listen } from '@tauri-apps/api/event';
import type { StreamChunk, WorkflowComplete } from '$types/streaming';

// Listen to streaming events
const unlistenChunk = await listen<StreamChunk>('workflow_stream', (event) => {
  // Handle token, tool_start, tool_end, reasoning, error chunks
});
const unlistenComplete = await listen<WorkflowComplete>('workflow_complete', (event) => {
  // Handle completion (status: completed | error)
});

// Start streaming execution
await invoke<WorkflowResult>('execute_workflow_streaming', { workflowId, message, agentId });
await invoke('cancel_workflow_streaming', { workflowId }); // Stub
```

## Type Synchronization

TypeScript and Rust types **must be kept in sync** for IPC communication.

### TypeScript Import Pattern

**IMPORTANT**: Always use the `$types` alias for imports. Never use `$lib/types`.

```typescript
// CORRECT - use $types alias
import type { Workflow, WorkflowResult } from '$types/workflow';
import type { AgentConfig, Lifecycle } from '$types/agent';
import type { LLMConfigResponse, ProviderStatus } from '$types/llm';

// INCORRECT - do not use
import type { Workflow } from '$lib/types/workflow';  // NO!
import type { Workflow } from '../types/workflow';     // NO!
```

The `$types` alias is configured in `svelte.config.js` and points to `src/types/`.

## Internationalization (i18n)

The application supports multiple languages (English and French) with a simple JSON-based translation system.

### Translation Files

Translation files are located in `src/messages/`:
- `src/messages/en.json` - English translations
- `src/messages/fr.json` - French translations (no anglicisms)

**File Structure**:
```json
{
  "$schema": "https://inlang.com/schema/inlang-message-format",
  "common_save": "Save",
  "common_cancel": "Cancel",
  "settings_title": "Settings",
  "agent_loading": "Loading agents..."
}
```

**Key Naming Convention**:
- Use `snake_case` for keys
- Prefix with section name: `common_`, `settings_`, `agent_`, `mcp_`, `theme_`, etc.
- Be descriptive: `agent_no_agents_description` not `agent_desc`

### Using Translations in Components

**Import Pattern**:
```typescript
import { i18n } from '$lib/i18n';
```

**In Templates** (reactive):
```svelte
<script lang="ts">
  import { i18n } from '$lib/i18n';
</script>

<h1>{$i18n('settings_title')}</h1>
<p>{$i18n('agent_loading_description')}</p>
<button>{$i18n('common_save')}</button>
```

**In Script** (non-reactive):
```typescript
import { t } from '$lib/i18n';

// For one-time use (not reactive to locale changes)
const message = t('common_error');
```

### Locale Store

The locale is managed via `$lib/stores/locale.ts`:

```typescript
import { localeStore, locale, localeInfo } from '$lib/stores/locale';

// Initialize on app start (in +layout.svelte)
localeStore.init();

// Change locale
localeStore.setLocale('fr');

// Access current locale (reactive)
$locale       // 'en' | 'fr'
$localeInfo   // { id: 'fr', nativeName: 'Francais', flag: 'FR', countryCode: 'FR' }
```

### Adding New Translations

1. Add keys to both `src/messages/en.json` and `src/messages/fr.json`
2. Use the `$i18n()` store in templates
3. Follow naming conventions: `section_action_detail`

**Example - Adding a new feature**:
```json
// en.json
{
  "feature_title": "New Feature",
  "feature_description": "This is a new feature.",
  "feature_button_enable": "Enable Feature"
}

// fr.json
{
  "feature_title": "Nouvelle fonctionnalite",
  "feature_description": "Ceci est une nouvelle fonctionnalite.",
  "feature_button_enable": "Activer la fonctionnalite"
}
```

### Configuration

The `$messages` alias is configured in:
- `svelte.config.js` (SvelteKit alias)
- `tsconfig.json` (TypeScript paths)

```javascript
// svelte.config.js
alias: {
  $messages: './src/messages',
  '$messages/*': './src/messages/*'
}
```

### French Translation Guidelines

When adding French translations, avoid anglicisms:
- "Settings" → "Parametres" (not "Settings")
- "Provider" → "Fournisseur" (not "Provider")
- "Workflow" → "Flux de travail" (not "Workflow")
- "Loading" → "Chargement" (not "Loading")
- "Server" → "Serveur" (not "Serveur")
- "Theme" → "Theme" (acceptable, same in French)

### Type Definition Examples

**TypeScript** (`src/types/`):
```typescript
export interface FeatureData {
  id: string;
  name: string;
  metadata: Record<string, unknown>;
}
```

**Rust** (`src-tauri/src/models/`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureData {
    pub id: String,
    pub name: String,
    pub metadata: serde_json::Value,
}
```

## Multi-Agent Configuration

**Agents are created dynamically via Settings UI** (no hardcoded agents).

### Agent CRUD Commands

```typescript
// List all agents (returns lightweight summaries)
const agents = await invoke<AgentSummary[]>('list_agents');

// Get full agent configuration
const config = await invoke<AgentConfig>('get_agent_config', { agentId });

// Create new agent
const agentId = await invoke<string>('create_agent', {
  config: {
    name: 'My Agent',
    lifecycle: 'permanent',
    llm: { provider: 'Mistral', model: 'mistral-large-latest', temperature: 0.7, max_tokens: 4096 },
    tools: ['MemoryTool', 'TodoTool', 'UserQuestionTool'],
    mcp_servers: ['serena'],
    system_prompt: 'You are a helpful assistant...'
  }
});

// Update agent (partial update)
const updated = await invoke<AgentConfig>('update_agent', { agentId, config: { name: 'New Name' } });

// Delete agent
await invoke('delete_agent', { agentId });
```

### Frontend Store Pattern

```typescript
import { agentStore, agents, isLoading, hasAgents } from '$lib/stores/agents';

// Load agents from backend
await agentStore.loadAgents();

// Access reactive derived stores
$agents       // AgentSummary[]
$isLoading    // boolean
$hasAgents    // boolean
```

## Tool Development Patterns

### New Utility Modules

The tools system has been refactored to reduce code duplication. Use these utilities when developing new tools:

#### `tools/utils.rs` - Database and Validation Utilities

```rust
use crate::tools::utils::{
    ensure_record_exists, delete_with_check, db_error,
    validate_not_empty, validate_length, validate_range, validate_enum_value,
    QueryBuilder
};

// Validate inputs
validate_not_empty(name, "name")?;
validate_length(content, 50000, "content")?;
validate_range(priority, 1, 5, "priority")?;
validate_enum_value(&status, &["pending", "done"], "status")?;

// Database operations
ensure_record_exists(&db, "memory", id, "Memory").await?;
delete_with_check(&db, "task", id, "Task").await?;

// Query building
let query = QueryBuilder::new("memory")
    .select(&["content", "memory_type"])
    .where_eq("memory_type", "knowledge")
    .order_by("created_at", true)
    .limit(10)
    .build();
```

#### `tools/constants.rs` - Centralized Constants

```rust
use crate::tools::constants::{memory, todo, user_question, sub_agent, commands};

// Memory tool constants
let max = memory::MAX_CONTENT_LENGTH;  // 50_000
let types = memory::VALID_TYPES;       // &["user_pref", "context", "knowledge", "decision"]

// Todo tool constants
let max_name = todo::MAX_NAME_LENGTH;  // 128
let statuses = todo::VALID_STATUSES;   // &["pending", "in_progress", "completed", "blocked"]

// User question tool constants
let max_question = user_question::MAX_QUESTION_LENGTH;  // 2000
let max_options = user_question::MAX_OPTIONS;           // 20
let question_types = user_question::VALID_TYPES;        // &["checkbox", "text", "mixed"]

// Sub-agent limit
let max_agents = sub_agent::MAX_SUB_AGENTS;  // 3

// Command validation constants (OPT-2)
use crate::tools::constants::commands as cmd_const;
let max_agent_name = cmd_const::MAX_AGENT_NAME_LEN;         // 64
let max_system_prompt = cmd_const::MAX_SYSTEM_PROMPT_LEN;   // 10_000
let valid_providers = cmd_const::VALID_PROVIDERS;           // &["Mistral", "Ollama", "Demo"]
let max_mcp_name = cmd_const::MAX_MCP_SERVER_NAME_LEN;      // 64
let max_tool_name = cmd_const::MAX_TOOL_NAME_LEN;           // 128
let max_message = cmd_const::MAX_MESSAGE_CONTENT_LEN;       // 100_000
```

#### `tools/response.rs` - JSON Response Builder

```rust
use crate::tools::response::ResponseBuilder;

// Standard success response
ResponseBuilder::ok("memory_id", id, "Memory created successfully")

// List response
ResponseBuilder::list(&items, items.len())

// Custom response with builder
ResponseBuilder::new()
    .success(true)
    .id("task_id", task_id)
    .message("Task updated")
    .metrics(duration_ms, tokens_in, tokens_out)
    .build()
```

#### `tools/registry.rs` - Tool Discovery

```rust
use crate::tools::registry::TOOL_REGISTRY;

// Check if tool exists
if TOOL_REGISTRY.has_tool("MemoryTool") { ... }

// Get tool metadata
let metadata = TOOL_REGISTRY.get("SpawnAgentTool");

// List tools by category
let basic = TOOL_REGISTRY.basic_tools();          // ["MemoryTool", "TodoTool", "CalculatorTool"]
let interaction = TOOL_REGISTRY.interaction_tools(); // ["UserQuestionTool"]
let sub_agent = TOOL_REGISTRY.sub_agent_tools();  // ["SpawnAgentTool", ...]

// Validate with error message
TOOL_REGISTRY.validate(tool_name)?;
```

#### `tools/sub_agent_executor.rs` - Sub-Agent Operations

```rust
use crate::tools::sub_agent_executor::{SubAgentExecutor, ExecutionResult};

// Permission and limit checks (static methods)
SubAgentExecutor::check_primary_permission(is_primary, "spawn")?;
SubAgentExecutor::check_limit(current_count, "spawn")?;

// Create executor instance
let executor = SubAgentExecutor::new(
    db, orchestrator, mcp_manager, app_handle,
    workflow_id, parent_agent_id
);

// Execution lifecycle
let exec_id = executor.create_execution_record(agent_id, name, prompt).await?;
executor.emit_start_event(agent_id, name, prompt);
let result = executor.execute_with_metrics(agent_id, task).await;
executor.update_execution_record(&exec_id, &result).await;
executor.emit_complete_event(agent_id, name, &result);
```

### MCP Server Identification

**IMPORTANT**: MCP servers are identified by **NAME** (not ID) throughout the system.

```rust
// MCPManager is keyed by server NAME
let tools = mcp_manager.list_server_tools("Serena").await?;
mcp_manager.call_tool("Serena", "tool_name", args).await?;

// Agent configuration uses names
AgentConfig {
    mcp_servers: vec!["Serena".to_string(), "Context7".to_string()],
    // ...
}

// Validate server names
let invalid = mcp_manager.validate_server_names(&names).await;
```

**System prompt format:**
```
### Available MCP Servers
- **Serena** [DIRECT] - Code analysis - 15 tools
- **Context7** [DELEGATE] - Documentation - 8 tools

To assign: {"mcp_servers": ["Serena"]}
```

## Database Schema (SurrealDB)

**19 tables** with graph relations. Key entities:

**Core**:
- **workflow**: Manages agent workflows (relations: agent, messages, tasks, validations)
- **agent**: User-created agent configurations (id, name, lifecycle, llm, tools, mcp_servers, system_prompt)
- **agent_state**: Persistent agent runtime state
- **message**: Conversation messages with role (user/assistant/system)
- **memory**: Vector embeddings for agent memory (user_pref, context, knowledge, decision types)

**Tracking**:
- **task**: Todo items with status tracking
- **validation_request**: Human-in-the-loop validation tracking
- **user_question**: Interactive questions from agents to users (pending/answered/skipped)
- **tool_execution**: Tool call persistence with metrics
- **thinking_step**: Agent reasoning chain-of-thought
- **sub_agent_execution**: Sub-agent spawn history

**Configuration**:
- **llm_model**: LLM model registry (builtin + custom)
- **provider_settings**: Provider configuration (mistral, ollama)
- **mcp_server**: MCP server configurations
- **mcp_call_log**: MCP tool audit log
- **prompt**: Prompt library
- **settings**: Key-value config storage

See `docs/DATABASE_SCHEMA.md` for full schema details.

### Agent Table Schema

```surql
DEFINE TABLE agent SCHEMAFULL;
DEFINE FIELD id ON agent TYPE string;
DEFINE FIELD name ON agent TYPE string;
DEFINE FIELD lifecycle ON agent TYPE string;  -- 'permanent' | 'temporary'
DEFINE FIELD llm ON agent TYPE object;
DEFINE FIELD llm.provider ON agent TYPE string;
DEFINE FIELD llm.model ON agent TYPE string;
DEFINE FIELD llm.temperature ON agent TYPE float;
DEFINE FIELD llm.max_tokens ON agent TYPE int;
DEFINE FIELD tools ON agent TYPE array<string>;
DEFINE FIELD mcp_servers ON agent TYPE array<string>;
DEFINE FIELD system_prompt ON agent TYPE string;
DEFINE FIELD created_at ON agent TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON agent TYPE datetime DEFAULT time::now();
```

### SurrealDB SDK 2.x Patterns

The SurrealDB Rust SDK 2.x has serialization issues with internal enum types. Use these patterns:

**Record Creation** - Use raw SurrealQL queries instead of `.create().content()`:
```rust
// WRONG - SDK has internal enum serialization issues
let _: Option<T> = db.create((table, id)).content(data).await?;

// CORRECT - Use raw query with bind()
let json_data = serde_json::to_value(&data)?;
let query = format!("CREATE {}:`{}` CONTENT $data", table, id);
db.query(&query).bind(("data", json_data)).await?;
```

**Record Queries** - Use `meta::id(id)` to extract clean UUIDs:
```rust
// WRONG - Returns ID with angle brackets: ⟨uuid⟩
"SELECT * FROM workflow"
"SELECT string::concat('', id) AS id FROM workflow"

// CORRECT - Returns clean UUID string
"SELECT meta::id(id) AS id, name, status FROM workflow"
```

**Custom Deserializers** - For enum fields stored as strings:
```rust
// In models/serde_utils.rs - handle SurrealDB's internal enum format
pub fn deserialize_workflow_status<'de, D>(deserializer: D) -> Result<WorkflowStatus, D::Error>

// Apply on struct field
#[serde(deserialize_with = "deserialize_workflow_status")]
pub status: WorkflowStatus,
```

**Query Results** - Extract as JSON first for custom deserializer support:
```rust
// Use query_json() then deserialize with serde_json
let json_results: Vec<serde_json::Value> = db.query_json(query).await?;
let data: Vec<T> = json_results
    .into_iter()
    .map(serde_json::from_value)
    .collect::<Result<Vec<T>, _>>()?;
```

**Write Operations (UPSERT/UPDATE/DELETE)** - Use `execute()` to avoid result deserialization:
```rust
// WRONG - query() tries to deserialize the returned record (Thing type issues)
let _: Vec<serde_json::Value> = db.query("UPSERT settings:`key` CONTENT {...}").await?;

// CORRECT - execute() runs query without deserializing result
db.execute("UPSERT settings:`key` CONTENT {...}").await?;
db.execute("UPDATE memory:`uuid` SET content = \"value\"").await?;
db.execute("DELETE memory:`uuid`").await?;
```

**String Values** - Use JSON encoding for proper escaping (apostrophes, quotes, etc.):
```rust
// WRONG - Single quote escaping doesn't work in SurrealDB
format!("content = '{}'", text.replace('\'', "''"))  // l'eau -> l''eau (parse error)

// CORRECT - JSON string encoding handles all special characters
let json_str = serde_json::to_string(&text)?;  // l'eau -> "l'eau"
format!("content = {}", json_str)
```

**Delete Records** - Use `execute()` with DELETE query, not `db.delete()`:
```rust
// WRONG - db.delete() has issues with table:id format
db.delete("memory:uuid").await?;  // Error: table name contains colon

// CORRECT - Use raw DELETE query with backtick-escaped ID
db.execute(&format!("DELETE memory:`{}`", uuid)).await?;
```

**ORDER BY with Explicit Fields** - Include ORDER BY fields in SELECT:
```rust
// WRONG - ORDER BY field must be in SELECT when using explicit field selection
"SELECT meta::id(id) AS id, content FROM memory ORDER BY created_at DESC"

// CORRECT - Either include the field or remove ORDER BY
"SELECT meta::id(id) AS id, content, created_at FROM memory ORDER BY created_at DESC"
// OR if order doesn't matter:
"SELECT meta::id(id) AS id, content FROM memory"
```

**Settings/Config Queries** - Use `SELECT field` instead of `SELECT *` to avoid Thing enum:
```rust
// WRONG - SELECT * returns id field which is Thing type (enum), causes serialization error
let query = "SELECT * FROM settings:`settings:embedding_config`";
let results: Vec<Value> = db.query_json(query).await?;  // Error: invalid type: enum

// CORRECT - Select only the fields you need, avoiding the id field
let query = "SELECT config FROM settings:`settings:embedding_config`";
let results: Vec<Value> = db.query_json(query).await?;  // Works!
```

**Direct Record Access** - Use backtick-escaped ID instead of WHERE clause:
```rust
// WRONG - WHERE id comparison may not match correctly
let query = "SELECT config FROM settings WHERE id = 'settings:embedding_config'";

// CORRECT - Direct record access with backtick-escaped ID
let query = "SELECT config FROM settings:`settings:embedding_config`";
```

**Dynamic Keys in SCHEMAFULL** - Store as JSON string for fields with user-defined keys:
```rust
// PROBLEM - SCHEMAFULL tables filter nested object keys not explicitly defined in schema
// If you define `DEFINE FIELD env ON table TYPE object`, SurrealDB will silently
// drop any nested keys that aren't pre-defined (e.g., env.API_KEY, env.SECRET)

// WRONG - Dynamic keys are lost after save/reload
DEFINE FIELD env ON mcp_server TYPE object;  // User adds {"API_KEY": "xxx"} -> returns {}

// CORRECT - Store as JSON string, parse on read
DEFINE FIELD env ON mcp_server TYPE string DEFAULT '{}';

// Rust: Serialize HashMap to JSON string on save
let env_str = serde_json::to_string(&config.env)?;  // {"API_KEY":"xxx"}

// Rust: Parse JSON string back to HashMap on load
let env: HashMap<String, String> = serde_json::from_str(env_str)?;
```
This pattern applies to any field with dynamic/user-defined keys: `env`, `metadata`, `custom_fields`, etc.

## Security Considerations

**Production-ready from v1**:
- API keys stored via Tauri secure storage (OS keychain) + AES-256 encryption
- Input validation on both frontend and backend
- Tauri allowlist for IPC commands (explicit permission model)
- MCP servers run in isolated processes (Docker containers for external servers)
- CSP configured in `tauri.conf.json`

## Code Quality Standards

### Strict Prohibitions

**Never include in code**:
- ❌ `any` type in TypeScript - strict typing required
- ❌ Mock data or placeholders in production code
- ❌ `TODO` comments for core functionality
- ❌ Incomplete implementations (`throw new Error("Not implemented")`)
- ❌ Emojis in code or comments

### Required Practices

- ✅ **Type Safety**: Strict TypeScript + Rust types synchronized
- ✅ **Documentation**: JSDoc/TSDoc for TypeScript, Rustdoc for Rust
- ✅ **Error Handling**: `Result<T, E>` in Rust, proper try/catch in TypeScript
- ✅ **Testing**: Critical paths coverage (~70% backend)
- ✅ **Async Patterns**: Tokio for Rust, async/await for TypeScript

## Custom Slash Commands

This project includes specialized workflows:

- `/Plan_Zileo <description>`: Create detailed technical planning spec without implementation
- `/Build_zileo <description>`: Full implementation workflow with quality validation

Both commands follow a rigorous process including parallel discovery, architectural analysis, and comprehensive validation.

## LLM Provider Configuration

**Phase 1 Providers**:
- **Mistral**: Cloud API (mistral-large, mistral-medium)
- **Ollama**: Local models (llama3, mistral, codellama)

**Future**: Claude, GPT-4, Gemini

Provider selection is user-controlled via Settings UI. The application uses Rig.rs for multi-provider abstraction.

## MCP Server Configuration

MCP servers are user-configured (not bundled) via Settings UI:

```json
{
  "mcpServers": {
    "server_name": {
      "command": "docker|npx|uvx",
      "args": ["array", "of", "arguments"],
      "env": { "VAR": "value" }
    }
  }
}
```

Supported deployment methods:
- **Docker**: Local containers (e.g., Serena for code analysis)
- **NPX**: Node.js-based servers (e.g., Context7 for docs)
- **UVX**: Python servers with isolated environments
- **SaaS**: Remote managed services

## Workflow Execution Pattern

1. User sends message in Agent page
2. Frontend invokes Tauri command with workflow_id and agent_id
3. Backend orchestrator:
   - Loads agent configuration
   - Initializes LLM provider (Rig.rs)
   - Executes agent with available tools and MCP servers
   - Streams response back to frontend
4. Human-in-the-loop validation if required (critical operations)
5. Agent generates markdown report with metrics
6. State persisted to SurrealDB

## Key Documentation Files

Essential reading for context:

- `docs/TECH_STACK.md`: Exact versions and requirements (Node 20.19+, Rust 1.91.1+)
- `docs/ARCHITECTURE_DECISIONS.md`: All architectural decisions with justifications
- `docs/MULTI_AGENT_ARCHITECTURE.md`: Agent hierarchy, communication, factory patterns
- `docs/API_REFERENCE.md`: Tauri command signatures and types
- `docs/AGENT_TOOLS_DOCUMENTATION.md`: Tool system documentation (7 tools)
- `docs/GETTING_STARTED.md`: Development setup and first workflow
- `docs/TESTING_STRATEGY.md`: Testing approach and coverage targets
- `docs/MCP_CONFIGURATION_GUIDE.md`: MCP server setup and configuration
- `docs/DESIGN_SYSTEM.md`: Complete UI design specifications (colors, typography, components)
- `docs/specs/2025-11-25_spec-complete-implementation-plan.md`: Full implementation plan (6 phases)

## Development Workflow Best Practices

### For New Features

1. **Discovery**: Read relevant docs, explore existing patterns
2. **Types First**: Define TypeScript and Rust types (synchronized)
3. **Backend**: Implement Rust command with proper error handling
4. **Frontend**: Create Svelte component with strict props
5. **Validation**: Lint, typecheck, tests - zero errors required
6. **Documentation**: JSDoc/Rustdoc with clear descriptions

### Agent Development

Agents are created dynamically via Settings UI and stored in SurrealDB:

1. **UI Creation**: Settings → Agents → Create new agent
2. **Configuration**: Name, lifecycle, LLM settings, tools, MCP servers, system prompt
3. **Persistence**: Agent stored in SurrealDB `agent` table
4. **Runtime**: LLMAgent loads config from DB and executes with Orchestrator

### For Bug Fixes

1. Reproduce issue with tests
2. Root cause analysis (never skip)
3. Fix systematically
4. Verify with full validation suite
5. Update tests to prevent regression

## Version Requirements

**Minimum**:
- Node.js: 20.19+ or 22.12+ (Vite 7 requirement)
- Rust: 1.80.1+ (SurrealDB SDK requirement)
- npm/pnpm/yarn: Latest stable

**Check versions**:
```bash
node --version    # >= 20.19
rustc --version   # >= 1.91.1
cargo --version   # >= 1.91.1
```

## CI/CD

**On Push** (feature branches):
- Linting (clippy + eslint)
- Unit tests
- Build verification

**On PR** (to main):
- Integration tests
- Security audit (`cargo audit`)
- Coverage report

**On Merge** (main):
- Release builds (Linux, macOS, Windows planned)
- E2E tests
- Artifact packaging

## OS Targets

**Priority**:
1. **Linux** (Phase 1): AppImage + .deb
2. **macOS** (Phase 1.5): .dmg
3. **Windows** (Phase 2): .msi

Build outputs in `src-tauri/target/release/bundle/`
