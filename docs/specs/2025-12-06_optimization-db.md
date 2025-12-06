# Plan d'Optimisation - Database (SurrealDB)

## Metadata
- **Date**: 2025-12-06
- **Domaine**: db
- **Stack**: SurrealDB 2.3.10 (embedded RocksDB) + Rust SDK 2.x + Tauri 2.9.4
- **Impact estime**: Performance, Securite, Maintenabilite

## Resume Executif

Le domaine database (SurrealDB) est fonctionnel et bien structure avec 18 tables et 30+ indexes. Les principales opportunites d'optimisation concernent:
1. **Securite**: Elimination des `format!()` pour queries (risque injection SQL)
2. **Performance**: Upgrade vers 2.4.0 + configuration production
3. **Crash-safety**: Documentation SURREAL_SYNC_DATA (critique)

Total estime: **9.5 heures** (Quick wins: 25 min, Strategiques: 7h, Nice to have: 2h)

---

## Etat Actuel

### Analyse du Code

| Fichier | Complexite | Points d'attention |
|---------|------------|-------------------|
| `src-tauri/src/db/client.rs:143-205` | Haute | 8 niveaux imbrication dans create() |
| `src-tauri/src/models/serde_utils.rs:41-145` | Haute | Nested visitors complexes |
| `src-tauri/src/commands/workflow.rs:86-200` | Haute | 10+ niveaux logique |
| `src-tauri/src/commands/agent.rs:320-344` | Moyenne | format!() pour queries |
| `src-tauri/src/commands/embedding.rs:119` | Moyenne | format!() pour queries |
| `src-tauri/src/commands/task.rs:168` | Moyenne | format!() pour queries |

### Patterns Identifies

**Patterns Corrects (a conserver):**
- Custom deserializers pour Thing type SDK 2.x (`serde_utils.rs`)
- Pattern `meta::id(id)` pour UUIDs clean
- Pattern `execute()` pour mutations (UPSERT/UPDATE/DELETE)
- Pattern backticks `` DELETE table:`id` ``
- JSON encoding pour strings avec caracteres speciaux
- Validation centralisee (`tools/utils.rs`, `tools/constants.rs`)

**Patterns Problematiques:**
- `format!()` utilise pour construire queries SQL (risque injection)
- QueryBuilder defini (`tools/utils.rs:123-193`) mais sous-utilise
- Double Arc wrapping: `Arc<RwLock<Option<Arc<EmbeddingService>>>>`
- Error messages generiques (perte de contexte)
- Pas de LIMIT sur certaines queries

### Metriques Actuelles

| Metrique | Valeur |
|----------|--------|
| Fichiers core DB | 3 (766 lignes) |
| Fichiers commands utilisant DB | 14 |
| Fichiers tools utilisant DB | 3+ |
| Tables SurrealDB | 18 |
| Indexes definis | 30+ |
| Lignes code DB+models | ~3000 |
| Version surrealdb | 2.3.10 |

---

## Best Practices (2024-2025)

