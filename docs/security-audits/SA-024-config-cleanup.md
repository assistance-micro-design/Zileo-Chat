# SA-024: Config & Dependency Cleanup

**Date**: 2026-02-26
**Branch**: `security/audit-remediation-tdd`
**Source**: Senior Review Full Audit 2026-02-26

---

## Scope

8 items from the senior review (recommendations 1-7, 10).

| # | ID | Description | Severity | Effort |
|---|-----|-------------|----------|--------|
| 1 | H4 | Replace `once_cell` with `std::sync::LazyLock` | HIGH | LOW |
| 2 | M1 | Replace `futures` with `futures_util::future::join_all` | MEDIUM | LOW |
| 3 | H3 | Fix inventory `total_commands` 106 -> 123 | HIGH | LOW |
| 4 | H1 | Convert LLM provider `.expect()` to `Result` | HIGH | MEDIUM |
| 5 | H2 | Audit `#[allow(dead_code)]` annotations | HIGH | MEDIUM |
| 6 | M4 | Move `@humanspeak/svelte-virtual-list` to dependencies | MEDIUM | LOW |
| 7 | M3 | Clean all OPT-* comments | MEDIUM | LOW |
| 10 | M7 | Pin `surrealdb` version to `~2.6` | MEDIUM | LOW |

---

## Phase 1: Dependency Cleanup (Items 1, 2, 10) - DONE

### 1.1 - Replace `once_cell` with `std::sync::LazyLock` (H4) - DONE

**Fichiers modifies**:
- `src-tauri/src/mcp/http_handle.rs`: `Lazy<Client>` -> `LazyLock<Client>`, import `std::sync::LazyLock`
- `src-tauri/src/tools/registry.rs`: `Lazy<ToolRegistry>` -> `LazyLock<ToolRegistry>`, import `std::sync::LazyLock`
- `src-tauri/Cargo.toml`: `once_cell = "1.20"` supprime + commentaire OPT-8

### 1.2 - Replace `futures` with `futures_util` (M1) - DONE

**Fichiers modifies**:
- `src-tauri/src/db/persistence.rs`: `futures::future::join_all` -> `join_all` + import `futures_util::future::join_all`
- `src-tauri/src/db/queries.rs`: idem (dans module `cascade`)
- `src-tauri/Cargo.toml`: `futures = "0.3.31"` supprime + commentaire OPT-WF-4. `futures-util` deja present.

### 1.3 - Pin surrealdb version (M7) - DONE

**Fichier modifie**: `src-tauri/Cargo.toml`
- `version = "2.5.0"` -> `version = "~2.6"` (resolu a 2.6.2)

**Verification**: cargo fmt PASS, cargo clippy PASS (0 warnings), cargo test PASS (2002 tests, 0 failures)

---

## Phase 2: Production Robustness (Item 4)

### 2.1 - Convert `.expect()` to `Result` dans les LLM providers (H1)

**Fichiers** (uniquement les `.expect()` hors startup/infaillible):
- `src-tauri/src/llm/manager.rs` (~lines 106, 145)
- `src-tauri/src/llm/embedding.rs` (~lines 413, 435)
- `src-tauri/src/llm/mistral.rs` (~line 593)
- `src-tauri/src/llm/ollama.rs` (~lines 68, 257)

**Ne PAS toucher** (startup/infaillible):
- `main.rs` (lines 98, 516) - startup init
- `state.rs` (line 72) - startup init
- `keystore.rs` (lines 281, 283) - base64 infaillible
- `prompt.rs` (lines 130, 166) - static regex infaillible
- `http_handle.rs` (line 64) - lazy static init

**Actions**:
- Identifier les fonctions contenant les `.expect()`
- Changer les signatures pour retourner `Result<T, String>` si pas deja fait
- Remplacer `.expect("msg")` par `.map_err(|e| format!("Failed to create HTTP client: {}", e))?`
- Propager les `Result` aux appelants si necessaire

**Verification**: `cargo clippy -- -D warnings` + `cargo test`

---

## Phase 3: Code Hygiene (Items 5, 7)

### 3.1 - Clean OPT-* comments (M3)

**Action**: Supprimer TOUS les commentaires OPT-* du codebase.

**Pattern a chercher**: `// OPT-` dans tous les fichiers `.rs`, `.ts`, `.svelte`

**Methode**: `grep -rn "// OPT-"` puis suppression systematique des lignes ou nettoyage du commentaire.

**Verification**: `grep -rn "OPT-" src-tauri/src/ src/` doit retourner 0 resultats

### 3.2 - Audit `#[allow(dead_code)]` (H2)

**Approche en 3 etapes**:

1. **Inventaire**: Lister tous les `#[allow(dead_code)]` hors `#[cfg(test)]`
2. **Categoriser**:
   - **Supprimer**: Code vraiment mort, jamais appele -> supprimer le code ET l'annotation
   - **Convertir**: Code test-only -> deplacer sous `#[cfg(test)]`
   - **Garder**: API surface intentionnelle documentee (avec commentaire justificatif)
3. **Nettoyer**: Meme traitement pour `#[allow(unused_imports)]` dans les `mod.rs`

**Verification**: `cargo clippy -- -D warnings` + `cargo test` + count avant/apres

---

## Phase 4: Config & Inventory (Items 3, 6)

### 4.1 - Fix inventory total_commands (H3)

**Fichier**: `.claude/registry/inventory.yml`

**Action**: Changer `total_commands: 106` en `total_commands: 123`

Optionnel: identifier et ajouter les 17 commandes manquantes dans l'inventaire detaille.

### 4.2 - Move svelte-virtual-list to dependencies (M4)

**Fichier**: `package.json`

**Action**:
1. Verifier que `@humanspeak/svelte-virtual-list` est utilise dans des `.svelte` (production)
2. Si oui: deplacer de `devDependencies` vers `dependencies`
3. `npm install` pour regenerer le lockfile

**Verification**: `npm run check` + `npm run build`

---

## Ordre d'execution

```
Phase 1 (Deps)     -> cargo build + cargo test
Phase 2 (Expect)   -> cargo clippy + cargo test
Phase 3 (Hygiene)  -> cargo clippy + cargo test + npm run lint
Phase 4 (Config)   -> npm run check
```

Chaque phase = 1 commit separe.

---

## Verification finale

```bash
# Backend (sequentiel, un cargo a la fois)
cargo fmt --check
cargo clippy -- -D warnings
cargo test

# Frontend
npm run lint
npm run check
npm run test
```

---

## Criteres de completion

- [x] `once_cell` supprime de Cargo.toml, remplace par `std::sync::LazyLock`
- [x] `futures` supprime de Cargo.toml, remplace par `futures_util`
- [x] `surrealdb` version pinnee a `~2.6` (resolu a 2.6.2)
- [ ] Zero `.expect()` dans les LLM providers (hors startup)
- [ ] `#[allow(dead_code)]` reduit de 120+ a minimum justifie
- [ ] Zero commentaire `OPT-*` dans le codebase
- [ ] `total_commands: 123` dans inventory.yml
- [ ] `@humanspeak/svelte-virtual-list` dans `dependencies` (si utilise en prod)
- [ ] Tous les checks passent (fmt, clippy, test, lint, check)
