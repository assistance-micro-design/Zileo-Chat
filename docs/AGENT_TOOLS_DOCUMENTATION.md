# Documentation des Outils Agents par Défaut

Documentation technique des outils natifs disponibles pour les agents du système multi-agents.

---

## Statut d'Implementation

| Outil | Statut | Fichier |
|-------|--------|---------|
| **TodoTool** | Implemented | `src-tauri/src/tools/todo/tool.rs` |
| **MemoryTool** | Implemented | `src-tauri/src/tools/memory/tool.rs` |
| **CalculatorTool** | Implemented | `src-tauri/src/tools/calculator/tool.rs` |
| **UserQuestionTool** | Implemented | `src-tauri/src/tools/user_question/tool.rs` |
| **SpawnAgentTool** | Implemented | `src-tauri/src/tools/spawn_agent.rs` |
| **DelegateTaskTool** | Implemented | `src-tauri/src/tools/delegate_task.rs` |
| **ParallelTasksTool** | Implemented | `src-tauri/src/tools/parallel_tasks.rs` |
| **Tool Execution** | Implemented | `src-tauri/src/agents/llm_agent.rs` |

**Note**: Les DB tools (SurrealDBTool, QueryBuilderTool, AnalyticsTool) ont été retirés - l'accès DB se fait via les commands Tauri IPC.

**Categories**:
- **Basic Tools**: MemoryTool, TodoTool, CalculatorTool (no special context required)
- **Interaction Tools**: UserQuestionTool (human-in-the-loop interactions)
- **Sub-Agent Tools**: SpawnAgentTool, DelegateTaskTool, ParallelTasksTool (require AgentToolContext)

**Sub-Agent Resilience Features (v1.0)**:
- Inactivity Timeout with Heartbeat: 300s timeout, 30s check interval
- Retry with Exponential Backoff: 3 attempts, 500ms-2000ms delay
- Circuit Breaker: 3 failures → open, 60s cooldown
- CancellationToken: Graceful shutdown support
- Correlation ID: Hierarchical tracing for batch operations

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

### Securite et Performance (OPT-MEM)

**Requetes Parametrees** (OPT-MEM-5):
Toutes les requetes DB utilisent des bind parameters pour prevenir l'injection SQL:
```rust
let params = vec![
    ("type".to_string(), serde_json::json!(memory_type)),
    ("workflow_id".to_string(), serde_json::json!(wf_id)),
];
let results: Vec<Memory> = db.query_with_params(&query, params).await?;
```

**Validation Typee** (OPT-MEM-7):
L'outil utilise `MemoryInput` struct pour parsing et validation typee:
```rust
struct MemoryInput {
    operation: String,
    memory_type: Option<String>,
    content: Option<String>,
    // ... valide tous les inputs avant execution
}
```

**Logique Partagee** (OPT-MEM-6):
La logique add_memory est consolidee dans `tools/memory/helpers.rs` pour eliminer la duplication entre tool et commands.

**Index Composites** (OPT-MEM-4):
- `memory_type_workflow_idx` - Optimise les recherches avec type + workflow_id
- `memory_type_created_idx` - Optimise les requetes de nettoyage par type + created_at

**Service d'Embedding Dynamique**:
L'embedding service est optionnel (`Option<EmbeddingService>`). Si absent, les memoires sont stockees sans embeddings vectoriels (text search uniquement).

### Bonnes Pratiques
- **Dimensionnalite** : Utiliser embeddings selon provider
  - 768D : Ollama (nomic-embed-text), BERT leger
  - 1024D : Mistral (mistral-embed), Ollama (mxbai-embed-large)
  - 1536D : OpenAI (text-embedding-3-small)
  - 3072D : OpenAI (text-embedding-3-large)
- **Indexation** : Creer index HNSW pour >1000 entrees (optimisation requetes)
- **Scope** : Separer memoires workflow-specific et generales pour isolation
- **Nettoyage** : Purger memoires temporaires post-workflow avec `delete_memory`

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

## 4. User Question Tool

**Objectif** : Permettre aux agents de poser des questions interactives aux utilisateurs pendant l'execution du workflow

**Implementation** : `src-tauri/src/tools/user_question/tool.rs` (UserQuestionTool)

**Statut** : Implemented

### Operations Disponibles (via JSON)

| Operation | Description | Parametres requis |
|-----------|-------------|-------------------|
| `ask` | Poser une question a l'utilisateur | `question`, `questionType` |

### Parametres de l'Operation "ask"

