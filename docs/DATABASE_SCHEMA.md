# Schéma Database SurrealDB

> **Version** : 1.0
> **SurrealDB** : 2.4.0
> **Type** : Graph relationnel avec support vectoriel

## Vue d'Ensemble

**Total : 19 tables**

```
workflow ─────────────┐
                      ├─→ agent (user-created configs)
                      ├─→ agent_state (runtime state)
                      ├─→ message
                      ├─→ task
                      ├─→ validation_request
                      ├─→ user_question
                      ├─→ memory (vectoriel)
                      ├─→ tool_execution
                      ├─→ thinking_step
                      └─→ sub_agent_execution

mcp_server ───────────→ mcp_call_log

llm_model ────────────→ provider_settings

prompt (standalone)
settings (key-value config)
```

## Tables Principales

### workflow

Représente un workflow multi-agents avec son cycle de vie complet.

**Champs**
- `id` : UUID
- `name` : string (éditable utilisateur)
- `status` : enum (idle, running, completed, error)
- `agent_id` : string (agent principal)
- `created_at` : datetime
- `updated_at` : datetime
- `completed_at` : datetime?

**Relations**
- → `agent_state` (créateur)
- → `message[]` (historique conversation)
- → `task[]` (tâches décomposées)
- → `validation_request[]` (demandes validation utilisateur)

**Indexes**
- `status` (queries filtrage actifs/complétés)
- `created_at` (tri chronologique)
- `agent_id` (workflows par agent)

**Requête type** : Récupérer workflows actifs agent spécifique

---

### agent_state

État persistant agents permanents (DB agent, API agent, etc.).

**Champs**
- `id` : UUID
- `agent_id` : string (identifiant unique agent)
- `lifecycle` : enum (permanent, temporary)
- `config` : object (configuration TOML)
- `metrics` : object (tasks_executed, success_rate, avg_duration, token_usage, cost)
- `last_active` : datetime
- `created_at` : datetime

**Relations**
- → `workflow[]` (workflows créés)
- → `memory[]` (mémoires agent-specific)

**Indexes**
- `agent_id` (unique)
- `lifecycle` (filter permanents/temporaires)
- `last_active` (cleanup temporaires inactifs)

**Requête type** : État agent + métriques performance

---

### memory

Stockage vectoriel pour RAG et contexte agent.

**Champs**
- `id` : UUID
- `type` : enum (user_pref, context, knowledge, decision)
- `content` : string (texte indexé)
- `embedding` : array<float> (vecteur 768D-3072D selon provider)
- `metadata` : object
  - `agent_source` : string
  - `timestamp` : datetime
  - `workflow_id` : UUID?
  - `priority` : float (0.0-1.0)
  - `tags` : array<string>
- `relations` : array<UUID> (liens vers autres mémoires)

**Relations**
- → `workflow` (optionnel, si workflow-specific)
- → `agent_state` (créateur)
- ↔ `memory[]` (relations sémantiques)

**Indexes**
- `type` (filtrage categorie)
- `embedding` (HNSW vectoriel, KNN search)
- `metadata.agent_source` (memoires par agent)
- `metadata.workflow_id` (scope workflow)
- `metadata.timestamp` (retention policy)

**Index Composites (OPT-MEM-4)**:
- `memory_type_workflow_idx` ON (type, workflow_id) - Optimise search_memories() avec type + workflow
- `memory_type_created_idx` ON (type, created_at) - Optimise requetes TTL/cleanup

**Requête type** : Recherche sémantique similarité cosinus top_k=5

---

### message

Messages conversation workflow (user, assistant, system).

**Champs**
- `id` : UUID
- `workflow_id` : UUID
- `role` : enum (user, assistant, system)
- `content` : string
- `tokens` : object (input, output)
- `reasoning_steps` : array<object>? (si modèle supporte)
- `timestamp` : datetime

**Relations**
- → `workflow` (appartenance)
- → `agent_state` (si role=assistant)

