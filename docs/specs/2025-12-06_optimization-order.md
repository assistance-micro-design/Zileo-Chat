# Ordre de Priorisation des Optimisations - Zileo-Chat-3

## Metadata
- **Date**: 2025-12-06
- **Type**: Document de coordination
- **Statut**: Valide apres analyse et deduplication

---

## Resume

Ce document etablit l'ordre d'implementation des optimisations definies dans les 6 specs du repertoire `/docs/specs/`. Une analyse de compatibilite a permis d'identifier et corriger 4 redondances/incoherences.

### Specifications Sources

| Fichier | Domaine | Items Actifs | Effort |
|---------|---------|--------------|--------|
| `optimization-security.md` | Securite | 8 | ~1h45 |
| `optimization-db.md` | Database | 11 | ~9.5h |
| `optimization-backend.md` | Backend | 6 | ~8.5h |
| `optimization-mcp.md` | MCP | 10 | ~14.75h |
| `optimization-stores.md` | Frontend Stores | 10 | ~12h |
| `optimization-types.md` | Types TS/Rust | 7 | ~20-23h |

**Total apres deduplication**: ~57h (vs ~83h brut)

---

## Corrections Appliquees

Les corrections suivantes ont ete appliquees a `optimization-backend.md`:

| Item Original | Action | Raison |
|---------------|--------|--------|
| OPT-1 (SURREAL_SYNC_DATA) | SUPPRIME | Doublon de db/OPT-DB-2 |
| OPT-4 (QueryBuilder) | SUPPRIME | QueryBuilder existe deja dans `tools/utils.rs` - voir db/OPT-DB-6 |
| OPT-6 (thiserror) | DIFFERE Phase 7 | Refactoring trop large (114 commandes) |

---

## Ordre d'Implementation

### Phase 0: Securite Critique [OBLIGATOIRE]
**Effort**: ~2h | **Risque si omis**: Vulnerabilites actives (10 CVEs)

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| security | OPT-1 | Upgrade SurrealDB 2.3.10 → 2.4.0 | 15 min |
| security | OPT-2 | Verifier tauri-plugin-opener >= 2.2.1 | 5 min |
| security | OPT-3 | Renforcer CSP tauri.conf.json | 10 min |
| db | OPT-DB-2 | Documenter SURREAL_SYNC_DATA | 10 min |
| security | OPT-4 | Rejeter newlines dans API keys | 5 min |
| security | OPT-5 | Normaliser messages erreur API key | 10 min |
| security | OPT-6 | Validation env vars MCP (injection) | 30 min |

**Validation**: `cargo test && npm run check`

---

### Phase 1: Stabilite Frontend [OBLIGATOIRE]
**Effort**: ~3h | **Risque si omis**: Memory leaks, crashes

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| stores | OPT-5 | Corriger memory leaks (streaming, validation, userQuestion) | 2h |
| stores | OPT-1 | Update Svelte 5.43.14 → 5.45.6 | 10 min |
| stores | OPT-2 | Creer utilitaire getErrorMessage() | 30 min |
| stores | OPT-3 | Completer exports index.ts | 15 min |
| stores | OPT-4 | Fix theme.ts toggle() inefficace | 10 min |

**Validation**: `npm run test && npm run check`

---

### Phase 2: DB & Backend Quick Wins
**Effort**: ~4h | **Impact**: Performance, maintenabilite

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| db | OPT-DB-1 | Upgrade surrealdb feature (deja fait en Phase 0) | - |
| db | OPT-DB-3 | Documenter log level production | 1 min |
| db | OPT-DB-4 | Optimiser Cargo release profile | 5 min |
| db | OPT-DB-5 | Activer feature protocol-http | 1 min |
| backend | OPT-2 | Centraliser constantes validation | 1h |
| backend | OPT-3 | Standardiser messages erreur | 30 min |

**Validation**: `cargo build --release && cargo test`

---

### Phase 3: Types & Frontend Polish
**Effort**: ~5h | **Impact**: Maintenabilite, DX

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| types | OPT-1 | Synchroniser user-question types TS/Rust | 1h |
| types | OPT-2 | Standardiser convention nullabilite | 2h |
| types | OPT-3 | Centraliser constantes AVAILABLE_TOOLS | 1h |
| types | OPT-4 | Documenter custom deserializers | 1h |

**Validation**: `npm run check && cargo check`

---

### Phase 4: MCP Quick Wins
**Effort**: ~3h | **Impact**: Performance, observabilite

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| mcp | OPT-4 | Supprimer duplicate lock checks | 15 min |
| mcp | OPT-2 | Ajouter metriques latency (p50/p95/p99) | 30 min |
| mcp | OPT-1 | Implementer tool discovery caching | 1h |
| mcp | OPT-3 | HTTP connection pooling (reqwest::Client partage) | 1h |
| mcp | OPT-5 | Fix HTTP handle cleanup dans Drop | 30 min |

**Validation**: `cargo test -- mcp`

---

### Phase 5: Strategic Backend/DB
**Effort**: ~10h | **Impact**: Securite (injection), performance

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| db | OPT-DB-6 | Migrer format!() vers QueryBuilder existant | 4h |
| db | OPT-DB-7 | Implementer transaction handling | 2h |
| db | OPT-DB-8 | Ajouter LIMIT sur queries illimitees | 1h |
| backend | OPT-5 | Refactoring agent.rs (split fonctions longues) | 4h |

