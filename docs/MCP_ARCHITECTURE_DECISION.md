# Décision Architecture MCP

> **Date** : 2025-11-23
> **Décision** : SDK Officiel Anthropic (`modelcontextprotocol/rust-sdk`)
> **Niveau** : Architecture conceptuelle (pas d'implémentation)

## Choix SDK : Officiel Anthropic

### Raison Principale
Conformité stricte avec spécification MCP officielle pour stabilité long terme et compatibilité garantie avec évolutions du protocole.

### Avantages Projet Zileo Chat
- **Maintenance** : Support Anthropic direct
- **Évolutions** : Mises à jour spec automatiques
- **Standards** : Base solide pour architecture complexe multi-agents
- **Fiabilité** : Production-ready sans surcouche communauté

### Trade-offs Acceptés
- Pas d'inspector web intégré → Solution : logging structuré + observability custom
- Tooling minimal → Solution : développement outils internes si besoin

---

## Rôle MCP dans Architecture

### Vue d'Ensemble

```
┌─────────────────────────────────────────────────┐
│           Frontend (SvelteKit)                  │
│  - Interface utilisateur workflows              │
│  - Affichage agents/tools status                │
└────────────────┬────────────────────────────────┘
                 │ Tauri IPC
                 ↓
┌─────────────────────────────────────────────────┐
│         Backend Rust (Tauri)                    │
│  ┌──────────────────────────────────────────┐  │
│  │  Agent Orchestrator                      │  │
│  │  - Gère workflows multi-agents           │  │
│  │  - Délègue tâches aux agents spécialisés│  │
│  └──────┬───────────────────────────────────┘  │
│         │                                        │
│  ┌──────▼───────────────────────────────────┐  │
│  │  MCP Client Layer (SDK Officiel)        │  │
│  │  - Communication serveurs MCP externes   │  │
│  │  - Transport stdio/HTTP/WebSocket        │  │
│  └──────┬───────────────────────────────────┘  │
│         │                                        │
│  ┌──────▼───────────────────────────────────┐  │
│  │  MCP Server Layer (SDK Officiel)        │  │
│  │  - Expose tools custom (DB, API, RAG)   │  │
│  │  - Expose resources (contexte, mémoire) │  │
│  └──────────────────────────────────────────┘  │
└─────────────────────────────────────────────────┘
          │                        │
          │                        ↓
          │              ┌─────────────────────┐
          │              │   SurrealDB         │
          │              │   - Workflows       │
          │              │   - Agent states    │
          │              │   - Memory vectors  │
          │              └─────────────────────┘
          ↓
┌─────────────────────────────────────────────────┐
│        MCP Servers Externes                     │
│  - serena (semantic code analysis)              │
│  - context7 (library documentation)             │
│  - playwright (browser automation)              │
│  - sequential-thinking (reasoning)              │
└─────────────────────────────────────────────────┘
```

---

## Double Rôle MCP

### 1. MCP Client (Consommateur)

**Objectif** : Appeler MCP servers externes pour capacités étendues

**Servers Externes Identifiés**
- **serena** : Analyse sémantique codebase, symbol operations
- **context7** : Documentation officielle bibliothèques (React, SurrealDB, etc.)
- **playwright** : Automation browser, tests E2E, validation UI
- **sequential-thinking** : Raisonnement multi-étapes structuré

**Fonctionnement Conceptuel**
1. Agent détermine besoin capability externe (ex: recherche code)
2. MCP Client route requête vers server approprié (serena)
3. Server traite requête, retourne résultat
4. Agent intègre résultat dans workflow

**Transport**
- **stdio** : Servers locaux (si déployés localement)
- **HTTP/SSE** : Servers cloud/distants
- **Configuration** : Définie par server (npx, uvx, docker)

### 2. MCP Server (Fournisseur)

**Objectif** : Exposer capabilities internes comme tools MCP pour LLMs

**Tools Custom à Exposer**

**Database Tools**
- `query_surrealdb` : Requêtes SurrealQL structurées
- `store_memory` : Persistance mémoire vectorielle
- `fetch_workflow_state` : Récupération état workflow

**Business Logic Tools**
- `validate_user_input` : Validation données métier
- `calculate_metrics` : Métriques workflow (tokens, coût, durée)
- `check_permissions` : Vérification droits agents

**Integration Tools**
- `call_external_api` : Wrapper APIs externes sécurisé
- `send_notification` : Notifications utilisateur (Tauri events)

**Resources à Exposer**
- **Contexte projet** : Configuration agents, prompts library
- **Mémoire sessions** : Historique conversations, préférences utilisateur
- **Documentation interne** : Guides agents, schemas DB

**Fonctionnement Conceptuel**
1. LLM (via agent) découvre tools disponibles (`list_tools`)
2. LLM décide d'utiliser tool (ex: `query_surrealdb`)
3. MCP Server valide requête, execute operation
4. Résultat retourné au LLM → workflow continue

---

## Communication Protocol

### JSON-RPC 2.0 (Obligatoire)

**Messages Standard**
- **Request** : `{ method, params, id }`
- **Response** : `{ result, id }` ou `{ error, id }`
- **Notification** : `{ method, params }` (sans id, pas de réponse)

**Methods Core**
- `initialize` : Handshake client/server
- `tools/list` : Liste tools disponibles
- `tools/call` : Exécution tool
- `resources/list` : Liste resources disponibles
- `resources/read` : Lecture resource

### Streaming (SSE)

**Pour LLM Responses**
- Tokens streamés en temps réel
- Events progressifs : `token`, `tool_start`, `tool_end`, `reasoning_step`
- Frontend update UI incrémental

**Pas pour MCP Protocol**
- MCP = requête/réponse (JSON-RPC)
- Streaming géré par layer LLM (Rig.rs), pas MCP

---

## Intégration avec Rig.rs

### Layer Séparation

```
┌─────────────────────────────────────────┐
│  Rig.rs (LLM Abstraction)              │
│  - Multi-provider (Claude, GPT, etc.)  │
│  - Streaming responses                  │
│  - Prompt management                    │
└────────────┬────────────────────────────┘
             │ Function Calling
             ↓
┌─────────────────────────────────────────┐
│  MCP Client (SDK Officiel)             │
│  - Translate function → MCP tool call  │
│  - Handle responses                     │
└─────────────────────────────────────────┘
```

**Pont Conceptuel**
1. **Rig.rs** : Gère conversation avec LLM, streaming, paramètres
2. **LLM** : Demande appel fonction (ex: "query database")
3. **MCP Client** : Exécute tool MCP correspondant
4. **Résultat** : Retour à Rig.rs → LLM → génération réponse

### Pas de Duplication

**Rig.rs gère** :
- Sélection provider LLM
- Paramètres inférence (temperature, tokens)
- Context window management
- Prompt templates

**MCP gère** :
- Tools discovery et execution
- Resources access
- Communication standardisée servers externes

**Pas de conflit** : Layers complémentaires

---

## Configuration Architecture

### MCP Servers Registry

**Concept** : Registry centralisé pour tous MCP servers (clients + serveur interne)

**Servers Externes** (MCP Client)
```
{
  "serena": {
    "type": "external",
    "transport": "stdio",
    "command": "docker",
    "args": ["run", "-i", "--rm", "serena-mcp"],
    "capabilities": ["code_search", "symbol_ops"]
  },
  "context7": {
    "type": "external",
    "transport": "http",
    "endpoint": "https://api.context7.io",
    "auth": "API_KEY_ENV",
    "capabilities": ["library_docs"]
  }
}
```

**Server Interne** (MCP Server)
```
{
  "zileo_tools": {
    "type": "internal",
    "transport": "stdio",
    "tools": [
      "query_surrealdb",
      "store_memory",
      "fetch_workflow_state"
    ],
    "resources": [
      "project_config",
      "session_memory",
      "prompts_library"
    ]
  }
}
```

### Dynamic Discovery

**Au démarrage application** :
1. Load MCP servers registry configuration
2. Initialize MCP Client pour servers externes
3. Start MCP Server interne (expose tools)
4. Agents découvrent tools disponibles via `list_tools`

**Pendant runtime** :
- Agents appellent tools selon besoins
- MCP Client/Server gèrent routing automatique
- Pas de hardcoding tool names dans agents

---

## Security & Isolation

### Validation Layer

**Avant execution tool** :
1. **Schema validation** : Params conformes au schema tool
2. **Permission check** : Agent autorisé utiliser ce tool ?
3. **Rate limiting** : Pas d'abus calls (DDoS interne)
4. **Input sanitization** : Prévention injections

### Sandboxing

**Tools sensibles** :
- `query_surrealdb` : Requêtes READ-ONLY par défaut
- `call_external_api` : Whitelist domaines autorisés
- Pas DELETE sans validation humaine (human-in-the-loop)

**Isolation processes** :
- MCP servers externes : Docker containers (pas accès host)
- Server interne : Scope limité aux operations autorisées

---

## Observability

### Logging Structuré

**Chaque call MCP logué** :
- Timestamp
- Tool/resource appelé
- Agent source
- Params (sanitized si secrets)
- Duration
- Success/error

**Format** : JSON structuré → agrégation facile

### Metrics

**Par MCP server** :
- Nombre calls
- Latency moyenne (P50, P95, P99)
- Taux erreurs
- Tools les plus utilisés

**Global** :
- MCP vs direct tools ratio
- Impact performance MCP layer

---

## Évolutivité

### Ajout Nouveau Server Externe

**Process** :
1. Ajouter config dans registry
2. Redémarrer application (ou hot-reload si implémenté)
3. Agents découvrent automatiquement nouveaux tools
4. Pas de code agent à modifier

### Ajout Nouveau Tool Interne

**Process** :
1. Implémenter tool avec signature MCP
2. Enregistrer dans server interne registry
3. Schema tool auto-exposé via `list_tools`
4. LLMs peuvent l'utiliser immédiatement

---

## Dépendances Rust

### Cargo.toml (Conceptuel)

```toml
[dependencies]
# MCP SDK Officiel
mcp-rust-sdk = "version_à_determiner"

# LLM Framework
rig-core = "0.24.0"

# Communication
tokio = { version = "1.48.0", features = ["full"] }
serde = { version = "1.0.228", features = ["derive"] }
serde_json = "1.0.145"

# Database
surrealdb = { version = "2.3.10", features = ["kv-rocksdb"] }

# Tauri
tauri = { version = "2.9.4", features = ["protocol-asset"] }
```

**Note** : Version exacte `mcp-rust-sdk` à vérifier sur crates.io lors phase développement

---

## Questions Ouvertes

### Configuration Servers Externes

**serena, context7, playwright, sequential-thinking** :
1. Déployés localement (Docker) ou SaaS distant ?
2. Si local : images Docker pré-configurées ou build custom ?
3. Si distant : API keys où stocker (Tauri secure storage) ?

### Hot-Reload Registry

1. Support ajout/retrait server sans restart app ?
2. Ou configuration statique au démarrage suffit ?

### Error Recovery

1. Si MCP server externe down : fallback strategy ?
   - Skip tool call et continuer workflow ?
   - Retry avec exponential backoff ?
   - Fail workflow et notifier utilisateur ?

### Performance Monitoring

1. Threshold latency acceptable par tool call ?
   - <100ms : optimal
   - 100-500ms : acceptable
   - >500ms : warning
   - >2s : error

---

## Prochaines Étapes (Phase Plan Développement)

**Après validation architecture conceptuelle** :
1. Définir schemas tools exacts (params, return types)
2. Détailler error handling strategy
3. Spécifier configuration format (TOML, JSON)
4. Designer logging/monitoring structure
5. Prototyper intégration Rig.rs ↔ MCP

**Pas maintenant** : Code implémentation (sera dans plan développement)
