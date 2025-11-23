# Zileo-Chat-3

> Application desktop multi-agent avec interface conversationnelle

**Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 | Rust 1.91.1 + Tauri 2.9.4 | SurrealDB 2.3.10

## Statut du Projet

**Phase actuelle**: Architecture et Documentation complète
**Prochaine étape**: Implémentation de la base (Phase 0-9)

## Description

Zileo-Chat-3 est une application desktop sophistiquée construite sur une architecture multi-agent, permettant l'orchestration intelligente de tâches via une interface conversationnelle.

### Caractéristiques Principales

- 🤖 **Système Multi-Agent**: Orchestration centralisée avec agents permanents et temporaires
- 💬 **Interface Conversationnelle**: Communication naturelle avec les agents
- 🗄️ **Base de Données Hybride**: SurrealDB avec support relationnel, graph et vectoriel (HNSW)
- 🔐 **Sécurité Production**: API keys chiffrées (OS keychain + AES-256), validation stricte, CSP
- 🎨 **Interface Moderne**: SvelteKit + Svelte 5 (runes) pour une UI réactive
- 🦀 **Backend Performant**: Rust avec Tauri pour une application native cross-platform
- 🔌 **Extensibilité MCP**: Support du Model Context Protocol pour intégration d'outils externes
- 📊 **Observabilité**: Logging structuré avec tracing, spans workflow/agent

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

Toute la documentation technique est disponible dans le répertoire `docs/`:

- **[TECH_STACK.md](docs/TECH_STACK.md)**: Versions exactes et requirements
- **[ARCHITECTURE_DECISIONS.md](docs/ARCHITECTURE_DECISIONS.md)**: 19 décisions techniques justifiées
- **[MULTI_AGENT_ARCHITECTURE.md](docs/MULTI_AGENT_ARCHITECTURE.md)**: Système multi-agent détaillé
- **[API_REFERENCE.md](docs/API_REFERENCE.md)**: Signatures des commandes Tauri
- **[DATABASE_SCHEMA.md](docs/DATABASE_SCHEMA.md)**: Schéma SurrealDB complet
- **[TESTING_STRATEGY.md](docs/TESTING_STRATEGY.md)**: Stratégie de tests (~70% coverage)
- **[CLAUDE.md](CLAUDE.md)**: Guidelines pour développement avec Claude Code
- **[specs/](docs/specs/)**: Spécifications d'implémentation détaillées

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

### v0.1.0 - Base (En cours)
✅ Infrastructure complète
✅ Documentation exhaustive
⏳ Implémentation fondations (15-20 jours estimés)

**Phases d'implémentation**:
- Phase 0: Setup Projet (1j)
- Phase 1: Database Foundation (2j)
- Phase 2: Types Synchronisés (1j)
- Phase 3: Infrastructure Multi-Agent (3j)
- Phase 4: Tauri Commands Core (2j)
- Phase 5: UI Basique (2j)
- Phase 6: Logging et Monitoring (1j)
- Phase 7: Sécurité de Base (2j)
- Phase 8: Tests et Documentation (2j)
- Phase 9: Build et Packaging (1j)

### v0.2.0 - LLM Functional (+1 semaine)
- Intégration LLM réelle (Mistral + Ollama)
- Streaming responses
- Token counting et cost tracking

### v0.3.0 - Multi-Agent Core (+2 semaines)
- Agents spécialisés (DB, API, RAG, UI, Code)
- MCP client integration
- Tools custom (SurrealDB, HTTP, Embeddings)

### v1.0.0 - Public Release (+5 semaines)
- Human-in-the-loop validation UI
- Système RAG complet
- Métriques temps-réel avancées
- macOS builds

### v1.1.0+
- Multi-provider LLM (Claude, GPT-4, Gemini)
- Windows builds
- Theme customization
- Export/Import workflows
- Auto-updates

## Développement

### Structure du Projet

```
zileo-chat-3/
├── src/                    # Frontend SvelteKit
│   ├── routes/            # File-based routing
│   ├── lib/               # Components, stores, utils
│   └── types/             # TypeScript interfaces
├── src-tauri/             # Backend Rust
│   └── src/
│       ├── commands/      # Tauri IPC handlers
│       ├── agents/        # Multi-agent system
│       ├── db/            # SurrealDB client
│       ├── llm/           # Rig.rs integration
│       ├── mcp/           # MCP protocol
│       └── models/        # Rust types
├── docs/                  # Documentation technique
│   └── specs/             # Spécifications détaillées
└── LICENSE                # Apache 2.0
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

**Statut**: 🚧 En développement actif - Phase Architecture complète, implémentation en cours
