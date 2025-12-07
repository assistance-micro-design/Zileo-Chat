# Décisions Architecture Projet

> **Date** : 2025-11-23 (décisions initiales) | 2025-12-06 (validation implémentation)
> **Phase** : Phase 5 Backend Features complète, Phase 6 Integration en cours
> **Statut** : Décisions validées et implémentées (111 commandes Tauri, 87 fichiers Rust, 77 composants Svelte)

---

## 1. Architecture & Stack

### Question 2 : Structure Projet

**Décision** : **From Scratch avec structure Tauri standard**

**Raisons** :
- Templates Tauri génériques = overhead inutile pour architecture multi-agents
- Contrôle total structure dossiers pour agents/prompts/tools
- Pas de dépendances template obsolètes

**Structure Conceptuelle** :
```
zileo-chat-3/
├─ src/                    # Frontend SvelteKit
│  ├─ routes/              # Pages (settings, agent)
│  ├─ lib/components/      # Composants UI réutilisables
│  └─ stores/              # State management Svelte
│
├─ src-tauri/              # Backend Rust
│  ├─ agents/              # Système multi-agents
│  │  ├─ core/             # Orchestrateur, registry
│  │  ├─ specialized/      # Agents permanents (DB, API, etc.)
│  │  ├─ config/           # Configs TOML agents
│  │  └─ prompts/          # System prompts + templates
│  ├─ llm/                 # Rig.rs integration, providers
│  ├─ mcp/                 # MCP client/server (SDK officiel)
│  ├─ tools/               # MCP tools custom
│  ├─ db/                  # SurrealDB client, schemas
│  └─ commands/            # Tauri commands (IPC)
│
└─ docs/                   # Documentation projet
```

**Avantages** :
- Clarté organisation (agents, llm, mcp séparés)
- Scalabilité (ajout agents/tools facile)
- Maintenance (responsabilités claires)

---

### Question 3 : Mono-repo ou Packages

**Décision** : **Mono-repo**

**Raisons** :
- Frontend + Backend couplés (Tauri natif)
- Pas de versioning indépendant nécessaire
- Simplification développement (un repo, un build)
- Coordination agents/tools/UI dans même codebase

**Pas Multi-packages** car :
- Overhead gestion versions entre packages
- Complexité inutile pour application desktop
- Pas de réutilisation externe prévue

---

## 2. Database & Persistence

### Question 4 : Schéma SurrealDB

**Décision** : **Schéma complet avec relations graph**

**Raisons** :
- Multi-agents = relations complexes (agent → workflow → tasks → memory)
- SurrealDB = graph natif, exploiter capacités
- Queries relationnelles essentielles (ex: "tous workflows agent X")

**Entités Principales** :

**workflow**
- Identité : id, name, status
- Relations : → agent (créateur), → messages, → tasks, → validations
- Temporal : created_at, updated_at, completed_at

**agent_state**
- Identité : agent_id, lifecycle (permanent/temporary)
- Relations : → workflows créés, → tools utilisés
- Persistence : configuration, metrics

**memory**
- Identité : id, type (user_pref, context, knowledge)
- Vectoriel : embedding, dimensions
- Relations : → workflow source, → agent créateur
- Metadata : tags, priority, timestamp

**message**
- Identité : id, role (user/assistant/system)
- Relations : → workflow, → agent
- Content : text, tokens, reasoning_steps

**validation_request**
- Identité : id, type (tool, sub_agent, mcp, file_op, db_op)
- Relations : → workflow, → agent
- Decision : approved/rejected, user_id, timestamp

**task**
- Identité : id, name, status (pending, in_progress, completed)
- Relations : → workflow, → agent_assigned
- Tracking : priority, duration, dependencies

**Relations Graph** :
- workflow ↔ agent (many-to-one)
- workflow → messages (one-to-many)
- workflow → tasks (one-to-many)
- workflow → validations (one-to-many)
- agent → memory (one-to-many)
- memory ↔ memory (many-to-many, relations sémantiques)

