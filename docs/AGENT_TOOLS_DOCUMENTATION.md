# Documentation des Outils Agents par Défaut

Documentation technique des outils natifs disponibles pour les agents du système multi-agents.

---

## Statut d'Implementation

| Outil | Statut | Fichier |
|-------|--------|---------|
| **TodoTool** | Implemented | `src-tauri/src/tools/todo/tool.rs` |
| **MemoryTool** | Implemented | `src-tauri/src/tools/memory/tool.rs` |
| **CalculatorTool** | Implemented | `src-tauri/src/tools/calculator/tool.rs` |
| **SpawnAgentTool** | Implemented | `src-tauri/src/tools/spawn_agent.rs` |
| **DelegateTaskTool** | Implemented | `src-tauri/src/tools/delegate_task.rs` |
| **ParallelTasksTool** | Implemented | `src-tauri/src/tools/parallel_tasks.rs` |
| **Tool Execution** | Implemented | `src-tauri/src/agents/llm_agent.rs` |

**Note**: Les DB tools (SurrealDBTool, QueryBuilderTool, AnalyticsTool) ont été retirés - l'accès DB se fait via les commands Tauri IPC.

**Categories**:
- **Basic Tools**: MemoryTool, TodoTool, CalculatorTool (no special context required)
- **Sub-Agent Tools**: SpawnAgentTool, DelegateTaskTool, ParallelTasksTool (require AgentToolContext)

### ToolFactory

Les outils sont instancies dynamiquement via `ToolFactory`:

```rust
use crate::tools::ToolFactory;

let factory = ToolFactory::new(db.clone(), embedding_service);
let tool = factory.create_tool("MemoryTool", Some("wf_001".into()), "agent_id".into())?;
```

---

## 1. Todo Tool

**Objectif** : Gestion hierarchique du workflow et orchestration des taches agents

**Implementation** : `src-tauri/src/tools/todo/tool.rs` (TodoTool)

**Statut** : Implemented

### Operations Disponibles (via JSON)

| Operation | Description | Parametres requis |
|-----------|-------------|-------------------|
| `create` | Creation tache | `name` |
| `get` | Lecture par ID | `task_id` |
| `update_status` | Mise a jour statut | `task_id`, `status` |
| `list` | Liste taches workflow | (aucun) |
| `complete` | Marquer complete | `task_id` |
| `delete` | Suppression | `task_id` |

### Structure de Tache

```json
{
  "id": "uuid",                    // Identifiant unique (genere)
  "workflow_id": "uuid",           // Workflow associe
  "name": "string",                // Nom court (max 128 chars)
  "description": "string",         // Details (max 1000 chars)
  "agent_assigned": "string?",     // Agent responsable (optionnel)
  "priority": 1-5,                 // 1=Critique, 5=Faible
  "status": "enum",                // pending | in_progress | completed | blocked
  "dependencies": ["uuid"],        // Taches prerequises
  "duration_ms": "number?",        // Duree execution (si complete)
  "created_at": "datetime",        // Timestamp creation
  "completed_at": "datetime?"      // Timestamp completion
}
```

### Exemples d'Utilisation

**Creation de tache**:
```json
{
  "operation": "create",
  "name": "Analyze code structure",
  "description": "Deep analysis of src/ directory",
  "priority": 1
}
```

**Mise a jour statut**:
```json
{
  "operation": "update_status",
  "task_id": "abc-123",
  "status": "in_progress"
}
```

**Completion avec metriques**:
```json
{
  "operation": "complete",
  "task_id": "abc-123",
  "duration_ms": 5000
}
```

**Liste filtree**:
```json
{
  "operation": "list",
  "status_filter": "pending"
}
```

### Commandes Tauri IPC (Frontend)

