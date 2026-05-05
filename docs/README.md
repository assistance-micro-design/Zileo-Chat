# Zileo Chat 3 - Documentation

Multi-agent desktop application with conversational interface.

## Tech Stack

**Frontend**: SvelteKit 2.55.0 + Svelte 5.55.1 + Vite 7.3.2
**Backend**: Rust 1.93.0 + Tauri 2
**Database**: SurrealDB ~2.6
**LLM Framework**: rig-core 0.34.0 (multi-provider)
**LLM Providers**: Mistral + Ollama + OpenAI-compatible (custom)
**Protocol**: MCP 2025-06-18 (Anthropic official SDK)

## Architecture

```
Frontend (SvelteKit)
    | Tauri IPC
Backend (Rust)
    |-- Agent Orchestrator
    |-- MCP Client/Server
    +-- Rig.rs (LLM)
    |
SurrealDB + External MCP Servers
```

## Documentation Index

### Architecture & Decisions

| Document | Description |
|----------|-------------|
| [ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md) | All architectural decisions with rationale |
| [TECH_STACK.md](TECH_STACK.md) | Exact versions, prerequisites, official resources |
| [DATABASE_SCHEMA.md](DATABASE_SCHEMA.md) | Full SurrealDB schema: tables, relations, indexes |

### Multi-Agent & LLM

| Document | Description |
|----------|-------------|
| [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md) | Hierarchical agent system, sub-agent delegation, communication |
| [WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md) | Parallel vs sequential orchestration, execution flows |
| [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md) | All 9 agent tools: Todo, Memory, Calculator, UserQuestion, FileManager, Spawn/Delegate/Parallel sub-agents, ReadSkill |

### Frontend & UX

| Document | Description |
|----------|-------------|
| [FRONTEND_SPECIFICATIONS.md](FRONTEND_SPECIFICATIONS.md) | Routes, components, stores, state management |
| [DESIGN_SYSTEM.md](DESIGN_SYSTEM.md) | Colors, typography, components, themes, accessibility |

### Development & Deployment

| Document | Description |
|----------|-------------|
| [GETTING_STARTED.md](GETTING_STARTED.md) | Installation, configuration, first run |
| [API_REFERENCE.md](API_REFERENCE.md) | Tauri IPC commands, types, events |
| [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md) | Build & packaging: Linux, macOS, Windows |
| [TESTING_STRATEGY.md](TESTING_STRATEGY.md) | Test strategy: unit, integration, CI/CD |

## Getting Started

1. [GETTING_STARTED.md](GETTING_STARTED.md) - Set up your environment
2. [TECH_STACK.md](TECH_STACK.md) - Understand versions and tools
3. [ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md) - Learn the design rationale
4. [API_REFERENCE.md](API_REFERENCE.md) - Explore the API

## Key Principles

- **Agent hierarchy**: Orchestrator > Specialized (persistent) > Temporary
- **Security first**: Encrypted API keys, input validation, audit logging, human-in-the-loop
- **Streaming**: Real-time LLM responses
- **Embedded DB**: SurrealDB with RocksDB for desktop
- **Extensible**: MCP servers, modular agents, provider switching via config

## External Resources

- **MCP**: https://modelcontextprotocol.io
- **Rig.rs**: https://rig.rs
- **Tauri v2**: https://v2.tauri.app
- **SvelteKit**: https://kit.svelte.dev
- **SurrealDB**: https://surrealdb.com

## Status

- 1300+ backend lib tests + 380+ frontend tests (see `cargo test` and `npm run test` for current counts)
- 24 security audits completed
- Last validation: 2026-05-05