> **Note Implémentation** : Le schéma final contient 18 tables incluant les entités ci-dessus plus : `agent`, `tool_execution`, `thinking_step`, `sub_agent_execution`, `mcp_server`, `mcp_call_log`, `llm_model`, `provider_settings`, `prompt`, `settings`. Voir `docs/DATABASE_SCHEMA.md` pour le schéma complet.

**Pas de schéma rigide** :
- SurrealDB = schemaless possible
- Mais DEFINE TABLE/FIELD pour validation données critiques

---

### Question 5 : Workflow Versioning

**Décision** : **Audit trail simplifié, pas versioning complet**

**Raisons** :
- Audit trail = suffisant pour debug et compliance
- Versioning complet = complexité excessive pour v1
- Focus : traçabilité actions, pas rollback workflows

**Audit Trail Contient** :
- Tous state changes workflow (idle → running → completed)
- Toutes validations user (approved/rejected avec timestamp)
- Tous tool calls (params, duration, success/error)
- Tous MCP server calls (latency, résultats)

**Pas Versioning Complet** :
- Pas snapshots état intermédiaires
- Pas rollback automatique
- Pas diff entre versions

**Suffisant pour** :
- Debugging (reproduire problème)
- Compliance (traçabilité décisions)
- Analytics (patterns usage)

---

### Question 6 : Retention Policy

**Décision** : **Retention différenciée par type**

**Workflows** :
- **Completed** : 90 jours → archivage (JSON export)
- **Error** : 180 jours (debug long terme)
- **Running** : Pas suppression auto (gestion manuelle)

**Logs** :
- **Application logs** : 30 jours
- **Audit logs** : 1 an (compliance)
- **Metrics** : 90 jours (agrégation mensuelle après)

**Memory** :
- **Temporary** (workflow-specific) : Suppression avec workflow
- **Permanent** (user preferences, knowledge) : Pas expiration
- **Pruning** : Manuel ou basé sur score pertinence

**Reports Agents** :
- **Recent** : 30 jours en DB
- **Archived** : Export filesystem, compression
- **Cleanup** : Automatique tâche schedulée

---

## 3. Security & Operations

### Question 7 : Security Level

**Décision** : **Production-ready dès v1**

**Raisons** :
- Application manipule données utilisateur sensibles
- Appels LLM avec API keys = surface attaque
- Desktop app = accès filesystem/system

**Mesures Production-Ready** :

**Secrets Management**
- API keys jamais en clair (encrypted storage)
- Rotation keys supportée (pas automatique v1)
- Variables env pour dev/staging

**Input Validation**
- Tous inputs utilisateur validés (types, ranges)
- Sanitization avant DB/LLM
- Protection injection (SQL, prompt injection)

**Process Isolation**
- MCP servers externes : Docker containers
- Sandboxing tools sensibles (file operations)
- Permissions minimales par agent

**Audit & Monitoring**
- Logging toutes opérations sensibles
- Anomaly detection basique (rate abuse)
- User notifications opérations critiques

**Pas Overkill** :
- Pas pentesting externe v1
- Pas certification SOC2 v1
- Pas multi-factor auth (desktop local)

---

### Question 8 : API Keys Storage

**Décision** : **Tauri secure storage + encryption**

**Raisons** :
- Tauri fournit API secure storage natif OS
- Encryption additionnelle = defense in depth
- Pas besoin vault externe (overhead)

**Stratégie** :

**Stockage** :
- MacOS : Keychain
- Windows : Credential Manager
- Linux : Secret Service (libsecret)

**Encryption Additionnelle** :
- Keys encryptées avant storage (AES-256)
- Clé encryption dérivée de machine ID + user
- Jamais keys en plaintext memory longtemps

**Accès** :
- Lecture keys uniquement au moment call LLM
- Cache temporaire encrypted (durée session)
- Cleanup automatique fin application

**Suffisant v1** :
- Pas HSM (Hardware Security Module)
- Pas rotation automatique
- Pas multi-user (desktop = single user)

---

### Question 9 : Logging Framework

