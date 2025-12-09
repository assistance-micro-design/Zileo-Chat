# Plan d'Optimisation - tools/MemoryTool

## Metadata
- **Date**: 2025-12-09
- **Domaine**: tools/MemoryTool
- **Stack**: Rust 1.91.1 + Tauri 2.9.3 + SurrealDB 2.4.0 + Rig.rs 0.24.0
- **Impact estime**: Maintenabilite (principal), Performance, Securite

## Resume Executif

Ce plan d'optimisation cible le systeme MemoryTool, responsable du stockage et de la recherche de memoires pour les agents LLM. L'analyse revele **220-250 lignes de code duplique** entre `tool.rs` et `commands/memory.rs`, une **complexite cyclomatique elevee** (CC=18 dans `validate_input()`), et des **patterns non utilises** (QueryBuilder defini mais jamais employe). Les optimisations proposees reduiront la duplication de 40%, la complexite de 50%, et amelioreront la securite en unifiant l'approche des requetes parametrees.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src-tauri/src/tools/memory/tool.rs` | 1701 | CC Total ~77 | Duplication scope filter 3x, format! sans parametrisation |
| `src-tauri/src/commands/memory.rs` | 781 | CC Total ~35 | Duplique add_memory de tool.rs |
| `src-tauri/src/tools/memory/mod.rs` | 72 | Faible | Module exports seulement |
| `src-tauri/src/tools/utils.rs` | 277 | Faible | QueryBuilder defini mais non utilise |
| `src-tauri/src/tools/constants.rs` | 196 | Faible | Bien structure |
| `src-tauri/src/tools/response.rs` | 192 | Faible | Bien utilise |

### Fonctions a Haute Complexite

| Fonction | Fichier:Ligne | CC | Probleme |
|----------|---------------|-----|----------|
| `validate_input()` | tool.rs:931-1042 | 18 | Nested match + if pour chaque operation |
| `execute()` | tool.rs:846-929 | 15 | Dispatch + extraction params inline |
| `add_memory()` | tool.rs:161-276 | 12 | 3 branches conditionnelles embedding |
| `list_memories()` | tool.rs:322-409 | 10 | Conditions dynamiques |

### Patterns Identifies

- **Pattern Scope Filter (duplique 3x)**:
  ```rust
  // Lignes 334-368, 479-497, 578-596 - code identique
  match scope {
      "workflow" => { if let Some(ref wf_id) = workflow_id { ... } }
      "general" => { conditions.push("workflow_id IS NONE".to_string()); }
      _ => { if let Some(ref wf_id) = workflow_id { ... } }
  }
  ```

- **Pattern add_memory (duplique 100%)**:
  - `tool.rs:161-276` vs `commands/memory.rs:54-175`
  - 120 lignes identiques: validation, embedding try/fallback, creation DB

- **Pattern error mapping (inconsistent)**:
  - `tool.rs`: utilise `map_err(|e| ToolError::DatabaseError(e.to_string()))`
  - Devrait utiliser `db_error()` de `utils.rs`

### Metriques Actuelles

- **Duplication estimee**: 220-250 lignes (9-10% du domaine)
- **Coverage tests**: 40+ unit tests, 150+ validation tests
- **Async/await calls**: 122 dans tool.rs

## Best Practices (2024-2025)

### Sources Consultees

