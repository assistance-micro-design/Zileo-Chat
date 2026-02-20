# Action Plan - Security Audit Remediation

**Date**: 2026-02-19 (updated with code verification)
**Source**: Cross-analysis of 11 audit sessions (SA-001 through SA-013)
**Original findings**: 82 (10 CRITICAL, 27 HIGH, 30 MEDIUM, 13 LOW, 2 INFO)
**After verification**: 82 (4 CRITICAL, 27 HIGH, 34 MEDIUM, 13 LOW, 2 INFO)
**Estimated total effort**: ~68h (reduced from ~83h after desktop context assessment)

## Cross-Session Analysis

### Convergent Findings (same issue flagged by multiple sessions)

| Root Issue | Sessions | Combined IDs |
|-----------|----------|-------------|
| **Import pipeline vulnerabilities** | SA-001, SA-002, SA-005, SA-012 | C3, S2-H1, S2-M1, S5-H2, F4 |
| **models.rs string interpolation** | SA-001, SA-002 | H3-H6, S2-L2 |
| **Error handling inconsistency** | SA-009, SA-010, SA-011 | F2, 30 try/catch, M-002/M-006 |
| **import_export.rs monolith** | SA-002, SA-005, SA-007 | S2-H1, S5-C1, 4 oversized functions |
| **Missing sanitize at boundaries** | SA-001, SA-002, SA-005, SA-012 | C3, S2-M1, S5-H2, F4 |
| **Over-broad Tauri permissions** | SA-005 | S5-M1, S5-M2, S5-M3 |
| **Dependency feature bloat** | SA-006, SA-008 | rig-core "all", surrealdb protocol-http |

### Severity Adjustments (post code verification + bias-check)

| Finding | Original | Verified | Reason |
|---------|----------|----------|--------|
| SA-001 C4 (type_filter) | CRITICAL | **HIGH** | Input from frontend dropdown, not free-form user text |
| SA-001 C5 (model lookup) | CRITICAL | **MEDIUM** | Model name from agent config in DB, not direct user input |
| SA-002 S2-C1 (HTTP base_url) | CRITICAL | **MEDIUM** | Desktop user configures own URL. Add UI warning, not hard block. |
| SA-002 S2-H3 (MCP HTTP) | HIGH | **MEDIUM** | Same reasoning as S2-C1: user-configured URL |
| SA-002 S2-M1 (missing sanitize) | MEDIUM | **HIGH** | Adversarial reframe: import files ARE external attack surface |
| SA-005 C1 (read_import_file) | CRITICAL | **HIGH** | Requires prior XSS/CSP bypass. Chain-of-attack, not direct exploit. |
| SA-006 (7 NPM CVEs) | HIGH/MODERATE | **N/A** | All 7 CVEs not applicable to desktop (SSR, server-side, dev-only) |
| SA-013 #1-4 (serde default) | CRITICAL | **HIGH** | Type bug, not security vulnerability. Value always present. |
| SA-013 #13 (RiskLevel) | CRITICAL | **CONFIRMED CRITICAL** | Deserialization panic = app crash |
| SA-013 #14-15 (ChunkType) | CRITICAL | **MEDIUM** | Orphan TS variants, never received. No crash. |
| SA-008 PERF-1 (clone) | P2 | P1 | Affects every LLM call, compounds with conversation |

### Emergent Patterns

1. **Import/Export is the #1 attack surface** - 5 sessions flagged it independently (SA-001, SA-002, SA-005, SA-007, SA-012)
2. **String interpolation** is the #1 security anti-pattern (24 occurrences across 6 files)
3. **Error handling inconsistency** is the #1 quality anti-pattern (stores + components + settings)
4. **Over-broad permissions** - Tauri capability defaults grant more access than needed (SA-005)
5. **Dependency feature bloat** - rig-core and surrealdb pull unnecessary features (SA-006)
6. **Excellent fundamentals**: 0 unwrap in prod, good architecture, proper CSP, secure API key storage, lock files committed, Dependabot configured

---

## P0 - Security Critical

