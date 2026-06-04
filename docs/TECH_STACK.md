# Tech Stack: Svelte + Rust + SurrealDB

> **Current project versions: 4 June 2026**
> Production versions used in the project (tested compatibility).

## Stack Overview

```
Frontend  : SvelteKit 2.63.0 | Svelte 5.55.7
Backend   : Rust 1.93.0 + Tauri 2
Database  : SurrealDB ~2.6 (kv-rocksdb, no protocol-http)
Desktop   : Tauri (cross-platform)
LLM       : Mistral, Ollama, OpenAI-compatible providers (OpenRouter, RouterLab, etc.)
```

## Technologies & Versions

### Frontend (package.json)

**Core Framework**:

- **svelte**: 5.55.7
- **@sveltejs/kit**: ^2.61.0
- **@sveltejs/adapter-static**: ^3.0.0
- **@sveltejs/vite-plugin-svelte**: ^6.2.4
- **vite**: ^7.3.3

**TypeScript**:

- **typescript**: ^6.0.3
- **svelte-check**: ^4.4.8
- Strict-mode flags enabled in `tsconfig.json`: `strict`, `noUncheckedIndexedAccess`, `noImplicitOverride`, `noFallthroughCasesInSwitch`

**Tauri Integration**:

- **@tauri-apps/api**: ^2.11.0
- **@tauri-apps/cli**: ^2.11.1
- **@tauri-apps/plugin-dialog**: ^2.7.1
- **@tauri-apps/plugin-opener**: ^2.5.4

**UI Components**:

- **@lucide/svelte**: ^1.16.0 (official Lucide icon library)

**Content Processing**:

- **dompurify**: ^3.4.2 (HTML sanitization)
- **marked**: ^18.0.3 (Markdown rendering)

**Testing**:

- **vitest**: ^4.1.5 (unit tests)
- **@playwright/test**: ^1.60.0 (E2E tests)
- **jsdom**: ^27.4.0 (DOM testing)

**Linting**:

- **eslint**: ^10.4.0
- **eslint-plugin-svelte**: ^3.17.1
- **@eslint/js**: ^9.39.4
- **typescript-eslint**: ^8.59.4
- **globals**: ^17.6.0
- ESLint enforces `no-console: error` and `@typescript-eslint/no-explicit-any: error` (build-breaking)

### Backend (Cargo.toml)

**Core Framework**:

- **Rust**: 1.93.0 (stable, edition 2021)
- **tauri**: 2 (framework)
- **tauri-build**: 2 (build dependency, version range)
- **tauri-plugin-opener**: 2 (version range)
- **tauri-plugin-dialog**: 2 (version range)

**LLM & Multi-Agent**:

- **async-trait**: 0.1 (agent trait definitions)
- **futures-util**: 0.3.31 (stream utilities for SSE)
- Providers: Mistral (native), Ollama (native), OpenAI-compatible (custom providers). Direct HTTP integration via `src-tauri/src/llm/` (no third-party abstraction layer).

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

- **uuid**: 1.23 (features: v4, serde)
- **chrono**: 0.4.43 (features: serde)
- **regex**: 1.10
- **globset**: 0.4 (glob pattern matching for FileManagerTool)
- **base64**: 0.22 (base64 encoding/decoding)
- **rand**: 0.10 (jittered retry backoff in `llm/retry.rs`)

**HTTP & Network**:

- **reqwest**: 0.12 (features: rustls-tls, json, stream, multipart — `multipart` added for Voxtral STT batch upload)
- **url**: 2.5 (typed host extraction `url::Host::{Ipv4,Ipv6}` for the MCP HTTP SSRF guard; declared direct, already transitive via reqwest)

**Security**:

- **keyring**: 3.6 (OS keychain: apple-native, windows-native, sync-secret-service)
- **aes-gcm**: 0.10 (AES-256 encryption)
- **MCP egress hardening**: SSRF resolver + Docker spawn guard + per-agent unattended-run tool allowlist (see `ARCHITECTURE_DECISIONS.md` Q30)

**Platform-conditional**:

- **webkit2gtk**: 2.0 (Linux only, under `[target.'cfg(target_os = "linux")'.dependencies]`) — exposes the WebKitGTK `permission-request` signal so `getUserMedia` can be allowed for the microphone (push-to-talk dictation). Without this hook, `wry`'s WebKitGTK backend denies all permission requests by default on Fedora / Ubuntu builds; macOS and Windows surface their own native prompts and don't need it.

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
- Vite build target: `es2022` / `chrome105` / `safari15` (matches Tauri 2 WebView baselines)

### Rust <-> SurrealDB

- Native embedded Rust client (surrealdb.rs)
- Type-safe queries with serde
- Parameterized queries for SQL injection prevention

### SvelteKit <-> Tauri

- Adapter-static for SPA mode (single `index.html` fallback)
- Asset protocol for local files

## LLM Providers

Three provider types with unified interface:

| Provider | Type              | Features                                                |
| -------- | ----------------- | ------------------------------------------------------- |
| Mistral  | Native API        | Thinking/reasoning, vision, tool calling, streaming, Voxtral speech-to-text (batch endpoint, used by push-to-talk dictation) |
| Ollama   | Native API        | Local models, thinking, vision, tool calling, streaming |
| Custom   | OpenAI-compatible | OpenRouter, RouterLab, etc. via `/v1/chat/completions`  |

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

| Constant               | Value | Scope                   |
| ---------------------- | ----- | ----------------------- |
| DEFAULT_LIST_LIMIT     | 1000  | agents, memories, tasks |
| DEFAULT_MODELS_LIMIT   | 100   | LLM models              |
| DEFAULT_MCP_LOGS_LIMIT | 500   | MCP call logs           |
| DEFAULT_MESSAGES_LIMIT | 500   | message history         |
| MAX_LIST_LIMIT         | 10000 | maximum allowed         |

## Testing

- **Backend**: 1300+ Rust tests (lib target)
- **Frontend**: 380+ Vitest unit tests
- **E2E**: Playwright (available, not counted in totals)
- **Total**: 1,700+ automated tests (run `cargo test --lib && npm run test` for the current count)

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

| Requirement   | Minimum Version  | Notes                           |
| ------------- | ---------------- | ------------------------------- |
| Node.js       | 20.19+ or 22.12+ | Node 18 not supported by Vite 7 |
| Rust          | 1.93.0+          | Required by SurrealDB SDK       |
| npm/pnpm/yarn | Latest stable    |                                 |

## Key Migration Notes

**Svelte 5** (from Svelte 4):

- `{#snippet}` + `{@render}` replaces `<slot>`
- `{@attach}` replaces `use:action`
- `$props()` replaces `export let`
- `onclick` replaces `on:click`

**Vitest 4** (from Vitest 2):

- `maxWorkers` replaces `maxThreads` / `maxForks`
- `projects` replaces `workspace`

**ESLint 10** (from ESLint 9):

- Flat config (`eslint.config.js`) is the only supported format; this project already used it, so the bump was a no-op for our config.
- Drops Node < 20.19 / < 22.12 (already our minimum).
- `@eslint/js` stays on its 9.x line and remains compatible as the recommended-config provider.

## Resources

- **Tauri**: https://tauri.app | https://v2.tauri.app
- **SvelteKit**: https://kit.svelte.dev
- **Svelte**: https://svelte.dev
- **SurrealDB**: https://surrealdb.com
- **surrealdb.rs**: https://docs.rs/surrealdb
- **Vite**: https://vite.dev
- **TypeScript**: https://www.typescriptlang.org