- [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices) - Dec 2024
- [MCP Best Practices Architecture](https://modelcontextprotocol.info/docs/best-practices/) - 2025
- [Rust Memory Management 2025](https://markaicode.com/rust-memory-management-2025/) - 2025
- [Experience-Following Property in LLM Agents](https://arxiv.org/html/2505.16067v1) - Mai 2025
- [Mem0 Research on Memory Systems](https://mem0.ai/research) - 2025

### Patterns Recommandes

1. **Hybrid Search (vector + text fallback)** - DEJA CONFORME
   - Zileo-Chat-3 implemente ce pattern correctement

2. **Query Parameterization** - PARTIELLEMENT CONFORME
   - `commands/memory.rs` utilise `query_with_params()` correctement
   - `tool.rs` utilise `format!()` avec user input - RISQUE INJECTION

3. **Dual Memory Architecture** - DEJA CONFORME
   - Types: UserPref, Context, Knowledge, Decision
   - Scope: workflow vs general

4. **QueryBuilder Pattern** - NON UTILISE
   - Defini dans `utils.rs:130-199` mais jamais employe
   - Recommande pour consistance et securite

### Anti-Patterns a Eviter

1. **Arc<Mutex<T>> overuse** - EVITE CORRECTEMENT
   - Zileo-Chat-3 utilise RwLock appropriement

2. **format!() avec user input** - PRESENT DANS tool.rs
   - `format!("workflow_id = '{}'", wf_id)` est vulnerable
   - Doit migrer vers requetes parametrees

3. **Code duplication cross-layers** - PRESENT
   - add_memory duplique entre tool et commands
   - Viole DRY et augmente risque de divergence

## Contraintes du Projet

**Source**: `CLAUDE.md`, `docs/ARCHITECTURE_DECISIONS.md`

- **Decision 1**: Tool trait avec async_trait - A respecter
- **Decision 2**: ResponseBuilder pour JSON - A utiliser systematiquement
- **Decision 3**: Validation via utils.rs - validate_not_empty, validate_length, validate_enum_value
- **Decision 4**: Constants dans constants.rs - MAX_CONTENT_LENGTH=50000, VALID_TYPES
- **Decision 5**: Query LIMIT obligatoire (OPT-DB-8) - DEFAULT_LIST_LIMIT=1000
- **Decision 6**: Requetes parametrees pour securite - query_with_params()
- **Decision 7**: SurrealDB patterns - meta::id(id), execute() pour writes

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-MEM-1: Utiliser db_error() systematiquement

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Remplacer tous les `map_err(|e| ToolError::DatabaseError(e.to_string()))` par `db_error()`
- **Lignes concernees**: ~8 occurrences dispersees
- **Benefice**: Consistance, reduction boilerplate
- **Effort**: 15 min
- **Risque regression**: Faible
- **Validation**: `cargo test --lib`

```rust
// AVANT (tool.rs multiple locations)
self.db.query(&query).await.map_err(|e| ToolError::DatabaseError(e.to_string()))?;

// APRES
use crate::tools::utils::db_error;
self.db.query(&query).await.map_err(db_error)?;
```

#### OPT-MEM-2: Centraliser la logique scope filter

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Extraire la logique de scope matching en fonction helper
- **Lignes concernees**: 334-368, 479-497, 578-596 (60 lignes dupliquees)
- **Benefice**: Elimination duplication, maintenabilite
- **Effort**: 30 min
- **Risque regression**: Faible
- **Validation**: Tests existants couvrent les 3 scopes

```rust
// NOUVEAU: tools/memory/tool.rs (helper prive)
fn build_scope_conditions(
    scope: &str,
    workflow_id: &Option<String>,
    conditions: &mut Vec<String>,
    params: &mut Vec<(String, serde_json::Value)>,
) {
    match scope {
        "workflow" => {
            if let Some(ref wf_id) = workflow_id {
                conditions.push("workflow_id = $workflow_id".to_string());
                params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            }
        }
        "general" => {
            conditions.push("workflow_id IS NONE".to_string());
        }
        _ => { // "both" ou default
            if let Some(ref wf_id) = workflow_id {
                conditions.push("(workflow_id = $workflow_id OR workflow_id IS NONE)".to_string());
                params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
            }
        }
    }
}
```

#### OPT-MEM-3: Mettre a jour uuid et chrono

- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**: Specifier versions exactes
- **Benefice**: Securite, performance, features recentes
- **Effort**: 5 min
- **Risque regression**: Aucun (no breaking changes)
- **Validation**: `cargo build`

```toml
# AVANT
uuid = { version = "1.0", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }

# APRES
uuid = { version = "1.18", features = ["v4", "serde"] }
chrono = { version = "0.4.42", features = ["serde"] }
```

#### OPT-MEM-4: Ajouter index composite SurrealDB

- **Fichiers**: Migration schema ou `src-tauri/src/db/init.rs`
- **Changement**: Ajouter index composite pour queries frequentes
- **Benefice**: Performance queries avec filtres multiples
- **Effort**: 15 min
- **Risque regression**: Aucun (additive)
- **Validation**: Query execution time

```sql
-- Index composite pour search_memories() avec type + workflow_id
DEFINE INDEX memory_type_workflow_idx ON memory FIELDS type, workflow_id;

-- Index pour cleanup TTL (preparation phase future)
DEFINE INDEX memory_type_created_idx ON memory FIELDS type, created_at;
```

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-MEM-5: Parametriser les requetes dans tool.rs

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Remplacer format!() par requetes parametrees
- **Lignes concernees**: Toutes les constructions de WHERE clauses
- **Benefice**: **SECURITE** - Prevention injection SQL
- **Effort**: 2h
- **Risque regression**: Moyen (changement de pattern)
- **Tests requis**: Verifier tous les cas de scope/filter

```rust
// AVANT (VULNERABLE)
conditions.push(format!("workflow_id = '{}'", wf_id));
let query = format!("SELECT ... WHERE {} ...", conditions.join(" AND "));

// APRES (SECURISE)
conditions.push("workflow_id = $workflow_id".to_string());
params.push(("workflow_id".to_string(), serde_json::json!(wf_id)));
let results = self.db.query_with_params(&query, params).await?;
```

#### OPT-MEM-6: Consolider add_memory implementations

- **Fichiers**:
  - `src-tauri/src/tools/memory/tool.rs`
  - `src-tauri/src/commands/memory.rs`
  - `src-tauri/src/tools/memory/mod.rs` (nouveau: shared logic)
- **Changement**: Extraire logique commune dans helper partage
- **Benefice**: Elimination de 120 lignes dupliquees, DRY
- **Effort**: 2-3h
- **Prerequis**: OPT-MEM-1, OPT-MEM-5
- **Risque regression**: Moyen
- **Tests requis**: Tests add_memory existants (20+ cas)

```rust
// NOUVEAU: tools/memory/helpers.rs
pub struct AddMemoryParams {
    pub memory_type: MemoryType,
    pub content: String,
    pub metadata: Option<serde_json::Value>,
    pub workflow_id: Option<String>,
}

pub async fn add_memory_core(
    params: AddMemoryParams,
    db: &DBClient,
    embedding_service: Option<&EmbeddingService>,
) -> Result<String, ToolError> {
    // Validation
    validate_not_empty(&params.content, "content")?;
    validate_length(&params.content, MAX_CONTENT_LENGTH, "content")?;

    let memory_id = Uuid::new_v4().to_string();
    let has_embedding = if let Some(embed_svc) = embedding_service {
        match embed_svc.embed(&params.content).await {
            Ok(embedding) => {
                // Create with embedding
                create_memory_with_embedding(db, &memory_id, &params, embedding).await?;
                true
            }
            Err(e) => {
                warn!(error = %e, "Embedding failed, storing without");
                create_memory_without_embedding(db, &memory_id, &params).await?;
                false
            }
        }
    } else {
        create_memory_without_embedding(db, &memory_id, &params).await?;
        false
    };

    Ok(memory_id)
}
```

#### OPT-MEM-7: Reduire complexite validate_input()

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Extraire parsing en struct MemoryInput
- **Benefice**: CC de 18 -> ~8, meilleure testabilite
- **Effort**: 2h
- **Risque regression**: Moyen
- **Tests requis**: 150+ validation tests existants

```rust
// NOUVEAU: Structure pour inputs parses
#[derive(Debug)]
pub struct MemoryInput {
    pub operation: String,
    pub workflow_id: Option<String>,
    pub memory_type: Option<String>,
    pub content: Option<String>,
    pub id: Option<String>,
    pub query: Option<String>,
    pub limit: Option<usize>,
    pub type_filter: Option<String>,
    pub scope: Option<String>,
    pub threshold: Option<f64>,
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

impl MemoryInput {
    pub fn from_json(input: &serde_json::Value) -> ToolResult<Self> {
        let obj = input.as_object()
            .ok_or_else(|| ToolError::ValidationFailed("Input must be object".into()))?;

        Ok(Self {
            operation: obj.get("operation")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ToolError::ValidationFailed("Missing operation".into()))?
                .to_string(),
            workflow_id: obj.get("workflow_id").and_then(|v| v.as_str()).map(String::from),
            // ... autres champs
        })
    }

    pub fn validate(&self) -> ToolResult<()> {
        match self.operation.as_str() {
            "activate_workflow" => self.validate_activate_workflow(),
            "add" => self.validate_add(),
            "search" => self.validate_search(),
            // ...
            _ => Ok(())
        }
    }
}
```

#### OPT-MEM-8: Reduire complexite execute()

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Utiliser MemoryInput de OPT-MEM-7
- **Benefice**: CC de 15 -> ~7, code plus lisible
- **Effort**: 1h
- **Prerequis**: OPT-MEM-7
- **Risque regression**: Moyen

```rust
// AVANT (execute actuel avec extraction inline)
async fn execute(&self, input: Value) -> ToolResult<Value> {
    let obj = input.as_object().ok_or(...)?;
    let operation = obj.get("operation").and_then(|v| v.as_str()).ok_or(...)?;
    match operation {
        "add" => {
            let memory_type = input["type"].as_str().ok_or(...)?;
            let content = input["content"].as_str().ok_or(...)?;
            // ...
        }
    }
}

// APRES (avec MemoryInput)
async fn execute(&self, input: Value) -> ToolResult<Value> {
    let params = MemoryInput::from_json(&input)?;
    params.validate()?;

    match params.operation.as_str() {
        "add" => {
            self.add_memory(
                &params.memory_type.unwrap(),
                &params.content.unwrap(),
                params.metadata.clone(),
                params.tags.clone(),
            ).await
        }
        // ... plus propre, params deja valides
    }
}
```

### Nice to Have (Impact faible, Effort faible)

#### OPT-MEM-9: Utiliser QueryBuilder

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Remplacer construction manuelle par QueryBuilder de utils.rs
- **Benefice**: Code plus lisible, pattern etabli
- **Effort**: 1h
- **Risque regression**: Faible

```rust
// AVANT
let mut conditions = Vec::new();
conditions.push(format!("type = '{}'", mem_type));
let where_clause = if conditions.is_empty() { String::new() }
    else { format!("WHERE {}", conditions.join(" AND ")) };
let query = format!("SELECT ... FROM memory {} LIMIT {}", where_clause, limit);

// APRES
use crate::tools::utils::QueryBuilder;
let query = QueryBuilder::new("memory")
    .select(&["meta::id(id) AS id", "type", "content", "workflow_id", "metadata", "created_at"])
    .where_eq("type", mem_type)
    .order_by("created_at", true)
    .limit(limit)
    .build();
```

#### OPT-MEM-10: Consolider branches creation memoire

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Helper pour creation avec/sans embedding et avec/sans workflow_id
- **Benefice**: Reduction 40 lignes dupliquees dans add_memory
- **Effort**: 30 min
- **Risque regression**: Faible

```rust
// Helper pour creation flexible
async fn create_memory_record(
    db: &DBClient,
    memory_id: &str,
    memory_type: &str,
    content: &str,
    metadata: Option<serde_json::Value>,
    workflow_id: Option<&String>,
    embedding: Option<Vec<f32>>,
) -> ToolResult<()> {
    // Une seule implementation pour tous les cas
}
```

#### OPT-MEM-11: Optimiser embedding_str formatting

- **Fichiers**: `src-tauri/src/tools/memory/tool.rs`
- **Changement**: Pre-allouer String au lieu de Vec intermediate
- **Lignes**: Dans vector_search(), construction de embedding_str
- **Benefice**: Moins d'allocations heap
- **Effort**: 15 min
- **Risque regression**: Aucun

```rust
// AVANT
let embedding_str: String = query_embedding
    .iter()
    .map(|v| v.to_string())
    .collect::<Vec<_>>()
    .join(", ");

// APRES
let mut embedding_str = String::with_capacity(query_embedding.len() * 12);
for (i, v) in query_embedding.iter().enumerate() {
    if i > 0 { embedding_str.push_str(", "); }
    use std::fmt::Write;
    write!(embedding_str, "{}", v).unwrap();
}
```

### Differe (Impact faible, Effort eleve)

| Item | Raison du report | Phase cible |
|------|-----------------|-------------|
| Selective addition (dedup semantique) | Necessite logique complexe de comparaison | Phase 4 |
| TTL/Expiration automatique | Necessite scheduler et nouvelle commande | Phase 4 |
| Reranking post-retrieval | Gain marginal, complexite elevee | Phase 5 |
| Graph-based memory (Mem0g) | Changement architectural majeur | Future |

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| uuid | 1.0 | 1.18 | Non |
| chrono | 0.4 (non specifie) | 0.4.42 | Non |

### Dependencies Inchangees (deja a jour)

| Package/Crate | Version | Status |
|---------------|---------|--------|
| surrealdb | 2.4.0 | Latest |
| serde | 1.0.228 | Latest |
| serde_json | 1.0.145 | Latest |
| tokio | 1.48.0 | Latest |
| async-trait | 0.1.x | Latest |
| rig-core | 0.24.0 | Latest |

## Verification Non-Regression

### Tests Existants

- [x] `cargo test --lib` - 760+ tests
- [x] `src-tauri/src/tools/memory/tool.rs` - 40+ unit tests
- [x] validate_input tests - 150+ cas couverts
- [x] Integration tests - 15+ tests DB

### Tests a Ajouter

- [ ] Test OPT-MEM-2: `test_build_scope_conditions_workflow`
- [ ] Test OPT-MEM-2: `test_build_scope_conditions_general`
- [ ] Test OPT-MEM-2: `test_build_scope_conditions_both`
- [ ] Test OPT-MEM-6: `test_add_memory_core_with_embedding`
- [ ] Test OPT-MEM-6: `test_add_memory_core_without_embedding`
- [ ] Test OPT-MEM-7: `test_memory_input_from_json`
- [ ] Test OPT-MEM-7: `test_memory_input_validate`

### Benchmarks

```bash
# Avant optimisation (baseline)
cargo bench --bench memory_operations 2>&1 | tee benchmark_before.txt

# Apres optimisation
cargo bench --bench memory_operations 2>&1 | tee benchmark_after.txt

# Comparaison
diff benchmark_before.txt benchmark_after.txt
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-MEM-1: db_error() | 15 min | Moyen | P1 |
| OPT-MEM-2: Scope filter | 30 min | Haut | P1 |
| OPT-MEM-3: Update crates | 5 min | Faible | P1 |
| OPT-MEM-4: Index composite | 15 min | Moyen | P1 |
| OPT-MEM-5: Parametrize queries | 2h | **CRITIQUE** | P1 |
| OPT-MEM-6: Consolidate add_memory | 2-3h | Haut | P2 |
| OPT-MEM-7: MemoryInput struct | 2h | Haut | P2 |
| OPT-MEM-8: Simplify execute() | 1h | Moyen | P2 |
| OPT-MEM-9: QueryBuilder | 1h | Moyen | P3 |
| OPT-MEM-10: Memory creation helper | 30 min | Faible | P3 |
| OPT-MEM-11: Embedding str optim | 15 min | Faible | P3 |

**Total estime**: 10-12h

**Phases d'implementation**:
- **Phase 1 (P1)**: 3h - Quick wins + securite critique
- **Phase 2 (P2)**: 5-6h - Refactoring structurel
- **Phase 3 (P3)**: 2h - Nice to have

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle | Moyenne | Eleve | 150+ tests existants, review PR |
| Performance degradee | Faible | Moyen | Benchmarks avant/apres |
| Conflits de merge | Moyenne | Faible | Branches courtes, commits atomiques |
| Breaking change API | Faible | Eleve | Signature functions inchangees |

## Prochaines Etapes

1. [x] Valider ce plan avec l'utilisateur
2. [x] Executer OPT-MEM-5 (securite critique - parametrisation) - COMPLETE 2025-12-09
3. [x] Executer OPT-MEM-1 a OPT-MEM-4 (quick wins) - COMPLETE 2025-12-09
4. [x] Mesurer impact via `cargo test` + review duplication - 761 tests OK
5. [x] OPT-MEM-6: Consolidate add_memory implementations - COMPLETE 2025-12-09
6. [x] OPT-MEM-7: Reduce validate_input() complexity - COMPLETE 2025-12-09
   - Added MemoryInput struct for typed parsing
   - Extracted per-operation validators into dedicated methods
   - Reduced CC from ~18 to ~8
   - 764 tests pass
7. [x] OPT-MEM-8: Reduce execute() complexity - COMPLETE 2025-12-09
   - Extended MemoryInput with limit, scope, metadata, tags fields
   - Refactored execute() to use MemoryInput instead of inline JSON extraction
   - Reduced CC from ~15 to ~7 (single parse, dispatch with unwrap())
   - 764 tests pass
8. [ ] Planifier Phase 3 (OPT-MEM-9, 10, 11) - Nice to have

## References

### Code Analyse

- `src-tauri/src/tools/memory/tool.rs` (1701 lignes)
- `src-tauri/src/commands/memory.rs` (781 lignes)
- `src-tauri/src/tools/memory/mod.rs` (72 lignes)
- `src-tauri/src/tools/utils.rs` (277 lignes)
- `src-tauri/src/tools/constants.rs` (196 lignes)
- `src-tauri/src/tools/response.rs` (192 lignes)
- `src-tauri/src/tools/todo/tool.rs` (700 lignes - comparaison)
- `src-tauri/src/tools/calculator/tool.rs` (1193 lignes - comparaison)

### Documentation Consultee

- `CLAUDE.md` - Tool Development Patterns, SurrealDB Patterns
- `docs/ARCHITECTURE_DECISIONS.md`
- `docs/AGENT_TOOLS_DOCUMENTATION.md`
- `docs/DATABASE_SCHEMA.md`
- `docs/API_REFERENCE.md`

### Sources Externes

- [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)
- [MCP Best Practices](https://modelcontextprotocol.info/docs/best-practices/)
- [Rust Memory Management 2025](https://markaicode.com/rust-memory-management-2025/)
- [Experience-Following in LLM Agents](https://arxiv.org/html/2505.16067v1)
- [Mem0 Memory Research](https://mem0.ai/research)
- [uuid crate - crates.io](https://crates.io/crates/uuid)
- [chrono crate - crates.io](https://crates.io/crates/chrono)
