# Roadmap vers 1.0 (Production)

> **Version actuelle**: 0.9.2
> **Statut**: Beta Stable - Pas encore GA (General Availability)

---

## Points d'Attention

| Probleme | Severite | Impact | Fichiers/Localisation |
|----------|----------|--------|----------------------|
| ~~69 `unwrap()`/`expect()` dans commands/~~ | ~~HAUTE~~ RESOLU | ~~Panic potentiel en prod~~ 68/69 dans tests (OK), 1 corrige | `src-tauri/src/commands/*.rs` |
| 66 doc tests ignores (necessitent DB) | MOYENNE | Couverture reduite | `src-tauri/src/**/*.rs` |
| npm: @sveltejs/kit (1 HIGH vuln) | MOYENNE | DoS + SSRF potentiel | `package.json` |
| cargo: GTK3 bindings "unmaintained" | BASSE | Dependance Tauri Linux | Tauri upstream |

---

## Haute Priorite (Bloquant 1.0)

### 1. ~~Remplacer `unwrap()`/`expect()` par proper error handling~~ RESOLU

> **Analyse detaillee (2025-12-14)**: Sur 69 occurrences detectees par grep, **68 sont dans des blocs `#[cfg(test)]`**.
> L'utilisation de `unwrap()`/`expect()` dans les tests est idiomatique en Rust et acceptable.
>
> **Seule occurrence en production corrigee**: `models.rs` ligne 804 (api_key.unwrap() -> match pattern)

**Localisation**: `src-tauri/src/commands/`

**Repartition reelle** (69 occurrences totales):
- `validation.rs` - 13 occurrences (toutes dans `#[cfg(test)]`)
- `memory.rs` - 13 occurrences (toutes dans `#[cfg(test)]`)
- `workflow.rs` - 9 occurrences (toutes dans `#[cfg(test)]`)
- `agent.rs` - 9 occurrences (toutes dans `#[cfg(test)]`)
- `task.rs` - 6 occurrences (toutes dans `#[cfg(test)]`)
- `user_question.rs` - 4 occurrences (toutes dans `#[cfg(test)]`)
- `mcp.rs` - 4 occurrences (toutes dans `#[cfg(test)]`)
- `llm.rs` - 3 occurrences (toutes dans `#[cfg(test)]`)
- `models.rs` - 3 occurrences (1 en prod **CORRIGEE**, 2 dans tests)
- `migration.rs` - 3 occurrences (toutes dans `#[cfg(test)]`)
- `embedding.rs` - 2 occurrences (toutes dans `#[cfg(test)]`)

**Correction appliquee** (`models.rs:788-796`):
```rust
// AVANT (risque theorique de panic)
let api_key = keystore.get_key("Mistral");
if api_key.is_none() { return Ok(ConnectionTestResult::failure(...)); }
// ... plus tard
.header("Authorization", format!("Bearer {}", api_key.unwrap()))

// APRES (pattern matching idiomatique)
let api_key = match keystore.get_key("Mistral") {
    Some(key) => key,
    None => return Ok(ConnectionTestResult::failure(...)),
};
// ... plus tard
.header("Authorization", format!("Bearer {}", api_key))
```

**Commande pour verifier**:
```bash
# Compter les occurrences hors tests (devrait retourner 0)
grep -rn "unwrap()\|expect(" src-tauri/src/commands/ | grep -v "#\[cfg(test)\]" | grep -v "fn test_"
```

---

### 2. Ajouter tests d'integration backend avec DB

**Probleme**: Les 66 doc tests sont ignores car ils necessitent un runtime SurrealDB.

**Solution**:
1. Creer un module `tests/integration/` avec setup/teardown DB
2. Utiliser `#[ignore]` avec `cargo test -- --ignored` pour CI
3. Ajouter fixture DB ephemere pour tests

**Structure proposee**:
```
src-tauri/
  tests/
    integration/
      mod.rs
      db_setup.rs      # Helper pour DB ephemere
      workflow_test.rs
      agent_test.rs
      memory_test.rs
```

**Commande CI**:
```bash
cargo test --all-features
cargo test -- --ignored  # Tests d'integration
```

---

### 3. Resoudre les 66 doc tests ignores

**Localisation**: Doc tests `/// ```rust,ignore` dans:
- `src/tools/` - Majorite des doc tests
- `src/mcp/` - Server handles et manager
- `src/models/` - Serialization utilities

**Options**:
1. **Convertir en unit tests** - Deplacer le code dans `#[cfg(test)]` modules
2. **Mocker les dependances** - Creer traits pour injection
3. **Marquer explicitement** - Documenter pourquoi ignores