**Prerequis**: Phase 2 completee (constantes centralisees pour OPT-5)
**Validation**: `cargo test && cargo clippy`

---

### Phase 6: Strategic MCP
**Effort**: ~8h | **Impact**: Fiabilite, resilience

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| mcp | OPT-6 | Implementer circuit breaker | 4h |
| mcp | OPT-7 | Ajouter ID lookup table (O(1) vs O(n)) | 2h |
| mcp | OPT-8 | Health checks periodiques | 2h |

**Note**: Peut etre parallelise avec Phase 5 (pas de dependances)
**Validation**: `cargo test -- mcp`

---

### Phase 7: Strategic Frontend
**Effort**: ~12h | **Impact**: Maintenabilite, performance

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| stores | OPT-6 | Supprimer duplication workflows.ts | 1.5h |
| stores | OPT-7 | Refactorer processChunk en handlers | 2h |
| stores | OPT-8 | Creer CRUD factory (agents/prompts) | 4h |
| types | OPT-5 | Implementer specta + tauri-specta | 8-10h |

**Prerequis**: stores/OPT-2 (error utility) pour OPT-8
**Validation**: `npm run test && npm run build`

---

### Phase 8: Nice-to-Have [Optionnel]
**Effort**: ~15h | **Impact**: Faible, polish

| Spec | Item | Description | Effort |
|------|------|-------------|--------|
| backend | OPT-7 | Reduire clonage excessif | 2h |
| backend | OPT-8 | Migrer lazy_static → once_cell | 30 min |
| backend | OPT-9 | Optimiser tokio features | 30 min |
| db | OPT-DB-9 | Simplifier Double Arc embedding | 30 min |
| db | OPT-DB-10 | Query::with_stats() monitoring | 1h |
| db | OPT-DB-11 | Review indexes write-heavy tables | 30 min |
| mcp | OPT-9 | Extract get_saved_configs() helpers | 2h |
| mcp | OPT-10 | Structured error types MCPErrorCategory | 1.5h |
| stores | OPT-9 | Audit derived stores inutilises | 1h |
| stores | OPT-10 | Documenter pattern canonique | 30 min |
| types | OPT-6 | Specialiser Record<string, unknown> | 3h |
| types | OPT-7 | Validation Zod runtime | 4h |

---

### Phase 9: Differe [Post-v1]
**Effort**: ~18h | **Impact**: Variable

| Spec | Item | Description | Raison du report |
|------|------|-------------|------------------|
| backend | ~~OPT-6~~ | thiserror CommandError | Refactoring 114 commandes |
| db | OPT-DB-12 | thiserror CommandError | Meme raison |
| db | OPT-DB-13 | Query caching | SDK ne supporte pas prepared statements |
| db | OPT-DB-14 | Live Query API | Feature 2.4.0, pas critique v1 |
| stores | - | Svelte 5 runes migration | Effort 8-16h, planification requise |
| stores | - | Tauri channels vs events | Changements backend requis |

---

## Parallelisation Possible

```
Phase 0 (Securite)
    |
    v
Phase 1 (Stabilite Frontend)
    |
    v
Phase 2 (DB/Backend Quick Wins)
    |
    +------------------+
    |                  |
    v                  v
Phase 3 (Types)    Phase 4 (MCP Quick Wins)
    |                  |
    v                  v
Phase 5 (Strategic) <- Phase 6 (Strategic MCP) [PARALLELE]
    |
    v
Phase 7 (Strategic Frontend)
    |
    v
Phase 8 (Nice-to-Have) [Optionnel]
```

---

## Metriques de Suivi

### Progression par Phase

| Phase | Status | % Complete | Notes |
|-------|--------|------------|-------|
| 0 | Pending | 0% | - |
| 1 | Pending | 0% | - |
| 2 | Pending | 0% | - |
| 3 | Pending | 0% | - |
| 4 | Pending | 0% | - |
| 5 | Pending | 0% | - |
| 6 | Pending | 0% | - |
| 7 | Pending | 0% | - |
| 8 | Pending | 0% | Optionnel |

### Effort Cumule

| Phases | Effort | % du Total |
|--------|--------|------------|
| 0-1 (Obligatoires) | 5h | 9% |
| 2-4 (Quick Wins) | 12h | 21% |
| 5-7 (Strategic) | 30h | 53% |
| 8 (Nice-to-Have) | 10h | 17% |
| **Total** | **~57h** | 100% |

---

## Validation Finale

Avant de considerer une phase complete:

1. [ ] Tous les tests passent: `cargo test && npm run test`
2. [ ] Pas de warnings: `cargo clippy -- -D warnings && npm run lint`
3. [ ] Build reussit: `cargo build --release && npm run build`
4. [ ] Types valides: `npm run check`
5. [ ] Pas de regression fonctionnelle (test manuel)

---

## References

- `optimization-security.md` - Securite (CVEs, CSP, validation)
- `optimization-db.md` - Database (SurrealDB, queries, transactions)
- `optimization-backend.md` - Backend (constantes, refactoring, patterns)
- `optimization-mcp.md` - MCP (caching, pooling, circuit breaker)
- `optimization-stores.md` - Stores (memory leaks, duplication, factory)
- `optimization-types.md` - Types (synchronisation, specta, nullabilite)
