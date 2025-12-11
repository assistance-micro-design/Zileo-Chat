# CLAUDE.md

Guidance for Claude Code working with Zileo-Chat-3. See `docs/` for detailed documentation.

## Project Overview

**Stack**: SvelteKit 2.49 + Svelte 5.45 + Vite 7.2 | Rust 1.91 + Tauri 2.9 | SurrealDB 2.4 | Rig.rs 0.24 | MCP 2025-06-18

**Agent System**: Zero agents at startup, full CRUD via Settings UI, persisted in SurrealDB (`agent` table), tool execution loop with human-in-the-loop via UserQuestionTool.

```
zileo-chat-3/
├─ src/                     # Frontend (SvelteKit)
│  ├─ routes/               # File-based routing (settings/, agent/)
│  ├─ lib/components/ui/    # Atomic UI components
│  ├─ lib/stores/           # Svelte state (theme, agents, workflows)
│  └─ types/                # TypeScript interfaces ($types alias)
├─ src-tauri/src/           # Backend (Rust)
│  ├─ commands/             # Tauri IPC commands
│  ├─ agents/               # Multi-agent system (core/, llm_agent.rs)
│  ├─ tools/                # MCP tools (constants.rs, utils.rs, registry.rs)
│  ├─ mcp/                  # MCP client/server
│  └─ models/               # Rust structs (sync with TS types)
└─ docs/                    # Comprehensive documentation
```

## Quick Commands

| Task | Command |
|------|---------|
| Dev (full) | `npm run tauri dev` |
| Frontend only | `npm run dev` |
| Frontend lint | `npm run lint && npm run check` |
| Frontend test | `npm run test` |
| Backend lint | `cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings` |
| Backend test | `cd src-tauri && cargo test` |
| Build release | `npm run build && cd src-tauri && cargo build --release` |

## Critical Patterns

### Tauri IPC Naming Convention

**Tauri auto-converts** `snake_case` (Rust) ↔ `camelCase` (TypeScript).

| Rust | TypeScript |
|------|------------|
| `workflow_id` | `workflowId` |
| `agent_id` | `agentId` |
| `api_key` | `apiKey` |
| `default_model_id` | `defaultModelId` |

```typescript
// CORRECT
await invoke('update_provider_settings', { defaultModelId: 'model', baseUrl: null });

// WRONG - snake_case won't work in TS
await invoke('update_provider_settings', { default_model_id: 'model' }); // FAILS!
```

### TypeScript Import Pattern

**Always use `$types` alias** (configured in `svelte.config.js`):

```typescript
// CORRECT
import type { Workflow } from '$types/workflow';
import type { AgentConfig } from '$types/agent';

// WRONG
import type { Workflow } from '$lib/types/workflow';  // NO!
import type { Workflow } from '../types/workflow';     // NO!
```

### Nullability Convention (TS ↔ Rust)

| Rust | TypeScript | JSON |
|------|------------|------|
| `Option<T>` + `skip_serializing_if` | `field?: T` | Absent when None |
| `Option<T>` (no skip) | `field: T \| null` | Explicit `null` |
| `T` (required) | `field: T` | Always present |

```typescript
// Check Rust struct: if skip_serializing_if exists, use ?:
workflow_id?: string;          // Option<String> with skip_serializing_if
error_message: string | null;  // Option<String> without skip

// WRONG patterns
workflow_id?: string | null;   // Never mix ? and | null
```

### SurrealDB SDK 2.x Patterns

The SDK has serialization quirks. Follow these patterns strictly:

**Record Creation** - Use raw queries, not `.create().content()`:
```rust
// CORRECT
let json_data = serde_json::to_value(&data)?;
db.query(&format!("CREATE {}:`{}` CONTENT $data", table, id))
    .bind(("data", json_data)).await?;

// WRONG - SDK serialization issues
db.create((table, id)).content(data).await?;
```

**Clean UUIDs** - Use `meta::id(id)`:
```rust
// CORRECT - Returns clean UUID
"SELECT meta::id(id) AS id, name FROM agent"

// WRONG - Returns ⟨uuid⟩ with brackets
"SELECT * FROM agent"
```

**Write Operations** - Use `execute()` to avoid deserialization:
```rust
// CORRECT
db.execute("UPDATE memory:`uuid` SET content = $c").await?;
db.execute(&format!("DELETE memory:`{}`", uuid)).await?;

// WRONG - query() tries to deserialize Thing type
db.query("UPDATE memory:`uuid` SET ...").await?;
```

**String Escaping** - Use JSON encoding:
```rust
// CORRECT - Handles apostrophes, quotes, etc.
let json_str = serde_json::to_string(&text)?;
format!("content = {}", json_str)

// WRONG - Fails on "l'eau", quotes, etc.
format!("content = '{}'", text.replace('\'', "''"))
```