**Deadline**: Before any release. ~10h total (reduced from ~12.5h after HTTPS enforcement moved to P1).

### P0-A: SurrealQL Injection - Direct User Input (3h)

**Findings**: SA-001 C1, C2, C4
**Files**: `commands/prompt.rs`, `commands/embedding.rs`
**What**: User-typed search terms and filter values directly interpolated in SurrealQL WHERE clauses.
**Why P0**: Any user can trigger injection by typing in the search box.

```
/Fix_Zileo

Contexte: Audit SA-001 - SurrealQL injection via string interpolation.

Fichiers a corriger:
1. src-tauri/src/commands/prompt.rs:304-313
   - search_term interpole dans WHERE CONTAINS (C1)
   - category interpole dans WHERE = (C2)
   - Fix: utiliser $search et $category comme bind parameters

2. src-tauri/src/commands/embedding.rs:500-503
   - type_filter interpole dans WHERE = (C4)
   - Fix: utiliser $type comme bind parameter

Pattern correct (deja utilise ailleurs dans le projet):
  let query = "SELECT ... FROM memory WHERE type = $type";
  db.query(query).bind(("type", type_filter)).await?;

Verifier: cargo clippy -- -D warnings && cargo test
Ne PAS toucher aux autres fichiers.
```

### P0-B: Import Pipeline Security (4h)

**Findings**: SA-001 C3, SA-002 S2-H1, SA-002 S2-M1, SA-012 F4
**Files**: `commands/embedding.rs:446-453`, `commands/import_export.rs`
**What**: External file content directly interpolated in CREATE/UPDATE queries. Missing `sanitize_for_surrealdb()`.
**Why P0**: Import files are fully user-controlled external data.

```
/Fix_Zileo

Contexte: Audit SA-001 C3 + SA-002 S2-H1/M1 + SA-012 F4 - Import pipeline injection.

4 problemes convergents dans le pipeline d'import:

1. commands/embedding.rs:446-453 (import_memories)
   - content interpole dans CREATE CONTENT avec simple replace
   - Fix: utiliser CONTENT $data bind avec serde_json::to_value()
   - Ajouter sanitize_for_surrealdb() avant insertion

2. commands/import_export.rs - execute_import() (SA-002 S2-H1)
   Champs interpoles sans echappement dans UPDATE/CREATE:
   - Agent: lifecycle (L821), llm.provider (L822), llm.model (L823)
   - MCP Server: command (L943), enabled (L951)
   - Model: provider (L1056, L1071)
   - Prompt: category (L1178, L1188)
   Fix: convertir en CONTENT $data bind pattern pour chaque entite

3. Ajouter sanitize_for_surrealdb() a l'entree de execute_import()
   et import_memories() - le helper existe deja dans db/utils.rs

Pattern correct (deja utilise pour MCP call logs dans mcp/manager.rs:953):
  let json_data = serde_json::to_value(&data)?;
  let json_data = sanitize_for_surrealdb(json_data);
  db.query("CREATE entity:`id` CONTENT $data").bind(("data", json_data)).await?;

Verifier: cargo clippy -- -D warnings && cargo test
```

### ~~P0-C~~ -> P1-I: HTTPS Enforcement for API Keys (1h) **[MOVED TO P1]**

**Findings**: SA-002 S2-C1 (MEDIUM after verification), SA-002 S2-H3 (MEDIUM after verification)
**Reason for demotion**: Desktop user explicitly configures their own provider URL. This is a user choice, not an externally exploitable vulnerability. A UI warning + localhost exception is sufficient.
**Files**: `commands/custom_provider.rs:127-129,223-227`, `mcp/http_handle.rs:160-178`

```
/Fix_Zileo

Contexte: Audit SA-002 S2-C1 + S2-H3 - HTTPS enforcement (downgraded to P1 after desktop context review).

Approche: UI warning au lieu de hard block. Autoriser localhost pour dev local.

1. commands/custom_provider.rs
   - create_custom_provider() + update_custom_provider(): ajouter log::warn si HTTP non-localhost
   - Ajouter un champ warning dans le retour pour affichage UI
   - Autoriser localhost/127.0.0.1 sans warning

2. mcp/http_handle.rs:160-178
   - Meme approche: warning log si HTTP non-localhost

Verifier: cargo clippy -- -D warnings && cargo test
```

