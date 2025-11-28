# Rapport - Correction outils sub-agent pour agent principal

## Metadata
- **Date**: 2025-11-28
- **Complexite**: medium
- **Stack**: Svelte 5.43 + Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objectif

Corriger le probleme ou l'agent principal (workflow starter) ne peut pas utiliser les outils de creation de sous-agents (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool).

## Contexte

L'utilisateur a signale que l'agent principal, lorsqu'on lui demande "quel tool dispose tu pour creer un agent ?", ne voyait pas les outils SpawnAgentTool, DelegateTaskTool, et ParallelTasksTool dans sa liste d'outils disponibles.

## Analyse du Probleme

### Cause Racine

1. **`LLMAgent.create_local_tools()`** utilisait `factory.create_tools()` qui ne cree que les outils de base (MemoryTool, TodoTool)
2. **Pas de `AgentToolContext`** disponible pour l'agent lors de la creation
3. **Pas de mecanisme** pour distinguer l'agent principal des sous-agents lors de l'execution

### Flux Avant Correction

```
streaming.rs --> orchestrator.execute_with_mcp() --> LLMAgent.execute_with_mcp()
                                                          |
                                                          v
                                                   create_local_tools(workflow_id)
                                                          |
                                                          v
                                                   factory.create_tools()  <-- Seulement MemoryTool, TodoTool!
```

## Solution Implementee

### 1. Modification de LLMAgent

**Nouveau champ:**
```rust
pub struct LLMAgent {
    config: AgentConfig,
    provider_manager: Arc<ProviderManager>,
    tool_factory: Option<Arc<ToolFactory>>,
    agent_context: Option<AgentToolContext>,  // NOUVEAU
}
```

**Nouveau constructeur:**
```rust
pub fn with_context(
    config: AgentConfig,
    provider_manager: Arc<ProviderManager>,
    tool_factory: Arc<ToolFactory>,
    agent_context: AgentToolContext,
) -> Self
```

**Mise a jour de `create_local_tools()`:**
```rust
fn create_local_tools(
    &self,
    workflow_id: Option<String>,
    is_primary_agent: bool,  // NOUVEAU parametre
) -> Vec<Arc<dyn Tool>> {
    // Si agent principal avec context, utiliser create_tools_with_context()
    if is_primary_agent {
        if let Some(ref context) = self.agent_context {
            return factory.create_tools_with_context(
                &self.config.tools,
                workflow_id,
                self.config.id.clone(),
                Some(context.clone()),
                true,
            );
        }
    }
    // Sinon, outils de base uniquement
    factory.create_tools(&self.config.tools, workflow_id, self.config.id.clone())
}
```

### 2. Modification de commands/agent.rs

Tous les agents sont maintenant crees avec `AgentToolContext`:

```rust
// Avant
let tool_factory = Arc::new(ToolFactory::new(state.db.clone(), None));
let llm_agent = LLMAgent::with_factory(agent_config, state.llm_manager.clone(), tool_factory);

// Apres
let agent_context = AgentToolContext::from_app_state_full(state.inner());
let llm_agent = LLMAgent::with_context(
    agent_config,
    state.llm_manager.clone(),
    state.tool_factory.clone(),
    agent_context,
);
```

### 3. Modification de streaming.rs

Le contexte de tache inclut maintenant `is_primary_agent`:

```rust
let history_context = serde_json::json!({
    "conversation_history": formatted_history,
    "is_primary_agent": true,  // NOUVEAU
    "workflow_id": validated_workflow_id.clone()
});
```

### Flux Apres Correction

```
streaming.rs --> task.context = { is_primary_agent: true }
                      |
                      v
              orchestrator.execute_with_mcp()
                      |
                      v
              LLMAgent.execute_with_mcp()
                      |
                      v
              is_primary_agent = task.context["is_primary_agent"]  // true
                      |
                      v
              create_local_tools(workflow_id, is_primary_agent=true)
                      |
                      v
              factory.create_tools_with_context()  <-- Inclut SpawnAgentTool, etc.!
```

## Fichiers Modifies

### Backend (Rust)

| Fichier | Modifications |
|---------|---------------|
| `src-tauri/src/agents/llm_agent.rs` | Ajout champ `agent_context`, constructeur `with_context()`, mise a jour `create_local_tools()` |
| `src-tauri/src/commands/agent.rs` | Utilisation de `with_context()` au lieu de `with_factory()` |
| `src-tauri/src/commands/streaming.rs` | Ajout `is_primary_agent: true` dans le contexte de tache |

## Validation

### Tests Backend
```
cargo clippy -- -D warnings  # PASS
cargo test                   # 474 tests PASS
```

### Tests Frontend
```
npm run lint                 # PASS
npm run check               # PASS
```

## Comportement Attendu

### Agent Principal (Workflow Starter)
- A acces a: MemoryTool, TodoTool, SpawnAgentTool, DelegateTaskTool, ParallelTasksTool
- Peut creer des sous-agents via SpawnAgentTool
- Peut deleguer des taches via DelegateTaskTool

### Sous-Agent (Cree par SpawnAgentTool)
- A acces a: MemoryTool, TodoTool (outils de base seulement)
- NE PEUT PAS creer d'autres sous-agents (single level constraint)
- Task context contient `"is_sub_agent": true` (pas `is_primary_agent`)

## Contraintes Respectees

- **Single Level Only**: Les sous-agents ne peuvent pas creer d'autres sous-agents
- **Maximum 3 Sub-Agents**: Limite par workflow (geree par SpawnAgentTool/DelegateTaskTool)
- **Primary Agent Only**: Seul l'agent starter a acces aux outils de sub-agents

## Prochaines Etapes

1. Tester manuellement en creant un agent et en lui demandant la liste de ses outils
2. Verifier que SpawnAgentTool fonctionne correctement
3. Phase F: Tests & Documentation (selon la spec)
