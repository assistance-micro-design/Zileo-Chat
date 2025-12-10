# Plan d'Optimisation - TodoTool

## Metadata
- **Date**: 2025-12-09
- **Domaine**: tools/TodoTool
- **Stack**: Rust 1.91.1 + Tauri 2.9.3 + SurrealDB 2.4.0 + Rig.rs 0.24.0
- **Impact estime**: Securite (CRITIQUE) / Performance / Maintenabilite

## Resume Executif

Le TodoTool presente des **vulnerabilites SQL injection critiques** dues a l'utilisation de `format!()` au lieu de queries parametrees. Ce plan priorise la securisation via `ParamQueryBuilder` (pattern OPT-MEM-5 deja utilise dans MemoryTool), l'optimisation des queries N+1, et l'ajout de tests d'integration. Score actuel: 6.5/10, cible: 8.5/10.

## Etat Actuel

### Analyse du Code

| Fichier | Complexite | Points d'attention |
|---------|------------|-------------------|
| `src-tauri/src/tools/todo/tool.rs:168` | Haute | SQL injection via format!() dans update_status |
| `src-tauri/src/tools/todo/tool.rs:181` | Haute | Status insere sans parametrage |
| `src-tauri/src/tools/todo/tool.rs:213-223` | Haute | list_tasks avec workflow_id et status non-parametres |
| `src-tauri/src/tools/todo/tool.rs:245-260` | Moyenne | get_task avec task_id non-parametre |
| `src-tauri/src/tools/todo/tool.rs:289,317` | Haute | complete_task avec task_id et duration non-parametres |
| `src-tauri/src/tools/todo/tool.rs:164-178` | Moyenne | N+1 queries (3 requetes au lieu de 2) |

### Patterns Identifies

**Bons patterns:**
- Validation via `tools/utils.rs` (validate_not_empty, validate_length, validate_range, validate_enum_value)
- Constantes centralisees dans `tools/constants.rs` (todo::MAX_NAME_LENGTH, VALID_STATUSES, etc.)
- ResponseBuilder pour reponses JSON coherentes
- Tracing/logging avec `#[instrument]`
- Event streaming via emit_task_event()

**Patterns problematiques:**
- `format!()` pour construire queries SQL (vs query_with_params)
- N+1 queries dans update_status (ensure_exists + get_name + update)
- db_error() utilise 1 seule fois (inconsistance)
- Pas de LIMIT dans list_tasks du Tool (existe dans commands)

### Metriques Actuelles

| Metrique | TodoTool | MemoryTool (reference) |
|----------|----------|------------------------|
| Lignes de code | 700 | 1716 |
| Tests unitaires | 6 | 13 |
| Couverture operations | 46% | ~50% |
| Queries parametrees | 0% | 100% |
| Score maturite | 6.5/10 | 8.5/10 |

## Best Practices (2024-2025)