**Décision** : **tracing (Tokio ecosystem)**

**Raisons** :
- Async-first (compatible Tokio natif)
- Structured logging (JSON)
- Distributed tracing (spans pour multi-agents)
- Écosystème mature Rust

**Capacités Nécessaires** :

**Structured Logs** :
- JSON format pour agrégation
- Fields contextuels (agent_id, workflow_id, tool_name)
- Corrélation events multi-agents

**Levels** :
- ERROR : Failures bloquants
- WARN : Problèmes non-bloquants (fallback utilisé)
- INFO : Opérations importantes (workflow start/end)
- DEBUG : Détails internes (dev seulement)
- TRACE : Verbeux extrême (disabled prod)

**Spans** :
- Trace workflow complet (start → end)
- Sub-spans par agent execution
- Sub-spans par tool call
- Latency automatique par span

**Output** :
- Dev : Console formaté (human-readable)
- Prod : Fichiers rotatifs + JSON
- Future : Possible export metrics endpoint

**Pas Alternatives** :
- `log` : Pas structured, pas spans
- `env_logger` : Trop simple, pas async-first
- `slog` : Moins écosystème Tokio

---

### Question 10 : Error Handling

**Décision** : **anyhow + thiserror combinés**

**Raisons** :
- Combinaison = best of both worlds
- anyhow : Propagation simple, context chaining
- thiserror : Custom errors typés publics

**Usage** :

**anyhow** :
- Applications code (main, commands)
- Propagation rapide erreurs (?)
- Context ajouté (.context("operation failed"))

**thiserror** :
- Libraries internes (agents, mcp, tools)
- Erreurs typées publiques
- Pattern matching erreurs

**Stratégie Globale** :

**Boundaries** :
- IPC Tauri : Conversion Error → String user-friendly
- LLM calls : Retry strategy (exponential backoff)
- DB operations : Transaction rollback auto
- MCP calls : Timeout + fallback

**User-Facing** :
- Jamais stack traces côté UI
- Messages clairs actions correctives
- Logging détails backend seulement

**Recovery** :
- Graceful degradation (fallback providers)
- Pas panic! en production
- Workflow pause si erreur critique, pas abort

---

## 4. Features Priority

### Question 11 : MCP Servers Priority

**Décision** : **Pas de serveurs MCP pré-intégrés - Configuration utilisateur**

**Raisons** :
- Flexibilité maximale : utilisateur choisit ses servers
- Pas de dépendances bundlées
- Configuration via Settings > MCP
- Support commandes standard (docker, npx, uvx)

**Fonctionnement** :

**Configuration Format** :
```json
{
  "mcpServers": {
    "nom_server": {
      "command": "docker|npx|uvx",
      "args": ["array", "arguments"],
      "env": { "VAR": "valeur" }
    }
  }
}
```

**Interface Settings** :
- Section MCP avec liste servers configurés
- Boutons Add/Edit/Delete/Test
- Templates suggérés (serena, context7, playwright)
- Documentation inline (command nécessite quoi)

**Application Responsabilité** :
- Exécuter command configurée
- Gérer communication (stdio/HTTP selon command)
- Monitoring status (online/offline)
- Logs erreurs

**Pas de Priorité Implémentation** :
- Utilisateur décide quels servers installer
- Pas de servers "par défaut"
- Documentation guide pour servers populaires

### Question 12 : Provider Routing

**Décision** : **User choice avec suggestions intelligentes**

**Raisons** :
- Utilisateur contrôle coûts (providers différents = pricing différent)
- Transparence : user sait quel LLM utilisé
- Suggestions = aide sans imposer

**Providers Phase 1** : Mistral + Ollama
- **Mistral** : Cloud API, performant, coût modéré
- **Ollama** : Local, gratuit, privacy-first

**Fonctionnement** :

**Sélection User** :
- Interface : Dropdown providers configurés
- Par workflow : User choisit provider au démarrage
- Persistance : Dernier choix mémorisé par défaut

