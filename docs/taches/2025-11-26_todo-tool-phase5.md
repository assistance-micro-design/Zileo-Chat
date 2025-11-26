# Rapport - Todo Tool Phase 5: Tool Framework et TodoTool

## Metadata
- **Date**: 2025-11-26
- **Complexite**: medium
- **Stack**: Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objectif

Implementer la Phase 5 de la spec `docs/specs/2025-11-25_spec-todo-tool.md`:
- Creer le Tool Framework (`src-tauri/src/tools/mod.rs`)
- Implementer TodoTool pour la gestion de taches par les agents LLM

## Travail Realise

### Fonctionnalites Implementees

1. **Tool Framework** (`src-tauri/src/tools/mod.rs`)
   - Trait `Tool` avec methodes `definition()`, `execute()`, `validate_input()`
   - Types `ToolDefinition`, `ToolError`, `ToolResult<T>`
   - Re-exports de tous les tools (DB + Todo)

2. **TodoTool** (`src-tauri/src/tools/todo/`)
   - Operations: create, get, update_status, list, complete, delete
   - Scope par workflow_id et agent_id
   - Description LLM-friendly pour tool selection
   - Validation des inputs avec JSON Schema

3. **Corrections existantes**
   - Fix imports dans tools DB (ToolDefinition maintenant dans tools/mod.rs)
   - Fix format string dans query_builder.rs (RELATE query)

### Fichiers Crees

| Fichier | Description |
|---------|-------------|
| `src-tauri/src/tools/mod.rs` | Framework avec Tool trait, types, re-exports |
| `src-tauri/src/tools/todo/mod.rs` | Module export pour TodoTool |
| `src-tauri/src/tools/todo/tool.rs` | Implementation complete de TodoTool |

### Fichiers Modifies

| Fichier | Action |
|---------|--------|
| `src-tauri/src/main.rs` | Ajoute `mod tools` |
| `src-tauri/src/tools/db/surrealdb_tool.rs` | Fix imports, #[allow(dead_code)] |
| `src-tauri/src/tools/db/query_builder.rs` | Fix imports et format string |
| `src-tauri/src/tools/db/analytics.rs` | Fix imports |

### Statistiques Git

```
7 files changed, 60 insertions(+), 14 deletions(-)

Nouveaux fichiers (untracked):
- src-tauri/src/tools/mod.rs
- src-tauri/src/tools/todo/mod.rs
- src-tauri/src/tools/todo/tool.rs
```

### Types Crees

**Rust** (`src-tauri/src/tools/mod.rs`):
```rust
pub struct ToolDefinition {
    pub id: String,
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub output_schema: Value,
    pub requires_confirmation: bool,
}

pub type ToolResult<T> = Result<T, ToolError>;

pub enum ToolError {
    InvalidInput(String),
    ExecutionFailed(String),
    NotFound(String),
    PermissionDenied(String),
    Timeout(String),
    DatabaseError(String),
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    async fn execute(&self, input: Value) -> ToolResult<Value>;
    fn validate_input(&self, input: &Value) -> ToolResult<()>;
    fn requires_confirmation(&self) -> bool { false }
}
```

**TodoTool** (`src-tauri/src/tools/todo/tool.rs`):
```rust
pub struct TodoTool {
    db: Arc<DBClient>,
    workflow_id: String,
    agent_id: String,
}

// Operations:
// - create: Cree une tache avec name, description, priority
// - get: Recupere une tache par ID
// - update_status: Met a jour le status (pending/in_progress/completed/blocked)
// - list: Liste les taches (optionnel: filter par status)
// - complete: Marque comme complete avec duration_ms optionnel
// - delete: Supprime une tache
```

## Decisions Techniques

### Architecture

1. **ToolDefinition dans tools/mod.rs**
   - Initialement prevu dans models/, mais plus logique avec le trait Tool
   - Permet aux outils d'importer tout depuis `crate::tools`

2. **Scoping TodoTool par workflow/agent**
   - Chaque instance est liee a un workflow_id et agent_id
   - Permet le tracking multi-agent

3. **Description LLM-friendly**
   - Description detaillee avec examples JSON
   - Best practices integrees pour guider le LLM

### Patterns Utilises

- **Builder Pattern**: pour TaskCreate.with_agent().with_dependencies()
- **Async Trait**: Tool est async-safe et thread-safe
- **Input Schema**: JSON Schema OpenAPI 3.0 pour validation

## Validation

### Tests Backend
- **Clippy**: PASS (0 erreurs avec -D warnings)
- **Cargo test**: 285/285 PASS
- **Cargo fmt**: PASS

### Qualite Code
- Types stricts (ToolResult<T>, ToolError)
- Documentation Rustdoc complete
- #[allow(dead_code)] pour code futur (Phase 6 integration)

## Notes

Les tools sont marques `#[allow(dead_code)]` car ils seront integres aux agents dans la Phase 6 (Integration). Le code est complet et pret pour l'integration avec:
- `LLMAgent::execute_with_mcp()`
- Tool discovery au runtime
- Tool execution via le trait unifie

## Prochaines Etapes

1. **Phase 6**: Integration des tools aux agents
   - Modifier AgentConfig pour supporter Vec<Box<dyn Tool>>
   - Implementer tool discovery dans LLMAgent
   - Ajouter tool execution loop

2. **Frontend**: Composant TaskList Svelte pour affichage