### Sources Consultees
- [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)
- [Rig.rs Tools Documentation](https://docs.rig.rs/docs/concepts/tools)
- [Building Effective Agents - Anthropic](https://www.anthropic.com/research/building-effective-agents)
- [Rust Async Best Practices 2025](https://tmandry.gitlab.io/blog/posts/making-async-reliable/)

### Patterns Recommandes

1. **Parameterized Queries (OBLIGATOIRE)**: Toujours utiliser `query_with_params()` ou `ParamQueryBuilder` pour user input
2. **Query Limits**: Inclure LIMIT sur toutes les operations list pour prevenir explosion memoire
3. **Champ booleen optimise**: Ajouter `is_completed: bool` pour WHERE optimise (vs enum status)
4. **Structured Errors**: Types d'erreur custom avec retry logic pour erreurs transitoires
5. **Tool Server Pattern (Rig.rs v0.22+)**: Eviter Arc<RwLock<T>> pour prevenir deadlocks

### Anti-Patterns a Eviter

1. **String interpolation dans queries**: `format!("WHERE id = '{}'", user_input)` -> SQL injection
2. **SELECT sans LIMIT**: Risque explosion memoire sur grandes tables
3. **N+1 queries**: Plusieurs requetes quand une suffirait
4. **block_on dans async**: Risque deadlock avec runtime single-thread

## Contraintes du Projet

- **Decision ADR-SEC-1**: Parameterized queries obligatoires - Source: `docs/ARCHITECTURE_DECISIONS.md`
- **Decision ADR-DB-1**: LIMIT sur toutes les list queries (OPT-DB-8) - Source: `CLAUDE.md`
- **Pattern OPT-MEM-5**: Reference implementation dans MemoryTool - Source: `tools_refactoring_complete` memory
- **Tauri IPC**: snake_case Rust / camelCase TypeScript conversion automatique

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible) - P1

#### OPT-TODO-1: Parameterized queries pour update_status()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:160-203`
- **Changement**: Remplacer format!() par query_with_params()
- **Benefice**: Securite - Prevention SQL injection
- **Risque regression**: Faible
- **Validation**: `cargo test`, verifier meme comportement

```rust
// AVANT (vulnerable):
let task_query = format!("SELECT name FROM task WHERE meta::id(id) = '{}'", task_id);
let update_query = format!("UPDATE task:`{}` SET status = '{}'", task_id, status);

// APRES (securise):
let params = vec![("task_id".to_string(), json!(task_id))];
let task_query = "SELECT name FROM task WHERE meta::id(id) = $task_id";
let task_data: Vec<Value> = self.db.query_with_params(task_query, params.clone()).await?;

let update_params = vec![
    ("task_id".to_string(), json!(task_id)),
    ("status".to_string(), json!(status)),
];
self.db.execute_with_params(
    "UPDATE task:`$task_id` SET status = $status",
    update_params
).await?;
```

#### OPT-TODO-2: Parameterized queries pour list_tasks()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:210-237`
- **Changement**: Utiliser ParamQueryBuilder pour construire query
- **Benefice**: Securite - Prevention SQL injection
- **Risque regression**: Faible
- **Validation**: Memes resultats avec differents filtres

```rust
// AVANT:
let query = format!(
    r#"SELECT ... FROM task WHERE workflow_id = '{}' AND status = '{}'"#,
    self.workflow_id, status
);

// APRES:
use crate::tools::utils::ParamQueryBuilder;
let mut builder = ParamQueryBuilder::new("task")
    .select(&["name", "description", "status", "priority", "agent_assigned", "created_at"])
    .where_eq_param("workflow_id", "wf_id", json!(self.workflow_id.clone()));

if let Some(status) = status_filter {
    builder = builder.where_eq_param("status", "status_filter", json!(status));
}
builder = builder.order_by("priority", false).order_by("created_at", false);

let (query, params) = builder.build();
let tasks: Vec<Value> = self.db.query_with_params(&query, params).await?;
```

#### OPT-TODO-3: Parameterized queries pour complete_task()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:286-343`
- **Changement**: Parametrer task_id et duration_ms
- **Benefice**: Securite
- **Risque regression**: Faible

#### OPT-TODO-4: Parameterized queries pour get_task()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:243-279`
- **Changement**: Parametrer task_id dans SELECT
- **Benefice**: Securite
- **Risque regression**: Faible

#### OPT-TODO-10: Ajouter LIMIT dans list_tasks() du Tool
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:210-237`
- **Changement**: Ajouter `.limit(query_limits::DEFAULT_LIST_LIMIT)`
- **Benefice**: Prevention explosion memoire (OPT-DB-8 compliance)
- **Risque regression**: Faible
- **Validation**: Verifier que commands utilise deja cette limite

```rust
use crate::tools::constants::query_limits;
// Dans builder:
builder = builder.limit(query_limits::DEFAULT_LIST_LIMIT);
```

#### OPT-TODO-7: Uniformiser utilisation de db_error()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs` (lignes 136, 173, 185, 267, 297, 324)
- **Changement**: Remplacer `.map_err(|e| ToolError::DatabaseError(e.to_string()))` par `.map_err(db_error)`
- **Benefice**: Maintenabilite - Consistance avec pattern existant
- **Risque regression**: Faible

```rust
// AVANT:
.map_err(|e| ToolError::DatabaseError(e.to_string()))?;

// APRES:
use crate::tools::utils::db_error;
.map_err(db_error)?;
```

### Optimisations Strategiques (Impact haut, Effort eleve) - P2

#### OPT-TODO-5: Reduire N+1 dans update_status()
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:160-203`
- **Changement**: Combiner 3 queries en 2
- **Phases**:
  1. Utiliser UPDATE ... RETURN pour obtenir le nom en meme temps
  2. Ou combiner existence check + get_name en une seule query
- **Prerequis**: OPT-TODO-1 complete
- **Risque regression**: Moyen
- **Tests requis**: Test comportement avec task inexistante

```rust
// OPTION A: UPDATE avec RETURN
let update_query = r#"
    UPDATE task SET status = $status
    WHERE meta::id(id) = $task_id
    RETURN name, status
"#;
let params = vec![
    ("task_id".to_string(), json!(task_id)),
    ("status".to_string(), json!(status)),
];
let result: Vec<Value> = self.db.query_with_params(update_query, params).await?;
if result.is_empty() {
    return Err(ToolError::NotFound(...));
}
let task_name = result[0]["name"].as_str().unwrap_or("Unknown Task");
```

#### OPT-TODO-6: Optimiser complete_task() N+1
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:286-343`
- **Changement**: Meme pattern que OPT-TODO-5
- **Prerequis**: OPT-TODO-3 complete
- **Risque regression**: Moyen

#### OPT-TODO-11: Tests d'integration avec DB
- **Fichiers**: Ajouter dans `src-tauri/src/tools/todo/tool.rs` section tests
- **Changement**: 6+ tests d'integration avec vraie DB temporaire
- **Tests requis**:
  - test_create_task_integration
  - test_update_status_integration
  - test_list_tasks_integration
  - test_complete_task_integration
  - test_delete_task_integration
  - test_get_task_not_found

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::tempdir;

    async fn create_test_tool() -> (TodoTool, tempfile::TempDir) {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test_todo_db");
        let db = Arc::new(DBClient::new(db_path.to_str().unwrap()).await.unwrap());
        db.initialize_schema().await.unwrap();

        let tool = TodoTool::new(
            db,
            "wf_test".to_string(),
            "test_agent".to_string(),
            None
        );
        (tool, temp_dir)
    }

    #[tokio::test]
    async fn test_create_task_integration() {
        let (tool, _temp) = create_test_tool().await;
        let input = json!({
            "operation": "create",
            "name": "Test task",
            "description": "Test description",
            "priority": 2
        });
        let result = tool.execute(input).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response["success"], true);
        assert!(response["task_id"].is_string());
    }

    // ... autres tests
}
```

#### OPT-TODO-12: Tests prevention SQL injection
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs` section tests
- **Changement**: Tests avec payloads malicieux
- **Prerequis**: OPT-TODO-1 a OPT-TODO-4 completes
- **Risque regression**: Aucun (tests seulement)