**Suggestions Intelligentes** :
- **Task simple** : Suggestion Mistral small (rapide, cheap)
- **Task complexe** : Suggestion Mistral large (reasoning)
- **Code generation** : Suggestion Ollama codellama (local)
- **Privacy prioritaire** : Suggestion Ollama (local, gratuit)

**Pas Auto-Routing Forcé** :
- Respect choix user même si suggestion différente
- Warning si choix non-optimal (ex: Opus pour tâche simple)

**Fallback Automatique** :
- Si provider choisi down : Proposition switch auto
- User approuve ou rejette fallback
- Pas switch silencieux (transparence)

---

### Question 13 : Testing Coverage

**Décision** : **Critical paths prioritaires + minimum coverage**

**Raisons** :
- 100% coverage = impossible/coûteux pour v1
- Focus : chemins critiques = ROI maximum
- Tests E2E prioritaires sur unit tests exhaustifs

**Critical Paths** :

**Workflow Execution** :
- User input → Agent processing → LLM call → Response streaming
- Validation human-in-the-loop (approve/reject)
- Error handling et recovery

**Agent Orchestration** :
- Création workflow multi-agents
- Communication inter-agents (reports)
- State persistence et reload

**Tools Execution** :
- MCP tool calls (success + error cases)
- Database operations (CRUD)
- Memory storage/retrieval vectorielle

**Target Coverage** :
- **Backend Rust** : ~70% critical modules
- **Frontend Svelte** : Tests E2E chemins principaux
- **Integration** : Tous workflows end-to-end

**Pas Testé Exhaustivement** :
- UI composants isolés (tests unitaires tous composants = overkill)
- Edge cases rares (< 1% usage)
- Configuration parsing (validation schéma suffit)

---

### Question 14 : CI/CD Pipeline

**Décision** : **GitHub Actions (si GitHub) ou GitLab CI**

**Raisons** :
- Gratuit pour projets privés/publics
- Intégration native repos
- Templates Rust + Tauri disponibles

**Pipeline Minimum** :

**On Push (branches feature)** :
- Linting (clippy Rust, eslint frontend)
- Unit tests backend
- Build check (compilation success)

**On PR (vers main)** :
- Tests integration
- Security audit (cargo audit)
- Coverage report

**On Merge (main)** :
- Build releases (Linux, macOS, Windows)
- Tests E2E complets
- Packaging artifacts

**Deployment** :
- Pas auto-deploy v1 (release manuelle)
- Artifacts publiés GitHub Releases
- Future : Auto-updates intégré app

---

## 5. Deployment

### Question 15 : OS Cibles

**Décision** : **Linux prioritaire, puis macOS, Windows Phase 2**

**Raisons** :

**Linux First** :
- Platform développement principale
- Testing simplifié
- SurrealDB embedded natif Linux

**macOS Phase 1.5** :
- Build facile (Tauri cross-platform)
- Large user base développeurs
- Testing nécessaire (signature app)

**Windows Phase 2** :
- Complexité packaging (MSI, code signing)
- Testing environnement requis
- Priorité moindre si app dev-focused

**Packaging Formats** :
- Linux : AppImage (universal) + .deb (Debian/Ubuntu)
- macOS : .dmg (distribution standard)
- Windows : .msi (future)

---

### Question 16 : Auto-Updates

**Décision** : **Non nécessaire v1, prévu v1.5**

**Raisons** :

**v1 : Manual Updates** :
- Application jeune = breaking changes fréquents
- User contrôle installation timing
- Pas infrastructure update server nécessaire

**v1.5 : Auto-Updates** :
- Tauri built-in updater (intégré framework)
- Notification user : "Update disponible"
- User choisit install maintenant ou plus tard

**Jamais Silent Updates** :
- Transparence : user approuve updates
- Rollback possible si problème
- Release notes affichées

**Infrastructure** :
- v1 : GitHub Releases (manual download)
- v1.5 : Tauri updater + releases JSON manifest

---

## 6. MCP Configuration (Nouvelles Questions)

### Question 17 : Guidance Deployment MCP (Documentation Utilisateur)

