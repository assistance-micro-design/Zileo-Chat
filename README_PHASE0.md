# Zileo-Chat-3 - Phase 0 Setup

## Statut

**Phase 0** (Setup Projet) - ✅ COMPLÉTÉ

## Environnement

- **Node.js**: >= 20.19
- **Rust**: >= 1.80.1
- **npm/pnpm/yarn**: Latest stable

## Installation

```bash
# Install frontend dependencies
npm install

# Check Rust setup
cargo check --manifest-path=src-tauri/Cargo.toml
```

## Validation

```bash
# Frontend
npm run lint        # ESLint
npm run check       # svelte-check + TypeScript strict

# Backend
cargo fmt --check --manifest-path=src-tauri/Cargo.toml
cargo clippy --manifest-path=src-tauri/Cargo.toml -- -D warnings
cargo test --manifest-path=src-tauri/Cargo.toml
```

## Structure

```
zileo-chat-3/
├─ src/                 # Frontend (SvelteKit)
│  ├─ routes/           # Pages (/agent, /settings)
│  ├─ lib/
│  │  ├─ components/    # Reusable components
│  │  └─ stores/        # State management
│  ├─ types/            # TypeScript interfaces
│  └─ styles/           # Global CSS
│
├─ src-tauri/           # Backend (Rust + Tauri)
│  ├─ src/
│  │  ├─ main.rs        # Entry point
│  │  ├─ commands/      # Tauri IPC commands
│  │  ├─ models/        # Data types
│  │  ├─ db/            # SurrealDB client
│  │  ├─ agents/        # Multi-agent system
│  │  └─ state.rs       # AppState
│  ├─ Cargo.toml
│  └─ tauri.conf.json
│
└─ docs/                # Documentation
```

## Prochaines Étapes

**Phase 1**: Database Foundation (SurrealDB)
**Phase 2**: Types Synchronisés (TS ↔ Rust)
**Phase 3**: Infrastructure Multi-Agent

Voir `docs/specs/2025-01-23_spec-base-implementation.md` pour le plan complet.