### P0-D: DB Schema TYPE object Fixes (2h)

**Findings**: SA-012 F2, F3, F5
**Files**: `db/schema.rs`
**What**: Dynamic MCP params/results stored as `TYPE object` - keys silently dropped (ERR_SURREAL_001).
**Why P0**: Silent data loss on every MCP call log.

```
/Fix_Zileo

Contexte: Audit SA-012 F2/F3/F5 - ERR_SURREAL_001 violation dans le schema.

3 champs utilisent TYPE object/array pour des donnees dynamiques:

1. db/schema.rs:163 - mcp_call_log.params TYPE object
   -> Changer en: DEFINE FIELD OVERWRITE params ON mcp_call_log TYPE string DEFAULT '{}'

2. db/schema.rs:164 - mcp_call_log.result TYPE array|object
   -> Changer en: DEFINE FIELD OVERWRITE result ON mcp_call_log TYPE string DEFAULT '[]'

3. db/schema.rs:107 - validation_request.details TYPE object
   -> Changer en: DEFINE FIELD OVERWRITE details ON validation_request TYPE string DEFAULT '{}'

Ensuite mettre a jour le code Rust qui lit/ecrit ces champs:
- Serialiser avec serde_json::to_string() avant ecriture
- Deserialiser avec serde_json::from_str() apres lecture
- Verifier les modeles Rust correspondants (serde attributes)
- Pattern: PAT_SERDE_001 (serialize_as_json_string/deserialize_json_string)

Attention: migration necessaire pour les donnees existantes.
Ajouter une migration dans db/client.rs qui convertit les donnees existantes.

Verifier: cargo clippy -- -D warnings && cargo test
```

### P0-F: Arbitrary Filesystem Read/Write via Unused Commands (30min) **[KEPT AT P0]**

**Findings**: SA-005 C1 (downgraded to HIGH after verification, but kept at P0 priority)
**Files**: `commands/import_export.rs:1251,1276`
**What**: `read_import_file` and `save_export_to_file` accept arbitrary filesystem paths from the webview. `read_import_file` is unused (frontend uses FileReader API instead).
**Why still P0**: While exploitation requires prior XSS/CSP bypass (hence HIGH not CRITICAL), the fix is trivial (remove unused command) and eliminates an unnecessary attack primitive. Low effort, high reward.

```
/Fix_Zileo

Contexte: Audit SA-005 C1 - Commandes IPC avec acces fichier non restreint.

Fichiers a corriger:

1. commands/import_export.rs:1276-1280 (read_import_file)
   - Cette commande est INUTILISEE (le frontend utilise FileReader)
   - Option A (recommandee): Supprimer la commande + retirer de generate_handler!
   - Option B: Ajouter validation de chemin (canonicalize + allowlist)

2. commands/import_export.rs:1251 (save_export_to_file)
   - Utilisee par ExportPanel.svelte via dialog save()
   - Le chemin vient du dialog natif (controle utilisateur)
   - Ajouter validation: canonicalize + restrict to allowed dirs
     let allowed = [dirs::data_dir(), dirs::download_dir(), dirs::document_dir(), std::env::temp_dir()];

Verifier: cargo clippy -- -D warnings && cargo test
```

### P0-E: models.rs + task.rs Injection (2h)

**Findings**: SA-001 H3-H9, M1
**Files**: `commands/models.rs`, `commands/task.rs`
**What**: User input (model name, api_name, base_url, task name/description) uses broken `replace('\'', "''")` escaping.
**Why P0**: Direct user input with known-broken escaping pattern.

