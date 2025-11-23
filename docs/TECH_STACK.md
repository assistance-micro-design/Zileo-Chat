# Stack Technique : Svelte + Rust + SurrealDB

> **Versions vérifiées le : 22 Novembre 2025**
> Toutes les versions indiquées sont les dernières versions stables disponibles.

## Stack Overview

```
Frontend  : SvelteKit 2.49.0 | Svelte 5.43.14
Backend   : Rust 1.91.1 + Tauri 2.9.4
Database  : SurrealDB 2.3.10
Desktop   : Tauri (cross-platform)
```

## Technologies & Versions

### Frontend
- **SvelteKit**: 2.49.0 (Nov 2025)
- **Svelte**: 5.43.14 (Nov 2025)
- **TypeScript**: 5.9.3
- **Vite**: 7.2.2
- **@sveltejs/adapter-static**: 3.0.0+
- **@sveltejs/vite-plugin-svelte**: 4.0.0+

### Backend
- **Rust**: 1.91.1 (stable)
- **Tauri CLI**: 2.9.4
- **@tauri-apps/api**: 2.9.0
- **@tauri-apps/cli**: 2.9.4
- **serde**: 1.0.228
- **serde_json**: 1.0.145
- **tokio**: 1.48.0 (features: full)
- **tauri-build**: 2.9.4

### Database
- **SurrealDB**: 2.3.10 (server + crate)
- **surrealdb.rs**: 2.3.10 (Rust client)
- **surrealdb.js**: Latest (JS client, optional)

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

```rust
// Tauri allowlist (tauri.conf.json)
{
  "app": {
    "security": {
      "csp": "default-src 'self'"
    }
  },
  "allowlist": {
    "all": false,
    "invoke": {
      "all": false,
      "commands": ["create_record", "query_db"]
    }
  }
}
```

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

**Nov 2025**:
- Vite 7.2.2 requires Node.js 20.19+ or 22.12+
- Svelte 5.43.14 includes latest runes improvements
- SvelteKit 2.49.0 has Vite 7 support
- Tauri 2.9.4 latest stable with mobile support
- SurrealDB 2.3.10 with improved HNSW vector cache
