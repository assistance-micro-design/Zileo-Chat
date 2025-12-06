# Zileo-Chat-3

> Application desktop multi-agent avec interface conversationnelle

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Statut du Projet

**Phase actuelle**: Phase 5 Backend Features complete
**Prochaine etape**: Phase 6 Integration & Polish

### Metriques du Code
- **99 commandes Tauri** (agent, workflow, memory, validation, streaming, MCP, LLM, prompts, etc.)
- **6 outils** (MemoryTool, TodoTool, CalculatorTool, SpawnAgentTool, DelegateTaskTool, ParallelTasksTool)
- **18 tables SurrealDB** (workflow, agent, message, memory, task, validation_request, etc.)
- **12 composants UI** (Button, Card, Modal, Input, Select, Badge, StatusIndicator, etc.)
- **14 stores Svelte** (agentStore, workflowStore, streamingStore, tokenStore, etc.)

## Description

Zileo-Chat-3 est une application desktop sophistiquée construite sur une architecture multi-agent, permettant l'orchestration intelligente de tâches via une interface conversationnelle.

### Caracteristiques Principales

- **Systeme Multi-Agent**: Orchestration avec sub-agents (spawn, delegate, parallel tasks)
- **Interface Conversationnelle**: Streaming temps-reel avec affichage tokens
- **Base de Donnees Hybride**: SurrealDB avec support relationnel, graph et vectoriel (HNSW)
- **Securite Production**: API keys chiffrees (OS keychain + AES-256), validation stricte, CSP
- **Interface Moderne**: SvelteKit + Svelte 5 (runes) avec 12 composants UI
- **Backend Performant**: Rust avec Tauri, 99 commandes IPC
- **Extensibilite MCP**: Model Context Protocol avec gestion serveurs dynamique
- **Observabilite**: Logging structure avec tracing, spans workflow/agent
- **i18n**: Support multilingue (EN/FR) avec detection systeme
- **Validation Human-in-the-loop**: Approbation des operations critiques
- **Bibliotheque de Prompts**: Gestion et reutilisation de prompts systeme
- **Import/Export**: Sauvegarde et restauration des configurations

## Architecture

```
Frontend (SvelteKit + Svelte 5)
         ↕ IPC (Tauri)
Backend (Rust + Tokio)
    ├─ Multi-Agent System (Registry + Orchestrator)
    ├─ LLM Integration (Rig.rs: Mistral + Ollama)
    ├─ MCP Protocol (Client/Server)
    └─ Database (SurrealDB embedded)
```

## Documentation

Toute la documentation technique est disponible dans le repertoire `docs/`:

| Document | Description |
|----------|-------------|
| **[TECH_STACK.md](docs/TECH_STACK.md)** | Versions exactes et requirements |
| **[ARCHITECTURE_DECISIONS.md](docs/ARCHITECTURE_DECISIONS.md)** | Decisions techniques justifiees |
| **[MULTI_AGENT_ARCHITECTURE.md](docs/MULTI_AGENT_ARCHITECTURE.md)** | Systeme multi-agent detaille |
| **[DATABASE_SCHEMA.md](docs/DATABASE_SCHEMA.md)** | Schema SurrealDB (18 tables) |
| **[FRONTEND_SPECIFICATIONS.md](docs/FRONTEND_SPECIFICATIONS.md)** | Composants et stores frontend |
| **[TOOLS_REFERENCE.md](docs/TOOLS_REFERENCE.md)** | Reference des 6 outils |
| **[MCP_CONFIGURATION_GUIDE.md](docs/MCP_CONFIGURATION_GUIDE.md)** | Configuration serveurs MCP |
| **[DESIGN_SYSTEM.md](docs/DESIGN_SYSTEM.md)** | Systeme de design UI |
| **[CLAUDE.md](CLAUDE.md)** | Guidelines pour developpement avec Claude Code |

## Requirements

### Minimum
- **Node.js**: 20.19+ ou 22.12+ (requis par Vite 7)
- **Rust**: 1.80.1+ (requis par SurrealDB SDK)
- **npm/pnpm/yarn**: Latest stable

### Vérification
```bash
node --version    # >= 20.19
rustc --version   # >= 1.91.1
cargo --version   # >= 1.91.1
```

## Installation (Future)

```bash
# Clone du repository
git clone https://github.com/your-org/zileo-chat-3.git
cd zileo-chat-3

# Installation des dépendances
npm install

# Développement
npm run tauri:dev

# Build production
npm run tauri:build
```

## Roadmap