**Indexes**
- `workflow_id` (historique conversation)
- `timestamp` (ordre chronologique)
- `role` (filtrage par type)

**Requête type** : Historique messages workflow ordonné

---

### validation_request

Demandes validation human-in-the-loop.

**Champs**
- `id` : UUID
- `workflow_id` : UUID
- `agent_id` : string
- `type` : enum (tool, sub_agent, mcp, file_op, db_op)
- `operation` : string (description)
- `details` : object (params opération)
- `risk_level` : enum (low, medium, high)
- `status` : enum (pending, approved, rejected)
- `user_id` : string? (si multi-user futur)
- `timestamp` : datetime
- `response_timestamp` : datetime?

**Relations**
- → `workflow` (contexte)
- → `agent_state` (demandeur)

**Indexes**
- `workflow_id` (validations workflow)
- `status` (pending pour UI)
- `timestamp` (ordre demandes)
- `type` + `risk_level` (analytics)

**Requête type** : Validations pending workflow actif

---

### user_question

Questions interactives agents vers utilisateurs (human-in-the-loop).

**Champs**
- `id` : UUID
- `workflow_id` : string
- `agent_id` : string
- `question` : string (texte de la question)
- `question_type` : enum (checkbox, text, mixed)
- `options` : array<{id: string, label: string}>? (pour checkbox/mixed)
- `text_placeholder` : string?
- `text_required` : boolean (default: false)
- `context` : string? (contexte additionnel)
- `status` : enum (pending, answered, skipped)
- `selected_options` : array<string>?
- `text_response` : string?
- `created_at` : datetime
- `answered_at` : datetime?

**Relations**
- → `workflow` (contexte)
- → `agent` (demandeur)

**Indexes**
- `workflow_id` (questions workflow)
- `status` (pending pour UI)
- `agent_id`

**Requête type** : Questions pending workflow actif

---

### task

Tâches décomposées workflow avec statut progression.

**Champs**
- `id` : UUID
- `workflow_id` : UUID
- `agent_assigned` : string? (agent responsable)
- `name` : string
- `description` : string
- `status` : enum (pending, in_progress, completed, blocked)
- `priority` : int (1-5, 1=critique)
- `duration` : int? (ms, si completed)
- `dependencies` : array<UUID> (autres tasks)
- `created_at` : datetime
- `completed_at` : datetime?

**Relations**
- → `workflow` (appartenance)
- → `agent_state` (assigné)
- → `task[]` (dépendances)

**Indexes**
- `workflow_id` (tasks workflow)
- `status` (filtrage actives/bloquées)
- `agent_assigned` (workload agent)
- `priority` (tri urgence)

**Requête type** : Tasks pending non-bloquées workflow

---

### mcp_server

Configuration des serveurs MCP utilisateur.

**Champs**
- `id` : string (unique identifier)
- `name` : string (unique, user-friendly name)
- `enabled` : boolean
- `command` : string (docker, npx, uvx, http)
- `args` : array<string>
- `env` : string (JSON-encoded HashMap for dynamic keys)
- `description` : string?
- `created_at` : datetime
- `updated_at` : datetime

**Indexes**
- `id` (UNIQUE)
- `name` (UNIQUE)

---

### mcp_call_log

Journal d'audit des appels MCP tools.

**Champs**
- `id` : UUID
- `workflow_id` : string?
- `server_name` : string
- `tool_name` : string
- `params` : object
- `result` : object
- `success` : boolean
- `duration_ms` : int
- `timestamp` : datetime

**Indexes**
- `workflow_id`
- `server_name`
- `timestamp`

---

### llm_model

Registre des modeles LLM (builtin + custom).

**Champs**
- `id` : string (UUID for custom, api_name for builtin)
- `provider` : string (mistral, ollama)
- `name` : string (human-readable)
- `api_name` : string (model identifier for API)
- `context_window` : int (1024-2000000)
- `max_output_tokens` : int (256-128000)
- `temperature_default` : float (0.0-2.0)
- `is_builtin` : boolean
- `is_reasoning` : boolean
- `input_price_per_mtok` : float?
- `output_price_per_mtok` : float?
- `created_at` : datetime
- `updated_at` : datetime

