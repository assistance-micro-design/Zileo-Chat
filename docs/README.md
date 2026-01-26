# Zileo Chat 3 - Documentation

Application desktop multi-agents avec interface conversationnelle.

## Stack Technique

**Frontend** : SvelteKit 2.49.1 + Svelte 5.48.0 + Vite 7.2.6
**Backend** : Rust 1.91.1 + Tauri 2.9.3
**Database** : SurrealDB 2.5.0
**LLM Framework** : Rig.rs 0.24.0 (multi-provider)
**LLM Providers Phase 1** : Mistral + Ollama
**Protocol** : MCP 2025-06-18 (SDK officiel Anthropic)

## Architecture

```
Frontend (SvelteKit)
    ↓ Tauri IPC
Backend (Rust)
    ├─ Agent Orchestrator
    ├─ MCP Client/Server
    └─ Rig.rs (LLM)
    ↓
SurrealDB + MCP Servers externes
```

## Documentation par Catégorie

### 🏗️ Architecture & Décisions

**[ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md)**
Toutes les décisions architecturales avec justifications : structure projet, database, sécurité, features, deployment

**[TECH_STACK.md](TECH_STACK.md)**
Versions exactes de toutes les technologies, prérequis, ressources officielles

**[DATABASE_SCHEMA.md](DATABASE_SCHEMA.md)**
Schéma complet SurrealDB : tables, relations, indexes, queries

### 🤖 Multi-Agents & LLM

**[MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)**
Système hiérarchique agents (Principal, Spécialisés, Temporaires), communication markdown, registry, prompts

**[WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md)**
Orchestration intra-workflow : exécution parallèle vs séquentielle des sous-agents/tools/MCP selon dépendances

**[AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)**
Outils natifs agents : Todo, Memory (vectorielle), UserQuestion, Sub-Agent tools

**[TOOLS_REFERENCE.md](TOOLS_REFERENCE.md)**
Reference complete des 7 tools avec exemples JSON et patterns de securite

**[SUB_AGENT_GUIDE.md](SUB_AGENT_GUIDE.md)**
Guide sous-agents : SpawnAgentTool, DelegateTaskTool, ParallelTasksTool

### 🔌 MCP (Model Context Protocol)

**[MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md)**
Guide configuration MCP servers : npx, uvx, docker, transports, sécurité

### 🎨 Frontend & UX

**[FRONTEND_SPECIFICATIONS.md](FRONTEND_SPECIFICATIONS.md)**
Spécifications complètes UI/UX : pages (Settings, Agent), multi-workflow, validation human-in-the-loop, composants et utilities réutilisables

**[DESIGN_SYSTEM.md](DESIGN_SYSTEM.md)**
Systeme de design complet : couleurs, typographie, composants UI, theme light/dark, accessibilite

### 🚀 Développement & Déploiement

**[GETTING_STARTED.md](GETTING_STARTED.md)**
Guide démarrage : installation, configuration, premier workflow

**[API_REFERENCE.md](API_REFERENCE.md)**
Référence Tauri commands : IPC, types, événements

**[DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md)**
Build et packaging : Linux (AppImage, .deb), macOS (.dmg), Windows (.msi)

**[TESTING_STRATEGY.md](TESTING_STRATEGY.md)**
Stratégie tests : unitaires, intégration, E2E, CI/CD

### 📋 Tâches Restantes & Roadmap

**[REMAINING_TASKS.md](REMAINING_TASKS.md)**
Tâches post-v1 différées : TYPE-OPT-5 (specta), SEC-OPT-7/8, DB-OPT-12/14, FE-OPT-12/13, OPT-FA-10/14/15

**[SPECS_REMAINING.md](SPECS_REMAINING.md)**
Spécifications OPT-FA restantes : migration streaming class runes, $inspect helpers, retry logic

## Workflows Documentation

### Nouveau Contributeur
1. [GETTING_STARTED.md](GETTING_STARTED.md) → Setup environnement
2. [TECH_STACK.md](TECH_STACK.md) → Versions et outils
3. [ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md) → Comprendre choix
4. [API_REFERENCE.md](API_REFERENCE.md) → Référence technique

### Implémentation Features
1. [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md) → Création agents
2. [WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md) → Orchestration parallèle/séquentielle
3. [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md) → Outils disponibles
4. [MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md) → Configuration MCP
5. [API_REFERENCE.md](API_REFERENCE.md) → Tauri commands

### Intégration LLM & MCP
1. [MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md) → Configuration serveurs MCP
2. [TOOLS_REFERENCE.md](TOOLS_REFERENCE.md) → Tools disponibles
3. [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md) → Documentation outils

### Deployment
1. [TESTING_STRATEGY.md](TESTING_STRATEGY.md) → Tests validation
2. [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md) → Build & packaging
3. [ARCHITECTURE_DECISIONS.md](ARCHITECTURE_DECISIONS.md) → Config production

## Principes Projet

### Architecture
- **Hiérarchie agents** : Orchestrateur → Spécialisés (permanent) → Temporaires
- **Communication** : Markdown reports standardisés
- **Abstraction** : Rig.rs pour multi-provider, MCP pour standardisation

### Sécurité
- **Production-ready dès v1** : API keys encryptées, validation inputs, audit logging
- **Human-in-the-loop** : Validation opérations critiques (suppression, modifications sensibles)
- **Isolation** : Sandboxing tools, permissions minimales par agent

### Performance
- **Streaming** : Réponses LLM temps réel
- **Caching** : Responses, embeddings, prompts
- **Embedded DB** : SurrealDB RocksDB pour desktop

### Évolutivité
- **Agents modulaires** : Factory pattern, registry dynamique
- **Provider switching** : Configuration uniquement (pas code)
- **MCP extensible** : Ajout servers sans modification agents

## Ressources Externes

**MCP** : https://modelcontextprotocol.io
**Rig.rs** : https://rig.rs
**Tauri v2** : https://v2.tauri.app
**SvelteKit** : https://kit.svelte.dev
**SurrealDB** : https://surrealdb.com

## Status Documentation

- **Cohérente** : Interdépendances vérifiées
- **Complète** : Toutes sections couvertes
- **Tests** : ~627 backend unit tests, ~179 frontend tests (~806 total)

Dernière validation : 2026-01-25
