# Architecture Multi-Agent

> **Stack**: Rust + Rig.rs + MCP + Tauri 2 + SurrealDB
> **Objectif**: Système hiérarchique d'agents réutilisables avec communication standardisée

## Principes Fondamentaux

### Gestion Dynamique des Agents (v1.0)

**Aucun agent par défaut** - L'utilisateur crée tous ses agents via l'interface Settings.

**CRUD Complet via UI**:
- **Create**: Formulaire avec configuration LLM, tools, MCP servers, system prompt
- **Read**: Liste des agents avec résumé (provider, model, tools count)
- **Update**: Modification des paramètres (lifecycle non modifiable)
- **Delete**: Suppression avec confirmation

**Persistence**: Agents stockés dans SurrealDB (table `agent`)

**Chargement**: Agents chargés automatiquement au démarrage via `load_agents_from_db()`

### Hiérarchie d'Agents
```
Agent Principal (Orchestrator)
├─ Agent Spécialisé 1 (permanent)
├─ Agent Spécialisé 2 (permanent)
└─ Agent Temporaire (lifecycle limité)
```

**Agent Principal**
- Orchestre les tâches complexes
- Délègue aux agents spécialisés
- Crée les agents temporaires
- Agrège les rapports
- Gère le cycle de vie des agents temporaires

**Agents Spécialisés** (permanents)
- Persistent state via SurrealDB
- Réutilisables cross-sessions

**Agents Temporaires**
- Créés pour tâches ponctuelles
- Auto-destruction après completion
- Pas de persistence state

## Communication Inter-Agent

### Protocol Standard: Markdown Reports

**Format Unifié**
```markdown
# Agent Report: [Agent_ID]
**Task**: [Description]
**Status**: ✅ Success | ❌ Failed | 🔄 In Progress
**Duration**: [temps]

## Results
[Données structurées]

## Tools Used
- `SurrealDBTool`: 3 queries (avg 45ms)
- `AnalyticsTool`: 1 aggregation (230ms)
- `CacheTool`: 2 hits, 1 miss

## MCP Servers Called
- `serena::find_symbol`: auth/user.rs → 12 symbols
- `context7::get_library_docs`: surrealdb/query → 4 examples

## Next Actions
- [ ] Action 1
- [ ] Action 2

## Metadata
- Provider: Mistral Large
- Tokens: Input 1.2K | Output 450
- Cost: €0.003
- Tools: 6 calls (280ms total)
- MCP: 2 servers (320ms total)
```

**Avantages**
- Human-readable & machine-parsable
- Chainable (output → input)
- Auditable

### Transport Layer

**Stdio** (agents locaux)
- Communication inter-process
- Performance optimale
- Synchronisation via channels Rust

## Création d'Agents

### Via Settings UI (Méthode Principale)

Les agents sont créés par l'utilisateur via l'interface Settings:

1. **Aller dans Settings > Agents**
2. **Cliquer "Create Agent"**
3. **Remplir le formulaire**:
   - Nom de l'agent (1-64 caractères)
   - Lifecycle (Permanent/Temporary)
   - Provider LLM (Mistral/Ollama)
   - Modèle (ex: mistral-large-latest)
   - Temperature (0.0-2.0)
   - Max tokens (256-128000)
   - Tools activés (MemoryTool, TodoTool)
   - MCP Servers (depuis ceux configurés)
   - System Prompt (instructions pour l'agent)

**Frontend Store** (`src/lib/stores/agents.ts`):
```typescript
import { agentStore } from '$lib/stores/agents';

// Créer un agent
const agentId = await agentStore.createAgent({
  name: 'My Agent',
  lifecycle: 'permanent',
  llm: { provider: 'Mistral', model: 'mistral-large-latest', temperature: 0.7, max_tokens: 4096 },
  tools: ['MemoryTool', 'TodoTool'],
  mcp_servers: ['serena'],
  system_prompt: 'You are a helpful assistant...'
});

// Lister les agents
await agentStore.loadAgents();
```

### Interface Rust

```rust
trait Agent {
    async fn execute(&self, task: Task) -> Report;
    fn capabilities(&self) -> Vec<Capability>;
    fn lifecycle(&self) -> Lifecycle;
    fn tools(&self) -> Vec<Tool>;
    fn mcp_servers(&self) -> Vec<String>;
    fn system_prompt(&self) -> String; // Instructions internes
    fn task_templates(&self) -> Vec<PromptTemplate>; // Templates tâches
}
```

**LLMAgent avec Tool Execution**
```rust
// Création avec support tools
let agent = LLMAgent::with_tools(
    config,
    provider.clone(),
    tool_factory.clone(),
    mcp_manager.clone()
);

// Exécution avec loop tool calls
let report = agent.execute_with_mcp(&task, mcp_manager).await?;
```

### Format Configuration TOML (Référence)

```toml
# agents/config/db_agent.toml
[agent]
id = "db_agent"
name = "Database Agent"
description = "Gestion requêtes et analytics DB"
lifecycle = "Permanent" # ou "Temporary"

[llm]
provider = "Mistral" # Phase 1: Mistral|Ollama
model = "mistral-large"
temperature = 0.7
max_tokens = 4096

[capabilities]
primary = ["DatabaseQuery", "Analytics"]
secondary = ["DataExport"]

[tools]
# MCP Tools custom exposés à l'agent
enabled = [
    "SurrealDBTool",
    "QueryBuilderTool",
    "AnalyticsTool"
]

[tools.SurrealDBTool]
connection = "ws://localhost:8000"
namespace = "zileo"
database = "chat"
permissions = ["SELECT", "CREATE", "UPDATE"] # pas DELETE

[tools.AnalyticsTool]
cache_ttl = 300 # 5min cache
max_aggregations = 10

[mcp_servers]
# MCP servers externes accessibles
enabled = ["serena", "context7"]
# Agents peuvent appeler ces MCP servers pour capabilities étendues

[mcp_servers.serena]
capabilities = ["find_symbol", "read_file", "search_pattern"]
scope = "project" # project|file|system

[mcp_servers.context7]
capabilities = ["get_library_docs"]
libraries = ["surrealdb", "tokio"]

[context]
max_history = 20 # messages
shared_pool = true # accès shared context
isolation_level = "agent" # agent|task|global

[monitoring]
metrics_enabled = true
trace_calls = true
log_level = "info"

[prompts]
# System prompt définissant rôle et comportement
system_prompt = """
You are a specialized Database Agent for the Zileo Chat application.

## Role
Expert in SurrealDB queries, data analytics, and database optimization.

## Expertise
- SurrealQL query construction and optimization
- Data aggregations and analytics
- Performance monitoring (slow queries, indexes)
- Schema validation and migrations

## Tools Usage
- `SurrealDBTool`: Direct DB access, use for all CRUD operations
  - Always use parameterized queries (prevent injection)
  - Respect permissions: SELECT, CREATE, UPDATE (no DELETE)
  - Timeout: 30s max per query

- `AnalyticsTool`: Use for aggregations, cache results 5min
  - Max 10 concurrent aggregations
  - Prefer pre-computed metrics when available

- `QueryBuilderTool`: Use for complex queries requiring validation
  - Validates syntax before execution
  - Suggests optimizations

## MCP Servers Usage
- `serena`: Use find_symbol to locate DB-related code before changes
  - Scope: project-wide search
  - Find schema definitions, query patterns

- `context7`: Get official SurrealDB documentation
  - Use for syntax reference, best practices
  - Libraries: surrealdb, tokio

## Constraints
- NEVER execute DELETE without explicit user confirmation
- ALWAYS validate input data before queries
- ALWAYS log slow queries (>100ms) for monitoring
- Return structured data in JSON format
- Include execution time in all reports

## Response Format
Generate markdown reports with:
- Query executed (sanitized)
- Results summary (count, time)
- Tools/MCP used with metrics
- Recommendations (indexes, optimizations)
"""

# Templates pour tâches courantes
[prompts.templates.query_users]
template = """
Task: Query users with filters
Filters: {filters}
Required fields: {fields}

Steps:
1. Use QueryBuilderTool to construct safe query
2. Execute via SurrealDBTool
3. Return results with count and execution time
"""

[prompts.templates.analytics]
template = """
Task: Generate analytics report
Metric: {metric}
Time range: {time_range}

Steps:
1. Check AnalyticsTool cache first
2. If miss, query via SurrealDBTool
3. Cache results (TTL 5min)
4. Format report with visualizable data
"""
```

**Exemples Configurations par Type**

```toml
# agents/config/api_agent.toml
[agent]
id = "api_agent"
lifecycle = "Permanent"

[tools]
enabled = ["HTTPClientTool", "RateLimiterTool", "CacheTool"]

[tools.HTTPClientTool]
timeout = 30
retry_attempts = 3
allowed_domains = ["api.example.com", "*.trusted.io"]

[mcp_servers]
enabled = ["playwright", "context7"]

[prompts]
system_prompt = """
You are an API Integration Agent specialized in external service communication.

## Role
Expert in REST/GraphQL APIs, rate limiting, caching strategies.

## Tools Usage
- `HTTPClientTool`: All external HTTP calls
  - Timeout: 30s, 3 retry attempts
  - Only call whitelisted domains
  - Log all 4xx/5xx errors

- `RateLimiterTool`: Enforce limits before calls
  - Check quota before each request
  - Implement exponential backoff

- `CacheTool`: Cache GET responses
  - TTL based on Cache-Control headers
  - Invalidate on related mutations

## MCP Servers Usage
- `playwright`: Validate API endpoints E2E
- `context7`: Get API client library docs

## Constraints
- NEVER expose API keys in logs/reports
- ALWAYS validate response schemas
- ALWAYS respect rate limits
- Implement circuit breaker (5 fails → pause 60s)
"""
```

```toml
# agents/config/ui_agent.toml
[agent]
id = "ui_agent"
lifecycle = "Temporary"
ttl = 3600

[llm]
provider = "Ollama" # Phase 1: Local, gratuit

[tools]
enabled = ["ComponentGeneratorTool", "A11yValidatorTool"]

[mcp_servers]
enabled = ["playwright", "context7"]

[prompts]
system_prompt = """
You are a UI Component Agent specialized in Svelte 5 components.

## Role
Expert in component generation, accessibility, responsive design.

## Tools Usage
- `ComponentGeneratorTool`: Generate Svelte 5 components
  - Follow project design system
  - Use runes syntax ($state, $derived, $effect)
  - TypeScript strict mode

- `A11yValidatorTool`: Validate WCAG AA compliance
  - Check semantic HTML
  - Validate ARIA labels
  - Test keyboard navigation

## MCP Servers Usage
- `playwright`: Visual regression tests, a11y audits
- `context7`: Get Svelte 5 official patterns

## Constraints
- ALWAYS generate accessible components (WCAG AA minimum)
- ALWAYS use semantic HTML
- ALWAYS include TypeScript types
- NEVER use deprecated Svelte syntax
- Components must be mobile-first responsive
"""

[prompts.templates.generate_form]
template = """
Task: Generate form component
Fields: {fields}
Validation: {validation_rules}

Steps:
1. Use context7 for Svelte 5 form patterns
2. Generate component with ComponentGeneratorTool
3. Validate accessibility with A11yValidatorTool
4. Test with playwright (keyboard navigation)
"""
```

### Système de Prompts

**Structure Prompt Complet**
```
[System Prompt de l'Agent]
+
[Contexte Partagé] (historique conversation, user preferences)
+
[Task Template] (si applicable)
+
[Task Spécifique] (paramètres utilisateur)
```

**Anatomy System Prompt**
```markdown
## Role
Définition claire: qui est l'agent, son domaine d'expertise

## Expertise
Compétences techniques spécifiques, domaines de connaissance

## Tools Usage
Pour chaque tool:
  - Quand l'utiliser
  - Comment l'utiliser (paramètres, contraintes)
  - Limites et timeouts

## MCP Servers Usage
Pour chaque MCP:
  - Capabilities utilisées
  - Patterns d'utilisation
  - Scope et limitations

## Constraints
Règles strictes (NEVER/ALWAYS)
Validations requises
Limites de sécurité

## Response Format
Structure attendue des rapports
Métriques à inclure
Format données (JSON, MD, etc.)
```

**Templates de Tâches**

Templates réutilisables pour opérations courantes avec placeholders:
```toml
[prompts.templates.crud_operation]
template = """
Task: {operation} on {entity}
Data: {data}
Validation: {rules}

Steps:
1. Validate input with {validation_tool}
2. Execute {operation} via {execution_tool}
3. Log operation with metadata
4. Return result with {format}
"""
variables = ["operation", "entity", "data", "rules", "validation_tool", "execution_tool", "format"]
```

**Composition Dynamique**

```rust
fn compose_prompt(agent: &Agent, task: &Task, context: &Context) -> String {
    let mut prompt = agent.system_prompt();

    // Ajouter contexte partagé
    if context.shared_pool {
        prompt.push_str(&format!("\n## Shared Context\n{}", context.history));
    }

    // Ajouter template si match
    if let Some(template) = agent.find_template(&task.type) {
        prompt.push_str(&template.render(&task.params));
    }

    // Ajouter task spécifique
    prompt.push_str(&format!("\n## Current Task\n{}", task.description));

    prompt
}
```

**Exemples Prompts par Spécialisation**

```toml
# agents/config/rag_agent.toml
[prompts]
system_prompt = """
You are a RAG (Retrieval-Augmented Generation) Agent.

## Role
Expert in semantic search, context retrieval, and relevance ranking.

## Expertise
- Vector embeddings generation (Mistral, Ollama)
- Semantic similarity search
- Hybrid search (keyword + vector)
- Context window optimization

## Tools Usage
- `EmbeddingsTool`: Generate embeddings for queries and documents
  - Model: text-embedding-3-small
  - Dimensions: 1536
  - Batch size: max 100 documents

- `VectorSearchTool`: Search in vector database
  - Algorithm: HNSW
  - top_k: configurable (default 5)
  - Distance: cosine similarity

## MCP Servers Usage
- `serena`: Find relevant code snippets semantically
  - search_pattern with semantic mode
- `context7`: Enrich results with official docs

## Constraints
- ALWAYS generate embeddings for user query first
- ALWAYS rank results by relevance score
- Include relevance scores in report (threshold: 0.7)
- Maximum context size: 8K tokens
"""

[prompts.templates.semantic_search]
template = """
Task: Semantic search for "{query}"
Top K: {top_k}
Filters: {filters}

Steps:
1. Generate query embedding via EmbeddingsTool
2. Search vectors via VectorSearchTool (top_k={top_k})
3. Filter by relevance threshold (>0.7)
4. Enrich top results with context7 if applicable
5. Return ranked results with scores
"""
```

```toml
# agents/config/code_agent.toml
[prompts]
system_prompt = """
You are a Code Quality Agent specialized in refactoring and optimization.

## Role
Expert in code analysis, refactoring, and quality improvements.

## Expertise
- AST-based refactoring (safe transformations)
- Code smell detection
- Performance optimizations
- Pattern enforcement

## Tools Usage
- `RefactorTool`: AST-based code transformations
  - safe_mode: enabled by default
  - Supports: extract method, inline, rename
  - Validates syntax before/after

## MCP Servers Usage
- `serena`: Symbol-level operations
  - find_symbol: locate refactor targets
  - find_referencing_symbols: impact analysis
  - rename_symbol: safe renaming with refs

- `context7`: Best practices and patterns
  - Language-specific style guides
  - Framework conventions

## Constraints
- NEVER refactor without running tests after
- ALWAYS use serena to find all references before rename
- ALWAYS preserve functionality (behavior-preserving)
- Safe mode CANNOT be disabled
- Maximum refactor scope: single file (use multi-agent for larger)
"""
```

**Best Practices Prompts**

1. **Spécificité**: Définir rôle précis, pas générique
2. **Tools First**: Expliquer QUAND et COMMENT utiliser chaque tool/MCP
3. **Contraintes Claires**: NEVER/ALWAYS pour règles strictes
4. **Format Structuré**: Sections standardisées (Role, Tools, Constraints)
5. **Templates Réutilisables**: Factoriser opérations courantes
6. **Variables Explicites**: Documenter placeholders dans templates
7. **Validation**: Inclure étapes de validation dans prompts
8. **Metrics**: Demander métriques spécifiques dans rapports

### Registry Pattern

```rust
AgentRegistry::register("db_agent", DBAgent::new());
AgentRegistry::spawn_temporary("task_123", TaskAgent::new());
AgentRegistry::get("db_agent").execute(task);
AgentRegistry::cleanup_temporary();
```

## Workflow Multi-Agent

### Règle Architecture Critique

**⚠️ LIMITATION SOUS-AGENTS** : Les sous-agents NE PEUVENT PAS lancer d'autres sous-agents

**Raison** :
- Réutilisabilité code maximale
- Contrôle centralisé orchestration
- Évite récursion complexe
- Simplifie debugging et traçabilité

**Seul l'Agent Principal (Orchestrateur)** peut spawner et coordonner des sous-agents, tools et MCP servers.

**Voir** : [WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md) pour détails sur orchestration intra-workflow

### Orchestration

**1. Task Decomposition** (Agent Principal uniquement)
```
Complex Task → [SubTask1, SubTask2, SubTask3]
```

**2. Agent Assignment**
```
SubTask1 → DB Agent (permanent)
SubTask2 → API Agent (permanent)
SubTask3 → Custom Agent (temporaire)
```

**3. Analyse Dépendances** (Parallel vs Sequential)
```
Si pas dépendances → Parallel
Si dépendances données → Sequential
```

**4. Parallel Execution** (opérations indépendantes)
```rust
let reports = join_all(vec![
    agent1.execute(task1),
    agent2.execute(task2),
    agent3.execute(task3),
]).await;
```

**5. Sequential Execution** (opérations dépendantes)
```rust
let result1 = agent1.execute(task1).await?;
let result2 = agent2.execute(task2_needs(result1)).await?;
let result3 = agent3.execute(task3_needs(result2)).await?;
```

**6. Report Aggregation**
```
[Report1, Report2, Report3] → Unified Report (MD)
```

**7. Cleanup**
```
Temporary agents → destroy()
Reports → SurrealDB storage
Metrics → monitoring
```

### Communication Patterns

**Request/Response**
```
Principal → Task → Agent Spécialisé
Agent Spécialisé → Report (MD) → Principal
```

**Event-Driven**
```
Agent → Event → Event Bus → Subscribers
```

**Streaming**
```
Agent → Stream<Chunk> → Principal (SSE)
```

### Reprise sur Erreur et Idempotence

Pour garantir la robustesse des workflows, notamment lors d'erreurs passagères (ex: réseau), le système intègre des stratégies de reprise.

- **Idempotence**: Chaque sous-tâche déléguée par l'orchestrateur se voit assigner un identifiant unique. Les agents utilisent cet identifiant pour s'assurer qu'une opération n'est pas exécutée plusieurs fois en cas de relance.

- **Politique de Reprise**: L'orchestrateur peut être configuré pour relancer automatiquement une tâche échouée, souvent avec un délai progressif (ex: exponential backoff) pour ne pas surcharger un service externe.

- **Journal des Tâches**: Un suivi persistant des tâches et de leur statut (ex: dans SurrealDB) permet à l'orchestrateur de ne reprendre que les étapes qui n'ont pas encore été complétées avec succès.

## State Management

### Agent State

**Permanent Agents** → SurrealDB
```sql
DEFINE TABLE agent_state SCHEMAFULL;
DEFINE FIELD agent_id ON agent_state TYPE string;
DEFINE FIELD state ON agent_state TYPE object;
DEFINE FIELD updated_at ON agent_state TYPE datetime;
```

**Temporary Agents** → In-memory (Tokio)
```rust
HashMap<AgentId, AgentState> // cleanup on destroy
```

### Conversation Context

**Shared Context Pool**
- Accessible à tous agents
- Évite redondance contexte
- Optimise token usage

**Agent-Specific Context**
- Isolé par agent
- Sécurité & privacy
- Cleanup automatique

## Extensibilité

### Ajouter Nouveau Agent

**1. Définir Capabilities**
```rust
enum Capability {
    DatabaseQuery,
    APICall,
    EmailSend, // nouveau
}
```

**2. Implémenter Trait**
```rust
struct EmailAgent;
impl Agent for EmailAgent {
    async fn execute(&self, task: Task) -> Report { /* */ }
    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::EmailSend]
    }
    fn lifecycle(&self) -> Lifecycle { Lifecycle::Permanent }
}
```

**3. Configuration**
```toml
# agents/config/email_agent.toml
id = "email_agent"
provider = "Mistral"
capabilities = ["EmailSend"]
lifecycle = "Permanent"
tools = ["SMTPTool"]
```

**4. Register**
```rust
AgentRegistry::register("email_agent", EmailAgent::new());
```

### Provider Switching

Change provider sans modifier agent logic:
```toml
# config avant
provider = "Mistral"

# config après
provider = "Ollama"
```

Agent interface reste identique grâce à abstraction Rig.rs.
**Phase 1** : Mistral ↔ Ollama

## Monitoring & Observability

### Métriques par Agent

```markdown
# Agent Metrics: db_agent
- Tasks executed: 142
- Success rate: 98.5%
- Avg duration: 230ms
- Token usage: 45K
- Cost: €0.23
- Errors: 2 (timeout)

## Tools Usage
- SurrealDBTool: 89 calls (avg 42ms) - 98% success
- AnalyticsTool: 34 calls (avg 180ms) - 100% success
- CacheTool: 156 calls - 87% hit rate

## MCP Servers Usage
- serena: 23 calls (avg 120ms)
  - find_symbol: 15 calls
  - read_file: 8 calls
- context7: 12 calls (avg 340ms)
  - get_library_docs: 12 calls
```

### Health Checks

```rust
AgentRegistry::health_check("db_agent") → AgentHealth {
    status: Healthy | Degraded | Down,
    last_success: DateTime,
    error_rate: f32,
}
```

### Distributed Tracing

- Request ID propagation
- Agent call chain tracking
- Performance bottleneck identification

## Exécution des Tools (v1.0)

### Format Tool Calls

Les agents utilisent un format XML pour appeler les tools:

**Appel Tool**:
```xml
<tool_call name="MemoryTool">
{"operation": "add", "type": "knowledge", "content": "Important info"}
</tool_call>
```

**Résultat Tool**:
```xml
<tool_result name="MemoryTool" success="true">
{"id": "mem_abc123", "message": "Memory added successfully"}
</tool_result>
```

### Boucle d'Exécution

L'agent LLM exécute une boucle jusqu'à ce qu'il n'y ait plus d'appels tools:

1. **Build System Prompt**: Injection des définitions tools disponibles
2. **Appel LLM**: Envoie le prompt au provider (Mistral/Ollama)
3. **Parse Tool Calls**: Extraction des balises `<tool_call>` de la réponse
4. **Exécution Tools**:
   - Tools locaux via `ToolFactory` (MemoryTool, TodoTool)
   - Tools MCP via `MCPManager`
5. **Format Results**: Conversion en `<tool_result>` XML
6. **Feedback Loop**: Retour des résultats au LLM pour continuation
7. **Répéter** jusqu'à 10 itérations max ou pas de tool calls

### Tools Disponibles

| Tool | Description | Opérations |
|------|-------------|------------|
| **MemoryTool** | Persistence vectorielle | add, get, list, search, delete, clear_by_type |
| **TodoTool** | Gestion tâches workflow | create, get, update_status, list, complete, delete |

## Sélection Intelligente Tools & MCP

### Decision Matrix

Les tools disponibles : MemoryTool et TodoTool (via ToolFactory)
Les MCP servers sont ajoutés par l'utilisateur via Settings.

### Agent Auto-Selection

```rust
// Agent choisit tool ou MCP selon contexte
impl Agent {
    async fn select_capability(&self, need: Need) -> Capability {
        match need {
            Need::DatabaseQuery => {
                if self.has_tool("SurrealDBTool") {
                    Capability::Tool("SurrealDBTool")
                } else {
                    Capability::Fallback // erreur
                }
            },
            Need::CodeSearch => {
                if self.has_mcp("serena") {
                    Capability::MCP("serena::find_symbol")
                } else {
                    Capability::Tool("GrepTool") // fallback
                }
            }
        }
    }
}
```

## Supervision et Intervention Humaine (Human-in-the-Loop)

Pour garantir la sécurité et la pertinence des actions critiques (ex: suppression de données, modifications de fichiers), l'architecture formalise un mécanisme de validation par l'utilisateur.

Le processus est le suivant :
1.  **Déclenchement**: Lorsqu'un agent doit exécuter une tâche sensible, il utilise un `ConfirmationTool` dédié.
2.  **Mise en Pause**: L'exécution de la tâche est suspendue. Son statut passe à `WaitingForInput` et une notification est envoyée à l'interface utilisateur.
3.  **Validation Utilisateur**: L'interface présente la demande de confirmation. La décision de l'utilisateur (approuver ou rejeter) est renvoyée à l'orchestrateur.
4.  **Reprise ou Annulation**: L'agent reçoit la réponse et poursuit l'opération uniquement en cas d'approbation. Sinon, il l'annule.

Ce mécanisme assure que l'utilisateur final conserve toujours le contrôle sur les opérations importantes.
L'utilisateur peux rentrer un message dans le input et validé. le message se met à la suite de la tache suivante et est intégré dans le processus agentique.

## Sécurité

### Isolation

- Sandboxing tools per agent
- Permission-based tool access
- Input validation strict

### Audit Trail

```markdown
# Agent Audit: api_agent
[2025-11-22 14:32] Task: external_api_call
[2025-11-22 14:32] Tool: HTTPClient → api.example.com
[2025-11-22 14:33] Status: ✅ Success (245ms)
[2025-11-22 14:33] Report: saved → reports/api_agent_20251122_143201.md
```

### Rate Limiting

- Per-agent limits (évite abuse)
- Per-provider limits (coûts)
- Fallback cascade si limite atteinte

## Architecture Fichiers

```
zileo-chat-3/
├─ src-tauri/
│  ├─ agents/
│  │  ├─ core/
│  │  │  ├─ agent.rs          # Trait Agent
│  │  │  ├─ registry.rs       # AgentRegistry
│  │  │  ├─ orchestrator.rs   # AgentOrchestrator
│  │  │  ├─ lifecycle.rs      # Lifecycle management
│  │  │  └─ prompt.rs         # Prompt composition
│  │  ├─ specialized/
│  │  │  ├─ db_agent.rs
│  │  │  ├─ api_agent.rs
│  │  │  ├─ rag_agent.rs
│  │  │  ├─ ui_agent.rs
│  │  │  └─ code_agent.rs
│  │  ├─ config/              # Configurations TOML
│  │  │  ├─ db_agent.toml
│  │  │  ├─ api_agent.toml
│  │  │  ├─ rag_agent.toml
│  │  │  ├─ ui_agent.toml
│  │  │  └─ templates/
│  │  │     └─ agent_template.toml
│  │  └─ prompts/             # System prompts & templates
│  │     ├─ db_agent.md       # System prompt DB agent
│  │     ├─ api_agent.md
│  │     ├─ rag_agent.md
│  │     ├─ ui_agent.md
│  │     ├─ code_agent.md
│  │     └─ templates/        # Task templates réutilisables
│  │        ├─ db/
│  │        │  ├─ query.md
│  │        │  ├─ analytics.md
│  │        │  └─ migration.md
│  │        ├─ api/
│  │        │  ├─ rest_call.md
│  │        │  └─ graphql.md
│  │        └─ shared/
│  │           ├─ error_handling.md
│  │           └─ validation.md
│  ├─ reports/                # Rapports MD générés par agents
│  │  ├─ db_agent/
│  │  ├─ api_agent/
│  │  └─ archive/             # Rapports archivés (>30j)
│  └─ tools/                  # MCP tools custom
│     ├─ surrealdb_tool.rs
│     ├─ http_client_tool.rs
│     └─ embeddings_tool.rs
└─ docs/
   └─ agents/
      ├─ README.md            # Guide agents
      ├─ creating_agents.md   # Comment créer nouvel agent
      └─ prompt_guidelines.md # Best practices prompts
```

**Organisation Prompts**

```markdown
# agents/prompts/db_agent.md (System Prompt)
---
agent_id: db_agent
version: 1.0.0
updated: 2025-11-22
---

You are a specialized Database Agent...
[Contenu complet du system prompt]
```

```markdown
# agents/prompts/templates/db/query.md (Task Template)
---
template_id: db_query
category: database
variables: [filters, fields, limit, offset]
---

Task: Query database with filters
Filters: {{filters}}
Required fields: {{fields}}
Limit: {{limit}} | Offset: {{offset}}

Steps:
1. Validate filters with QueryBuilderTool
2. Build safe query (parameterized)
3. Execute via SurrealDBTool
4. Return results with metadata (count, time, cache_hit)
```

### Mapping Agent → Tools/MCP

```toml
# Référence rapide configuration par agent

[db_agent]
tools = ["SurrealDBTool", "QueryBuilderTool", "AnalyticsTool", "CacheTool"]
mcp = ["serena", "context7"]

[api_agent]
tools = ["HTTPClientTool", "RateLimiterTool", "CacheTool", "AuthTool"]
mcp = ["playwright", "context7"]

[rag_agent]
tools = ["EmbeddingsTool", "VectorSearchTool", "CacheTool"]
mcp = ["serena", "context7"]

[ui_agent]
tools = ["ComponentGeneratorTool", "A11yValidatorTool"]
mcp = ["playwright", "context7"]

[code_agent]
tools = ["RefactorTool"]
mcp = ["serena", "context7"]

[migration_agent]
tools = ["SchemaValidatorTool"]
mcp = ["serena"]
```

## Références

**Frameworks Rust**
- Rig.rs: Agent framework + multi-provider
- Swarms-rs: Enterprise multi-agent orchestration
- Ractor: Actor model pour Rust

**Protocols**
- MCP 2025-03-26: Communication standardisée
- A2A Protocol: Agent-to-Agent communication
- JSON-RPC 2.0: Message exchange

**Patterns**
- Actor Model: Isolation + message-passing
- Factory Pattern: Création agents uniformisée
- Registry Pattern: Découverte dynamique
- Strategy Pattern: Provider switching
- Chain of Responsibility: Tool chains