```
/Fix_Zileo

Contexte: Audit SA-001 H3-H9 + M1 - replace escaping anti-pattern.

Fichiers a corriger:

1. commands/models.rs
   - update_model() L444,471: name, api_name utilisant replace -> bind params
   - update_provider_settings() L700: base_url utilisant replace -> bind param
   - L448-450: WHERE clause avec 3 valeurs interpolees -> $provider, $api_name, $exclude_id
   - L930-931: sync_builtin_models() -> bind params pour model.name, model.api_name

2. commands/task.rs
   - update_task() L364,371,375: name, description, agent_assigned -> bind params

Pattern: remplacer les SET avec format!() par des bind parameters:
  // AVANT (broken):
  format!("UPDATE ... SET name = '{}'", name.replace('\'', "''"))
  // APRES (correct):
  "UPDATE ... SET name = $name"  +  .bind(("name", &name))

Verifier: cargo clippy -- -D warnings && cargo test
```

---

## P1 - Bugs Silencieux / High Impact

**Deadline**: Before next feature release. ~19h total.

### P1-A: Type Mismatches TS <-> Rust (3h)

**Findings**: SA-013 #1-4, #6, #12, #13, #14-15
**Files**: Multiple type files in `src/types/` and `src-tauri/src/models/`

```
/Fix_Zileo

Contexte: Audit SA-013 - Incoherences de types TS <-> Rust.

Corrections par ordre de risque:

1. RiskLevel missing Critical variant (SA-013 #13)
   - src-tauri/src/models/validation.rs: ajouter Critical au RiskLevel enum
   - Si TS envoie 'critical', Rust panique actuellement

2. AgentConfig serde(default) fields (SA-013 #1-4)
   - src/types/agent.ts: changer max_tool_iterations et enable_thinking
     de optional (?) a required (sans ?)
   - Rust les serialise TOUJOURS (serde default = deserialization only)

3. MessageCreate.tokens (SA-013 #6)
   - src/types/message.ts: ajouter tokens: number a MessageCreate

4. ProviderSettings.base_url (SA-013 #12)
   - src-tauri/src/models/ OU src/types/: aligner la convention
   - Option: retirer skip_serializing_if du cote Rust (prefere)
   - OU changer TS de string|null a string? (optional)

5. ChunkType orphans (SA-013 #14-15) - INVESTIGUER d'abord
   - Verifier si user_question_start/complete sont emis via
     un event Tauri separe (validation_required) ou via StreamChunk
   - Si separe: retirer les variantes orphelines de TS
   - Si prevu: ajouter au Rust

Verifier: npm run check && cargo clippy -- -D warnings
```

### P1-B: Performance - messages.clone() in LLM Loop (3h)

**Findings**: SA-008 PERF-1
**Files**: `agents/llm_agent.rs:1240`, `llm/manager.rs`

```
/Build_zileo

Contexte: Audit SA-008 PERF-1 - messages.clone() dans la boucle de tool execution.

Probleme: Dans execute_with_mcp() (llm_agent.rs), messages est clone a chaque
iteration de la boucle tool. Avec 30 iterations et 50KB de contexte moyen,
ca represente ~1.5MB d'allocations inutiles.

Fix: Passer les messages par reference dans la chaine d'appel.

1. llm/manager.rs - complete_with_tools()
   - Changer la signature: messages: Vec<Value> -> messages: &[Value]
   - Clone uniquement dans la closure de retry (max 3 retries)

2. agents/llm_agent.rs - execute_with_mcp()
   - Passer &messages au lieu de messages.clone()
   - Le clone dans la closure retry est acceptable (3 max vs N iterations)

3. Adapter les providers qui consomment messages
   - Si un provider a besoin d'ownership, cloner au point d'appel

Impact estime: reduction de O(iterations * message_size) a O(retries * message_size)
Soit ~50x moins d'allocations pour une conversation typique.

Verifier: cargo clippy -- -D warnings && cargo test
Tester manuellement: lancer un workflow avec tools pour verifier le comportement.
```

### P1-C: Frontend Robustness (3h)

**Findings**: SA-011 H-001, H-002, H-003
**Files**: `stores/activity.ts`, `stores/workflows.ts`, `routes/agent/+page.svelte`

