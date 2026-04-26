# Tech Stack: Svelte + Rust + SurrealDB

> **Current project versions: 24 April 2026**
> Production versions used in the project (tested compatibility).

## Stack Overview

```
Frontend  : SvelteKit 2.55.0 | Svelte 5.55.1
Backend   : Rust 1.93.0 + Tauri 2
Database  : SurrealDB ~2.6 (kv-rocksdb, no protocol-http)
Desktop   : Tauri (cross-platform)
LLM       : Mistral, Ollama, OpenAI-compatible providers (OpenRouter, RouterLab, etc.)
```

## Technologies & Versions

### Frontend (package.json)

**Core Framework**:
- **svelte**: 5.55.1
- **@sveltejs/kit**: ^2.55.0
- **@sveltejs/adapter-static**: ^3.0.0
- **@sveltejs/vite-plugin-svelte**: ^6.2.4
- **vite**: ^7.3.2

**TypeScript**:
- **typescript**: ^5.9.3
- **svelte-check**: ^4.4.4

**Tauri Integration**:
- **@tauri-apps/api**: ^2.9.0
- **@tauri-apps/cli**: ^2.10.1
- **@tauri-apps/plugin-dialog**: ^2.7.0
- **@tauri-apps/plugin-opener**: ^2.5.3

**UI Components**:
- **@lucide/svelte**: ^0.563.1 (official Lucide icon library)

**Content Processing**:
- **dompurify**: ^3.4.1 (HTML sanitization)
- **marked**: ^17.0.5 (Markdown rendering)
- **zod**: ^4.3.6 (schema validation)

**Testing**:
- **vitest**: ^4.0.15 (unit tests)
- **@playwright/test**: ^1.58.0 (E2E tests)
- **jsdom**: ^27.4.0 (DOM testing)

**Linting**:
- **eslint**: ^9.0.0
- **eslint-plugin-svelte**: ^3.14.0
- **@eslint/js**: ^9.39.1
- **@typescript-eslint/eslint-plugin**: ^8.0.0
- **@typescript-eslint/parser**: ^8.54.0
- **typescript-eslint**: ^8.53.1
- **globals**: ^17.4.0

### Backend (Cargo.toml)

**Core Framework**:
- **Rust**: 1.93.0 (stable, edition 2021)
- **tauri**: 2 (framework)
- **tauri-build**: 2 (build dependency, version range)
- **tauri-plugin-opener**: 2 (version range)
- **tauri-plugin-dialog**: 2 (version range)

**LLM & Multi-Agent**:
- **rig-core**: 0.34.0 (LLM abstraction framework)
- **async-trait**: 0.1 (agent trait definitions)
- **futures-util**: 0.3.31 (stream utilities for SSE)
- Providers: Mistral (native), Ollama (native), OpenAI-compatible (custom providers)

**Database**:
- **surrealdb**: ~2.6 (features: kv-rocksdb, default-features: false)

**Serialization**:
- **serde**: 1.0.228 (features: derive)
- **serde_json**: 1.0.149

**Async Runtime**:
- **tokio**: 1.51.1 (features: rt, rt-multi-thread, macros, sync, time, fs, io-util, net, process)
- **tokio-util**: 0.7 (features: rt)

**Error Handling**:
- **anyhow**: 1.0
- **thiserror**: 2.0

**Logging**:
- **tracing**: 0.1
- **tracing-subscriber**: 0.3 (features: json, env-filter)

**Utilities**:
- **uuid**: 1.20 (features: v4, serde)
- **chrono**: 0.4.43 (features: serde)
- **regex**: 1.10
- **globset**: 0.4 (glob pattern matching for FileManagerTool)
- **base64**: 0.22 (base64 encoding/decoding)

**HTTP & Network**:
- **reqwest**: 0.12 (features: rustls-tls, json, stream)

**Security**:
- **keyring**: 3.6 (OS keychain: apple-native, windows-native, sync-secret-service)
- **aes-gcm**: 0.10 (AES-256 encryption)

**Dev Dependencies**:
- **tempfile**: 3.24

### Database

- **SurrealDB**: ~2.6 (embedded with kv-rocksdb, default-features disabled)
- No protocol-http or protocol-ws features (embedded-only)

## Architecture

```
+-------------------------------------+
|         SvelteKit (Frontend)        |
|  - Components (.svelte)             |
|  - Routes (file-based)              |
|  - Stores (state management)        |
+-----------------+-------------------+
                  | Tauri IPC
                  v
+-------------------------------------+
|       Rust Backend (Tauri)          |
|  - Commands (API layer)             |
|  - Multi-agent LLM orchestration    |
|  - SurrealDB client                 |
+-----------------+-------------------+
                  | surrealdb.rs
                  v
+-------------------------------------+
|           SurrealDB (embedded)      |
|  - Multi-model database             |
|  - RocksDB storage engine           |
+-------------------------------------+
```

