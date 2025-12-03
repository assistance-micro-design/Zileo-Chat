# Tools Reference - Zileo-Chat-3

Documentation complete des tools disponibles pour les agents dans Zileo-Chat-3.

## Table des matieres

- [Architecture](#architecture)
- [MemoryTool](#1-memorytool)
- [TodoTool](#2-todotool)
- [SpawnAgentTool](#3-spawnagentool)
- [DelegateTaskTool](#4-delegatetasktool)
- [ParallelTasksTool](#5-paralleltaskstool)
- [Fichiers source](#fichiers-source)

---

## Architecture

### Hierarchie des Tools

```
ToolFactory
├── Basic Tools (tous agents)
│   ├── MemoryTool      - Memoire persistante avec recherche semantique
│   └── TodoTool        - Gestion des taches de workflow
│
└── Sub-Agent Tools (Primary Agent uniquement)
    ├── SpawnAgentTool      - Creation de sous-agents temporaires
    ├── DelegateTaskTool    - Delegation a agents permanents
    └── ParallelTasksTool   - Execution parallele multi-agents
```

### Trait Tool

Tous les tools implementent le trait `Tool` (`src-tauri/src/tools/mod.rs`):

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, input: Value) -> ToolResult<Value>;
    fn validate_input(&self, input: &Value) -> ToolResult<()>;
    fn requires_confirmation(&self) -> bool;
}
```

### Creation via ToolFactory

```rust
// Basic tools
let tool = factory.create_tool("MemoryTool", Some(workflow_id), agent_id, app_handle)?;

// Sub-agent tools (avec contexte)
let tool = factory.create_tool_with_context(
    "SpawnAgentTool",
    Some(workflow_id),
    agent_id,
    context,
    is_primary_agent
)?;
```

---

## 1. MemoryTool

**Fichier**: `src-tauri/src/tools/memory/tool.rs`

Gestion de la memoire persistante avec recherche semantique via embeddings vectoriels (HNSW index).

### Cas d'usage

- Stocker des informations importantes pour reference future
- Rechercher des memoires par similarite semantique
- Maintenir le contexte entre conversations
- Organiser les connaissances par type

### Modes de fonctionnement

| Mode | Description |
|------|-------------|
| **Workflow** | Memoires scopees a un workflow specifique |
| **General** | Memoires accessibles cross-workflow |

### Types de memoire

| Type | Description | Exemple |
|------|-------------|---------|
| `user_pref` | Preferences utilisateur | "L'utilisateur prefere les reponses concises" |
| `context` | Informations contextuelles | "Projet actuel: migration DB vers SurrealDB" |
| `knowledge` | Faits et expertise domaine | "SurrealDB supporte les index HNSW vectoriels" |
| `decision` | Rationale des decisions | "Choisi Mistral car meilleur rapport qualite/prix" |

### Operations

#### `activate_workflow` - Activer le scope workflow

Active l'isolation des memoires pour un workflow specifique.

```json
{
  "operation": "activate_workflow",
  "workflow_id": "wf_abc123"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "Workflow scope activated",
  "scope": "workflow",
  "workflow_id": "wf_abc123"
}
```

---

#### `activate_general` - Mode general

Bascule vers le mode general (acces cross-workflow).

```json
{
  "operation": "activate_general"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "General mode activated",
  "scope": "general"
}
```

---

#### `add` - Ajouter une memoire

Stocke une nouvelle memoire avec generation automatique d'embedding.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `type` | string | Oui | Type de memoire (`user_pref`, `context`, `knowledge`, `decision`) |
| `content` | string | Oui | Contenu de la memoire (max 50000 chars) |
| `metadata` | object | Non | Metadata additionnelle JSON |
| `tags` | array | Non | Tags de classification |

**Exemple minimal:**
```json
{
  "operation": "add",
  "type": "knowledge",
  "content": "SurrealDB supports HNSW vector indexing for semantic search"
}
```

**Exemple complet avec optionnels:**
```json
{
  "operation": "add",
  "type": "knowledge",
  "content": "SurrealDB supports HNSW vector indexing for semantic search with configurable distance metrics (cosine, euclidean, manhattan)",
  "metadata": {
    "source": "official_docs",
    "version": "2.0",
    "confidence": 0.95,
    "last_verified": "2024-01-15"
  },
  "tags": ["database", "vector", "surrealdb", "indexing"]
}
```

**Reponse:**
```json
{
  "success": true,
  "memory_id": "mem_xyz789",
  "message": "Memory stored successfully",
  "embedding_generated": true
}
```

---

#### `get` - Recuperer une memoire

Recupere une memoire specifique par son ID.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `memory_id` | string | Oui | ID de la memoire |

```json
{
  "operation": "get",
  "memory_id": "mem_xyz789"
}
```

**Reponse:**
```json
{
  "success": true,
  "memory": {
    "id": "mem_xyz789",
    "type": "knowledge",
    "content": "SurrealDB supports HNSW vector indexing...",
    "metadata": {"source": "official_docs"},
    "tags": ["database", "vector"],
    "created_at": "2024-01-15T10:30:00Z",
    "agent_id": "primary_agent"
  }
}
```

---

#### `list` - Lister les memoires

Liste les memoires avec filtrage optionnel.

**Parametres:**

| Parametre | Type | Requis | Default | Description |
|-----------|------|--------|---------|-------------|
| `type_filter` | string | Non | - | Filtrer par type |
| `limit` | integer | Non | 10 | Nombre max de resultats (max 100) |

**Exemple minimal:**
```json
{
  "operation": "list"
}
```

**Exemple avec filtres:**
```json
{
  "operation": "list",
  "type_filter": "knowledge",
  "limit": 25
}
```

**Reponse:**
```json
{
  "success": true,
  "memories": [
    {
      "id": "mem_xyz789",
      "type": "knowledge",
      "content": "SurrealDB supports HNSW...",
      "created_at": "2024-01-15T10:30:00Z"
    }
  ],
  "count": 1
}
```

---

#### `search` - Recherche semantique

Recherche par similarite vectorielle.

**Parametres:**

| Parametre | Type | Requis | Default | Description |
|-----------|------|--------|---------|-------------|
| `query` | string | Oui | - | Requete de recherche |
| `limit` | integer | Non | 10 | Nombre max de resultats (max 100) |
| `type_filter` | string | Non | - | Filtrer par type |
| `threshold` | number | Non | 0.7 | Seuil de similarite (0-1) |

**Exemple minimal:**
```json
{
  "operation": "search",
  "query": "vector database indexing"
}
```

**Exemple complet:**
```json
{
  "operation": "search",
  "query": "how to implement semantic search with embeddings",
  "limit": 15,
  "type_filter": "knowledge",
  "threshold": 0.6
}
```

**Reponse:**
```json
{
  "success": true,
  "results": [
    {
      "memory": {
        "id": "mem_xyz789",
        "type": "knowledge",
        "content": "SurrealDB supports HNSW vector indexing..."
      },
      "similarity": 0.89
    }
  ],
  "count": 1,
  "search_type": "vector"
}
```

---

#### `delete` - Supprimer une memoire

Supprime une memoire par ID.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `memory_id` | string | Oui | ID de la memoire |

```json
{
  "operation": "delete",
  "memory_id": "mem_xyz789"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "Memory deleted successfully"
}
```

---

#### `clear_by_type` - Supprimer par type

Supprime toutes les memoires d'un type specifique.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `type` | string | Oui | Type de memoire a supprimer |

```json
{
  "operation": "clear_by_type",
  "type": "context"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "Cleared all memories of type 'context'",
  "count": 15
}
```

---

## 2. TodoTool

**Fichier**: `src-tauri/src/tools/todo/tool.rs`

Gestion des taches pour le suivi structure de l'execution des workflows.

### Cas d'usage

- Decomposer le travail complexe en taches trackables
- Mettre a jour la progression du travail
- Coordonner avec d'autres agents via assignation
- Tracker les completions avec metriques de timing

### Statuts des taches

| Statut | Description |
|--------|-------------|
| `pending` | Tache en attente, pas encore commencee |
| `in_progress` | Tache en cours d'execution |
| `completed` | Tache terminee avec succes |
| `blocked` | Tache bloquee par une dependance |

### Priorites

| Niveau | Description |
|--------|-------------|
| 1 | Critique / Bloquant |
| 2 | Haute priorite |
| 3 | Normale (defaut) |
| 4 | Basse priorite |
| 5 | Nice-to-have |

### Operations

#### `create` - Creer une tache

Cree une nouvelle tache pour le workflow.

**Parametres:**

| Parametre | Type | Requis | Default | Description |
|-----------|------|--------|---------|-------------|
| `name` | string | Oui | - | Nom de la tache (max 128 chars) |
| `description` | string | Non | "" | Description detaillee (max 1000 chars) |
| `priority` | integer | Non | 3 | Priorite 1-5 |
| `dependencies` | array | Non | [] | IDs des taches dependantes |

**Exemple minimal:**
```json
{
  "operation": "create",
  "name": "Analyze database schema"
}
```

**Exemple complet:**
```json
{
  "operation": "create",
  "name": "Implement user authentication",
  "description": "Add JWT-based authentication with refresh tokens. Include login, logout, and token refresh endpoints.",
  "priority": 1,
  "dependencies": ["task_db_setup", "task_user_model"]
}
```

**Reponse:**
```json
{
  "success": true,
  "task_id": "task_abc123",
  "message": "Task created successfully",
  "task": {
    "id": "task_abc123",
    "name": "Implement user authentication",
    "status": "pending",
    "priority": 1
  }
}
```

---

#### `get` - Recuperer une tache

Recupere les details d'une tache.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `task_id` | string | Oui | ID de la tache |

```json
{
  "operation": "get",
  "task_id": "task_abc123"
}
```

**Reponse:**
```json
{
  "success": true,
  "task": {
    "id": "task_abc123",
    "name": "Implement user authentication",
    "description": "Add JWT-based authentication...",
    "status": "in_progress",
    "priority": 1,
    "dependencies": ["task_db_setup"],
    "created_at": "2024-01-15T10:00:00Z",
    "started_at": "2024-01-15T10:30:00Z"
  }
}
```

---

#### `update_status` - Mettre a jour le statut

Change le statut d'une tache.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `task_id` | string | Oui | ID de la tache |
| `status` | string | Oui | Nouveau statut |

```json
{
  "operation": "update_status",
  "task_id": "task_abc123",
  "status": "in_progress"
}
```

**Reponse:**
```json
{
  "success": true,
  "task_id": "task_abc123",
  "new_status": "in_progress",
  "message": "Status updated successfully"
}
```

---

#### `list` - Lister les taches

Liste les taches du workflow avec filtrage optionnel.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `status_filter` | string | Non | Filtrer par statut |

**Exemple minimal:**
```json
{
  "operation": "list"
}
```

**Exemple avec filtre:**
```json
{
  "operation": "list",
  "status_filter": "pending"
}
```

**Reponse:**
```json
{
  "success": true,
  "tasks": [
    {
      "id": "task_abc123",
      "name": "Implement user authentication",
      "status": "in_progress",
      "priority": 1
    },
    {
      "id": "task_def456",
      "name": "Write unit tests",
      "status": "pending",
      "priority": 2
    }
  ],
  "count": 2
}
```

---

#### `complete` - Marquer comme terminee

Marque une tache comme completee avec metriques optionnelles.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `task_id` | string | Oui | ID de la tache |
| `duration_ms` | integer | Non | Duree d'execution en millisecondes |

**Exemple minimal:**
```json
{
  "operation": "complete",
  "task_id": "task_abc123"
}
```

**Exemple avec metriques:**
```json
{
  "operation": "complete",
  "task_id": "task_abc123",
  "duration_ms": 45000
}
```

**Reponse:**
```json
{
  "success": true,
  "task_id": "task_abc123",
  "new_status": "completed",
  "duration_ms": 45000,
  "message": "Task completed successfully"
}
```

---

#### `delete` - Supprimer une tache

Supprime une tache du workflow.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `task_id` | string | Oui | ID de la tache |

```json
{
  "operation": "delete",
  "task_id": "task_abc123"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "Task deleted successfully"
}
```

---

## 3. SpawnAgentTool

**Fichier**: `src-tauri/src/tools/spawn_agent.rs`

Creation et execution de sous-agents temporaires pour paralleliser le travail.

### Contraintes importantes

| Contrainte | Valeur |
|------------|--------|
| Max sous-agents par workflow | 3 |
| Niveaux de profondeur | 1 (pas de sous-sous-agents) |
| Disponibilite | Primary Agent uniquement |
| Contexte partage | Aucun (prompt = seule entree) |
| Nettoyage | Automatique apres execution |

### Tools disponibles pour sous-agents

Les sous-agents peuvent uniquement utiliser les **basic tools**:
- `MemoryTool`
- `TodoTool`

Les sub-agent tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool) sont **interdits** pour les sous-agents.

### Pattern de communication

```
Primary Agent                    Sub-Agent
     │                               │
     │──── prompt (complet) ────────>│
     │                               │
     │<─── rapport markdown ─────────│
     │     + metriques               │
```

### Operations

#### `spawn` - Creer un sous-agent

Cree et execute un sous-agent temporaire.

**Parametres:**

| Parametre | Type | Requis | Default | Description |
|-----------|------|--------|---------|-------------|
| `name` | string | Oui | - | Nom du sous-agent |
| `prompt` | string | Oui | - | Prompt COMPLET (seule entree du sous-agent) |
| `system_prompt` | string | Non | Default | System prompt personnalise |
| `tools` | array | Non | Parent's tools | Tools a activer |
| `mcp_servers` | array | Non | Parent's servers | Serveurs MCP a utiliser |
| `provider` | string | Non | Parent's provider | Provider LLM |
| `model` | string | Non | Parent's model | Modele LLM |

**Exemple minimal:**
```json
{
  "operation": "spawn",
  "name": "CodeAnalyzer",
  "prompt": "Analyze the authentication module for security vulnerabilities. Return a markdown report."
}
```

**Exemple complet:**
```json
{
  "operation": "spawn",
  "name": "SecurityAuditor",
  "prompt": "## Task: Security Audit of Database Module\n\n### Context\nWe are auditing the SurrealDB integration layer in src-tauri/src/db/.\n\n### Focus Areas\n1. SQL injection vulnerabilities\n2. Input validation\n3. Access control patterns\n4. Error handling (no sensitive data leakage)\n\n### Expected Output\nProvide a markdown report with:\n1. **Executive Summary** (2-3 sentences)\n2. **Critical Issues** (severity: critical/high/medium/low)\n3. **Code Snippets** showing problematic patterns\n4. **Recommended Fixes** with example code\n5. **Metrics**: files analyzed, issues found by severity",
  "system_prompt": "You are an expert security auditor specializing in Rust backend applications. Focus on OWASP Top 10 vulnerabilities and Rust-specific security patterns.",
  "tools": ["MemoryTool"],
  "mcp_servers": ["serena"],
  "provider": "Mistral",
  "model": "mistral-large-latest"
}
```

**Reponse:**
```json
{
  "success": true,
  "child_id": "sub_xyz789",
  "report": "## Security Audit Report\n\n### Executive Summary\n...",
  "metrics": {
    "duration_ms": 12500,
    "tokens_input": 1250,
    "tokens_output": 3400
  }
}
```

---

#### `list_children` - Lister les sous-agents

Liste les sous-agents actifs et les slots restants.

```json
{
  "operation": "list_children"
}
```

**Reponse:**
```json
{
  "success": true,
  "children": [
    {
      "id": "sub_xyz789",
      "name": "SecurityAuditor",
      "status": "running",
      "started_at": "2024-01-15T10:30:00Z"
    }
  ],
  "count": 1,
  "remaining_slots": 2
}
```

---

#### `terminate` - Arreter un sous-agent

Force l'arret d'un sous-agent en cours.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `child_id` | string | Oui | ID du sous-agent |

```json
{
  "operation": "terminate",
  "child_id": "sub_xyz789"
}
```

**Reponse:**
```json
{
  "success": true,
  "message": "Sub-agent terminated successfully"
}
```

---

## 4. DelegateTaskTool

**Fichier**: `src-tauri/src/tools/delegate_task.rs`

Delegation de taches a des agents permanents existants.

### Contraintes importantes

| Contrainte | Valeur |
|------------|--------|
| Max operations par workflow | 3 (partage avec spawn) |
| Type d'agents cibles | Permanents uniquement |
| Disponibilite | Primary Agent uniquement |
| Contexte partage | Aucun (prompt = seule entree) |

### Difference avec SpawnAgentTool

| Aspect | DelegateTaskTool | SpawnAgentTool |
|--------|------------------|----------------|
| Agent cible | Existant (permanent) | Cree (temporaire) |
| Configuration | Agent's config | Custom config |
| Persistence | Reste apres execution | Supprime apres |
| Cas d'usage | Expertise specialisee | Tache ad-hoc |

### Operations

#### `delegate` - Deleguer une tache

Execute une tache via un agent permanent existant.

**Parametres:**

| Parametre | Type | Requis | Description |
|-----------|------|--------|-------------|
| `agent_id` | string | Oui | ID de l'agent cible (utiliser `list_agents` pour voir les disponibles) |
| `prompt` | string | Oui | Prompt COMPLET pour l'agent |

**Exemple:**
```json
{
  "operation": "delegate",
  "agent_id": "db_agent",
  "prompt": "## Task: Database Performance Analysis\n\n### Objective\nAnalyze the current database schema and query patterns.\n\n### Data\nTable: users (50,000 rows)\nTable: orders (1,200,000 rows)\nTable: products (5,000 rows)\n\n### Expected Output\n1. Slow query identification\n2. Missing index recommendations\n3. Schema optimization suggestions\n4. Estimated performance improvements"
}
```

**Reponse:**
```json
{
  "success": true,
  "agent_id": "db_agent",
  "report": "## Database Performance Analysis\n\n### Slow Queries Identified\n...",
  "metrics": {
    "duration_ms": 8500,
    "tokens_input": 850,
    "tokens_output": 2100
  }
}
```

---

#### `list_agents` - Lister les agents disponibles

Liste les agents LLM disponibles pour delegation (exclut soi-meme et les temporaires).

```json
{
  "operation": "list_agents"
}
```

**Reponse:**
```json
{
  "success": true,
  "agents": [
    {
      "id": "db_agent",
      "name": "Database Agent",
      "lifecycle": "permanent",
      "capabilities": ["database", "sql", "optimization"]
    },
    {
      "id": "api_agent",
      "name": "API Agent",
      "lifecycle": "permanent",
      "capabilities": ["rest", "graphql", "security"]
    }
  ],
  "count": 2,
  "remaining_slots": 3
}
```

---

## 5. ParallelTasksTool

**Fichier**: `src-tauri/src/tools/parallel_tasks.rs`

Execution parallele de taches sur plusieurs agents simultanement.

### Contraintes importantes

| Contrainte | Valeur |
|------------|--------|
| Max taches par batch | 3 |
| Temps total | Duree de la tache la plus lente |
| Disponibilite | Primary Agent uniquement |
| Dependances entre taches | Non supportees |

### Avantages de l'execution parallele

```
Sequential:  Task1(5s) -> Task2(3s) -> Task3(4s) = 12s total
Parallel:    Task1(5s)
             Task2(3s)  } = 5s total (max duration)
             Task3(4s)
```

### Operations

#### `execute_batch` - Execution parallele

Execute plusieurs taches simultanement.

**Parametres:**

| Parametre | Type | Requis | Default | Description |
|-----------|------|--------|---------|-------------|
| `tasks` | array | Oui | - | Liste des taches (max 3) |
| `wait_all` | boolean | Non | true | Attendre toutes les completions |

**Structure de `tasks[]`:**

| Champ | Type | Requis | Description |
|-------|------|--------|-------------|
| `agent_id` | string | Oui | ID de l'agent cible |
| `prompt` | string | Oui | Prompt COMPLET pour cet agent |

**Exemple minimal:**
```json
{
  "operation": "execute_batch",
  "tasks": [
    {
      "agent_id": "db_agent",
      "prompt": "List all tables with row counts."
    },
    {
      "agent_id": "api_agent",
      "prompt": "List all API endpoints."
    }
  ]
}
```

**Exemple complet:**
```json
{
  "operation": "execute_batch",
  "tasks": [
    {
      "agent_id": "db_agent",
      "prompt": "## Database Analysis\n\nAnalyze query performance for the users table.\n\n### Expected Output\n1. Slow queries (>100ms)\n2. Missing indexes\n3. Recommendations"
    },
    {
      "agent_id": "api_agent",
      "prompt": "## API Security Audit\n\nReview all public endpoints for security issues.\n\n### Expected Output\n1. Authentication gaps\n2. Input validation issues\n3. Rate limiting status"
    },
    {
      "agent_id": "ui_agent",
      "prompt": "## Accessibility Audit\n\nCheck UI components for WCAG 2.1 compliance.\n\n### Expected Output\n1. WCAG violations by severity\n2. Affected components\n3. Remediation steps"
    }
  ],
  "wait_all": true
}
```

**Reponse:**
```json
{
  "success": true,
  "completed": 3,
  "failed": 0,
  "results": [
    {
      "agent_id": "db_agent",
      "success": true,
      "report": "## Database Analysis\n\n### Slow Queries...",
      "metrics": {
        "duration_ms": 5200,
        "tokens_input": 450,
        "tokens_output": 1800
      }
    },
    {
      "agent_id": "api_agent",
      "success": true,
      "report": "## API Security Audit\n\n### Authentication...",
      "metrics": {
        "duration_ms": 4100,
        "tokens_input": 380,
        "tokens_output": 1500
      }
    },
    {
      "agent_id": "ui_agent",
      "success": true,
      "report": "## Accessibility Audit\n\n### WCAG Violations...",
      "metrics": {
        "duration_ms": 6800,
        "tokens_input": 420,
        "tokens_output": 2200
      }
    }
  ],
  "aggregated_report": "# Parallel Analysis Results\n\n## Database Analysis\n...\n\n## API Security Audit\n...\n\n## Accessibility Audit\n..."
}
```

**Reponse avec echec partiel:**
```json
{
  "success": false,
  "completed": 2,
  "failed": 1,
  "results": [
    {
      "agent_id": "db_agent",
      "success": true,
      "report": "..."
    },
    {
      "agent_id": "api_agent",
      "success": false,
      "error": "Agent not found: 'api_agent'"
    },
    {
      "agent_id": "ui_agent",
      "success": true,
      "report": "..."
    }
  ]
}
```

---

## Fichiers source

| Fichier | Description |
|---------|-------------|
| `src-tauri/src/tools/mod.rs` | Trait Tool, ToolDefinition, ToolError, exports |
| `src-tauri/src/tools/factory.rs` | ToolFactory pour creation d'instances |
| `src-tauri/src/tools/context.rs` | AgentToolContext pour sub-agent tools |
| `src-tauri/src/tools/memory/tool.rs` | Implementation MemoryTool |
| `src-tauri/src/tools/memory/mod.rs` | Module memory exports |
| `src-tauri/src/tools/todo/tool.rs` | Implementation TodoTool |
| `src-tauri/src/tools/todo/mod.rs` | Module todo exports |
| `src-tauri/src/tools/spawn_agent.rs` | Implementation SpawnAgentTool |
| `src-tauri/src/tools/delegate_task.rs` | Implementation DelegateTaskTool |
| `src-tauri/src/tools/parallel_tasks.rs` | Implementation ParallelTasksTool |
| `src-tauri/src/tools/validation_helper.rs` | Helpers de validation |

---

## Bonnes pratiques

### Pour les prompts de sub-agents

1. **Inclure tout le contexte necessaire** - Le sous-agent n'a acces a rien d'autre
2. **Specifier le format de sortie attendu** - Markdown structure recommande
3. **Definir des contraintes claires** - Limites, focus, exclusions
4. **Utiliser des sections** - Headers markdown pour structurer

### Pour l'utilisation des tools

1. **MemoryTool**: Rechercher avant d'ajouter pour eviter les doublons
2. **TodoTool**: Creer les taches AVANT de commencer le travail complexe
3. **SpawnAgentTool**: Utiliser pour taches ad-hoc necessitant config specifique
4. **DelegateTaskTool**: Utiliser pour leverager l'expertise d'agents specialises
5. **ParallelTasksTool**: Utiliser quand les taches sont independantes

### Gestion des erreurs

Tous les tools retournent un objet avec `success: boolean`. En cas d'erreur:

```json
{
  "success": false,
  "error": "Description de l'erreur",
  "error_code": "INVALID_INPUT"
}
```

Codes d'erreur courants:
- `INVALID_INPUT` - Parametres invalides
- `NOT_FOUND` - Ressource non trouvee
- `EXECUTION_FAILED` - Echec d'execution
- `VALIDATION_FAILED` - Validation echouee
- `MAX_LIMIT_EXCEEDED` - Limite atteinte (ex: 3 sub-agents)
