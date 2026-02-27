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

## Phase 2: Production Robustness (Item 4) - DONE

### 2.1 - Convert `.expect()` to `Result` dans les LLM providers (H1) - DONE

**Fichiers modifies** (7 `.expect()` convertis + 3 `Default` impls supprimes):

**Conversions `.expect()` -> `.map_err()?`:**
- `src-tauri/src/llm/manager.rs`: `ProviderManager::new()` et `with_retry_config()` -> `Result<Self, String>`
- `src-tauri/src/llm/embedding.rs`: `EmbeddingService::with_provider()` -> `Result<Self, String>`, `new()` -> `#[cfg(test)]` only
- `src-tauri/src/llm/ollama.rs`: `OllamaProvider::with_url()` -> `Result<Self, String>`

**`Default` impls supprimes** (test-only, aucun appelant production):
- `src-tauri/src/llm/manager.rs`: `impl Default for ProviderManager` supprime
- `src-tauri/src/llm/ollama.rs`: `impl Default for OllamaProvider` supprime
- `src-tauri/src/llm/mistral.rs`: `impl Default for MistralProvider` supprime
- `src-tauri/src/llm/embedding.rs`: `impl Default for EmbeddingService` supprime

**Propagation aux appelants production:**
- `src-tauri/src/state.rs`: `AppState::new()` -> `.map_err(|e| anyhow::anyhow!(e))?`
- `src-tauri/src/state.rs`: `load_embedding_config()` -> `match` avec log erreur
- `src-tauri/src/commands/embedding.rs`: `update_embedding_service_internal()` -> `match` avec log erreur

**Tests mis a jour** (14 fichiers):
- `manager.rs`, `ollama.rs`, `mistral.rs`, `embedding.rs`: helpers `test_*_provider()` + `.expect("test")`
- `state.rs`, `test_utils.rs`, `agents/llm_agent.rs`, `tools/context.rs`
- `commands/agent.rs`, `commands/task.rs`, `commands/memory.rs`, `commands/validation.rs`, `commands/workflow.rs`
- `tests/sub_agent_tools_integration.rs`

**Ne PAS touche** (startup/infaillible - confirme):
- `main.rs` (lines 98, 516) - startup init
- `state.rs` (line 72) - MCPManager startup init
- `keystore.rs` (lines 281, 283) - base64 infaillible
- `prompt.rs` (lines 130, 166) - static regex infaillible
- `http_handle.rs` (line 64) - lazy static init

**Verification**: cargo fmt PASS, cargo clippy PASS (0 warnings), cargo test PASS (2000 tests, 0 failures)

---

## Phase 3: Code Hygiene (Item 7) - DONE

### 3.1 - Clean OPT-* comments (M3) - DONE

**Action**: Supprimer TOUS les marqueurs OPT-* du codebase tout en gardant le texte descriptif utile.

**171 marqueurs supprimes** dans 52 fichiers (32 Rust + 20 frontend).

**Patterns traites**:
- `(OPT-XXX)` en fin de commentaire -> supprime
- `(OPT-XXX: texte)` -> garde `(texte)`
- `// OPT-XXX: texte` -> `// texte`
- `(OPT-XXX, OPT-YYY)` -> supprime
- `-- OPT-XXX: texte` (SQL) -> `-- texte`
- `OPT-XXX` inline apres du code -> supprime

**Fichiers modifies (Rust - 32)**: commands/ (agent, llm_models, mcp, memory, streaming, task, user_question, workflow), constants.rs, db/ (queries, schema), llm/ (manager, mistral, mod, ollama), mcp/manager.rs, models/sub_agent.rs, security/validation.rs, state.rs, tools/ (constants, context, delegate_task, memory/tool, parallel_tasks, spawn_agent, sub_agent_circuit_breaker, sub_agent_executor, user_question/circuit_breaker, user_question/mod, user_question/tool, utils, validation_helper)

**Fichiers modifies (Frontend - 20)**: components/settings/ (MCPSection, SettingsSectionHeader, agents/, memory/, prompts/, providers/), components/ui/ (DeleteConfirmModal, ErrorBanner), stores/ (llm, mcp, user-question), routes/ (agent, settings), styles/global.css

**Verification**: `grep -rn "OPT-" src-tauri/src/ src/` retourne 0 resultats. cargo clippy PASS (0 warnings), cargo test PASS.

### 3.2 - Audit `#[allow(dead_code)]` (H2) - DEFERRED

Reporte a une phase ulterieure. Les annotations existantes (~165) restent en place.

---

## Phase 4: Config & Inventory (Items 3, 6) - DONE

### 4.1 - Fix inventory total_commands (H3) - DONE

**Fichier**: `.claude/registry/inventory.yml`

**Action realisee**:
- `total_commands: 106` -> `total_commands: 123`
- Audit complet: l'inventaire ne listait que **84** commandes (pas 106 comme indique dans le header)
- **39 commandes ajoutees** (pas 17 comme estime initialement):
  - 5 modules entiers manquants: `llm` (9 cmd), `thinking` (5), `migration` (4), `user_question` (3), `sub_agent_execution` (2)
  - 6 modules partiellement documentes: `tool_execution` (+5), `mcp` (+1), `llm_models` (+1), `validation` (+3), `embedding` (+2), `task` (+1)
- 0 commandes obsoletes trouvees (toutes les commandes listees existent dans le code)

### 4.2 - Move svelte-virtual-list to dependencies (M4) - DONE

**Fichiers modifies**: `package.json`, `package-lock.json`

**Action realisee**:
1. Verifie: `@humanspeak/svelte-virtual-list` est importe dans `MemoryList.svelte` (composant production)
2. Deplace de `devDependencies` vers `dependencies`
3. `npm install` pour regenerer le lockfile

**Verification**: `npm run lint` PASS, `npm run check` PASS (0 errors, 0 warnings)

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
- [x] Zero `.expect()` dans les LLM providers (hors startup/tests)
- [ ] `#[allow(dead_code)]` reduit de 120+ a minimum justifie (DEFERRED)
- [x] Zero commentaire `OPT-*` dans le codebase (171 supprimes dans 52 fichiers)
- [x] `total_commands: 123` dans inventory.yml (+ 39 commandes ajoutees a l'inventaire detaille)
- [x] `@humanspeak/svelte-virtual-list` dans `dependencies` (utilise dans MemoryList.svelte)
- [ ] Tous les checks passent (fmt, clippy, test, lint, check)