### Phases Completees

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 0 | Base implementation (agents, LLM, DB, security, 19 commands) | Complete |
| Phase 1 | Design System Foundation (theme, 12 UI components) | Complete |
| Phase 2 | Layout Components (AppContainer, Sidebar, FloatingMenu) | Complete |
| Phase 3 | Chat & Workflow Components (MessageBubble, ChatInput) | Complete |
| Phase 4 | Pages Refactoring (agent page, settings page) | Complete |
| **Phase 5** | **Backend Features (validation, memory, streaming, 99 commands)** | **Complete** |

### Phase Actuelle

| Phase | Description | Status |
|-------|-------------|--------|
| Phase 6 | Integration & Polish (E2E tests, accessibility audit) | En cours |

### Fonctionnalites Implementees

- Integration LLM (Mistral + Ollama via Rig.rs)
- Streaming responses avec affichage tokens
- Systeme multi-agent complet (6 outils)
- Sub-agents (spawn, delegate, parallel tasks)
- Human-in-the-loop validation UI
- Bibliotheque de prompts
- Import/Export configurations
- i18n (EN/FR)
- Onboarding wizard

### Prochaines Etapes

- Tests E2E complets (Playwright)
- Audit accessibilite
- macOS builds
- Windows builds

## Développement

### Structure du Projet

```
zileo-chat-3/
├── src/                    # Frontend SvelteKit
│   ├── routes/             # File-based routing (3 pages)
│   │   ├── agent/          # Page agent (chat, workflows)
│   │   └── settings/       # Page parametres
│   ├── lib/
│   │   ├── components/ui/  # 12 composants UI atomiques
│   │   ├── stores/         # 14 stores Svelte
│   │   └── i18n/           # Internationalisation
│   ├── types/              # TypeScript interfaces
│   └── messages/           # Traductions (en.json, fr.json)
├── src-tauri/              # Backend Rust
│   └── src/
│       ├── commands/       # 99 commandes Tauri (18 fichiers)
│       ├── tools/          # 6 outils (memory, todo, calculator, sub-agent)
│       ├── agents/         # Systeme multi-agent
│       ├── db/             # SurrealDB (18 tables)
│       ├── llm/            # Rig.rs (Mistral, Ollama)
│       ├── mcp/            # MCP protocol client
│       └── models/         # Types Rust
├── docs/                   # Documentation technique
└── LICENSE                 # Apache 2.0
```

### Validation

```bash
# Frontend
npm run check      # Svelte + TypeScript
npm run lint       # ESLint
npm run test       # Vitest
npm run build      # Production build

# Backend
cd src-tauri
cargo fmt --check         # Format
cargo clippy -- -D warnings  # Linting strict
cargo test                # Tests
cargo build --release     # Release build
```

## Contribuer

Les contributions sont les bienvenues ! Consultez [CLAUDE.md](CLAUDE.md) pour les guidelines de développement.

### Workflow
1. Fork le projet
2. Créer une branche feature (`git checkout -b feature/amazing-feature`)
3. Commit les changements (`git commit -m 'Add amazing feature'`)
4. Push vers la branche (`git push origin feature/amazing-feature`)
5. Ouvrir une Pull Request

### Standards de Code
- ✅ TypeScript strict mode (no `any`)
- ✅ Rust clippy warnings as errors
- ✅ Tests pour critical paths (~70% coverage)
- ✅ JSDoc/Rustdoc pour API publiques
- ✅ Commits conventionnels (feat/fix/docs/refactor)

## Sécurité

**Production-ready dès v1.0**:
- API keys stockées via OS keychain + AES-256
- Validation inputs stricte (frontend + backend)
- Content Security Policy (CSP) configurée
- Tauri allowlist explicite (no wildcard)
- Audit trail avec structured logging

Pour signaler une vulnérabilité, contactez: security@zileo.example.com

## Licence

Ce projet est distribué sous **licence Apache 2.0**. Voir le fichier [LICENSE](LICENSE) pour plus de détails.

```
Copyright 2025 Zileo-Chat-3 Contributors

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

    http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
```

### Permissions Apache 2.0
- ✅ Usage commercial
- ✅ Modification
- ✅ Distribution
- ✅ Usage privé
- ✅ Utilisation de brevets

## Ressources

- **Documentation**: [docs/](docs/)
- **Spécifications**: [docs/specs/](docs/specs/)
- **Tauri**: https://v2.tauri.app
- **SvelteKit**: https://kit.svelte.dev
- **SurrealDB**: https://surrealdb.com
- **Rig.rs**: https://docs.rs/rig-core

## Support

- 📖 Documentation complète dans `/docs`
- 💬 Issues GitHub pour bugs et features requests
- 🤝 Discussions pour questions et support communautaire

---

**Statut**: En developpement actif - Phase 5 complete (99 commands, 6 tools, 18 tables), Phase 6 Integration en cours