**Indexes**
- `id` (UNIQUE)
- `provider`
- `(provider, api_name)` (UNIQUE)

---

### provider_settings

Configuration des providers LLM.

**Champs**
- `provider` : string (UNIQUE, mistral/ollama)
- `enabled` : boolean
- `default_model_id` : string?
- `base_url` : string?
- `updated_at` : datetime

**Indexes**
- `provider` (UNIQUE)

---

### tool_execution

Persistance des executions d'outils.

**Champs**
- `id` : UUID
- `workflow_id` : string
- `message_id` : string
- `agent_id` : string
- `tool_type` : string (local, mcp)
- `tool_name` : string
- `server_name` : string? (for MCP tools)
- `input_params` : object
- `output_result` : object
- `success` : boolean
- `error_message` : string?
- `duration_ms` : int
- `iteration` : int
- `created_at` : datetime

**Indexes**
- `workflow_id`
- `message_id`
- `agent_id`
- `tool_type`

---

### thinking_step

Etapes de raisonnement agent (chain-of-thought).

**Champs**
- `id` : UUID
- `workflow_id` : string
- `message_id` : string
- `agent_id` : string
- `step_number` : int
- `content` : string
- `duration_ms` : int?
- `tokens` : int?
- `created_at` : datetime

**Indexes**
- `workflow_id`
- `message_id`
- `agent_id`

---

### sub_agent_execution

Historique des executions de sub-agents.

**Champs**
- `id` : UUID
- `workflow_id` : string
- `parent_agent_id` : string
- `sub_agent_id` : string
- `sub_agent_name` : string
- `task_description` : string
- `status` : enum (running, completed, error)
- `duration_ms` : int?
- `tokens_input` : int?
- `tokens_output` : int?
- `result_summary` : string?
- `error_message` : string?
- `created_at` : datetime
- `completed_at` : datetime?

**Indexes**
- `workflow_id`
- `parent_agent_id`
- `status`

---

### agent

Configuration des agents crees par l'utilisateur.

**Champs**
- `id` : UUID
- `name` : string (1-64 chars)
- `lifecycle` : enum (permanent, temporary)
- `llm` : object
  - `provider` : string
  - `model` : string
  - `temperature` : float (0.0-2.0)
  - `max_tokens` : int (256-128000)
- `tools` : array<string>
- `mcp_servers` : array<string>
- `system_prompt` : string (1-10000 chars)
- `max_tool_iterations` : int?
- `enable_thinking` : boolean?
- `created_at` : datetime
- `updated_at` : datetime

**Indexes**
- `id` (UNIQUE)
- `name`
- `llm.provider`

---

## Schéma Vectoriel (HNSW)

**Configuration Index**
```
Table: memory
Field: embedding
Algorithm: HNSW (Hierarchical Navigable Small World)
Distance: Cosine similarity
Dimensions: 768 | 1024 | 1536 | 3072
M: 16 (connexions par noeud)
ef_construction: 200 (qualité construction)
ef_search: 50 (qualité recherche)
```

**Recherche KNN** : Retour top_k résultats avec score similarité

---

## Query Patterns (Implementation - Phase 5)

### Parameterized Queries

**Safe Pattern** - Use bind parameters for user input:
```rust
// src-tauri/src/db/client.rs

// SELECT with parameters
let results: Vec<Memory> = db.query_with_params(
    "SELECT * FROM memory WHERE type = $type AND workflow_id = $wf_id",
    vec![
        ("type".to_string(), serde_json::json!("knowledge")),
        ("wf_id".to_string(), serde_json::json!(workflow_id)),
    ]
).await?;

// JSON results with parameters
let results: Vec<serde_json::Value> = db.query_json_with_params(
    "SELECT meta::id(id) AS id FROM memory WHERE type = $type",
    vec![("type".to_string(), serde_json::json!(user_input))]
).await?;

// Mutations with parameters
db.execute_with_params(
    "UPDATE task:`uuid` SET status = $status",
    vec![("status".to_string(), serde_json::json!("completed"))]
).await?;
```