**Décision** : **Documenter approches multiples, laisser choix utilisateur**

**Clarification** :
- Application n'impose pas méthode déploiement
- Utilisateur choisit selon besoins (privacy, coût, performance)
- Application exécute configuration fournie

**Approches Supportées** :

**1. Docker Local**
- **Commande** : `docker run -i --rm image:tag`
- **Avantages** : Privacy (données locales), gratuit, offline
- **Inconvénients** : Nécessite Docker, maintenance images
- **Use Cases** : Serena (code sensible), Playwright (tests contrôlés)

**2. NPX (Node.js)**
- **Commande** : `npx -y @package/mcp-server`
- **Avantages** : Installation simple, peut être local ou SaaS
- **Inconvénients** : Nécessite Node.js
- **Use Cases** : Servers JS/TS, peut pointer vers SaaS si package client

**3. UVX (Python)**
- **Commande** : `uvx mcp-server-package`
- **Avantages** : Isolation environnements Python, rapide
- **Inconvénients** : Nécessite Python + uv
- **Use Cases** : Servers Python (SQLite, data processing)

**4. SaaS Distant**
- **Commande** : Client pointant vers API (via NPX/binaire)
- **Avantages** : Pas maintenance, performance élevée, scaling auto
- **Inconvénients** : Coûts, dépendance réseau, privacy moindre
- **Use Cases** : Context7 (docs publiques), servers externes managés

**Documentation Utilisateur** :

**Templates Settings** :
- Pré-remplis pour servers populaires
- Explications inline (Docker vs NPX vs UVX)
- Trade-offs clairement indiqués

**Guide Installation** :
- Comment installer Docker / Node / Python
- Exemples configurations complètes
- Troubleshooting erreurs communes

**Recommandations par Use Case** :
- **Privacy critique** → Docker local
- **Performance/scaling** → SaaS managé
- **Développement** → NPX/UVX local
- **Production** → Hybride selon sensibilité données

**Application Rôle** :
- Exécuter toute command valide (docker, npx, uvx, binaire custom)
- Pas préférence imposée
- Monitoring status indépendant méthode

---

### Question 18 : Hot-Reload Registry

**Décision** : **Configuration statique au démarrage (v1)**

**Raisons** :
- Hot-reload = complexité technique élevée
- Use case rare (combien fois ajout MCP server ?)
- Restart app acceptable v1

**v1 Behavior** :
- Configuration chargée au startup
- Modification config → Restart app required
- Message user : "Restart to apply changes"

**Future v2** :
- Hot-reload si demande forte users
- Complexité justifiée si usage fréquent

---

### Question 19 : Error Recovery Strategy

**Décision** : **Graceful degradation + User notification**

**Strategy Multi-Level** :

**Level 1 : Retry Automatique**
- Timeout transient (réseau) : 3 retries exponential backoff
- Pas notification user si success après retry

**Level 2 : Fallback**
- MCP server down : Skip tool call, continuer workflow
- Notification user : "Tool X unavailable, continuing without"
- Workflow pas aborted

**Level 3 : User Decision**
- Erreur persistante (3 retries failed) : Pause workflow
- Notification user : "Server X down. Retry / Skip / Abort ?"
- User choisit action

**Jamais Silent Failure** :
- Toujours logging erreur (même si recovered)
- User informé si impact workflow
- Metrics erreurs pour monitoring

---

## Récapitulatif Décisions

### ✅ Architecture
- Structure : From scratch, mono-repo
- MCP : SDK officiel Anthropic

### ✅ Database
- Schéma : Complet avec relations graph
- Versioning : Audit trail simplifié
- Retention : Différenciée par type (30-180 jours)

### ✅ Security
- Level : Production-ready v1
- API Keys : Tauri secure storage + encryption
- Logging : tracing (structured, spans)
- Errors : anyhow + thiserror

### ✅ Features
- MCP : Configuration utilisateur (pas de serveurs pré-intégrés)
- Provider : User choice + suggestions intelligentes (Mistral + Ollama Phase 1)
- Testing : Critical paths (~70% backend)
- CI/CD : GitHub Actions / GitLab CI