### Sources Consultees
- [Performance Best Practices | SurrealDB](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)
- [Security Best Practices | SurrealDB](https://surrealdb.com/docs/surrealdb/reference-guide/security-best-practices)
- [Schema Creation Best Practices | SurrealDB](https://surrealdb.com/docs/surrealdb/reference-guide/schema-creation-best-practices)
- [SurrealDB Rust SDK Setup](https://surrealdb.com/docs/sdk/rust/setup)
- [Tips and Tricks on Using the Rust SDK | SurrealDB Blog](https://surrealdb.com/blog/tips-and-tricks-on-using-the-rust-sdk)
- [Indexing & Data Model Considerations](https://surrealdb.com/learn/fundamentals/performance/index-data-model)

### Patterns Recommandes

1. **Parameterized Queries (OBLIGATOIRE)**
   ```rust
   // CORRECT
   db.query("SELECT * FROM memory WHERE type = $type")
       .bind(("type", memory_type))
       .await?;

   // INCORRECT (risque injection)
   let query = format!("SELECT * FROM memory WHERE type = '{}'", user_input);
   ```

2. **Cargo Release Profile**
   ```toml
   [profile.release]
   lto = true
   strip = true
   opt-level = 3
   panic = 'abort'
   codegen-units = 1
   ```

3. **Log Level Production**
   ```bash
   SURREAL_LOG=error  # Significant performance impact
   ```

4. **Transaction Handling**
   ```rust
   db.query("BEGIN TRANSACTION").await?;
   // operations...
   db.query("COMMIT TRANSACTION").await?;
   ```

5. **Query Statistics (new 2.4.0)**
   ```rust
   let stats = db.query("SELECT * FROM table")
       .with_stats()
       .await?;
   ```

### Anti-Patterns a Eviter

1. **format!() pour queries SQL**
   - Risque: SQL injection
   - Solution: Toujours utiliser `.bind()`

2. **SELECT * avec ORDER BY sur champs non selectionnes**
   ```sql
   -- INCORRECT
   SELECT meta::id(id) AS id, content FROM memory ORDER BY created_at DESC

   -- CORRECT
   SELECT meta::id(id) AS id, content, created_at FROM memory ORDER BY created_at DESC
   ```

3. **Queries sans LIMIT sur tables potentiellement grandes**
   ```sql
   -- INCORRECT
   SELECT * FROM mcp_call_log

   -- CORRECT
   SELECT * FROM mcp_call_log LIMIT 100
   ```

4. **Over-indexing write-heavy tables**
   - Chaque index ralentit INSERT/UPDATE/DELETE
   - Equilibrer read vs write performance

---

## Contraintes du Projet

### Decisions Existantes a Respecter

- **Decision**: Schema graph avec 18 tables relationnelles
  - Source: `docs/ARCHITECTURE_DECISIONS.md`
  - Raison: Multi-agents = relations complexes

- **Decision**: SCHEMAFULL pour tables critiques
  - Source: `CLAUDE.md`
  - Raison: Validation schema strict

- **Decision**: 10 patterns SDK 2.x obligatoires
  - Source: `CLAUDE.md` section "SurrealDB SDK 2.x Patterns"
  - Impact: NE PAS modifier ces patterns

- **Decision**: Audit trail simplifie (pas versioning complet)
  - Source: `docs/ARCHITECTURE_DECISIONS.md`
  - Raison: Complexite excessive pour v1

- **Decision**: Retention differenciee par type
  - Source: `docs/ARCHITECTURE_DECISIONS.md`
  - 90 jours workflows, 180 jours errors, 1 an audit

### Workarounds SDK 2.x Non-Negociables

1. Raw SurrealQL + bind() pour creation records
2. meta::id(id) pour retourner UUIDs clean
3. execute() pour UPSERT/UPDATE/DELETE
4. Backticks `` DELETE table:`id` ``
5. JSON encoding pour apostrophes/guillemets
6. JSON string pour dynamic keys (SCHEMAFULL limitation)
7. SELECT fields specifiques (pas SELECT *)
8. ORDER BY fields dans SELECT
9. Backtick-escaped ID pour direct access
10. Custom deserializers pour enums

---

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible) - P1

#### OPT-DB-1: Upgrade surrealdb 2.3.10 vers 2.4.0
- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**:
  ```toml
  # Avant
  surrealdb = { version = "2.3.10", features = ["kv-rocksdb"] }

  # Apres
  surrealdb = { version = "2.4.0", features = ["kv-rocksdb"] }
  ```
- **Benefice**: Nouvelles features (Query::with_stats(), RocksDB tuning), bug fixes, security patches
- **Risque regression**: Faible (backward compatible, minor version)
- **Validation**: `cargo test --lib && cargo check`
- **Effort**: 5 minutes

#### OPT-DB-2: Documenter SURREAL_SYNC_DATA (CRITIQUE)
- **Fichiers**: `docs/DEPLOYMENT_GUIDE.md`
- **Changement**: Ajouter section Environment Variables:
  ```markdown
  ## Environment Variables

  ### SURREAL_SYNC_DATA (CRITICAL)
  Set `SURREAL_SYNC_DATA=true` in production environment.
  Without this, RocksDB/SurrealKV is NOT crash-safe and data corruption may occur.

  ```bash
  # Production
  export SURREAL_SYNC_DATA=true
  ```
  ```
- **Benefice**: Crash-safety RocksDB, prevention corruption donnees
- **Risque regression**: Aucun (documentation)
- **Validation**: Review documentation
- **Effort**: 10 minutes

#### OPT-DB-3: Configuration log level production
- **Fichiers**: `docs/DEPLOYMENT_GUIDE.md`, scripts de deploiement
- **Changement**: Documenter et configurer:
  ```bash
  # Production (significant performance impact)
  export SURREAL_LOG=error

  # Development
  export SURREAL_LOG=debug
  ```
- **Benefice**: Performance amelioree en production
- **Risque regression**: Aucun
- **Effort**: 1 minute

#### OPT-DB-4: Optimiser Cargo release profile
- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**:
  ```toml
  [profile.release]
  lto = true
  strip = true
  opt-level = 3
  panic = 'abort'
  codegen-units = 1
  ```
- **Benefice**: Binary plus petit, execution plus rapide
- **Risque regression**: Aucun (build config)
- **Validation**: `cargo build --release`
- **Effort**: 5 minutes

#### OPT-DB-5: Activer feature protocol-http
- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**:
  ```toml
  surrealdb = { version = "2.4.0", features = ["kv-rocksdb", "protocol-http"] }
  ```
- **Benefice**: Future cloud connectivity SurrealDB Cloud
- **Risque regression**: Aucun (additive feature)
- **Effort**: 1 minute

---

### Optimisations Strategiques (Impact haut, Effort eleve) - P2

#### OPT-DB-6: Migrer format!() vers QueryBuilder + bind()
- **Fichiers**:
  - `src-tauri/src/commands/agent.rs:320-344`
  - `src-tauri/src/commands/embedding.rs:119`
  - `src-tauri/src/commands/task.rs:168`
  - `src-tauri/src/commands/workflow.rs:115`
  - `src-tauri/src/commands/memory.rs`
  - `src-tauri/src/commands/validation.rs`
  - `src-tauri/src/commands/models.rs`
- **Changement**: Remplacer tous les `format!()` par QueryBuilder ou queries parametrees
  ```rust
  // AVANT (risque injection)
  let query = format!("SELECT * FROM agent WHERE name = '{}'", name);
  db.query(&query).await?;

  // APRES (securise)
  use crate::tools::utils::QueryBuilder;
  let (query, bindings) = QueryBuilder::new("agent")
      .select(&["meta::id(id) AS id", "name", "lifecycle"])
      .where_eq("name", name)
      .build();
  db.query(&query).bind(bindings).await?;

  // OU directement
  db.query("SELECT meta::id(id) AS id, name FROM agent WHERE name = $name")
      .bind(("name", name))
      .await?;
  ```
- **Phases**:
  1. Auditer tous les usages de format!() dans commands/
  2. Etendre QueryBuilder si necessaire
  3. Migrer fichier par fichier avec tests
  4. Verifier avec clippy
- **Prerequis**: Aucun
- **Risque regression**: Moyen (tester chaque migration)
- **Tests requis**:
  - Test avec special chars (apostrophes, guillemets)
  - Test injection attempt rejection
  - Test bind() avec tous types
- **Effort**: 4 heures

#### OPT-DB-7: Implementer Transaction Handling
- **Fichiers**:
  - `src-tauri/src/db/client.rs` (ajouter helpers)
  - `src-tauri/src/commands/workflow.rs` (workflow creation)
  - `src-tauri/src/commands/agent.rs` (agent creation with state)
- **Changement**:
  ```rust
  // Ajouter dans DBClient
  impl DBClient {
      pub async fn transaction<F, T, E>(&self, f: F) -> Result<T, E>
      where
          F: FnOnce(&Surreal<Db>) -> futures::future::BoxFuture<'_, Result<T, E>>,
          E: From<surrealdb::Error>,
      {
          self.db.query("BEGIN TRANSACTION").await?;
          match f(&self.db).await {
              Ok(result) => {
                  self.db.query("COMMIT TRANSACTION").await?;
                  Ok(result)
              }
              Err(e) => {
                  self.db.query("CANCEL TRANSACTION").await?;
                  Err(e)
              }
          }
      }
  }

  // Usage
  db_client.transaction(|db| Box::pin(async move {
      db.query("CREATE workflow...").await?;
      db.query("CREATE message...").await?;
      Ok(())
  })).await?;
  ```
- **Benefice**: Atomicite des operations multi-tables
- **Risque regression**: Faible
- **Tests requis**: Test rollback, test commit, test concurrent
- **Effort**: 2 heures

#### OPT-DB-8: Ajouter LIMIT sur queries illimitees
- **Fichiers**: Tous les fichiers commands/ avec SELECT sans LIMIT
- **Changement**:
  ```rust
  // AVANT
  "SELECT * FROM mcp_call_log WHERE workflow_id = $wid"

  // APRES
  "SELECT * FROM mcp_call_log WHERE workflow_id = $wid LIMIT 1000"
  ```
- **Fichiers concernes**:
  - `src-tauri/src/commands/mcp.rs` (list_mcp_servers)
  - `src-tauri/src/commands/models.rs` (list_models)
  - `src-tauri/src/commands/memory.rs` (list_memories)
  - `src-tauri/src/commands/task.rs` (list_tasks)
  - `src-tauri/src/commands/message.rs` (list_messages)
- **Benefice**: Prevention memory explosion
- **Risque regression**: Faible
- **Tests requis**: Test pagination, test default limit
- **Effort**: 1 heure

---

### Nice to Have (Impact faible, Effort faible) - P3

#### OPT-DB-9: Simplifier Double Arc embedding service
- **Fichiers**: `src-tauri/src/state.rs:44`
- **Changement**:
  ```rust
  // AVANT
  pub embedding_service: Arc<RwLock<Option<Arc<EmbeddingService>>>>

  // APRES
  pub embedding_service: Arc<RwLock<Option<EmbeddingService>>>
  ```
- **Benefice**: Code plus lisible, moins d'indirection
- **Risque regression**: Faible (tester concurrency)
- **Effort**: 30 minutes

#### OPT-DB-10: Implementer Query::with_stats() monitoring
- **Fichiers**: `src-tauri/src/db/client.rs`
- **Changement**:
  ```rust
  // Ajouter methode de diagnostic
  impl DBClient {
      pub async fn query_with_stats(&self, query: &str) -> Result<(Vec<Value>, QueryStats)> {
          let response = self.db.query(query).with_stats().await?;
          // Extract stats from response
          Ok((results, stats))
      }
  }
  ```
- **Benefice**: Monitoring performance queries
- **Risque regression**: Aucun (additive)
- **Effort**: 1 heure

#### OPT-DB-11: Review indexes write-heavy tables
- **Fichiers**: `src-tauri/src/db/schema.rs`
- **Changement**: Analyser et potentiellement supprimer indexes sur:
  - `message.timestamp` (frequent writes)
  - `mcp_call_log` indexes multiples
- **Benefice**: Amelioration performance write
- **Risque regression**: Faible (benchmark avant/apres)
- **Effort**: 30 minutes

---

### Differe (Impact faible, Effort eleve) - P4

#### OPT-DB-12: Migration vers thiserror CommandError
- **Raison differe**: Refactoring large touchant tous les commands, defer Phase 7
- **Effort**: 6 heures
- **Alternative actuelle**: map_err avec String (fonctionne)

#### OPT-DB-13: Query caching
- **Raison differe**: SDK SurrealDB ne supporte pas vraiment les prepared statements
- **Alternative**: Queries parametrees avec bind() suffisent

#### OPT-DB-14: Live Query API
- **Raison differe**: Nouvelle feature 2.4.0, pas critique pour v1
- **Effort**: 4 heures
- **Candidat Phase 7**: Real-time agent state updates

---

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| surrealdb | 2.3.10 | 2.4.0 | Non - backward compatible |

### Nouvelles Dependencies (si justifie)

Aucune nouvelle dependance requise.

### Features a Activer

| Feature | Status | Raison |
|---------|--------|--------|
| kv-rocksdb | Active | Embedded RocksDB - obligatoire |
| protocol-http | A activer | Future cloud connectivity |

---

## Verification Non-Regression

### Tests Existants
- [x] `cargo test --lib` - Tests backend (couvre commands/, tools/)
- [x] `cargo clippy -- -D warnings` - Linting strict
- [x] `cargo check` - Verification compilation

### Tests a Ajouter

Pour OPT-DB-6 (QueryBuilder migration):
- [ ] Test parameterized queries avec special chars
- [ ] Test injection attempt rejection
- [ ] Test bind() avec tous types (string, int, array, object)

Pour OPT-DB-7 (Transactions):
- [ ] Test rollback on error
- [ ] Test commit success
- [ ] Test concurrent transactions

Pour OPT-DB-8 (LIMIT):
- [ ] Test pagination avec LIMIT + START
- [ ] Test default LIMIT value

### Benchmarks

```bash
# Avant optimisations
cd src-tauri
time cargo build --release
# Note: X.XXs

# Apres OPT-DB-4 (release profile)
time cargo build --release
# Expected: reduction ~10-20%

# Query performance (avec OPT-DB-10)
# Utiliser Query::with_stats() pour mesurer latency
```

---

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-DB-1 Upgrade 2.4.0 | 5 min | Haut | P1 |
| OPT-DB-2 Doc SURREAL_SYNC | 10 min | Critique | P1 |
| OPT-DB-3 Log level prod | 1 min | Moyen | P1 |
| OPT-DB-4 Release profile | 5 min | Moyen | P1 |
| OPT-DB-5 protocol-http | 1 min | Faible | P1 |
| OPT-DB-6 QueryBuilder migration | 4h | Haut (securite) | P2 |
| OPT-DB-7 Transaction handling | 2h | Moyen | P2 |
| OPT-DB-8 LIMIT queries | 1h | Moyen | P2 |
| OPT-DB-9 Simplify Arc | 30 min | Faible | P3 |
| OPT-DB-10 Query stats | 1h | Moyen | P3 |
| OPT-DB-11 Review indexes | 30 min | Faible | P3 |

**Total Quick Wins (P1)**: ~25 minutes
**Total Strategiques (P2)**: ~7 heures
**Total Nice to Have (P3)**: ~2 heures
**Total Global**: ~9.5 heures

---

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression OPT-DB-6 (QueryBuilder) | Moyenne | Eleve | Tests progressifs, migration fichier par fichier, review |
| Performance degradee OPT-DB-7 (transactions) | Faible | Moyen | Benchmark avant/apres |
| Data corruption (pas SURREAL_SYNC_DATA) | Faible | Critique | OPT-DB-2 documentation + env var |
| SurrealDB 2.4.0 bugs | Faible | Moyen | cargo test complet, rollback possible |

---

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer OPT-DB-1 a OPT-DB-5 (Quick Wins - 25 min)
3. [ ] Mesurer baseline performance
4. [ ] Executer OPT-DB-6 (QueryBuilder migration - priorite securite)
5. [ ] Tests de non-regression
6. [ ] Executer OPT-DB-7, OPT-DB-8
7. [ ] Evaluer P3 selon temps disponible

---

## References

### Code Analyse
- `src-tauri/src/db/mod.rs` (45 lignes)
- `src-tauri/src/db/client.rs` (371 lignes)
- `src-tauri/src/db/schema.rs` (350 lignes)
- `src-tauri/src/models/serde_utils.rs` (327 lignes)
- `src-tauri/src/tools/utils.rs` (250 lignes)
- `src-tauri/src/commands/*.rs` (14 fichiers)

### Documentation Consultee
- `CLAUDE.md` - Patterns SDK 2.x obligatoires
- `docs/DATABASE_SCHEMA.md` - Schema complet
- `docs/ARCHITECTURE_DECISIONS.md` - Decisions etablies
- `docs/TECH_STACK.md` - Versions

### Sources Externes
- [SurrealDB Performance Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/performance-best-practices)
- [SurrealDB Security Best Practices](https://surrealdb.com/docs/surrealdb/reference-guide/security-best-practices)
- [SurrealDB Rust SDK](https://surrealdb.com/docs/sdk/rust)
- [SurrealDB Releases](https://surrealdb.com/releases)

### Memoires Serena Consultees
- `surrealdb_summary_reference`
- `surrealdb_schema_analysis`
- `surrealdb_gaps_and_improvements`
- `surrealdb_relationships_and_queries`