**Unsafe Pattern** - DO NOT use string concatenation:
```rust
// WRONG - SQL injection risk
let query = format!("SELECT * FROM memory WHERE type = '{}'", user_input);

// WRONG - Apostrophes cause parse errors
format!("content = '{}'", text.replace('\'', "''"))  // l'eau -> parse error
```

### Transaction Support

For multi-query operations that must succeed or fail together:
```rust
// Simple transaction
db.transaction(vec![
    "CREATE workflow:`123` CONTENT { name: 'Test' }".to_string(),
    "CREATE message:`456` CONTENT { workflow_id: '123' }".to_string(),
]).await?;  // Rolls back on any failure

// Parameterized transaction
db.transaction_with_params(vec![
    (
        "CREATE workflow:`123` CONTENT $data".to_string(),
        vec![("data".to_string(), json!({"name": "Test"}))]
    ),
    (
        "UPDATE agent:`456` SET status = $status".to_string(),
        vec![("status".to_string(), json!("active"))]
    ),
]).await?;  // Rolls back on any failure
```

### Query Limits (OPT-DB-8)

All list operations enforce LIMIT to prevent memory explosion:
```rust
use crate::tools::constants::query_limits;

let query = format!(
    "SELECT * FROM memory ORDER BY created_at DESC LIMIT {}",
    query_limits::DEFAULT_LIST_LIMIT  // 1000
);
```

**Constants** (`src-tauri/src/tools/constants.rs`):
| Constant | Value | Usage |
|----------|-------|-------|
| `DEFAULT_LIST_LIMIT` | 1000 | agents, memories, tasks |
| `DEFAULT_MODELS_LIMIT` | 100 | LLM models |
| `DEFAULT_MCP_LOGS_LIMIT` | 500 | MCP call logs |
| `DEFAULT_MESSAGES_LIMIT` | 500 | message history |
| `MAX_LIST_LIMIT` | 10000 | absolute maximum |

### MCP Latency Metrics Query

The `mcp_call_log` table is used for latency percentile calculations:
```sql
-- Command: get_mcp_latency_metrics
SELECT
    server_name,
    math::percentile(duration_ms, 0.50) AS p50_ms,
    math::percentile(duration_ms, 0.95) AS p95_ms,
    math::percentile(duration_ms, 0.99) AS p99_ms,
    count() AS total_calls
FROM mcp_call_log
WHERE timestamp > time::now() - 1h
GROUP BY server_name
```

---

## Sécurité

**Permissions**
- Agents : Scope par `agent_id` (pas accès autres agents sauf orchestrateur)
- Workflows : Isolation par `workflow_id`
- Operations : SELECT/CREATE/UPDATE (DELETE nécessite validation)

**Encryption**
- At rest : SurrealDB embedded (délégué OS encryption)
- In transit : TLS si remote mode
- API keys : Jamais stockées DB (Tauri secure storage)

**Audit**
- Toutes operations sensibles loggées
- Validation requests = audit trail built-in

---

## Performance

**Indexes Critiques**
- `workflow.status` + `agent_id` : Queries fréquentes UI
- `memory.embedding` : HNSW pour KNN rapide
- `message.workflow_id` + `timestamp` : Historique ordonné

**Optimisations**
- Batch inserts messages/tasks (transactions)
- Cache queries fréquentes (ex: agent_state metrics)
- Pagination results (50-100 items par page)

---

## Références

**SurrealDB Docs**
- Vector Database : https://surrealdb.com/docs/surrealdb/models/vector
- Graph Relations : https://surrealdb.com/docs/surrealdb/models/graph
- HNSW Index : https://surrealdb.com/docs/surrealdb/reference-guide/vector-search

**Embeddings**
- OpenAI : 1536D/3072D
- Mistral : 1024D
- Ollama : 768D/1024D
