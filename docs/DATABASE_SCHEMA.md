# Schéma Database SurrealDB

> **Version** : 1.0
> **SurrealDB** : 2.3.10
> **Type** : Graph relationnel avec support vectoriel

## Vue d'Ensemble

```
workflow ─┐
          ├─→ agent_state
          ├─→ message
          ├─→ task
          ├─→ validation_request
          └─→ memory (vectoriel)
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
- `type` (filtrage catégorie)
- `embedding` (HNSW vectoriel, KNN search)
- `metadata.agent_source` (mémoires par agent)
- `metadata.workflow_id` (scope workflow)
- `metadata.timestamp` (retention policy)

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

## Relations Graph

```sql
-- Workflow → Messages
RELATE workflow:abc -> contains -> message:xyz

-- Workflow → Tasks
RELATE workflow:abc -> has_task -> task:123

-- Agent → Memories
RELATE agent_state:db_agent -> created -> memory:456

-- Memory ↔ Memory (relations sémantiques)
RELATE memory:aaa -> relates_to -> memory:bbb
```

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

## Retention Policy

**Workflows**
- Completed : 90 jours → archivage JSON
- Error : 180 jours (debug long terme)
- Running : Pas suppression auto

**Logs** (table séparée, non schématisée ici)
- Application : 30 jours
- Audit : 1 an
- Metrics : 90 jours (agrégation mensuelle après)

**Memory**
- Temporary (workflow-specific) : Suppression avec workflow
- Permanent : Pas expiration
- Pruning : Manuel ou score pertinence

**Messages**
- Liés à workflow : Même retention que workflow
- Compression : Embeddings pour old messages (économie tokens)

**Validation Requests**
- Audit trail : 1 an
- Suppression : Avec workflow parent si <1an

**Tasks**
- Même retention que workflow parent

---

## Queries Exemples (Conceptuel)

### Workflow Actif avec Contexte
Récupérer workflow + agent + messages + tasks pending

### Recherche Sémantique Memory
Vector search embedding similarity + filtres metadata

### Agent Metrics
Agrégation metrics tous workflows agent (success rate, avg tokens, cost)

### Validation Pending
Liste validations pending workflow running pour UI notification

### Task Dependencies
Graph traversal tasks bloquées par dépendances non-complétées

---

## Migration & Évolution

**Versioning Schema**
- Migrations SurrealDB via fichiers `.surql`
- Tracking version dans table `schema_version`

**Ajout Champs**
- Compatibilité backward : Champs optionnels
- Validation schéma : SCHEMAFULL pour tables critiques

**Refactoring Relations**
- Graph queries facilitent évolution relations
- Pas de foreign keys rigides (NoSQL graph)

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

**Monitoring**
- Slow queries >100ms loggées
- Index usage tracking
- Vector search latency P95/P99

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