| Commande | TypeScript | Rust |
|----------|------------|------|
| `create_task` | workflowId, name, description, priority?, agentAssigned?, dependencies? | workflow_id, name, description, priority, agent_assigned, dependencies |
| `get_task` | taskId | task_id |
| `list_workflow_tasks` | workflowId | workflow_id |
| `list_tasks_by_status` | status, workflowId? | status, workflow_id |
| `update_task` | taskId, updates | task_id, updates |
| `update_task_status` | taskId, status | task_id, status |
| `complete_task` | taskId, durationMs? | task_id, duration_ms |
| `delete_task` | taskId | task_id |

### Cas d'Usage
- **Orchestration multi-agents** : Coordination de workflows complexes entre plusieurs agents
- **Tracabilite** : Suivi de progression pour operations longues (>3 etapes)
- **Gestion de dependances** : Organisation sequentielle ou parallele des taches
- **Metriques** : Tracking duree execution pour optimisation

### Configuration Agent

Pour activer le TodoTool sur un agent:
```toml
[tools]
enabled = ["TodoTool", "SurrealDBTool"]
```

---

## 2. Memory Tool

**Objectif** : Persistance vectorielle dans SurrealDB pour memoire contextuelle agents

**Implementation** : `src-tauri/src/tools/memory/tool.rs` (MemoryTool)

**Statut** : Implemented

### Architecture