### ✅ Deployment
- OS : Linux → macOS → Windows (progressif)
- Updates : Manual v1, auto v1.5

### ✅ MCP Ops
- Deployment : Hybride (Docker local dev, SaaS prod option)
- Hot-reload : Non v1 (restart required)
- Error recovery : Retry → Fallback → User decision

### ✅ Frontend State
- Store pattern : CRUD factory canonique, pure functions acceptable
- Event-driven : Cleanup lifecycle obligatoire
- Runes migration : Différée post-v1

---

## Statut Implémentation (Décembre 2025)

**Phases Complétées** :
1. ✅ Phase 0 : Base implementation (agents, LLM, DB, security, 19 commandes initiales)
2. ✅ Phase 1 : Design System Foundation (theme, 12 composants UI)
3. ✅ Phase 2 : Layout Components (AppContainer, Sidebar, FloatingMenu)
4. ✅ Phase 3 : Chat & Workflow Components
5. ✅ Phase 4 : Pages Refactoring (agent page, settings page)
6. ✅ Phase 5 : Backend Features (validation, memory, streaming - 111 commandes total)
7. ✅ Phase 6 : Strategic Optimizations (MCP, backend, DB)
8. ✅ Phase 7 : Quick Wins Frontend (Vite 7, utilities, modal pattern)

**En Cours** :
- Phase 8 : Strategic Refactoring (settings page decomposition)

**Documentation Technique** :
- Schéma DB : `docs/DATABASE_SCHEMA.md`
- API Reference : `docs/API_REFERENCE.md`
- Stack Technique : `docs/TECH_STACK.md`
- Guide MCP : `docs/MCP_CONFIGURATION_GUIDE.md`

---

## 7. Frontend State Management (Stores)

### Question 20 : Store Pattern Canonique

**Décision** : **Factory CRUD comme pattern canonique pour entités persistées**

**Raisons** :
- Pattern agents.ts/prompts.ts prouvé et maintenable
- Réduction duplication via createCRUDStore factory
- Derived stores pour consommation optimisée
- Intégration Tauri IPC standardisée

**Patterns Reconnus** :

| Pattern | Stores | Use Case | Status |
|---------|--------|----------|--------|
| **CRUD Factory** | agents.ts, prompts.ts | Entités avec CRUD backend | ✅ Canonique |
| **Pure Functions** | llm.ts, mcp.ts | API calls sans state local | ✅ Acceptable |
| **Event-Driven** | streaming.ts, validation.ts, userQuestion.ts | Events Tauri real-time | ✅ Acceptable |
| **Custom Factory** | theme.ts, locale.ts, onboarding.ts | Persistence localStorage | ✅ Acceptable |
| **Hybrid** | workflows.ts (legacy) | Duplication pure + reactive | ⚠️ Deprecated |

**Pattern CRUD Factory (Canonique)** :

```typescript
// src/lib/stores/factory/createCRUDStore.ts
const store = createCRUDStore<TFull, TCreate, TUpdate, TSummary>({
  name: 'entity',
  commands: {
    list: 'list_entities',
    get: 'get_entity',
    create: 'create_entity',
    update: 'update_entity',
    delete: 'delete_entity'
  }
});

// Exports derived stores
export const items = derived(store, $s => $s.items);
export const selected = derived(store, $s => $s.selected);
export const isLoading = derived(store, $s => $s.loading);
```

**Pattern Pure Functions (Acceptable)** :

```typescript
// Utilisé pour: opérations async sans state local nécessaire
// Exemple: llm.ts, mcp.ts

export async function listModels(provider: string): Promise<LLMModel[]> {
  return invoke('list_models', { provider });
}

// Pas de store writable - le composant gère son propre state
```

**Pattern Event-Driven (Acceptable)** :