```
/Fix_Zileo

Contexte: Audit SA-011 - 3 problemes HIGH dans le frontend.

1. H-003: Double-submit protection (agent/+page.svelte)
   - Ajouter un guard dans handleSend(): si deja en cours, return
   - Desactiver le bouton Send pendant l'invocation initiale
   - Reactiver sur erreur ou quand le streaming demarre

2. H-002: Retry sur echec loadWorkflows (workflowStore)
   - Si loadWorkflows() echoue au mount, afficher un etat erreur
   - Ajouter un bouton "Retry" qui relance le chargement
   - Optionnel: retry automatique avec backoff (1s, 2s, 4s, max 3)

3. H-001: Race condition capture activites (activityStore)
   - Ajouter un guard capturedForWorkflow: string | null
   - Si captureStreamingActivities() est appele 2x pour le meme workflow, ignorer
   - Reset le guard quand on change de workflow

Verifier: npm run lint && npm run check
```

### P1-D: Export/Validation Query Parameterization (3h)

**Findings**: SA-002 S2-H2, SA-001 C5, M3, SA-012 F6, F7
**Files**: `commands/import_export.rs`, `commands/streaming.rs`, `commands/validation.rs`, `db/queries.rs`

```
/Fix_Zileo

Contexte: Audit SA-001/SA-002/SA-012 - Bind parameters manquants (defense-in-depth).

1. commands/import_export.rs (SA-002 S2-H2)
   - 8 occurrences de format!("SELECT ... WHERE meta::id(id) = '{}'", id)
   - Fix: utiliser $id bind parameter pour les 8

2. commands/streaming.rs:472-479 (SA-001 C5)
   - model et provider_lower interpoles dans WHERE
   - Fix: $api_name et $provider bind params

3. commands/streaming.rs:535-549 (SA-001 M3)
   - model_id interpole dans UPDATE SET
   - Fix: $model_id bind param

4. commands/validation.rs:128,163 (SA-012 F6)
   - SELECT * retourne Thing-format IDs
   - Fix: SELECT meta::id(id) AS id, ... avec champs explicites

5. db/queries.rs:115 (SA-012 F7)
   - workflow_id interpole dans DELETE WHERE
   - Fix: $wf_id bind parameter

Verifier: cargo clippy -- -D warnings && cargo test
```

### P1-E: Parallel DB Writes in Streaming (2h)

**Findings**: SA-008 PERF-2
**Files**: `commands/streaming.rs:588-635`

```
/Build_zileo

Contexte: Audit SA-008 PERF-2 - Ecritures DB sequentielles dans streaming.rs.

Probleme: Les tool_executions et reasoning_steps sont persistes sequentiellement
(~5ms chacun). 10 tools + 5 steps = 75ms de latence serie.

Fix: Utiliser futures::join_all() pour paralleliser les ecritures.

Location: commands/streaming.rs:588-635

// AVANT:
for (idx, te) in tool_executions.iter().enumerate() {
    state.db.create(...).await?;
}

// APRES:
let tool_futures: Vec<_> = tool_executions.iter().enumerate()
    .map(|(idx, te)| {
        let db = state.db.clone();
        async move { db.create(...).await }
    })
    .collect();
let results = futures::future::join_all(tool_futures).await;
for result in results {
    result.map_err(|e| format!("Failed to persist tool execution: {}", e))?;
}

Faire pareil pour reasoning_steps juste apres.

Impact: ~75ms -> ~10ms (7x speedup).

Verifier: cargo clippy -- -D warnings && cargo test
```

### P1-G: Dependency Security Updates (1h)

**Findings**: SA-006 Priority 1
**Files**: `package.json`
**What**: @sveltejs/kit 2.49.1 has 2 HIGH CVEs (GHSA-j2f3-wq62-6q46 memory DoS, GHSA-j62c-4x62-9r35 DoS/SSRF). Svelte 5.49.1 has 3 MODERATE SSR XSS. Both mitigated by desktop/static-adapter context but should be updated for hygiene.

