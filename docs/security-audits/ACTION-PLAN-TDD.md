# Action Plan TDD - Security Audit Remediation

**Date**: 2026-02-19 (updated with code verification adjustments)
**Source**: ACTION-PLAN.md + EVALUATION-2026-02-19.md + analyse des risques de regression
**Principe**: RED (test qui echoue) -> GREEN (fix minimal) -> REFACTOR (cleanup)
**Objectif**: Chaque fix est protege par des tests AVANT implementation

> **Note**: Les severites ont ete ajustees apres verification du code reel (voir EVALUATION-2026-02-19.md).
> Principal changement: P0-C (HTTPS enforcement) demote en P1-I (desktop context: URLs configurees par l'utilisateur).
> Les vrais CRITICAL restants: SA-001 C1-C3 (injection) et SA-013 #13 (deserialization panic).

---

## Implementation Status (2026-02-20)

> **Branch**: `security/audit-remediation-tdd` | **Detailed report**: [REMEDIATION-STATUS.md](./REMEDIATION-STATUS.md)

| Phase | Item | Status | Tests |
|-------|------|--------|-------|
| **0-A** | Rust test_utils module | **DONE** | 310 lines, 9 seed helpers |
| **0-B** | Frontend test helpers | **NOT DONE** | No factories.ts created |
| **0-C** | Characterization tests | **PARTIAL** | Inline in command tests, no separate file |
| **1-A** | SurrealQL Injection prompt.rs + embedding.rs | **DONE** | 6 tests |
| **1-B** | Import Pipeline Security | **DONE** | Parameterized queries + sanitize + entity limit + path validation |
| ~~1-C~~ | ~~HTTPS Enforcement~~ | Moved to 2-I | |
| **1-D** | DB Schema TYPE object Fixes | **DONE** | 3 tests (MCP serde cycle + backward compat) |
| **1-E** | Filesystem Access | **DONE** | read_import_file removed, save path validated |
| **1-F** | models.rs + task.rs Injection | **DONE** | 3 tests |
| **2-A** | Type Mismatches TS/Rust | **PARTIAL** | RiskLevel::Critical DONE (1 test), enable_thinking DONE, MessageCreate.tokens DONE (3 tests), ProviderSettings.base_url #12 DONE (3 tests), max_tool_iterations #1-4 DONE (AgentConfigCreate required + AgentForm + Zod schema). #14-15 NOT DONE |
| **2-B** | Performance messages.clone() | **DONE** | Signatures changed to &[Value] |
| **2-C** | Frontend Robustness | **DONE** | H-003 double-submit DONE. H-001 race DONE (8 TS tests + backend cancellation token propagation through 7 Rust files). H-002 retry DONE (5 store tests + WorkflowList error state + retry button + i18n) |
| **2-D** | Export/Validation Parameterization | **DONE** | All bind params |
| **2-E** | Parallel DB Writes | **DONE** | futures::join_all |
| **2-F** | Dependency Updates NPM | **NOT DONE** | No package.json changes |
| **2-G** | Fonts self-hosted + Migration Guard | **DONE** | Fonts DONE (4 woff2 + @font-face). Migration guard DONE (migration_log table + guards + 7 tests) |
| **2-H** | Dead Code Cleanup + Validation Refactor | **DONE** | 6 items removed + validation_helper refactored (SA-012 F8: extracted pure function, unified event emission, 5 tests) |
| **2-I** | HTTPS Warning Custom Providers | **DONE** | 8 tests |
| **3-A** | Error Handling Standardization | **DONE** | 18+ components, 22 TS tests. SA-010 ERR-2 complete. SA-013 #16-20 complete: all 28 console.* removed from services (silent return), stores/i18n (silent fallback), agent page (toast notifications), settings pages (error state UI), components (UI error state) |
| **3-B** | Defense-in-Depth | **DONE** | URL scheme (11 tests), depth limit (2 tests), entity limit |
| **3-C** | Dependency Feature Bloat | **DONE** | rig-core features removed. SurrealDB pruned: `default-features = false, features = ["kv-rocksdb"]`. Removed protocol-http, protocol-ws, rustls. 902 tests pass. OPT-WF-5 outdated comment cleaned up. |
| **3-D** | Tauri Permissions Hardening | **PARTIAL** | SA-005 M1+M4 DONE (opener scope + StepImport openUrl). M2-M3 NOT DONE |
| **3-E** | Type Alignment Medium | **PARTIAL** | Workflow.model_id DONE. Task priority NOT DONE |

## Philosophie

### Pourquoi TDD ici?

| Sans TDD | Avec TDD |
|----------|----------|
| bind param mal type -> query retourne 0 resultats silencieusement | Test de caracterisation verifie que la recherche retourne des donnees |
| Schema migration casse la lecture -> sidebar vide | Test verifie la lecture avant ET apres migration |
| Feature bloat reduction casse un import -> build OK mais runtime KO | Test d'integration verifie un appel LLM reel |
| Permissions trop restrictives -> feature cassee | Pas testable en TDD (test manuel obligatoire, documente) |

### Ce que TDD ne couvre PAS (test manuel obligatoire)

- Rendu visuel des fonts self-hosted (P1-H)
- Permissions Tauri (P2-H) - necessite l'app complete
- CSP enforcement (P1-H) - necessite le webview Tauri
- Performance reelle (P1-B, P1-E) - benchmarks manuels

### Infrastructure existante

| Couche | Framework | Pattern | Isolation |
|--------|-----------|---------|-----------|
| Rust DB/Commands | `#[tokio::test]` + SurrealDB reel en tempdir | `setup_test_state()` | tempdir par test |
| Rust Agents/LLM | `#[tokio::test]` + mock agents | `OrchestratorTestAgent` | In-memory |
| Frontend Stores | Vitest + `vi.mock('@tauri-apps/api/core')` | `get(store)` assertions | `beforeEach` reset |
| E2E | Playwright | `page.locator()` | Dev server |

---

## Phase 0: Test Harness Foundation (2h)

> Avant tout fix, creer les utilitaires de test partages.

### 0-A: Rust test_utils module (1h)

**But**: Eliminer la duplication de `setup_test_state()` (copie dans 5 fichiers)
et fournir des helpers pour les tests de query.

```
RED:  Creer src-tauri/src/test_utils.rs avec:
      - setup_test_state() -> AppState (version unique)
      - seed_test_agent(db) -> String (retourne l'id)
      - seed_test_workflow(db, agent_id) -> String
      - seed_test_prompt(db) -> String
      - seed_test_memory(db) -> String
      - seed_test_mcp_call_log(db) -> String (avec params object, pour P0-D)
      - assert_query_returns_rows(db, query, expected_min: usize)
      - assert_query_returns_empty(db, query)

GREEN: Implementer chaque helper avec des donnees minimales valides.

REFACTOR: Remplacer les 5 copies existantes de setup_test_state()
          par use crate::test_utils::setup_test_state.
```

**Fichier**: `src-tauri/src/test_utils.rs`
**Ajouter dans** `src-tauri/src/lib.rs`: `#[cfg(test)] pub mod test_utils;`
**Verifier**: `cargo test` (tous les tests existants passent encore)

### 0-B: Frontend test helpers (30min)

**But**: Factories de mock partagees pour les types modifies par les fixes.

```
Creer src/lib/test-utils/factories.ts:
  - createMockMCPCallLog() -> MCPCallLog (avec params object ET string)
  - createMockValidationRequest() -> ValidationRequest
  - createMockProviderSettings() -> ProviderSettings (avec base_url null ET string)
  - createMockAgentConfig() -> AgentConfig (avec max_tool_iterations et enable_thinking)
```

**Verifier**: `npm run check`

### 0-C: Caracterisation des queries critiques (30min)

**But**: Capturer le comportement actuel des queries qui vont changer.
Ce sont des tests qui PASSENT maintenant et qui doivent continuer a passer apres les fixes.

```
Creer src-tauri/src/commands/characterization_tests.rs:

#[tokio::test]
async fn char_search_prompts_returns_results() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    let results = search_prompts(state.clone(), None, None).await;
    assert!(results.is_ok());
    assert!(!results.unwrap().is_empty());
}

#[tokio::test]
async fn char_import_memories_creates_records() {
    let state = setup_test_state().await;
    // import un JSON minimal valide
    // verifier que les records existent en DB
}

#[tokio::test]
async fn char_export_contains_all_entities() {
    let state = setup_test_state().await;
    seed_test_agent(&state.db).await;
    // exporter et verifier que le JSON contient l'agent
}

#[tokio::test]
async fn char_mcp_call_log_params_readable() {
    let state = setup_test_state().await;
    seed_test_mcp_call_log(&state.db).await;
    // lire les params et verifier qu'elles sont correctes
}
```

**Verifier**: `cargo test characterization` - tous PASSENT (c'est le but)

---

## Phase 1: P0 Security Critical - TDD (14h total: 6h tests + 8h fixes)

> Apres evaluation: 1-C (HTTPS) deplace en Phase 2 (P1-I). Phase 1 = 5 items au lieu de 6.

### 1-A: SurrealQL Injection - Direct User Input (P0-A)
**Findings**: SA-001 C1 (CRITICAL), C2 (CRITICAL), C4 (HIGH apres verification - inclus car meme fichier)
**Fichiers**: `commands/prompt.rs`, `commands/embedding.rs`
**Temps**: 1h tests + 1.5h fix

```
RED (tests qui echouent ou qui verifient le comportement):

// Test 1: La recherche fonctionne avec des caracteres normaux
#[tokio::test]
async fn test_search_prompts_with_valid_query() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await; // nom = "Test Prompt"
    let results = search_prompts(state, Some("Test".into()), None).await;
    assert!(results.unwrap().len() >= 1);
}

// Test 2: La recherche avec apostrophe ne casse PAS (injection test)
#[tokio::test]
async fn test_search_prompts_injection_safe() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    let results = search_prompts(
        state, Some("'; DELETE prompt WHERE '1'='1".into()), None
    ).await;
    assert!(results.is_ok()); // ne plante pas
    assert!(results.unwrap().is_empty()); // ne retourne rien (pas de match)
}

// Test 3: Apres une tentative d'injection, les donnees sont intactes
#[tokio::test]
async fn test_search_prompts_injection_preserves_data() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    // Tenter l'injection
    let _ = search_prompts(
        state.clone(), Some("'; DELETE prompt WHERE '1'='1".into()), None
    ).await;
    // Verifier que le prompt existe toujours
    let results = search_prompts(state, None, None).await;
    assert!(!results.unwrap().is_empty(), "Data should not be deleted");
}

// Test 4: La categorie fonctionne
#[tokio::test]
async fn test_search_prompts_with_category() {
    let state = setup_test_state().await;
    // seed prompt avec category = "coding"
    let results = search_prompts(state, None, Some("coding".into())).await;
    assert!(results.unwrap().len() >= 1);
}

// Test 5: regenerate_embeddings avec type_filter
#[tokio::test]
async fn test_regenerate_embeddings_type_filter_injection_safe() {
    let state = setup_test_state().await;
    seed_test_memory(&state.db).await;
    let result = regenerate_embeddings(
        state, Some("'; DROP TABLE memory; --".into())
    ).await;
    assert!(result.is_ok()); // ne plante pas
}

GREEN: Convertir format!() -> bind parameters dans prompt.rs et embedding.rs.
       Pattern: conditions.push("field CONTAINS $search"); + .bind(("search", val))

REFACTOR: Rien (changement minimal).
```

**Verifier**: `cargo test test_search_prompts && cargo clippy -- -D warnings`

### 1-B: Import Pipeline Security (P0-B)
**Findings**: SA-001 C3, SA-002 S2-H1/M1, SA-012 F4
**Fichiers**: `commands/embedding.rs:446`, `commands/import_export.rs`
**Temps**: 2h tests + 3h fix

```
RED:

// Test 1: Import d'un fichier JSON valide fonctionne
#[tokio::test]
async fn test_import_valid_agent_json() {
    let state = setup_test_state().await;
    let import_json = r#"{
        "version": "1.0",
        "agents": [{
            "id": "test-uuid-1234",
            "name": "Test Agent",
            "lifecycle": "permanent",
            "llm": { "provider": "mistral", "model": "large", "temperature": 0.7, "max_tokens": 1000 }
        }]
    }"#;
    let preview = validate_import(state.clone(), import_json.into()).await;
    assert!(preview.is_ok());
    let result = execute_import(state, import_json.into(), /* options */).await;
    assert!(result.is_ok());
}

// Test 2: Import avec injection dans lifecycle ne casse pas
#[tokio::test]
async fn test_import_injection_in_lifecycle() {
    let state = setup_test_state().await;
    let malicious = r#"{
        "version": "1.0",
        "agents": [{
            "id": "test-uuid-5678",
            "name": "Evil Agent",
            "lifecycle": "permanent'; DELETE agent WHERE '1'='1",
            "llm": { "provider": "mistral", "model": "large", "temperature": 0.7, "max_tokens": 1000 }
        }]
    }"#;
    let result = execute_import(state.clone(), malicious.into(), /* options */).await;
    // Soit ca echoue proprement, soit ca insere la valeur litterale
    // Mais ca ne doit PAS executer le DELETE
}

// Test 3: Import avec null bytes ne plante pas
#[tokio::test]
async fn test_import_null_bytes_sanitized() {
    let state = setup_test_state().await;
    let with_nulls = r#"{
        "version": "1.0",
        "agents": [{
            "id": "test-uuid-9012",
            "name": "Agent\u0000WithNulls",
            "lifecycle": "permanent",
            "llm": { "provider": "mistral", "model": "large", "temperature": 0.7, "max_tokens": 1000 }
        }]
    }"#;
    let result = execute_import(state, with_nulls.into(), /* options */).await;
    assert!(result.is_ok(), "Null bytes should be sanitized, not crash");
}

// Test 4: Import memories avec content injection
#[tokio::test]
async fn test_import_memories_injection_safe() {
    let state = setup_test_state().await;
    let content = "Normal text'; DELETE memory WHERE '1'='1; --";
    // Appeler import_memories avec ce content
    // Verifier que le content est stocke tel quel (echappé)
    // Verifier que les autres memories ne sont pas supprimees
}

// Test 5: Export round-trip (import -> export -> re-import)
#[tokio::test]
async fn test_import_export_roundtrip() {
    let state = setup_test_state().await;
    seed_test_agent(&state.db).await;
    let exported = generate_export_file(state.clone(), /* all entities */).await.unwrap();
    // Supprimer l'agent
    // Re-importer
    // Verifier que l'agent est restaure identique
}

GREEN: Convertir execute_import() vers CONTENT $data bind pattern.
       Ajouter sanitize_for_surrealdb() a l'entree de execute_import() et import_memories().

REFACTOR: Rien.
```

**Verifier**: `cargo test test_import && cargo clippy -- -D warnings`

### ~~1-C~~: HTTPS Enforcement -> **DEPLACE en 2-I (P1-I)**

> **Demote apres evaluation**: SA-002 S2-C1 et S2-H3 passes de CRITICAL/HIGH a MEDIUM.
> Raison: Desktop app - l'utilisateur configure lui-meme l'URL du provider.
> Ce n'est pas un vecteur d'attaque externe mais un choix utilisateur.
> Approche changee: warning UI au lieu de hard reject.
> Voir Phase 2 section 2-I ci-dessous.

### 1-D: DB Schema TYPE object Fixes (P0-D)
**Findings**: SA-012 F2, F3, F5
**Fichiers**: `db/schema.rs`, modeles Rust, code lecture/ecriture
**Temps**: 1.5h tests + 2h fix
**RISQUE**: TRES ELEVE - migration de donnees existantes

```
RED:

// --- TESTS DE CARACTERISATION (doivent passer AVANT le fix) ---

#[tokio::test]
async fn char_mcp_call_log_write_read_cycle() {
    let state = setup_test_state().await;
    // Ecrire un mcp_call_log avec params = {"key": "value"}
    // Relire et verifier que params est correct
    // NOTE: Ce test peut ECHOUER avec l'ancien schema (ERR_SURREAL_001)
    //       car les cles dynamiques sont droppees. C'est le bug qu'on veut fixer.
}

#[tokio::test]
async fn char_validation_request_details_preserved() {
    let state = setup_test_state().await;
    // Ecrire une validation_request avec details = {"operation": "tool_call", "tool": "memory"}
    // Relire et verifier les cles
}

// --- TESTS DE MIGRATION ---

#[tokio::test]
async fn test_schema_migration_preserves_existing_mcp_logs() {
    let state = setup_test_state().await;
    // 1. Inserer un mcp_call_log en format ANCIEN (object direct)
    let old_format_query = r#"
        CREATE mcp_call_log:`test-log` CONTENT {
            workflow_id: 'wf-1',
            server_name: 'test-server',
            tool_name: 'test-tool',
            params: { key: 'value', nested: { a: 1 } },
            result: [{ output: 'hello' }],
            duration_ms: 100,
            success: true,
            timestamp: time::now()
        }
    "#;
    state.db.execute(old_format_query).await.unwrap();

    // 2. Appliquer la migration
    // migrate_mcp_call_log_schema(&state.db).await.unwrap();

    // 3. Relire en nouveau format
    let logs = state.db.query_json(
        "SELECT meta::id(id) AS id, params, result FROM mcp_call_log"
    ).await.unwrap();

    // 4. Verifier: params doit etre une string JSON valide
    let log = &logs[0];
    let params_str = log.get("params").unwrap().as_str().unwrap();
    let params: serde_json::Value = serde_json::from_str(params_str).unwrap();
    assert_eq!(params["key"], "value");
    assert_eq!(params["nested"]["a"], 1);
}

#[tokio::test]
async fn test_new_mcp_call_log_uses_string_format() {
    let state = setup_test_state().await;
    // Apres migration, ecrire un nouveau log avec le code applicatif
    // Verifier que params est stocke en string
    // Verifier que la relecture fonctionne
}

// --- TEST DE BACKWARD COMPAT (PAT_SERDE_001) ---

#[tokio::test]
async fn test_deserialize_both_object_and_string_params() {
    // Tester que le deserializer custom gere les deux formats:
    // Format ancien: {"key": "value"} (object direct)
    // Format nouveau: "{\"key\": \"value\"}" (string JSON)
    let old_format = serde_json::json!({"key": "value"});
    let new_format = serde_json::json!("{\"key\": \"value\"}");
    // Les deux doivent deserializer vers la meme Value
}

GREEN:
  1. Changer schema.rs: TYPE object -> TYPE string pour les 3 champs
  2. Ajouter migration: convertir les donnees existantes (UPDATE ... SET params = <string>$params)
  3. Mettre a jour les modeles Rust avec serialize_as_json_string/deserialize_json_string
  4. Ajouter migration_log tracking (P1-H H3)

REFACTOR: Rien.
```

**IMPORTANT**: Faire un backup DB avant. La migration est irreversible.
**Verifier**: `cargo test test_schema_migration && cargo test test_new_mcp && cargo test char_mcp`

### 1-E: Filesystem Access (P0-F) **[KEPT AT P0 - trivial fix, high reward]**
**Findings**: SA-005 C1 (downgrade CRITICAL -> HIGH apres verification, mais fix trivial)
**Fichiers**: `commands/import_export.rs`
**Temps**: 15min test + 15min fix

```
RED:

// Verifier que read_import_file n'est appele nulle part dans le frontend
// -> Grep dans src/ pour 'read_import_file' (si 0 resultats, supprimer)

// Test: si on garde la commande, elle refuse les chemins hors allowlist
#[tokio::test]
async fn test_save_export_rejects_sensitive_paths() {
    let result = save_export_to_file(
        "/etc/shadow".into(), "malicious content".into()
    ).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_save_export_accepts_download_dir() {
    let tmp = tempdir().unwrap();
    let path = tmp.path().join("export.json");
    let result = save_export_to_file(
        path.to_str().unwrap().into(), "{}".into()
    ).await;
    assert!(result.is_ok());
}

GREEN: Supprimer read_import_file du generate_handler!
       Ajouter path validation a save_export_to_file.

REFACTOR: Rien.
```

### 1-F: models.rs + task.rs Injection (P0-E)
**Findings**: SA-001 H3-H9, M1
**Fichiers**: `commands/models.rs`, `commands/task.rs`
**Temps**: 1h tests + 1.5h fix

```
RED:

#[tokio::test]
async fn test_update_model_name_with_apostrophe() {
    let state = setup_test_state().await;
    let model_id = seed_test_model(&state.db).await;
    let result = update_model(
        state, model_id, Some("Model's Test Name".into()), None, None
    ).await;
    assert!(result.is_ok());
    // Relire et verifier que le nom est correct
}

#[tokio::test]
async fn test_update_model_injection_safe() {
    let state = setup_test_state().await;
    let model_id = seed_test_model(&state.db).await;
    let _ = update_model(
        state.clone(), model_id.clone(),
        Some("'; DELETE llm_model WHERE '1'='1".into()), None, None
    ).await;
    // Verifier que TOUS les models existent encore
    let count = count_models(&state.db).await;
    assert!(count >= 1, "Injection should not delete models");
}

#[tokio::test]
async fn test_update_task_name_with_special_chars() {
    let state = setup_test_state().await;
    let wf_id = seed_test_workflow(&state.db, "agent-1").await;
    let task_id = seed_test_task(&state.db, &wf_id).await;
    let result = update_task(
        state, task_id, Some("Task with 'quotes' and \"doubles\"".into()),
        None, None
    ).await;
    assert!(result.is_ok());
}

GREEN: Convertir SET avec format!() -> bind parameters.

REFACTOR: Rien.
```

---

## Phase 2: P1 High Impact - TDD (18h total: 7h tests + 11h fixes)

> Apres evaluation: inclut maintenant 2-I (HTTPS warning, ex-P0-C) en plus des items originaux.

### 2-A: Type Mismatches TS <-> Rust (P1-A)
**Findings**: SA-013 #13 (CRITICAL - deserialization panic), #1-4 (HIGH - type bug), #6 (HIGH), #12 (HIGH), #14-15 (MEDIUM - orphan variants)
**Temps**: 1h tests + 2h fix

> Apres evaluation: #13 est le seul CRITICAL restant de SA-013.
> #1-4 downgrade a HIGH (type bug, pas securite). #14-15 downgrade a MEDIUM (orphan variants, pas de crash).
> Prioriser #13 en premier (crash app), puis #1-4 et #6/#12 (bugs reels).
> #14-15 peut etre fait en P2 si le temps manque.

```
RED (Rust):

// PRIORITE 1: Fix le crash (#13)
#[test]
fn test_risk_level_deserializes_critical() {
    let json = "\"critical\"";
    let level: Result<RiskLevel, _> = serde_json::from_str(json);
    assert!(level.is_ok(), "RiskLevel should handle 'critical' variant");
    // CE TEST ECHOUE ACTUELLEMENT -> c'est le bug -> crash app
}

// PRIORITE 2: Confirmer le comportement serde(default) (#1-4)
#[test]
fn test_agent_config_always_serializes_max_tool_iterations() {
    let config = AgentConfig {
        max_tool_iterations: 50, // serde(default) value
        enable_thinking: false,
        // ... autres champs
    };
    let json = serde_json::to_string(&config).unwrap();
    assert!(json.contains("max_tool_iterations"), "Field must always be present");
    assert!(json.contains("enable_thinking"), "Field must always be present");
}

RED (TypeScript - vitest):

describe('Type contracts', () => {
    it('AgentConfig.max_tool_iterations is required (not optional)', () => {
        const config: AgentConfig = {
            id: '1', name: 'test', lifecycle: 'permanent',
            max_tool_iterations: 50,  // doit compiler sans '?'
            enable_thinking: false,   // doit compiler sans '?'
            // ...
        };
        expect(config.max_tool_iterations).toBe(50);
    });
});

GREEN:
  Rust: Ajouter Critical a RiskLevel enum (PRIORITE 1 - empeche crash).
  TS: Changer max_tool_iterations?: number -> max_tool_iterations: number
      Changer enable_thinking?: boolean -> enable_thinking: boolean
      Ajouter tokens: number a MessageCreate
  #14-15 (orphan ChunkType variants): Investiguer puis nettoyer.
         Si user_question est via event separe, retirer les orphelins TS.

REFACTOR: Rien.
```

**Verifier**: `cargo test test_risk_level && npm run check`

### 2-B: Performance - messages.clone() (P1-B)
**Findings**: SA-008 PERF-1
**Fichiers**: `agents/llm_agent.rs`, `llm/manager.rs`
**Temps**: 1h tests + 2h fix

```
RED:

// Test fonctionnel: le tool loop fonctionne toujours apres le refactoring
#[tokio::test]
async fn test_complete_with_tools_returns_response() {
    // Utiliser un mock provider qui retourne une reponse fixe
    // Verifier que le resultat est correct
    // Ce test verifie que le changement de signature ne casse pas le flow
}

// Test fonctionnel: retry fonctionne toujours
#[tokio::test]
async fn test_complete_with_tools_retries_on_transient_error() {
    // Mock provider qui echoue 1 fois puis reussit
    // Verifier que le retry fonctionne avec la nouvelle signature &[Value]
}

// Test: l'agent execute() produit un Report valide
#[tokio::test]
async fn test_llm_agent_execute_with_mcp_produces_report() {
    // Test d'integration avec mock provider
    // Verifier Report { status: Completed, content: non-vide }
}

GREEN: Changer complete_with_tools() pour accepter &[Value].
       Clone uniquement dans la closure retry.

REFACTOR: Adapter tous les call sites.
```

**NOTE**: Ce test ne mesure PAS la performance, seulement la non-regression fonctionnelle.
Pour la perf, benchmark manuel avec `std::time::Instant` en debug.

### 2-C: Frontend Robustness (P1-C)
**Findings**: SA-011 H-001, H-002, H-003
**Fichiers**: stores + page
**Temps**: 1h tests + 2h fix

```
RED (Vitest):

describe('Double-submit protection', () => {
    it('should prevent concurrent sends', async () => {
        // Mock invoke qui prend 100ms
        mockInvoke.mockImplementation(() => new Promise(r => setTimeout(r, 100)));
        // Appeler handleSend() deux fois rapidement
        // Verifier que invoke n'est appele qu'une fois
    });
});

describe('Workflow load retry', () => {
    it('should expose error state when loadWorkflows fails', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('DB unavailable'));
        await workflowStore.loadWorkflows();
        expect(get(workflowsError)).toBeTruthy();
    });

    it('should recover on retry after failure', async () => {
        mockInvoke.mockRejectedValueOnce(new Error('DB unavailable'));
        await workflowStore.loadWorkflows();
        mockInvoke.mockResolvedValueOnce([/* workflow list */]);
        await workflowStore.loadWorkflows(); // retry
        expect(get(workflowsError)).toBeNull();
        expect(get(workflows)).toHaveLength(1);
    });
});

describe('Activity capture guard', () => {
    it('should not duplicate capture for same workflow', () => {
        activityStore.captureStreamingActivities('wf-1');
        activityStore.captureStreamingActivities('wf-1'); // doublon
        // Verifier que les activites ne sont pas dupliquees
    });
});

GREEN: Implementer les guards et retries.

REFACTOR: Rien.
```

### 2-D: Export/Validation Query Parameterization (P1-D)
**Findings**: SA-002 S2-H2, SA-001 C5 (MEDIUM apres verification), M3, SA-012 F6, F7
**Temps**: 1h tests + 2h fix

```
RED:

// Test: l'export fonctionne toujours apres parametrisation
#[tokio::test]
async fn test_export_with_valid_ids() {
    let state = setup_test_state().await;
    let agent_id = seed_test_agent(&state.db).await;
    let result = generate_export_file(state, vec![agent_id], vec![], vec![], vec![]).await;
    assert!(result.is_ok());
    let json: serde_json::Value = serde_json::from_str(&result.unwrap()).unwrap();
    assert!(!json["agents"].as_array().unwrap().is_empty());
}

// Test: le cascade delete fonctionne avec bind params
#[tokio::test]
async fn test_cascade_delete_workflow() {
    let state = setup_test_state().await;
    let agent_id = seed_test_agent(&state.db).await;
    let wf_id = seed_test_workflow(&state.db, &agent_id).await;
    seed_test_message(&state.db, &wf_id).await;
    seed_test_task(&state.db, &wf_id).await;
    let result = delete_workflow(state, wf_id).await;
    assert!(result.is_ok());
    // Verifier que messages et tasks sont aussi supprimes
}

// Test: validation_request retourne des IDs propres (pas Thing format)
#[tokio::test]
async fn test_validation_request_returns_clean_ids() {
    let state = setup_test_state().await;
    // Creer une validation_request
    let requests = get_pending_validations(state).await.unwrap();
    for req in &requests {
        assert!(!req.id.contains(':'), "ID should be clean UUID, not Thing format");
    }
}

GREEN: Convertir les 8 format!() d'export + cascade delete + validation en bind params.

REFACTOR: Rien.
```

### 2-E: Parallel DB Writes (P1-E)
**Findings**: SA-008 PERF-2
**Fichiers**: `commands/streaming.rs:588-635`
**Temps**: 30min tests + 1h fix

```
RED:

// Test: les tool_executions sont toutes persistees
#[tokio::test]
async fn test_persist_tool_executions_all_saved() {
    let state = setup_test_state().await;
    let wf_id = seed_test_workflow(&state.db, "agent-1").await;
    let executions = vec![
        create_test_tool_execution("tool-1"),
        create_test_tool_execution("tool-2"),
        create_test_tool_execution("tool-3"),
    ];
    persist_tool_executions(&state.db, &wf_id, &executions).await.unwrap();

    // Verifier que les 3 sont en DB
    let count = state.db.query_json(
        &format!("SELECT count() FROM tool_execution WHERE workflow_id = '{}' GROUP ALL", wf_id)
    ).await.unwrap();
    assert_eq!(extract_count(&count), 3);
}

// Test: les thinking_steps sont tous persistes
#[tokio::test]
async fn test_persist_thinking_steps_all_saved() {
    // Meme pattern avec 5 thinking steps
}

GREEN: Remplacer la boucle for sequentielle par futures::join_all().

REFACTOR: Extraire persist_tool_executions() et persist_thinking_steps()
          comme fonctions separees (decomposition de streaming.rs).
```

### 2-F: Dependency Updates (P1-G)
**Findings**: SA-006 Priority 1
**Fichiers**: `package.json`
**Temps**: 30min

```
PAS DE TDD: C'est un bump de version. Le test est:
1. npm install
2. npm run lint
3. npm run check
4. npm run test
5. Test manuel: demarrer l'app

Si tout passe, c'est bon. Sinon, investiguer les breaking changes.
```

### 2-G: CSP Google Fonts + Migration Guard (P1-H)
**Findings**: SA-005 H1, H3
**Fichiers**: `tauri.conf.json`, layout, migration.rs
**Temps**: 1h tests + 1.5h fix

```
RED (migration guard):

#[tokio::test]
async fn test_migration_idempotent() {
    let state = setup_test_state().await;
    seed_test_memory_with_embedding(&state.db).await;

    // Premiere migration: OK
    let result1 = migrate_memory_schema(state.clone()).await;
    assert!(result1.is_ok());

    // Deuxieme migration: ne detruit PAS les embeddings
    seed_test_memory_with_embedding(&state.db).await;
    let result2 = migrate_memory_schema(state.clone()).await;
    assert!(result2.is_ok());
    assert!(result2.unwrap().contains("Already applied"));

    // Verifier que les embeddings sont intacts
    let memories = state.db.query_json(
        "SELECT embedding FROM memory WHERE embedding IS NOT NONE"
    ).await.unwrap();
    assert!(!memories.is_empty(), "Embeddings should survive second migration");
}

PAS DE TDD (fonts): Test visuel seulement.
  1. Telecharger les fonts
  2. Creer @font-face dans app.css
  3. Supprimer le lien Google Fonts
  4. Lancer l'app et comparer visuellement

GREEN: Ajouter migration_log check + guard.

REFACTOR: Rien.
```

### 2-H: Dead Code Cleanup (P1-F)
**Findings**: SA-007, SA-012 F1/F8/F9/F11/F12
**Temps**: 30min

```
PAS DE TDD: C'est de la suppression.
  1. Verifier les references avec find_referencing_symbols
  2. Supprimer
  3. cargo clippy -- -D warnings && cargo test
  Si ca compile et les tests passent, c'est bon.
```

### 2-I: HTTPS Warning for Custom Providers (ex-P0-C, demote a P1-I)
**Findings**: SA-002 S2-C1 (MEDIUM apres verification), S2-H3 (MEDIUM apres verification)
**Fichiers**: `commands/custom_provider.rs`, `mcp/http_handle.rs`
**Temps**: 30min tests + 30min fix

> **Changement d'approche apres evaluation**: UI warning au lieu de hard reject.
> L'utilisateur desktop configure ses propres URLs. Bloquer HTTP casserait les
> setups de dev local (Ollama, LM Studio, etc.). On avertit, on ne bloque pas.
> Exception: localhost/127.0.0.1 n'affiche meme pas de warning.

```
RED:

#[tokio::test]
async fn test_create_provider_http_returns_warning() {
    let state = setup_test_state().await;
    let result = create_custom_provider(
        state, "test".into(), "http://remote-api.com/v1".into(), None
    ).await;
    // Doit REUSSIR (pas de rejet) mais inclure un warning
    assert!(result.is_ok());
    let response = result.unwrap();
    assert!(response.warning.is_some());
    assert!(response.warning.unwrap().contains("HTTPS"));
}

#[tokio::test]
async fn test_create_provider_https_no_warning() {
    let state = setup_test_state().await;
    let result = create_custom_provider(
        state, "test".into(), "https://api.example.com/v1".into(), None
    ).await;
    assert!(result.is_ok());
    assert!(result.unwrap().warning.is_none());
}

#[tokio::test]
async fn test_create_provider_localhost_http_no_warning() {
    let state = setup_test_state().await;
    let result = create_custom_provider(
        state, "test-local".into(), "http://localhost:8080/v1".into(), None
    ).await;
    assert!(result.is_ok());
    assert!(result.unwrap().warning.is_none()); // localhost = pas de warning
}

GREEN: Ajouter un champ warning: Option<String> au retour de create/update_custom_provider().
       Emettre warning si HTTP non-localhost. Log tracing::warn() cote Rust.
       Frontend: afficher le warning dans un toast ou banner.

REFACTOR: Rien.
```

**Verifier**: `cargo test test_create_provider && cargo clippy -- -D warnings`

---

## Phase 3: P2 Quality - TDD (selectif) (10h total: 3h tests + 7h fixes)

> NOTE: Seuls les P2 avec un bon ratio risque/benefice sont inclus.
> P2-A (agent dedup) et P2-D (streaming decomp) sont REPORTES car trop risques sans plus de tests.

### 3-A: Error Handling Standardization (P2-C)
**Temps**: 30min tests + 2h fix

```
RED (Vitest):

describe('getErrorMessage consistency', () => {
    it('handles Error instances', () => {
        expect(getErrorMessage(new Error('test'))).toBe('test');
    });
    it('handles string errors', () => {
        expect(getErrorMessage('raw string')).toBe('raw string');
    });
    it('handles unknown errors', () => {
        expect(getErrorMessage({ code: 42 })).toMatch(/42/);
    });
    it('handles null/undefined', () => {
        expect(getErrorMessage(null)).toBeTruthy();
        expect(getErrorMessage(undefined)).toBeTruthy();
    });
});

GREEN: Remplacer les 30 patterns manuels par getErrorMessage().
       Supprimer les console.error/warn.
       Corriger le catch vide de AgentForm.svelte:251.

REFACTOR: Envisager withAsyncAction() utility si le pattern est stable.
```

### 3-B: Defense-in-Depth (P2-E)
**Temps**: 1h tests + 1.5h fix

```
RED:

// URL scheme validation
// (Vitest)
describe('MarkdownRenderer URL validation', () => {
    it('allows https URLs', () => {
        expect(isAllowedScheme('https://example.com')).toBe(true);
    });
    it('allows http URLs', () => {
        expect(isAllowedScheme('http://example.com')).toBe(true);
    });
    it('blocks javascript: URLs', () => {
        expect(isAllowedScheme('javascript:alert(1)')).toBe(false);
    });
    it('blocks data: URLs', () => {
        expect(isAllowedScheme('data:text/html,...')).toBe(false);
    });
});

// Recursion depth limit
#[test]
fn test_sanitize_deeply_nested_json() {
    // Creer un JSON avec 200 niveaux de nesting
    let mut value = serde_json::json!("leaf");
    for _ in 0..200 {
        value = serde_json::json!({"nested": value});
    }
    let result = sanitize_for_surrealdb(value);
    // Ne doit PAS stack overflow
    // Les niveaux > max doivent etre tronques a Null
}

// Import entity limits
#[tokio::test]
async fn test_import_rejects_excessive_entities() {
    let state = setup_test_state().await;
    // Creer un JSON avec 200 agents
    let result = validate_import(state, huge_json.into()).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("limit"));
}

GREEN: Implementer les 4 defenses (URL scheme, depth limit, entity limits, env var default-exclude).

REFACTOR: Rien.
```

### 3-C: Dependency Feature Bloat (P2-I)
**Temps**: 30min test + 1h fix

```
PAS DE TDD CLASSIQUE, mais:

TEST 1 (build):
  Modifier Cargo.toml: rig-core features = ["derive"]
  cargo build -> doit compiler

TEST 2 (integration manuelle):
  cargo test -> tous les tests passent
  Lancer l'app -> configurer un provider -> envoyer un message -> reponse recue

TEST 3 (build):
  Modifier Cargo.toml: surrealdb default-features = false, features = ["kv-rocksdb"]
  cargo build -> doit compiler

TEST 4 (integration manuelle):
  cargo test -> tous les tests passent
  Lancer l'app -> creer un workflow -> les donnees persistent

SI UN TEST ECHOUE: Revert et documenter quel feature est necessaire.
```

### 3-D: Tauri Permissions Hardening (P2-H)
**Temps**: 1h fix (pas de TDD possible)

```
PAS DE TDD: Les permissions Tauri ne sont testables qu'avec l'app complete.

PROCEDURE:
  1. Modifier capabilities/default.json
  2. Lancer l'app
  3. Tester manuellement:
     - Export fonctionne (dialog:allow-save)
     - Liens markdown s'ouvrent (opener scoped)
     - Import fonctionne
     - Onboarding fonctionne
  4. Si une feature casse: ajouter la permission manquante
```

### 3-E: Type Alignment Medium Items (P2-F)
**Temps**: 30min tests + 1.5h fix

```
RED:

#[test]
fn test_task_priority_bounded() {
    // Priority 0 devrait echouer
    let result = validate_task_priority(0);
    assert!(result.is_err());
    // Priority 6 devrait echouer
    let result = validate_task_priority(6);
    assert!(result.is_err());
    // Priority 1-5 OK
    for p in 1..=5 {
        assert!(validate_task_priority(p).is_ok());
    }
}

// Vitest
describe('Workflow.model_id convention', () => {
    it('handles null model_id from backend', () => {
        const workflow: Workflow = {
            // ... avec model_id: null
        };
        expect(workflow.model_id).toBeNull(); // pas undefined
    });
});

GREEN: Ajouter validation, aligner les conventions.

REFACTOR: Rien.
```

---

## Phase 4: P2 Refactoring - REPORTE

> Ces items sont reportes tant que la couverture tests n'est pas suffisante.
> Les phases 1-3 ajoutent ~60 tests Rust et ~20 tests TS, ce qui ameliore
> significativement la couverture. Une fois stabilises, on peut envisager:

| Item | Prerequis | Pourquoi reporter |
|------|-----------|-------------------|
| P2-A: Agent system dedup (-480 lignes) | Tests agents/llm a >50% | Touche le code path critique LLM |
| P2-B: import_export.rs refactoring | Tests import/export a >80% | Le plus gros fichier, le plus duplique |
| P2-D: streaming.rs decomposition | Tests streaming complets | 616 lignes qui fonctionnent |
| P2-G: Component quality (SA-011 M-001 to M-012) | Phases 1-2 stabilisees | UX/qualite, pas securite (4h) |
| P3 entier | Phases 1-3 stabilisees | Benefice marginal vs risque |

---

## Ordre d'Execution Recommande

> Mis a jour apres evaluation: 1-C (HTTPS) deplace en Semaine 2.
> Semaine 1 gagne du temps pour les vrais CRITICAL.

```
Semaine 1: Foundation + P0 (18h)
  Jour 1 (4h):
    Phase 0: test_utils + helpers + caracterisation     [2h]
    1-E: Filesystem access (tests + fix)                [0.5h]
    1-A: SurrealQL injection prompt.rs + embedding.rs   [1.5h] -- DEBUT

  Jour 2 (4h):
    1-A: fin                                            [1h]
    1-F: models.rs + task.rs bind params                [3h]

  Jour 3 (4h):
    1-B: Import pipeline security (tests)               [2h]
    1-B: Import pipeline security (fix)                 [2h]   -- DEBUT

  Jour 4 (4h):
    1-B: Import pipeline security (fin fix)             [1h]
    1-D: Schema TYPE migration (tests)                  [1.5h]
    --- BACKUP DB ---
    1-D: Schema TYPE migration (fix debut)              [1.5h]

  Jour 5 (4h):
    1-D: Schema TYPE migration (fin + verification)     [1h]
    Validation globale Phase 1:                         [2h]
      cargo test (TOUT)
      npm run check && npm run test
      Test manuel: recherche, import, export, creation agent/modele

Semaine 2: P1 + P2 selectif (18h)
  Jour 1 (4h):
    2-A: Type mismatches TS/Rust (tests + fix)          [3h]
      -> #13 RiskLevel en premier (CRITICAL: crash app)
    2-F: Dependency updates npm                         [1h]

  Jour 2 (4h):
    2-C: Frontend robustness (tests + fix)              [3h]
    2-H: Dead code cleanup                              [0.5h]
    2-I: HTTPS warning (ex-P0-C)                        [0.5h] -- trivial apres demotion

  Jour 3 (4h):
    2-D: Export/Validation parameterization             [3h]
    2-E: Parallel DB writes                             [1.5h] -- DEBUT

  Jour 4 (4h):
    2-E: fin                                            [0.5h]
    2-G: Migration guard + fonts                        [2.5h]
    3-A: Error handling standardization                 [2h]   -- DEBUT

  Jour 5 (4h):
    3-A: fin                                            [0.5h]
    3-B: Defense-in-depth                               [2.5h]
    3-C: Dependency feature bloat                       [1.5h] -- SI build OK

Semaine 3: Finalisation (4h)
  Jour 1 (4h):
    3-D: Tauri permissions (test manuel)                [1h]
    3-E: Type alignment medium                          [2h]
    Validation finale:                                  [2h]
      cargo test (TOUT)
      cargo clippy -- -D warnings
      npm run lint && npm run check && npm run test
      Test manuel complet: tous les flows utilisateur
```

---

## Metriques attendues

| Metrique | Avant | Apres Phase 1 | Apres Phase 3 |
|----------|-------|---------------|---------------|
| Tests Rust | ~52 | ~85 (+33) | ~105 (+53) |
| Tests TS | ~11 | ~15 (+4) | ~25 (+14) |
| Couverture commands/ | ~5% | ~35% | ~45% |
| Couverture agents/llm | ~29% | ~35% | ~35% |
| CRITICAL findings | 4 | 0 | 0 |
| Injection vectors | 35 | 0 | 0 |
| ERR_SURREAL violations | 3 | 0 | 0 |
| Temps total | - | 16h | 44h |

## Tests qui ne peuvent PAS etre automatises

| Item | Raison | Procedure manuelle |
|------|--------|--------------------|
| Fonts self-hosted | Rendu visuel | Comparer screenshots avant/apres |
| Tauri permissions | Necessite webview reel | Tester chaque feature dans l'app |
| CSP enforcement | Necessite Tauri runtime | Verifier dans DevTools |
| Performance LLM | Necessite provider reel | Mesurer manuellement |
| Import fichier reel | Taille + complexite | Tester avec un backup reel |

---

## Regles TDD pour ce projet

1. **RED obligatoire**: Ne jamais commencer un fix sans au moins 1 test qui echoue ou 1 test de caracterisation
2. **Caracterisation d'abord**: Pour les queries, ecrire un test qui verifie le comportement actuel AVANT de toucher au code
3. **Un commit par cycle**: `test: add injection tests for prompt.rs` puis `fix: use bind params in search_prompts`
4. **Pas de refactoring sans filet**: Les P2 refactoring ne commencent que quand la couverture du fichier depasse 50%
5. **Backup avant migration**: Phase 1-D (schema) et 3-C (features) necessitent un backup/revert possible
6. **Test manuel = test documente**: Pour chaque item non-automatisable, ecrire la procedure exacte dans ce document