```typescript
// Utilisé pour: événements Tauri real-time
// Exemple: streaming.ts, validation.ts

const store = writable<State>(initialState);
let unlisteners: UnlistenFn[] = [];

export const streamingStore = {
  subscribe: store.subscribe,

  async init() {
    // IMPORTANT: Cleanup obligatoire pour éviter memory leaks
    if (isInitialized) await this.cleanup();

    unlisteners.push(
      await listen<StreamChunk>('workflow_stream', (event) => {
        store.update(s => processChunk(s, event.payload));
      })
    );
  },

  async cleanup() {
    for (const unlisten of unlisteners) unlisten();
    unlisteners = [];
  }
};
```

**Pattern Deprecated (À Éviter)** :

```typescript
// NE PAS FAIRE: Duplication pure functions + reactive store
// Exemple legacy: workflows.ts avait les deux patterns

// ❌ Deprecated
export async function loadWorkflows(): Promise<Workflow[]> { ... }
export const workflowStore = { loadWorkflows: async () => { ... } };

// ✅ Correct: Un seul pattern par store
```

**Règles** :

1. **Nouvelle entité persistée** → Utiliser `createCRUDStore` factory
2. **API calls pures** → Pure functions (si pas besoin de state réactif)
3. **Events Tauri** → Event-driven avec cleanup obligatoire
4. **Persistence locale** → Custom factory avec localStorage

**Gestion Mémoire (Event-Driven)** :

Les stores event-driven DOIVENT implémenter:
- `init()` avec guard contre double initialisation
- `cleanup()` pour libérer tous les listeners
- `reset()` appelant cleanup + reinitialisation state

```typescript
// Pattern obligatoire pour stores avec listeners
let isInitialized = false;

async init() {
  if (isInitialized) {
    console.warn('Store already initialized');
    await this.cleanup();
  }
  isInitialized = true;
  // ... setup listeners
}

async cleanup() {
  for (const unlisten of unlisteners) unlisten();
  unlisteners = [];
  isInitialized = false;
}
```

---

### Question 21 : Migration Svelte 5 Runes

**Décision** : **Différée - Migration post-v1 si bénéfice prouvé**

**Raisons** :
- Stores Svelte 4 actuels fonctionnent avec Svelte 5
- Migration = effort 8-16h sans gain fonctionnel immédiat
- Runes = optimisation performance (2-3x) mais overhead migration
- Priorité v1 = stabilité, pas refactoring

**Stratégie Migration Future** :
1. Bottom-up : composants feuilles d'abord
2. Un store à la fois
3. Tests avant/après chaque migration
4. Documentation patterns runes

**Coexistence** :
- Stores writable/derived coexistent avec runes
- Pas de breaking change Svelte 5 sur stores
- Migration graduelle possible

---

### Question 22 : Utility Factories (Phase 7 Quick Win)

**Décision** : **Factories pour patterns répétitifs (modal, async handlers)**

**Raisons** :
- Réduction duplication code (~30 lignes par modal)
- Patterns cohérents dans toute l'application
- Svelte 5 runes pour state réactif hors composants

**Modal Controller Factory** :

```typescript
// src/lib/utils/modal.svelte.ts
// Utilise .svelte.ts pour accès aux runes ($state) hors composants

const mcpModal = createModalController<MCPServerConfig>();

// API
mcpModal.show       // boolean - modal visible
mcpModal.mode       // 'create' | 'edit'
mcpModal.editing    // T | undefined - item en cours d'édition
mcpModal.openCreate()   // Ouvre en mode création
mcpModal.openEdit(item) // Ouvre en mode édition
mcpModal.close()        // Ferme et reset
```

**Async Handler Factory** :

```typescript
// src/lib/utils/async.ts
// Élimine pattern try/catch/finally répétitif

const handleSave = createAsyncHandler(
  () => invoke('save_data', { data }),
  {
    setLoading: (l) => saving = l,
    onSuccess: (result) => { /* ... */ },
    onError: (error) => { /* ... */ }
  }
);
```

**Usage dans Settings** :
- `mcpModal` : Modal création/édition serveurs MCP
- `modelModal` : Modal création/édition modèles LLM
- Réduit ~60 lignes de boilerplate dans +page.svelte

---