```
/Fix_Zileo

Contexte: Audit SA-006 - CVEs dans les dependances NPM.

1. package.json: Mettre a jour @sveltejs/kit
   - Changer "^2.49.1" -> "^2.52.2"
   - Corrige GHSA-j2f3-wq62-6q46 (memory DoS) et GHSA-j62c-4x62-9r35 (DoS/SSRF prerendering)

2. package.json: Mettre a jour svelte
   - Changer "5.49.1" -> "5.53.0"
   - Corrige 3 XSS SSR (GHSA-m56q-vw4c-c2cp, GHSA-f7gr-6p89-r883, GHSA-h7h7-mm68-gmrc)

3. Executer: npm install
4. Verifier: npm run lint && npm run check && npm run test
5. Tester manuellement: demarrer l'app avec npm run tauri:dev
```

### P1-H: CSP Google Fonts + Migration Guard (2h)

**Findings**: SA-005 H1, H3
**Files**: `tauri.conf.json`, `routes/+layout.svelte`, `commands/migration.rs`

```
/Fix_Zileo

Contexte: Audit SA-005 H1 + H3.

1. H1 - Google Fonts CSP gap (tauri.conf.json + routes/+layout.svelte)
   Option recommandee: Self-host les fonts (desktop app = pas de CDN necessaire)
   - Telecharger Signika (400,500,600,700) et JetBrains Mono de Google Fonts
   - Placer dans static/fonts/
   - Creer un @font-face CSS dans app.css
   - Supprimer le <link> Google Fonts de +layout.svelte
   - Pas de modification CSP necessaire

2. H3 - Migration guard (commands/migration.rs:77)
   - migrate_memory_schema() detruit tous les embeddings sans guard "already applied"
   - Ajouter une table migration_log ou un check avant de re-executer
   - Pattern:
     SELECT count() FROM migration_log WHERE name = 'memory_schema_v1' GROUP ALL
     Si > 0: return Ok("Already applied")
     Sinon: executer migration + CREATE migration_log:memory_schema_v1

Verifier: cargo clippy -- -D warnings && cargo test
```

### P1-F: Dead Code + Schema Cleanup (1h)

**Findings**: SA-007 dead code, SA-012 F1/F8/F9/F11/F12
**Files**: `commands/agent.rs`, `db/client.rs`, `db/schema.rs`, `tools/utils.rs`

```
/Fix_Zileo

Contexte: Audit SA-007 + SA-012 - Dead code et schema cleanup.

1. commands/agent.rs: load_agents_from_db() marque #[allow(dead_code)]
   - Supprimer la fonction (111 lignes) si vraiment inutilisee
   - Verifier avec find_referencing_symbols d'abord

2. db/client.rs:226-239: DBClient::update() marque #[allow(dead_code)]
   - Supprimer (utilise le SDK anti-pattern ERR_SURREAL_002)

3. db/schema.rs:41-47: Table agent_state jamais ecrite
   - Supprimer du schema ou documenter pourquoi preservee

4. db/client.rs:78-81: MCP migration utilise DEFINE FIELD sans OVERWRITE
   - Changer en DEFINE FIELD OVERWRITE (PAT_DB_003)

5. tools/utils.rs:131-199: QueryBuilder non-parametre (unsafe)
   - Supprimer si ParamQueryBuilder est utilise partout
   - Verifier les references d'abord

Verifier: cargo clippy -- -D warnings && cargo test
```

---

## P2 - Performance / Qualite

**Timeline**: Next sprint. ~25h total.

### P2-A: Agent System Deduplication (6h)

**Findings**: SA-008 DUP-1 to DUP-5
**Effort**: 6h across `agents/`, `llm/`

- DUP-1: Add `Report::failed()` constructor (-100 lines)
- DUP-2: Merge execute() into execute_with_mcp() (-60 lines)
- DUP-3: Unify Mistral/OpenAI adapters into `ChoicesBasedAdapter` (-100 lines)
- DUP-4: Extract `dispatch_with_retry()` in manager.rs (-180 lines)
- DUP-5: `From<ToolExecutionData>` impl (-40 lines)