---

## Moyenne Priorite

### 1. Resoudre vulnerabilites npm

**Vulnerabilites actuelles** (janvier 2026):
- 1 HIGH: DoS + SSRF dans @sveltejs/kit
- 1 LOW: cookie < 0.7.0

**Commande**:
```bash
npm audit
npm update @sveltejs/kit
# ou si breaking change:
npm audit fix --force  # Attention: peut casser
```

**Note**: Verifier compatibilite avant mise a jour majeure de SvelteKit.

---

### 2. Implementer les 4 OPT-TODO-* restants

**Liste des optimisations** (dans `src-tauri/src/tools/todo/tool.rs`):

| ID | Description | Impact | Statut |
|----|-------------|--------|--------|
| OPT-TODO-2 | ParamQueryBuilder pour SQL injection safety | Securite | TODO |
| ~~OPT-TODO-4~~ | ~~Parameterized query pour SQL injection safety~~ | ~~Securite~~ | DONE |
| ~~OPT-TODO-5~~ | ~~Reduire N+1 queries (3->1) avec UPDATE RETURN~~ | ~~Perf~~ | DONE |
| ~~OPT-TODO-6~~ | ~~Reduire N+1 queries (2->1) avec UPDATE RETURN~~ | ~~Perf~~ | DONE |
| ~~OPT-TODO-7~~ | ~~Utiliser db_error() pour consistance~~ | ~~Code quality~~ | DONE |
| ~~OPT-TODO-9~~ | ~~TASK_SELECT_FIELDS constant pour DRY~~ | ~~Code quality~~ | DONE |
| OPT-TODO-10 | Ajouter LIMIT pour prevenir memory explosion | Stabilite | TODO |
| OPT-TODO-11 | Tests d'integration avec vraie DB | Tests | TODO |
| OPT-TODO-12 | Tests prevention SQL injection | Securite | TODO |

**Commande pour localiser**:
```bash
grep -rn "OPT-TODO" src-tauri/src/
```

---

### 3. ~~Tests E2E (Playwright)~~ IMPLEMENTE

**Statut**: 7 fichiers de tests E2E existent dans `tests/e2e/`:
- `workflow-crud.spec.ts` - CRUD workflows
- `workflow-persistence.spec.ts` - Persistence et reload
- `chat-interaction.spec.ts` - Interactions chat
- `settings-config.spec.ts` - Configuration settings
- `agent-crud.spec.ts` - CRUD agents
- `mcp-management.spec.ts` - Gestion MCP
- `memory-management.spec.ts` - Gestion memoire

**Commande**:
```bash
npm run test:e2e
```

---

## Basse Priorite (Post-1.0)

### GTK3 -> GTK4 Migration

**Probleme**: Les bindings GTK3 (atk, gdk, gtk) sont marques "unmaintained" dans RustSec.

**Solution**: Attendre que Tauri supporte GTK4 nativement. C'est un probleme upstream, pas dans notre code.

**Tracking**: https://github.com/tauri-apps/tauri/issues

---

## Checklist Pre-Release 1.0

- [x] 0 `unwrap()`/`expect()` dans commands/ en production (68/69 sont dans tests, 1 corrige)
- [x] Tests E2E implementes (7 fichiers)
- [x] CHANGELOG.md a jour (v0.9.2)
- [x] Tous les fichiers committes
- [ ] Couverture tests backend >= 50%
- [ ] npm audit = 0 vulnerabilites HIGH/CRITICAL (1 HIGH actuel)
- [ ] cargo audit = 0 vulnerabilites (warnings OK)
- [ ] Version bump 0.9.2 -> 1.0.0
- [ ] Tag git v1.0.0
- [ ] Build release teste sur Linux/Windows/macOS

---

## Commandes Utiles

```bash
# Verification complete
npm run lint && npm run check
cd src-tauri && cargo clippy -- -D warnings && cargo test

# Audit securite
npm audit
cd src-tauri && cargo audit

# Build production
npm run build
cd src-tauri && cargo build --release

# Localiser les problemes
grep -rn "unwrap()\|expect(" src-tauri/src/commands/
grep -rn "OPT-TODO" src-tauri/src/
```

---

*Document mis a jour le 2026-01-25 - Zileo-Chat-3 v0.9.2*