```rust
#[tokio::test]
async fn test_sql_injection_prevention_task_id() {
    let (tool, _temp) = create_test_tool().await;

    // Payload malicieux
    let malicious_input = json!({
        "operation": "get",
        "task_id": "'; DROP TABLE task; --"
    });

    let result = tool.execute(malicious_input).await;
    // Doit retourner NotFound, pas executer le DROP
    assert!(matches!(result, Err(ToolError::NotFound(_))));
}

#[tokio::test]
async fn test_sql_injection_prevention_status() {
    let (tool, _temp) = create_test_tool().await;

    let malicious_input = json!({
        "operation": "update_status",
        "task_id": "valid-uuid",
        "status": "pending' OR '1'='1"
    });

    let result = tool.execute(malicious_input).await;
    // Doit echouer validation, pas executer injection
    assert!(matches!(result, Err(ToolError::ValidationFailed(_))));
}
```

### Nice to Have (Impact faible, Effort faible) - P3

#### OPT-TODO-9: Extraire TASK_SELECT_FIELDS constant
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs`
- **Changement**: Creer constante pour champs SELECT communs
- **Benefice**: DRY, maintenabilite
- **Risque regression**: Faible

```rust
// Dans constants.rs ou en haut de tool.rs:
const TASK_SELECT_FIELDS: &str =
    "meta::id(id) AS id, name, description, status, priority, agent_assigned, dependencies, duration_ms, created_at, completed_at";