### P2-B: import_export.rs Refactoring (4h)

**Findings**: SA-007 (4 oversized functions), SA-002 structural
**Effort**: 4h

- Extract `EntityHandler<T>` trait or macro for 4 entity types
- Target: 1291 -> ~700 lines
- Each entity (agent, mcp_server, model, prompt) has identical: preview, export, conflict, import logic

### P2-C: Error Handling Standardization (3h)

**Findings**: SA-009 F2, SA-010 (30 try/catch), SA-011 M-002/M-006
**Effort**: 3h across stores + components

- Replace all `e instanceof Error ? e.message : String(e)` with `getErrorMessage(e)`
- Remove all `console.error`/`console.warn` in production code
- Fix empty `catch {}` in AgentForm.svelte:251
- Consider `withAsyncAction()` utility for settings components

### P2-D: Streaming Function Decomposition (3h)

**Findings**: SA-007 (616-line function)
**File**: `commands/streaming.rs`
**Effort**: 3h

- Extract 8 responsibilities into helper functions
- Keep `execute_workflow_streaming` as orchestrator (~80 lines)

### P2-E: Defense-in-Depth (2h)

**Findings**: SA-002 S2-M2 to S2-M5
**Effort**: 2h

- S2-M2: Per-entity import limits (max 100 agents, 50 servers, etc.)
- S2-M3: URL scheme validation in MarkdownRenderer
- S2-M4: Default-exclude sensitive env vars from export
- S2-M5: Recursion depth limit in sanitize_for_surrealdb()

### P2-F: Type Alignment - Medium Items (3h)

**Findings**: SA-013 #5, #7, #9, #16, #17
**Effort**: 3h

- Workflow.model_id convention alignment
- MessageCreate.role: use enum in Rust
- CreateMemoryParams.metadata nullability
- TaskPriority bounded validation
- task_status/task_priority proper enums in streaming

### P2-H: Tauri Permissions Hardening (2h)

**Findings**: SA-005 M1, M2, M3, M4
**Files**: `capabilities/default.json`, `components/onboarding/steps/StepImport.svelte`

- M1: Scope opener plugin URLs - deny `file://` and `tel:` schemes
- M2: Replace `dialog:default` with `dialog:allow-save`, `dialog:allow-message`, `dialog:allow-confirm`
- M3: Define deny patterns for sensitive IPC commands (`clear_memories_by_type`, `migrate_*`, `read_import_file`)
- M4: Replace `window.open()` in StepImport.svelte with `openUrl()` from `@tauri-apps/plugin-opener`

### P2-I: Dependency Feature Bloat Reduction (2h)

**Findings**: SA-006 Priority 2
**Files**: `src-tauri/Cargo.toml`

- Change rig-core: `features = ["all"]` -> `features = ["derive"]` (removes lopdf, rayon)
- Change surrealdb: add `default-features = false`, keep `features = ["kv-rocksdb"]` (removes protocol-http, protocol-ws, rustls from surrealdb subtree)
- Test: `cargo clippy -- -D warnings && cargo test` + manual app test
- Impact: reduces transitive dependency count and attack surface

### P2-G: Component Quality (4h)

**Findings**: SA-011 M-001 to M-012
**Effort**: 4h

- M-001: Clipboard try/catch with toast
- M-005: Validation timeout (5min auto-reject)
- M-008: Replace setTimeout with tick() for focus
- M-010: Cleanup timer guard in backgroundWorkflows
- M-003/M-004: Extract large derivations to utilities
- M-012: Retry button for ToolDetailsPanel

---

## P3 - Qualite / Maintenabilite

**Timeline**: Ongoing maintenance. ~10h total.

### P3-A: Settings Template Extraction (4h)

**Findings**: SA-010
- ValidationSettings entity loop (-300 lines)
- Import/Export entity loops (-270 lines)
- CRUDSettingsLayout wrapper (-150 lines)

### P3-B: CSS Deduplication (2h)