**Base de donnees** : SurrealDB avec support embeddings vectoriels ([Doc officielle](https://surrealdb.com/docs/surrealdb/models/vector))

**Indexation** : HNSW (Hierarchical Navigable Small World) avec dimension 1024 (Mistral/Ollama compatible)

**Recherche** : Similarite cosinus pour retrieval semantique

**Embedding Service** : Abstraction multi-provider (`src-tauri/src/llm/embedding.rs`)
- Mistral: `mistral-embed` (1024D)
- Ollama: `nomic-embed-text` (768D), `mxbai-embed-large` (1024D)

### Operations Disponibles (via JSON)

| Operation | Description | Parametres requis |
|-----------|-------------|-------------------|
| `activate_workflow` | Activation scope workflow | `workflow_id` |
| `activate_general` | Mode general (cross-workflow) | (aucun) |
| `add` | Ajout memoire avec embedding | `type`, `content` |
| `get` | Lecture par ID | `memory_id` |
| `list` | Liste avec filtres | (aucun) |
| `search` | Recherche semantique | `query` |
| `delete` | Suppression | `memory_id` |
| `clear_by_type` | Suppression en masse par type | `type` |

### Exemples d'Utilisation

**Activation scope workflow**:
```json
{
  "operation": "activate_workflow",
  "workflow_id": "wf_abc123"
}
```

**Ajout memoire avec embedding**:
```json
{
  "operation": "add",
  "type": "knowledge",
  "content": "SurrealDB supports HNSW vector indexing for semantic search",
  "metadata": {"priority": 0.8},
  "tags": ["database", "vector-search"]
}
```

**Recherche semantique**:
```json
{
  "operation": "search",
  "query": "vector database indexing",
  "limit": 5,
  "threshold": 0.7
}
```

**Liste filtree**:
```json
{
  "operation": "list",
  "type_filter": "knowledge",
  "limit": 20
}
```

### Structure de Memoire

```json
{
  "id": "uuid",
  "type": "user_pref | context | knowledge | decision",
  "content": "string (max 50000 chars)",
  "embedding": [0.1, 0.2, ...],
  "workflow_id": "string?",
  "metadata": {
    "agent_source": "string",
    "priority": 0.0-1.0,
    "tags": ["string"]
  },
  "created_at": "datetime"
}
```

### Cas d'Usage
- **Préférences utilisateur** : Stockage personnalisation interface, modèles préférés
- **Contexte conversationnel** : Continuité dialogue entre sessions
- **Base de connaissances** : Accumulation expertise projet-specific
- **Décisions architecturales** : Historique choix techniques et justifications

### Bonnes Pratiques
- **Dimensionnalité** : Utiliser embeddings selon provider
  - 768D : Ollama (nomic-embed-text), BERT léger
  - 1024D : Mistral (mistral-embed), Ollama (mxbai-embed-large)
  - 1536D : OpenAI (text-embedding-3-small)
  - 3072D : OpenAI (text-embedding-3-large)
- **Indexation** : Créer index HNSW pour >1000 entrées (optimisation requêtes)
- **Scope** : Séparer mémoires workflow-specific et générales pour isolation
- **Nettoyage** : Purger mémoires temporaires post-workflow avec `delete_memory`

---

## 3. Calculator Tool

**Objectif** : Evaluation d'expressions mathematiques pour les agents

**Implementation** : `src-tauri/src/tools/calculator/tool.rs` (CalculatorTool)

**Statut** : Implemented

### Operations Disponibles (via JSON)

| Operation | Description | Parametres requis |
|-----------|-------------|-------------------|
| `eval` | Evaluation expression | `expression` |

### Exemples d'Utilisation

**Evaluation simple**:
```json
{
  "operation": "eval",
  "expression": "2 + 2 * 3"
}
```

**Resultat**:
```json
{
  "success": true,
  "result": 8.0,
  "expression": "2 + 2 * 3"
}
```

### Operations Supportees
- Arithmetique basique : `+`, `-`, `*`, `/`
- Parentheses : `(2 + 3) * 4`
- Nombres decimaux : `3.14 * 2`
- Nombres negatifs : `-5 + 3`

### Cas d'Usage
- **Calculs de metriques** : Tokens, couts, durees
- **Conversions** : Unites, pourcentages
- **Validations numeriques** : Verification de calculs dans les rapports

---

## 4. Internal Report Tool (Future - Not Implemented)

**Statut** : NOT IMPLEMENTED - Design specification only

**Objectif** : Communication inter-agents via rapports Markdown persistés localement

> **Note** : Cet outil est une specification future. Il n'est pas encore implémenté dans le système.

### Opérations Prevues
- `read` : Lecture rapports existants
- `write` : Création nouveaux rapports
- `write_diff` : modifier un document
- `glob` : Recherche pattern-based de rapports
- `delete` : Suppression rapports obsolètes

### Localisation Tauri (Future)

**Répertoire** : `appDataDir()` résolu comme `${dataDir}/${bundleIdentifier}`
([Référence officielle](https://v2.tauri.app/plugin/file-system/))

---

## 5. Tool Execution Integration (LLMAgent)

**Objectif** : Permettre aux agents d'exécuter des tools de manière autonome via une boucle d'exécution

**Implementation** : `src-tauri/src/agents/llm_agent.rs`

**Statut** : Implemented (Phase 5)

### Architecture d'Exécution

```
LLM Provider (Mistral/Ollama)
       ↓ Response with <tool_call>
   LLMAgent
       ↓ parse_tool_calls()
   ┌───┴───┐
   ↓       ↓
Local    MCP
Tools    Tools
   ↓       ↓
ToolFactory  MCPManager
   ↓       ↓
   └───┬───┘
       ↓ <tool_result>
   LLM Provider (continue)
```

### Format des Tool Calls (XML)

**Appel d'un tool** (dans la réponse LLM):
```xml
<tool_call name="MemoryTool">
{"operation": "add", "type": "knowledge", "content": "Important information to remember"}
</tool_call>
```

**Résultat d'un tool** (retourné au LLM):
```xml
<tool_result name="MemoryTool" success="true">
{"id": "mem_abc123", "message": "Memory added successfully"}
</tool_result>
```

### Boucle d'Exécution

```rust
// Pseudo-code de la boucle d'exécution
loop {
    // 1. Build system prompt with tool definitions
    let prompt = build_system_prompt_with_tools(&config, &tools, &mcp_tools);

    // 2. Call LLM provider
    let response = provider.complete(&prompt).await?;

    // 3. Parse tool calls from response
    let tool_calls = parse_tool_calls(&response);

    if tool_calls.is_empty() {
        break; // No more tool calls, done
    }

    // 4. Execute tools
    for call in tool_calls {
        let result = if is_local_tool(&call.name) {
            execute_local_tool(&factory, &call).await?
        } else {
            execute_mcp_tool(&mcp_manager, &call).await?
        };
        results.push(result);
    }

    // 5. Format results and feed back to LLM
    let formatted = format_tool_results(&results);
    append_to_context(&formatted);

    iteration += 1;
    if iteration >= MAX_ITERATIONS {
        break; // Safety limit (10 iterations)
    }
}
```

### Constructeurs LLMAgent

```rust
// Sans tools (comportement basique)
let agent = LLMAgent::new(config, provider);

// Avec tools (exécution complète)
let agent = LLMAgent::with_tools(config, provider, tool_factory, mcp_manager);

// Avec factory seulement (tools locaux uniquement)
let agent = LLMAgent::with_factory(config, provider, tool_factory);
```

### Méthodes Clés

| Méthode | Description |
|---------|-------------|
| `create_local_tools()` | Crée les instances de tools via ToolFactory |
| `get_mcp_tool_definitions()` | Récupère les définitions des tools MCP |
| `build_system_prompt_with_tools()` | Injecte les définitions dans le system prompt |
| `parse_tool_calls()` | Parse les balises `<tool_call>` de la réponse |
| `execute_local_tool()` | Exécute un tool via ToolFactory |
| `execute_mcp_tool()` | Exécute un tool via MCPManager |
| `format_tool_results()` | Formate les résultats en XML |
| `strip_tool_calls()` | Supprime les balises tool de la réponse finale |

### Tests

```bash
# Tests unitaires tool execution
cargo test llm_agent::tests::test_parse_tool_calls
cargo test llm_agent::tests::test_execute_local_tool
cargo test llm_agent::tests::test_tool_execution_loop
```

---

## Intégration et Orchestration

### Workflow Type
1. **Initialisation** : Agent active workflow via `activate_workflow`
2. **Planification** : Création tâches avec Todo Tool
3. **Contexte** : Chargement mémoires pertinentes via `search_for_pattern`
4. **Exécution** : Progression tâches + écriture mémoires intermédiaires
5. **Communication** : Génération rapports pour handoff si multi-agents
6. **Finalisation** : Validation `think_about_whether_you_are_done`, cleanup temporaires

### Exemple Séquence
```
activate_workflow("code_review")
→ search_for_pattern("preferences_code_style")
→ TodoWrite([
    {nom: "analyze_files", priorité: 1, status: "in_progress"},
    {nom: "generate_report", priorité: 2, status: "pending"}
  ])
→ [Exécution analyse]
→ write_memory(type: "decision", content: "patterns_found")
→ write_report("analysis_results.md")
→ think_about_whether_you_are_done()
→ delete_memory(workflow_temps)
```

---

## Références Techniques

### SurrealDB
- [Vector Database Introduction](https://surrealdb.com/docs/surrealdb/models/vector)
- [Vector Search Reference](https://surrealdb.com/docs/surrealdb/reference-guide/vector-search)
- [Embeddings Integration](https://surrealdb.com/docs/integrations/embeddings)

### Tauri
- [File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Path API Reference](https://v2.tauri.app/reference/javascript/api/namespacepath/)
- [App Data Discussion](https://github.com/tauri-apps/tauri/discussions/5557)

---

**Version** : 1.5
**Derniere mise a jour** : 2025-12-05
**Phase** : Functional Agent System Phase 5 Complete (6 Tools: MemoryTool, TodoTool, CalculatorTool, SpawnAgentTool, DelegateTaskTool, ParallelTasksTool)

### Test Coverage

| Component | Tests | Coverage |
|-----------|-------|----------|
| **MemoryTool Unit** | 40+ tests | validate_input, operations |
| **MemoryTool Integration** | 15+ tests | CRUD, workflow isolation, search |
| **TodoTool Unit** | 25+ tests | CRUD operations, status transitions |
| **LLMAgent Tool Execution** | 10+ tests | parse_tool_calls, execute, format_results |
| **Embedding Types (TS)** | 20+ tests | Constants, types, validation |
| **Memory Types (TS)** | 15+ tests | Type structure, compatibility |
| **Agent Store (TS)** | 24 tests | CRUD, form management, error handling |