| Parametre | Type | Requis | Contraintes | Description |
|-----------|------|--------|-------------|-------------|
| `operation` | string | Oui | "ask" | Operation a effectuer |
| `question` | string | Oui | Max 2000 chars, non vide | La question a poser |
| `questionType` | string | Oui | "checkbox" \| "text" \| "mixed" | Type de question |
| `options` | array | Conditionnel | Requis pour "checkbox"/"mixed", max 20 options | Options de choix |
| `textPlaceholder` | string | Non | - | Placeholder du champ texte |
| `textRequired` | boolean | Non | Default: false | Texte requis (pour "mixed") |
| `context` | string | Non | Max 5000 chars | Contexte additionnel a afficher |

### Structure QuestionOption

```json
{
  "id": "option_1",      // Identifiant unique (non vide)
  "label": "Option 1"    // Texte affiche (max 256 chars)
}
```

### Types de Questions

| Type | Description | Options requises | Texte disponible |
|------|-------------|------------------|------------------|
| `checkbox` | Choix multiple avec cases a cocher | Oui | Non |
| `text` | Champ de texte libre | Non | Oui |
| `mixed` | Combinaison checkbox + texte | Oui | Oui |

### Exemples d'Utilisation

**Question checkbox (choix multiple)**:
```json
{
  "operation": "ask",
  "question": "Quelles fonctionnalites voulez-vous implementer ?",
  "questionType": "checkbox",
  "options": [
    {"id": "auth", "label": "Authentification"},
    {"id": "api", "label": "API REST"},
    {"id": "db", "label": "Base de donnees"}
  ],
  "context": "Selectionnez toutes les options applicables"
}
```

**Question texte**:
```json
{
  "operation": "ask",
  "question": "Quel nom voulez-vous donner au projet ?",
  "questionType": "text",
  "textPlaceholder": "Entrez le nom du projet..."
}
```

**Question mixte**:
```json
{
  "operation": "ask",
  "question": "Comment souhaitez-vous proceder ?",
  "questionType": "mixed",
  "options": [
    {"id": "option_a", "label": "Approche A"},
    {"id": "option_b", "label": "Approche B"}
  ],
  "textPlaceholder": "Ou decrivez votre propre approche...",
  "textRequired": false
}
```

### Structure de Reponse

**Succes**:
```json
{
  "success": true,
  "selectedOptions": ["auth", "api"],
  "textResponse": "Details additionnels...",
  "message": "User response received"
}
```

**Erreur (question ignoree)**:
```json
{
  "success": false,
  "error": "Question skipped by user"
}
```

### Mecanisme de Polling

Le tool utilise un pattern de polling progressif pour attendre la reponse utilisateur:

| Etape | Intervalle |
|-------|------------|
| 1-2 | 500ms |
| 3-4 | 1000ms |
| 5-6 | 2000ms |
| 7+ | 5000ms (jusqu'au timeout) |

**Timeout (OPT-UQ-7)** : Apres 5 minutes (300 secondes) sans reponse, le statut devient "timeout" et une erreur est retournee. Le circuit breaker enregistre ce timeout (voir section Circuit Breaker ci-dessous).

### Events Emis

| Event | Type | Description |
|-------|------|-------------|
| `workflow_stream` | `user_question_start` | Question envoyee, en attente de reponse |
| `workflow_stream` | `user_question_complete` | Reponse recue, question ignoree, ou timeout |

### Commandes Tauri IPC (Frontend)

| Commande | Description | Parametres |
|----------|-------------|------------|
| `submit_user_response` | Soumettre une reponse | `questionId`, `selectedOptions`, `textResponse?` |
| `get_pending_questions` | Lister questions en attente | `workflowId` |
| `skip_question` | Ignorer une question | `questionId` |

### Cas d'Usage

- **Clarification** : Demander des precisions sur les requirements
- **Choix d'implementation** : Proposer des options d'architecture
- **Validation** : Confirmer avant operations critiques
- **Collecte d'information** : Recueillir parametres manquants

### Configuration Agent

Pour activer le UserQuestionTool sur un agent:
```json
{
  "tools": ["UserQuestionTool", "MemoryTool", "TodoTool"]
}
```

### Constantes de Validation

```rust
pub const MAX_QUESTION_LENGTH: usize = 2000;
pub const MAX_OPTION_ID_LENGTH: usize = 64;        // OPT-UQ-2
pub const MAX_OPTION_LABEL_LENGTH: usize = 256;
pub const MAX_OPTIONS: usize = 20;
pub const MAX_CONTEXT_LENGTH: usize = 5000;
pub const MAX_TEXT_RESPONSE_LENGTH: usize = 10000; // OPT-UQ-1
pub const VALID_TYPES: &[&str] = &["checkbox", "text", "mixed"];
pub const VALID_STATUSES: &[&str] = &["pending", "answered", "skipped", "timeout"];

// Timeout (OPT-UQ-7)
pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

// Circuit Breaker (OPT-UQ-12)
pub const CIRCUIT_FAILURE_THRESHOLD: u32 = 3;  // Opens after 3 consecutive timeouts
pub const CIRCUIT_COOLDOWN_SECS: u64 = 60;     // 60s cooldown before recovery attempt
```

### Circuit Breaker (OPT-UQ-12)

Le UserQuestionTool implemente un circuit breaker pour prevenir le spam de questions quand l'utilisateur ne repond pas.

**Etats du Circuit**:

| Etat | Description | Comportement |
|------|-------------|--------------|
| **Closed** | Fonctionnement normal | Questions autorisees |
| **Open** | Trop de timeouts (3 consecutifs) | Questions rejetees immediatement |
| **HalfOpen** | Test de recuperation (apres 60s) | Une question autorisee pour tester |

**Transitions**:
```
Closed → [3 timeouts] → Open → [60s cooldown] → HalfOpen
                                                    ↓
                                            [success] → Closed
                                            [timeout] → Open
```

**Reponse si circuit ouvert**:
```json
{
  "success": false,
  "error": "User appears unresponsive (3 consecutive timeouts). Question rejected. Retry in 45 seconds."
}
```

**Reset Conditions**:
- **Success** : Utilisateur repond → reset compteur, ferme circuit
- **Skip** : Utilisateur ignore → traite comme success (reponse active)
- **Autres erreurs** : Pas d'effet sur le circuit breaker

**Implementation** : `src-tauri/src/tools/user_question/circuit_breaker.rs`

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

**Version** : 2.2
**Derniere mise a jour** : 2026-01-25
**Phase** : Functional Agent System v1.0 Complete + OPT-MEM + OPT-TODO + OPT-UQ + OPT-TD Optimizations

**Features (v2.1)**:
- 7 Tools: MemoryTool, TodoTool, CalculatorTool, UserQuestionTool, SpawnAgentTool, DelegateTaskTool, ParallelTasksTool
- Sub-Agent Resilience: Inactivity Timeout (OPT-SA-1), CancellationToken (OPT-SA-7), Circuit Breaker (OPT-SA-8), Retry (OPT-SA-10), Correlation ID (OPT-SA-11)
- MemoryTool Optimizations: Parameterized queries (OPT-MEM-5), MemoryInput struct (OPT-MEM-7), helpers.rs consolidation (OPT-MEM-6), composite indexes (OPT-MEM-4)
- TodoTool Optimizations: Parameterized queries (OPT-TODO-1 to 4), N+1 reduction (OPT-TODO-5,6), db_error uniformization (OPT-TODO-7), TASK_SELECT_FIELDS (OPT-TODO-9), query limits (OPT-TODO-10)
- UserQuestionTool Optimizations (OPT-UQ-1 to 12): Text response validation (OPT-UQ-1), Option ID length (OPT-UQ-2), Strict error handling (OPT-UQ-3), Queue limit 50 (OPT-UQ-4), Logger unified (OPT-UQ-5), SQL injection tests (OPT-UQ-6), Configurable timeout 5min (OPT-UQ-7), Unit tests (OPT-UQ-8), Integration tests (OPT-UQ-9), Refactor ask_question (OPT-UQ-10), Refactor submit_response (OPT-UQ-11), Circuit breaker (OPT-UQ-12)
- Tool Description Optimizations (OPT-TD-1 to 8): Enriched descriptions with structured sections, dynamic constant injection, sub-agent template helper, CLAUDE.md guidelines

### Test Coverage

| Component | Tests | Coverage |
|-----------|-------|----------|
| **MemoryTool Unit** | 40+ tests | validate_input, operations |
| **MemoryTool Integration** | 15+ tests | CRUD, workflow isolation, search |
| **TodoTool Unit** | 6 tests | validate_input, definition |
| **TodoTool Integration** | 11 tests | CRUD operations, status transitions |
| **TodoTool SQL Injection** | 8 tests | SQL injection prevention |
| **UserQuestionTool Unit** | 25 tests | validate_input, definition, constants |
| **UserQuestionTool Integration** | 21 tests | commands, SQL injection, validation |
| **UserQuestionTool Circuit Breaker** | 12 tests | state transitions, recovery |
| **LLMAgent Tool Execution** | 10+ tests | parse_tool_calls, execute, format_results |
| **Embedding Types (TS)** | 20+ tests | Constants, types, validation |
| **Memory Types (TS)** | 15+ tests | Type structure, compatibility |
| **Agent Store (TS)** | 24 tests | CRUD, form management, error handling |