**Findings**: SA-010
- 9 repeated CSS patterns across settings components
- Extract to shared CSS module or component-level design tokens

### P3-C: Accessibility Improvements (2h)

**Findings**: SA-010 (7 gaps), SA-011 (8 gaps)
- ARIA attributes for tabs, collapsibles, live regions
- aria-labels on icon-only buttons
- Focus management after modal close

### P3-D: Store Cleanup (2h)

**Findings**: SA-009 F4, F6, F7
- Replace subscribe/unsub hack with `get(store)` in userQuestion.ts
- Remove 5 deprecated symbols
- Consistent store naming

### P3-E: CSP Documentation + Minor Tweaks (30min)

**Findings**: SA-005 L1, L2, L3
- L1: Document `unsafe-inline` justification (required by Svelte 5)
- L2: Add `blob:` to CSP `default-src` if memory export download is needed
- L3: `img-src` currently falls back to secure `default-src 'self'` - no change unless image rendering needed

### P3-F: Major Dependency Version Planning (ongoing)

**Findings**: SA-006 Priority 3-4
- **surrealdb 3.0.0**: Major upgrade from 2.6.1. Plan migration when API stabilizes.
- **reqwest 0.13**: Major upgrade. Check rig-core/surrealdb alignment first.
- **rig-core 0.31.0**: Pre-1.0 minor bump, may have breaking changes. Test after feature bloat fix.
- **eslint 10.0.0**: Would resolve minimatch/ajv transitive vulns (dev-only, low priority).
- **Monitor**: GTK3 deprecation in Tauri, glib unsoundness (RUSTSEC-2024-0429)

---

## Implementation Order (Recommended)

```
Week 1: P0 (security critical)
  Day 1: P0-A (injection - prompt.rs, embedding.rs) + P0-F (filesystem read)
  Day 2: P0-B (import pipeline) - biggest, most complex
  Day 3: P0-D (schema TYPE) + P0-E (models.rs, task.rs)

Week 2: P1 (bugs + performance)
  Day 1: P1-A (type mismatches) + P1-G (npm security updates)
  Day 2: P1-C (frontend robustness) + P1-F (dead code) + P1-I (HTTPS warning)
  Day 3: P1-B (clone optimization) + P1-E (parallel writes)
  Day 4: P1-D (remaining parameterization) + P1-H (CSP fonts + migration guard)

Week 3-4: P2 (quality)
  P2-A through P2-I in order (includes permissions hardening + feature bloat)

Ongoing: P3 (maintenance)
  As time permits (includes CSP docs, major dep version planning)
```

## Metrics (Updated After Code Verification)

| Priority | Items | Est. Hours | Files Touched | Security Impact |
|----------|-------|------------|---------------|-----------------|
| **P0** | 5 groups | ~10h | 8 files | Eliminates all injection + import pipeline security + filesystem access |
| **P1** | 9 groups | ~19h | 19 files | Fixes type crashes, races, perf, CVE patches, CSP, HTTPS warnings |
| **P2** | 9 groups | ~29h | 30+ files | -700 lines duplication, permissions hardening, feature bloat |
| **P3** | 6 groups | ~11h | 20+ files | -720 lines templates, a11y, CSP docs, dep planning |
| **Total** | 29 groups | **~69h** | | |

### Changes from Code Verification (2026-02-19)

- **P0-C moved to P1-I**: HTTPS enforcement downgraded from P0 to P1 (desktop context: user-configured URLs)
- **SA-006 CVEs**: 7/7 NPM CVEs confirmed NOT APPLICABLE to desktop (update for hygiene only)
- **SA-013 #1-4**: Downgraded from CRITICAL to HIGH (type bug, not security vulnerability)
- **SA-013 #14-15**: Downgraded from CRITICAL to MEDIUM (orphan variants, no crash)
- **SA-002 S2-M1**: Upgraded from MEDIUM to HIGH (import files are real external attack surface)
- **Net effect on P0**: Reduced from ~12.5h to ~10h. Focused on real injection risks.