## Key Integrations

### Tauri IPC (Frontend <-> Backend)
- Communication via `invoke()` (frontend) to `#[tauri::command]` (backend)
- Type-safe with TypeScript + Rust types (camelCase auto-converted to snake_case)
- Async/await on both sides

### Rust <-> SurrealDB
- Native embedded Rust client (surrealdb.rs)
- Type-safe queries with serde
- Parameterized queries for SQL injection prevention

### SvelteKit <-> Tauri
- Adapter-static for SPA mode (single `index.html` fallback)
- Asset protocol for local files

## LLM Providers

Three provider types with unified interface:

| Provider | Type | Features |
|----------|------|----------|
| Mistral | Native API | Thinking/reasoning, vision, tool calling, streaming |
| Ollama | Native API | Local models, thinking, vision, tool calling, streaming |
| Custom | OpenAI-compatible | OpenRouter, RouterLab, etc. via `/v1/chat/completions` |

**Resilience patterns**: rate limiting (1 req/s), exponential backoff retry (3 max, 1-30s), circuit breaker (3 failures, 60s cooldown), connection pooling (5 idle/host, 300s timeout).

## Security

**Features**:
- **CSP**: Strict Content Security Policy (frame-ancestors 'none', object-src 'none')
- **API Key Storage**: OS keychain via `keyring` crate + AES-256 encryption
- **API Key Validation**: Rejects newlines (HTTP header injection prevention)
- **MCP Env Validation**: Shell injection prevention (alphanumeric names, no metacharacters)
- **Tauri v2**: Capability-based permissions (no v1 allowlist)
- **SQL Injection Prevention**: Parameterized queries enforced
- **Memory Protection**: Query LIMIT enforcement on all list queries

**Query limits** (defined in `src-tauri/src/constants.rs`):

| Constant | Value | Scope |
|----------|-------|-------|
| DEFAULT_LIST_LIMIT | 1000 | agents, memories, tasks |
| DEFAULT_MODELS_LIMIT | 100 | LLM models |
| DEFAULT_MCP_LOGS_LIMIT | 500 | MCP call logs |
| DEFAULT_MESSAGES_LIMIT | 500 | message history |
| MAX_LIST_LIMIT | 10000 | maximum allowed |

## Testing

- **Backend**: 1000+ Rust tests (lib target)
- **Frontend**: 280+ Vitest unit tests
- **E2E**: Playwright (available, not counted in totals)
- **Total**: 1,400+ automated tests (run `cargo test --lib && npm run test` for the current count)

## Build & Release

**Build outputs**:
```
src-tauri/target/release/bundle/
  appimage/   (Linux)
  deb/        (Debian)
  dmg/        (macOS)
  msi/        (Windows)
```

**Release profile**: LTO enabled, symbols stripped, opt-level 3, panic=abort, codegen-units=1.

## Version Requirements

| Requirement | Minimum Version | Notes |
|-------------|-----------------|-------|
| Node.js | 20.19+ or 22.12+ | Node 18 not supported by Vite 7 |
| Rust | 1.93.0+ | Required by SurrealDB SDK |
| npm/pnpm/yarn | Latest stable | |

## Key Migration Notes

**Svelte 5** (from Svelte 4):
- `{#snippet}` + `{@render}` replaces `<slot>`
- `{@attach}` replaces `use:action`
- `$props()` replaces `export let`
- `onclick` replaces `on:click`

**Zod 4** (from Zod 3):
- `{ error: "..." }` replaces `{ message: "..." }`
- `z.email()` / `z.uuid()` / `z.url()` replaces `z.string().email()` etc.
- `z.record(keySchema, valueSchema)` requires 2 args
- `z.treeifyError()` replaces `.format()` / `.flatten()`

**Vitest 4** (from Vitest 2):
- `maxWorkers` replaces `maxThreads` / `maxForks`
- `projects` replaces `workspace`

## Resources

- **Tauri**: https://tauri.app | https://v2.tauri.app
- **SvelteKit**: https://kit.svelte.dev
- **Svelte**: https://svelte.dev
- **SurrealDB**: https://surrealdb.com
- **surrealdb.rs**: https://docs.rs/surrealdb
- **Vite**: https://vite.dev
- **TypeScript**: https://www.typescriptlang.org
