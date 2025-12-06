# Stack Technique : Svelte + Rust + SurrealDB

> **Versions actuelles du projet : 5 Decembre 2025**
> Versions de production utilisees dans le projet (compatibilite testee).

## Stack Overview

```
Frontend  : SvelteKit 2.49.0 | Svelte 5.43.14
Backend   : Rust 1.91.1 + Tauri 2.9.4
Database  : SurrealDB 2.4.0
Desktop   : Tauri (cross-platform)
```

## Technologies & Versions

### Frontend (package.json)

**Core Framework**:
- **svelte**: ^5.43.14
- **@sveltejs/kit**: ^2.49.0
- **@sveltejs/adapter-static**: ^3.0.0
- **@sveltejs/vite-plugin-svelte**: ^4.0.0
- **vite**: ^5.4.0

**TypeScript**:
- **typescript**: ^5.9.3
- **svelte-check**: ^4.0.0

**Tauri Integration**:
- **@tauri-apps/api**: ^2.9.0
- **@tauri-apps/cli**: ^2.9.4
- **@tauri-apps/plugin-dialog**: ^2.2.0

**UI Components**:
- **lucide-svelte**: ^0.554.0 (icon library)

**Testing**:
- **vitest**: ^2.0.0 (unit tests)
- **@playwright/test**: ^1.47.0 (E2E tests)
- **jsdom**: ^27.2.0 (DOM testing)

**Linting**:
- **eslint**: ^9.0.0
- **eslint-plugin-svelte**: ^2.46.0
- **@eslint/js**: ^9.39.1
- **@typescript-eslint/eslint-plugin**: ^8.0.0
- **@typescript-eslint/parser**: ^8.0.0
- **typescript-eslint**: ^8.48.0
- **globals**: ^16.5.0

### Backend (Cargo.toml)

**Core Framework**:
- **Rust**: 1.91.1 (stable, edition 2021)
- **tauri**: 2 (framework)
- **tauri-build**: 2 (build dependencies)
- **tauri-plugin-opener**: 2
- **tauri-plugin-dialog**: 2

**LLM & Multi-Agent**:
- **rig-core**: 0.24.0 (features: all) - LLM abstraction framework
- **async-trait**: 0.1 (agent trait definitions)
- **futures**: 0.3 (parallel execution)
- **futures-util**: 0.3

**Database**:
- **surrealdb**: 2.4.0 (features: kv-rocksdb)

**Serialization**:
- **serde**: 1.0.228 (features: derive)
- **serde_json**: 1.0.145

**Async Runtime**:
- **tokio**: 1.48.0 (features: full)
- **tokio-util**: 0.7 (features: rt)

**Error Handling**:
- **anyhow**: 1.0
- **thiserror**: 1.0

**Logging**:
- **tracing**: 0.1
- **tracing-subscriber**: 0.3 (features: json, env-filter)

**Utilities**:
- **uuid**: 1.0 (features: v4, serde)
- **chrono**: 0.4 (features: serde)
- **regex**: 1.10
- **lazy_static**: 1.5

**HTTP & Network**:
- **reqwest**: 0.12 (features: rustls-tls, json, stream)

**Security**:
- **keyring**: 2.0 (OS keychain integration)
- **aes-gcm**: 0.10 (AES-256 encryption)

**Dev Dependencies**:
- **tempfile**: 3.14

### Database
- **SurrealDB**: 2.4.0 (embedded with kv-rocksdb feature)
- **surrealdb.rs**: 2.4.0 (Rust client via Cargo)

## Architecture

```
┌─────────────────────────────────────┐
│         SvelteKit (Frontend)        │
│  - Components (.svelte)             │
│  - Routes (file-based)              │
│  - Stores (state management)        │
└──────────────┬──────────────────────┘
               │ Tauri IPC
               ↓
┌─────────────────────────────────────┐
│       Rust Backend (Tauri)          │
│  - Commands (API layer)             │
│  - Business logic                   │
│  - SurrealDB client                 │
└──────────────┬──────────────────────┘
               │ surrealdb.rs
               ↓
┌─────────────────────────────────────┐
│           SurrealDB                 │
│  - Multi-model database             │
│  - Embedded or Server mode          │
└─────────────────────────────────────┘
```

## Project Structure


## Key Integrations

### Tauri ↔ Svelte
- Communication via `invoke()` (frontend) → `#[tauri::command]` (backend)
- Type-safe with TypeScript + Rust types
- Async/await on both sides

### Rust ↔ SurrealDB
- Native Rust client (`surrealdb.rs`)
- Embedded or remote connection
- Type-safe queries with serde

### SvelteKit ↔ Tauri
- Adapter-static for SPA mode
- Single `index.html` fallback
- Asset protocol for local files

## Performance Tips

1. **Use embedded SurrealDB** (RocksDB) for desktop apps
2. **Enable Svelte compiler optimizations** (production build)
3. **Use Tauri's asset protocol** for local resources
4. **Implement lazy loading** for large datasets
5. **Use Svelte stores** for reactive state management

## Security

```json
// Tauri v2 security (tauri.conf.json)
{
  "app": {
    "security": {
      "csp": "default-src 'self'; style-src 'self' 'unsafe-inline'"
    }
  }
}
```

**Security Features**:
- **CSP**: Content Security Policy restricts resource loading
- **API Key Storage**: OS keychain via `keyring` crate + AES-256 encryption
- **Tauri v2**: Capability-based permissions (no v1 allowlist)

## Build Outputs

```
src-tauri/target/release/
├── bundle/
│   ├── appimage/        # Linux
│   ├── deb/             # Debian
│   ├── dmg/             # macOS
│   └── msi/             # Windows
└── app                   # Binary
```

## Version Requirements

### Minimum Requirements
- **Node.js**: 20.19+ ou 22.12+ (Node.js 18 n'est plus supporté par Vite 7)
- **Rust**: 1.80.1+ (requis par SurrealDB SDK)
- **npm/pnpm/yarn**: Latest stable

### Recommended Setup
```bash
# Vérifier les versions installées
node --version    # >= 20.19
rustc --version   # >= 1.91.1
cargo --version   # >= 1.91.1
```

## Resources

- **Tauri**: https://tauri.app | https://v2.tauri.app
- **SvelteKit**: https://kit.svelte.dev
- **Svelte**: https://svelte.dev
- **SurrealDB**: https://surrealdb.com
- **surrealdb.rs**: https://docs.rs/surrealdb
- **Vite**: https://vite.dev
- **TypeScript**: https://www.typescriptlang.org

## Version Update Notes

**Dec 2025 - Actual Project Versions**:
- Vite 5.4.0 used for compatibility with Node.js 20.19
- Svelte 5.43.14 includes latest runes improvements
- SvelteKit 2.49.0 stable release
- Tauri 2.x with plugin-dialog and plugin-opener
- SurrealDB 2.4.0 with kv-rocksdb for embedded desktop use
- **rig-core 0.24.0** for multi-provider LLM abstraction (Mistral, Ollama)
- async-trait 0.1 and futures 0.3 for multi-agent async patterns
- keyring 2.0 + aes-gcm 0.10 for secure API key storage
- lucide-svelte 0.554.0 for UI icons
