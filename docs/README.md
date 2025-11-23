# Zileo Chat 3 - Documentation

Application desktop multi-agents avec interface conversationnelle.

## Stack Technique

**Frontend** : SvelteKit 2.49.0 + Svelte 5.43.14
**Backend** : Rust 1.91.1 + Tauri 2.9.4
**Database** : SurrealDB 2.3.10
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
Outils natifs agents : Todo, Memory (vectorielle), Internal Reports

**[LLM_INTEGRATION_RECOMMENDATIONS.md](LLM_INTEGRATION_RECOMMENDATIONS.md)**
Recommandations intégration LLM : abstraction Rig.rs, MCP protocol, architecture layers

**[MULTI_PROVIDER_SPECIFICATIONS.md](MULTI_PROVIDER_SPECIFICATIONS.md)**
Spécifications multi-provider : paramètres, streaming, tokens, capacités spécifiques (Phase 1: Mistral, Ollama | Future: Claude, GPT, Gemini)

### 🔌 MCP (Model Context Protocol)

**[MCP_ARCHITECTURE_DECISION.md](MCP_ARCHITECTURE_DECISION.md)**
Choix SDK officiel Anthropic, double rôle (Client + Server), intégration Rig.rs

**[MCP_CONFIGURATION_GUIDE.md](MCP_CONFIGURATION_GUIDE.md)**
Guide configuration MCP servers : npx, uvx, docker, transports, sécurité

### 🎨 Frontend & UX

**[FRONTEND_SPECIFICATIONS.md](FRONTEND_SPECIFICATIONS.md)**
Spécifications complètes UI/UX : pages (Settings, Agent), multi-workflow, validation human-in-the-loop, composants réutilisables

### 🚀 Développement & Déploiement

**[GETTING_STARTED.md](GETTING_STARTED.md)**
Guide démarrage : installation, configuration, premier workflow

**[API_REFERENCE.md](API_REFERENCE.md)**
Référence Tauri commands : IPC, types, événements

**[DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md)**
Build et packaging : Linux (AppImage, .deb), macOS (.dmg), Windows (.msi)

**[TESTING_STRATEGY.md](TESTING_STRATEGY.md)**
Stratégie tests : unitaires, intégration, E2E, CI/CD

### 📋 Audit & Conformité

**[DOCUMENTATION_AUDIT.md](DOCUMENTATION_AUDIT.md)**
Audit cohérence documentation, vérification interdépendances, status corrections

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

### Intégration LLM
1. [LLM_INTEGRATION_RECOMMENDATIONS.md](LLM_INTEGRATION_RECOMMENDATIONS.md) → Architecture
2. [MULTI_PROVIDER_SPECIFICATIONS.md](MULTI_PROVIDER_SPECIFICATIONS.md) → Providers
3. [MCP_ARCHITECTURE_DECISION.md](MCP_ARCHITECTURE_DECISION.md) → MCP integration

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

✅ **Cohérente** : Interdépendances vérifiées
✅ **Complète** : Toutes sections couvertes
✅ **À jour** : Versions confirmées Nov 2025

Dernière validation : 2025-11-23