// Utilisation:
let query = format!("SELECT {} FROM task WHERE ...", TASK_SELECT_FIELDS);
```

### Differe (Impact moyen, Effort eleve)

#### OPT-TODO-8: Creer tools/todo/helpers.rs
- **Justification**: Reporter apres stabilisation pattern OPT-MEM-6 dans MemoryTool
- **Contenu futur**:
  - build_task_query() helper
  - task_exists() helper
  - Shared logic entre Tool et Commands
- **Prerequis**: Valider pattern helpers.rs avec MemoryTool en production

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| anyhow | 1.0 | 1.0.100 | Non - patch only |
| reqwest | 0.12 | 0.12.24 | Non - patch only |
| uuid | 1.18 | 1.18.1 | Non - patch only |

### Mises a Jour Bloquees

| Package/Crate | Actuel | Derniere | Raison |
|---------------|--------|----------|--------|
| thiserror | 1.0 | 2.0.17 | Breaking change majeur - analyse migration requise |

## Verification Non-Regression

### Tests Existants
- [x] `cargo test` - 538+ tests backend (6 specifiques TodoTool)
- [x] `cargo clippy -- -D warnings` - Zero warnings
- [x] `npm run check` - TypeScript validation

### Tests a Ajouter
- [ ] test_create_task_integration
- [ ] test_update_status_integration
- [ ] test_list_tasks_integration
- [ ] test_complete_task_integration
- [ ] test_delete_task_integration
- [ ] test_get_task_not_found
- [ ] test_sql_injection_prevention_task_id
- [ ] test_sql_injection_prevention_status

### Benchmarks

```bash
# Avant optimisation (baseline)
cargo test --release -- todo --nocapture 2>&1 | grep -E "(test|time)"

# Apres optimisation
# Comparer temps d'execution des tests
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-TODO-1 | 15min | Haut (securite) | P1 |
| OPT-TODO-2 | 15min | Haut (securite) | P1 |
| OPT-TODO-3 | 15min | Haut (securite) | P1 |
| OPT-TODO-4 | 10min | Haut (securite) | P1 |
| OPT-TODO-10 | 5min | Moyen (memoire) | P1 |
| OPT-TODO-7 | 10min | Faible (DRY) | P1 |
| OPT-TODO-5 | 30min | Moyen (perf) | P2 |
| OPT-TODO-6 | 30min | Moyen (perf) | P2 |
| OPT-TODO-11 | 2h | Haut (qualite) | P2 |
| OPT-TODO-12 | 30min | Haut (securite) | P2 |
| OPT-TODO-9 | 10min | Faible (DRY) | P3 |

**Total P1 (Quick Wins)**: ~1h10
**Total P2 (Strategic)**: ~3h30
**Total P3 (Nice to Have)**: ~10min
**Total General**: ~5h

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle queries | Moyenne | Eleve | Tests integration avant/apres |
| Performance degradee | Faible | Moyen | Benchmark avant/apres |
| Incompatibilite SurrealDB params | Faible | Eleve | Tester avec version 2.4.0 |
| Events streaming casses | Faible | Moyen | Test manuel emit_task_event |

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer P1: OPT-TODO-1 a OPT-TODO-4 (securite) - 55min
3. [ ] Executer P1: OPT-TODO-10 + OPT-TODO-7 (cleanup) - 15min
4. [ ] Verifier non-regression: cargo test + cargo clippy
5. [ ] Executer P2: OPT-TODO-5 + OPT-TODO-6 (performance) - 1h
6. [ ] Executer P2: OPT-TODO-11 + OPT-TODO-12 (tests) - 2h30
7. [ ] Executer P3: OPT-TODO-9 (nice to have) - 10min
8. [ ] Documentation: Mettre a jour CLAUDE.md si nouveaux patterns

## References

### Code Analyse
- `src-tauri/src/tools/todo/tool.rs` - Implementation principale
- `src-tauri/src/tools/todo/mod.rs` - Module export
- `src-tauri/src/tools/utils.rs` - Utilities partagees (ParamQueryBuilder)
- `src-tauri/src/tools/constants.rs` - Constantes (todo::*)
- `src-tauri/src/tools/memory/tool.rs` - Reference OPT-MEM-5 pattern
- `src-tauri/src/commands/task.rs` - IPC commands (reference LIMIT)

### Documentation Consultee
- `docs/AGENT_TOOLS_DOCUMENTATION.md` - Specs TodoTool
- `docs/API_REFERENCE.md` - Reference API
- `CLAUDE.md` - Patterns et contraintes projet

### Sources Externes
- [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)
- [Rig.rs Tools Documentation](https://docs.rig.rs/docs/concepts/tools)
- [Building Effective Agents - Anthropic](https://www.anthropic.com/research/building-effective-agents)

### Memories Projet
- `tools_refactoring_complete` - Pattern OPT-MEM-5/6
- `todo_tool_specification` - Specs originales
- `todo_tool_implementation_patterns` - Patterns implementation