**Parameterized Queries** - Prevent SQL injection:
```rust
// CORRECT
db.query_with_params(
    "SELECT * FROM memory WHERE type = $type",
    vec![("type".to_string(), json!("knowledge"))]
).await?;

// Use format!() only for validated UUIDs and table names
```

**Dynamic Keys in SCHEMAFULL** - Store as JSON string:
```rust
// WRONG - Dynamic keys silently dropped
DEFINE FIELD env ON mcp_server TYPE object;  // {API_KEY: "x"} -> {}

// CORRECT - Store as JSON string, parse on read
DEFINE FIELD env ON mcp_server TYPE string DEFAULT '{}';
let env_str = serde_json::to_string(&config.env)?;
```

**ORDER BY** - Include field in SELECT:
```rust
// CORRECT
"SELECT meta::id(id) AS id, content, created_at FROM memory ORDER BY created_at DESC"

// WRONG - ORDER BY field not in SELECT
"SELECT meta::id(id) AS id, content FROM memory ORDER BY created_at DESC"
```

**Settings Queries** - Avoid `SELECT *` (Thing enum issues):
```rust
// CORRECT
"SELECT config FROM settings:`settings:embedding_config`"

// WRONG - id field causes serialization error
"SELECT * FROM settings:`settings:embedding_config`"
```

### Query Limits

Always use LIMIT to prevent memory explosion:
```rust
use crate::tools::constants::query_limits;
// DEFAULT_LIST_LIMIT: 1000, DEFAULT_MODELS_LIMIT: 100, DEFAULT_MESSAGES_LIMIT: 500
```

### i18n Pattern

Translation files: `src/messages/{en,fr}.json`

```svelte
<script lang="ts">
  import { i18n } from '$lib/i18n';
</script>

<h1>{$i18n('settings_title')}</h1>
<button>{$i18n('common_save')}</button>
```

Key naming: `section_action_detail` (e.g., `agent_no_agents_description`)

### Store Cleanup Pattern

Stores with Tauri listeners must implement cleanup:
```typescript
store.cleanup();  // Remove listeners, reset state
store.reset();    // cleanup() + initial state
```

### Utility Imports

```typescript
import { getErrorMessage } from '$lib/utils/error';
import { createModalController } from '$lib/utils/modal.svelte';
import { createAsyncHandler } from '$lib/utils/async';
```

## Code Standards

**Prohibited**:
- `any` type in TypeScript
- Mock data in production
- TODO comments for core functionality
- Incomplete implementations
- Emojis in code/comments

**Required**:
- Strict TypeScript + Rust types synchronized
- JSDoc/Rustdoc documentation
- `Result<T, E>` in Rust, try/catch in TypeScript
- Critical paths test coverage (~70% backend)

## MCP Server Identification

Servers identified by **NAME** (not ID):
```rust
mcp_manager.call_tool("Serena", "tool_name", args).await?;
AgentConfig { mcp_servers: vec!["Serena".to_string()], ... }
```

## Documentation Index

| Document | Content |
|----------|---------|
| `TECH_STACK.md` | Exact versions, requirements (Node 20.19+, Rust 1.91+) |
| `ARCHITECTURE_DECISIONS.md` | ADRs with justifications |
| `MULTI_AGENT_ARCHITECTURE.md` | Agent hierarchy, communication, factory patterns |
| `API_REFERENCE.md` | Tauri command signatures and types |
| `AGENT_TOOLS_DOCUMENTATION.md` | Tool system (7 tools), development patterns |
| `TOOLS_REFERENCE.md` | Tool utilities (constants.rs, utils.rs, registry.rs) |
| `DATABASE_SCHEMA.md` | Full SurrealDB schema |
| `MCP_CONFIGURATION_GUIDE.md` | MCP server setup (Docker, NPX, UVX) |
| `FRONTEND_SPECIFICATIONS.md` | Component specs, stores, routing |
| `WORKFLOW_ORCHESTRATION.md` | Execution flow, streaming, validation |
| `DESIGN_SYSTEM.md` | UI specs (colors, typography, components) |
| `TESTING_STRATEGY.md` | Testing approach, coverage targets |
| `GETTING_STARTED.md` | Development setup guide |
| `DEPLOYMENT_GUIDE.md` | Build and release process |
| `SUB_AGENT_GUIDE.md` | Sub-agent spawning and delegation |

## Security

- API keys: Tauri secure storage + AES-256
- Input validation: `Validator` struct (frontend + backend)
- SQL injection: Parameterized queries (`query_with_params()`)
- CSP: `default-src 'self'; script-src 'self'`
- MCP: Isolated processes, env injection prevention

## Version Requirements

- **Node.js**: 20.19+ or 22.12+ (Vite 7)
- **Rust**: 1.80.1+ (SurrealDB SDK)

```bash
node --version  # >= 20.19
rustc --version # >= 1.91.1
```
