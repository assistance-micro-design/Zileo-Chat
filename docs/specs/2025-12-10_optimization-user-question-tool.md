# Plan d'Optimisation - UserQuestionTool

## Metadata
- **Date**: 2025-12-10
- **Domaine**: tools/UserQuestionTool
- **Stack**: Rust 1.91.1 + Tauri 2.9.3 + tokio 1.48.0 | SvelteKit 2.49.1 + Svelte 5.45.6 | SurrealDB 2.4.0
- **Impact estime**: Securite, Performance, Maintenabilite

## Resume Executif

Ce plan d'optimisation vise a ameliorer le UserQuestionTool (human-in-the-loop pour agents LLM) sans ajouter de nouvelles fonctionnalites. Les principaux axes sont: (1) combler les validations manquantes (MAX_TEXT_RESPONSE_LENGTH), (2) ajouter un timeout configurable au polling infini actuel, (3) ameliorer la maintenabilite via refactoring des fonctions longues, et (4) couvrir le composant avec des tests unitaires et d'integration.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src-tauri/src/tools/user_question/tool.rs` | 447 | Haute | `ask_question()` 137 lignes, polling infini |
| `src-tauri/src/commands/user_question.rs` | 224 | Haute | `submit_user_response()` 107 lignes |
| `src/lib/stores/userQuestion.ts` | 277 | Moyenne | console.log en prod, queue illimitee |
| `src/types/user-question.ts` | 41 | Basse | Types bien synchronises |
| `src-tauri/src/models/user_question.rs` | 105 | Basse | Modeles corrects |

### Patterns Identifies

- **Pattern Tool Framework**: Implemente trait `Tool` avec `async_trait` (tool.rs:337-417)
- **Pattern Polling Progressif**: Intervalles 500ms -> 5s, SANS timeout global (tool.rs:249-334)
- **Pattern Event Streaming**: Emit `workflow_stream` avec `user_question_start/complete` (tool.rs:202-239)
- **Pattern Store Event-Driven**: Listeners Tauri avec cleanup obligatoire (userQuestion.ts:82-241)
- **Pattern Validation Multi-niveaux**: `validate_not_empty()`, `validate_length()` (tool.rs:107-153)

### Metriques Actuelles

- **Couverture tests**: 0% (aucun test specifique)
- **Complexite cyclomatique**: ~8 (ask_question), ~7 (submit_user_response)
- **DB queries par question**: 2 (CREATE + verify)
- **DB queries par poll**: 1 (SELECT status)
- **Events emis**: 2 par question (start + complete)

## Best Practices (2024-2025)

### Sources Consultees

- [rs-graph-llm - High-performance multi-agent workflow](https://github.com/a-agmon/rs-graph-llm) - Pattern workflow pausing
- [Tauri Events Official Docs](https://v2.tauri.app/develop/calling-frontend/) - Event cleanup obligatoire
- [Tokio Async Patterns](https://tokio.rs/tokio/tutorial/async) - `tokio::select!` avec timeout
- [HULA Framework - Atlassian ICSE 2025](https://www.atlassian.com/blog/atlassian-engineering/hula-blog-autodev-paper-human-in-the-loop-software-development-agents) - Human-in-the-loop metrics
- [Svelte 5 Runes for Real-time](https://dev.to/polliog/real-world-svelte-5-handling-high-frequency-real-time-data-with-runes-3i2f) - $state vs writable

### Patterns Recommandes

1. **tokio::select! avec timeout**: Remplace polling infini par attente avec timeout configurable
2. **oneshot::channel**: Alternative zero-polling (differe car refactor majeur)
3. **Circuit Breaker**: Ouvrir circuit apres N timeouts consecutifs
4. **Semaphore**: Limiter questions concurrentes (max 3 recommande)
5. **Queue limit frontend**: Eviter memory leak sur pending illimite

### Anti-Patterns a Eviter

1. **Polling DB infini sans timeout** (actuel) - Risque agent bloque indefiniment
2. **unwrap_or_default() silencieux** (actuel) - Masque erreurs deserialisation
3. **console.log en production** (actuel) - Utiliser logger unifie
4. **Event listeners non nettoyes** - Memory leaks SPA (actuel: OK avec cleanup)

## Contraintes du Projet

- **Decision architecturale**: Store pattern Event-Driven avec cleanup (ARCHITECTURE_DECISIONS.md Q20)
- **Validation obligatoire**: `validate_not_empty()`, `validate_length()` de `tools/utils.rs`
- **Queries parametrees**: `execute_with_params()`, `query_json_with_params()` pour user input
- **Constants centralisees**: `tools/constants.rs` module `user_question`
- **MSRV**: 1.80.1 (> 1.75, permet native async traits)

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-UQ-1: Valider MAX_TEXT_RESPONSE_LENGTH

- **Fichiers**: `src-tauri/src/commands/user_question.rs:58-59`
- **Changement**: Ajouter validation `text_response` avant UPDATE
- **Benefice**: Securite - empeche injection texte > 10KB
- **Risque regression**: Tres faible
- **Validation**: Test unitaire avec text > 10000 chars

```rust
// Ajouter avant params.push() ligne 59
if let Some(ref text) = text_response {
    if text.len() > crate::tools::constants::user_question::MAX_TEXT_RESPONSE_LENGTH {
        return Err(format!(
            "Text response too long: {} chars (max {})",
            text.len(),
            crate::tools::constants::user_question::MAX_TEXT_RESPONSE_LENGTH
        ));
    }
}
```

#### OPT-UQ-2: Valider option.id length

- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:139-147`
- **Changement**: Ajouter constante `MAX_OPTION_ID_LENGTH` et validation
- **Benefice**: Securite - IDs bornes
- **Risque regression**: Tres faible

```rust
// Dans constants.rs, ajouter:
pub const MAX_OPTION_ID_LENGTH: usize = 64;

// Dans tool.rs, ajouter apres validate_not_empty(&opt.id):
validate_length(&opt.id, uq_const::MAX_OPTION_ID_LENGTH, "option.id")?;
```

#### OPT-UQ-3: Error handling strict (remplacer unwrap_or_default)

- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:281-282`
- **Changement**: Remplacer `unwrap_or_default()` par proper error handling
- **Benefice**: Maintenabilite - erreurs visibles au debug
- **Risque regression**: Faible

```rust
// Remplacer:
let selected: Vec<String> = serde_json::from_str(selected_json).unwrap_or_default();

// Par:
let selected: Vec<String> = serde_json::from_str(selected_json)
    .map_err(|e| ToolError::ExecutionFailed(format!(
        "Failed to parse selected_options JSON: {}", e
    )))?;
```

#### OPT-UQ-4: Queue limit frontend

- **Fichiers**: `src/lib/stores/userQuestion.ts:130-136`
- **Changement**: Limiter `pendingQuestions` a 50 items
- **Benefice**: Performance - evite memory leak
- **Risque regression**: Tres faible

```typescript
// Dans handleQuestionStart(), ajouter:
const MAX_PENDING_QUESTIONS = 50;

store.update((s) => {
    // Limit queue size to prevent memory issues
    const newPending = [...s.pendingQuestions, question].slice(-MAX_PENDING_QUESTIONS);
    return {
        ...s,
        pendingQuestions: newPending,
        currentQuestion: s.currentQuestion ?? question,
        isModalOpen: true,
        error: null
    };
});
```

#### OPT-UQ-5: Remplacer console.log par logger

- **Fichiers**: `src/lib/stores/userQuestion.ts` (lignes 146, 150, 156, 157, 169, 186, 191, 195, 196, 206)
- **Changement**: Utiliser logger unifie ou supprimer logs debug
- **Benefice**: Observabilite - logs propres en prod
- **Risque regression**: Zero

```typescript
// Option 1: Supprimer tous les console.log (recommande pour prod)
// Option 2: Conditionner sur import.meta.env.DEV
if (import.meta.env.DEV) {
    console.log('[userQuestionStore] submitResponse called:', response);
}
```

#### OPT-UQ-6: Tests SQL injection

- **Fichiers**: Nouveau `src-tauri/src/commands/user_question_tests.rs`
- **Changement**: Ajouter tests avec payloads malicieux
- **Benefice**: Securite - validation queries parametrees
- **Risque regression**: Zero

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_submit_response_sql_injection() {
        // Test avec question_id malicieux
        let result = submit_user_response(
            "'; DROP TABLE user_question; --".to_string(),
            vec![],
            None,
            // ... mock state
        ).await;

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid question_id"));
    }
}
```

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-UQ-7: Timeout configurable pour wait_for_response

- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:249-334`, `src-tauri/src/tools/constants.rs`
- **Changement**: Ajouter timeout avec `tokio::select!`
- **Phases**:
  1. Ajouter constante `DEFAULT_TIMEOUT_SECS = 300` (5 min)
  2. Refactorer `wait_for_response()` avec `tokio::select!`
  3. Retourner erreur `ToolError::Timeout` si expire
  4. Mettre a jour status DB a "timeout"
- **Prerequis**: Aucun
- **Risque regression**: Moyen - comportement change (indefini -> 5 min)
- **Tests requis**: Test timeout, test reponse avant timeout

```rust
// Dans constants.rs:
pub const DEFAULT_TIMEOUT_SECS: u64 = 300; // 5 minutes

// Dans tool.rs, refactorer wait_for_response():
async fn wait_for_response(&self, question_id: &str) -> ToolResult<Value> {
    let timeout = Duration::from_secs(uq_const::DEFAULT_TIMEOUT_SECS);
    let start = std::time::Instant::now();
    let mut interval_idx = 0;

    loop {
        // Check timeout
        if start.elapsed() > timeout {
            // Update DB status to timeout
            let update_query = format!(
                "UPDATE user_question:`{}` SET status = 'timeout'",
                question_id
            );
            let _ = self.db.execute(&update_query).await;

            return Err(ToolError::ExecutionFailed(format!(
                "Timeout waiting for user response after {} seconds",
                uq_const::DEFAULT_TIMEOUT_SECS
            )));
        }

        // ... reste du code existant
    }
}
```

#### OPT-UQ-8: Tests unitaires tool.rs

- **Fichiers**: Nouveau module `src-tauri/src/tools/user_question/tests.rs`
- **Changement**: Couvrir validation, definition, execute
- **Phases**:
  1. Tests validate_input (required fields, types)
  2. Tests definition() (schema JSON correct)
  3. Tests ask_question validation (limites, types)
  4. Mock DB pour tests execute
- **Risque regression**: Zero
- **Tests requis**: 15-20 tests unitaires

#### OPT-UQ-9: Tests integration commands

- **Fichiers**: `src-tauri/src/commands/user_question.rs`
- **Changement**: Tests end-to-end avec DB reelle
- **Phases**:
  1. Setup DB test
  2. Test submit_user_response flow complet
  3. Test skip_question flow
  4. Test get_pending_questions
  5. Tests edge cases (question not found, already answered)
- **Risque regression**: Zero

#### OPT-UQ-10: Refactoring ask_question()

- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:106-242`
- **Changement**: Extraire en 3 fonctions
- **Phases**:
  1. `validate_ask_input()` - validation (lignes 107-153)
  2. `create_question_record()` - DB creation (lignes 155-200)
  3. `emit_question_event()` - event emission (lignes 202-223)
- **Risque regression**: Bas (meme logique, meilleure organisation)

#### OPT-UQ-11: Refactoring submit_user_response()

- **Fichiers**: `src-tauri/src/commands/user_question.rs:11-117`
- **Changement**: Extraire en sous-fonctions
- **Phases**:
  1. `validate_question_pending()` - verification status
  2. `update_question_answered()` - UPDATE DB
  3. `verify_update_success()` - verification
- **Risque regression**: Bas

### Nice to Have (Impact faible, Effort faible)

#### OPT-UQ-12: Circuit breaker pour timeouts consecutifs

- **Fichiers**: Nouveau `src-tauri/src/tools/user_question/circuit_breaker.rs`
- **Changement**: Ouvrir circuit apres 3 timeouts consecutifs
- **Prerequis**: OPT-UQ-7 (timeout requis)
- **Benefice**: Resilience - evite spam questions si user absent
- **Risque regression**: Bas

### Differe (Impact faible, Effort eleve)

#### OPT-UQ-13: Pattern oneshot::channel (zero polling)

- **Raison report**: Refactor majeur, risque haut, polling actuel fonctionne
- **Impact**: Performance (elimine queries DB repetitives)
- **Effort**: 8-10h
- **Quand**: v2 si performance polling devient probleme

#### OPT-UQ-14: Migration Svelte 5 runes ($state)

- **Raison report**: Store actuel fonctionne bien, risque moyen
- **Impact**: Modernisation, perf marginale
- **Effort**: 4-6h
- **Quand**: Lors refactoring global frontend

#### OPT-UQ-15: Native async traits (supprimer async-trait)

- **Raison report**: Micro-optimisation, impact faible
- **Impact**: -1 heap allocation par appel Tool::execute
- **Effort**: 2h (mais affecte tous les tools)
- **Quand**: Lors refactoring global tools

#### OPT-UQ-16: Cleanup automatique questions > 7 jours

- **Raison report**: DB size non critique actuellement
- **Impact**: Maintenance DB
- **Effort**: 3h (background job)
- **Quand**: Post-v1 si DB grandit significativement

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| uuid | 1.18.0 | 1.18.1 | Non - patch mineur |

Toutes les autres dependances sont a jour (serde 1.0.228, tokio 1.48.0, surrealdb 2.4.0, tauri 2.9.3).

### Nouvelles Dependencies

Aucune nouvelle dependance requise.

## Verification Non-Regression

### Tests Existants

- [x] `npm run check` - svelte-check + TypeScript (frontend)
- [x] `cargo clippy` - Linting Rust
- [x] `cargo test` - Tests backend (0 tests specifiques UQT)

### Tests a Ajouter

- [ ] OPT-UQ-6: Tests SQL injection (1h)
- [ ] OPT-UQ-8: Tests unitaires tool.rs (4h)
- [ ] OPT-UQ-9: Tests integration commands (3h)
- [ ] Test timeout behavior (avec OPT-UQ-7)
- [ ] Test queue limit frontend (avec OPT-UQ-4)

### Validation Manuelle

```bash
# Scenario 1: Question normale
# 1. Demarrer workflow avec agent
# 2. Agent pose question -> modal apparait
# 3. User repond -> agent continue
# Attendu: Flow complet sans erreur

# Scenario 2: Timeout (apres OPT-UQ-7)
# 1. Agent pose question
# 2. Attendre 5+ minutes sans repondre
# Attendu: Erreur timeout, status DB = "timeout"

# Scenario 3: Skip question
# 1. Agent pose question
# 2. User clique Skip
# Attendu: status = "skipped", agent recoit erreur
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-UQ-1: Validate text_response | 0.5h | Haut | P1 |
| OPT-UQ-2: Validate option.id | 0.25h | Moyen | P1 |
| OPT-UQ-3: Error handling strict | 1h | Moyen | P1 |
| OPT-UQ-4: Queue limit frontend | 0.25h | Moyen | P1 |
| OPT-UQ-5: Logger unifie | 0.5h | Bas | P2 |
| OPT-UQ-6: Tests SQL injection | 1h | Haut | P1 |
| OPT-UQ-7: Timeout configurable | 2h | Haut | P2 |
| OPT-UQ-8: Tests unitaires | 4h | Haut | P2 |
| OPT-UQ-9: Tests integration | 3h | Haut | P2 |
| OPT-UQ-10: Refactor ask_question | 3h | Moyen | P3 |
| OPT-UQ-11: Refactor submit_response | 2h | Moyen | P3 |
| OPT-UQ-12: Circuit breaker | 2h | Moyen | P3 |

**Total estime**: ~19.5h

### Ordre d'Implementation Recommande

**Phase 1 - Securite (3h)**:
1. OPT-UQ-1 (validate text_response)
2. OPT-UQ-2 (validate option.id)
3. OPT-UQ-6 (tests SQL injection)

**Phase 2 - Quick Wins (2h)**:
4. OPT-UQ-3 (error handling)
5. OPT-UQ-4 (queue limit)
6. OPT-UQ-5 (logger)

**Phase 3 - Timeout (2h)**:
7. OPT-UQ-7 (timeout configurable)

**Phase 4 - Tests (7h)**:
8. OPT-UQ-8 (tests unitaires)
9. OPT-UQ-9 (tests integration)

**Phase 5 - Refactoring (5h)**:
10. OPT-UQ-10 (refactor ask_question)
11. OPT-UQ-11 (refactor submit_response)

**Phase 6 - Nice to Have (2h)**:
12. OPT-UQ-12 (circuit breaker)

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression timeout (agents existants attendent indefiniment) | Moyenne | Eleve | Documentation changement, config timeout |
| Performance degradee avec validations | Faible | Faible | Validations O(1), negligeable |
| Breaking change status "timeout" | Moyenne | Moyen | Ajouter au schema DB, update types TS |
| Tests flaky avec timing | Moyenne | Moyen | Utiliser tokio::time::pause() pour tests |

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer Phase 1 - Securite (OPT-UQ-1, 2, 6)
3. [ ] Valider avec `cargo test` et `cargo clippy`
4. [ ] Continuer Phase 2...

## References

### Code Analyse
- `src-tauri/src/tools/user_question/tool.rs` (447 lignes)
- `src-tauri/src/tools/user_question/mod.rs` (53 lignes)
- `src-tauri/src/commands/user_question.rs` (224 lignes)
- `src/lib/stores/userQuestion.ts` (277 lignes)
- `src/types/user-question.ts` (41 lignes)
- `src-tauri/src/models/user_question.rs` (105 lignes)
- `src-tauri/src/tools/constants.rs` (section user_question)

### Documentation Consultee
- `CLAUDE.md` (Tool Development Patterns, SurrealDB SDK 2.x)
- `docs/ARCHITECTURE_DECISIONS.md` (Q19-25)
- `docs/API_REFERENCE.md` (User Questions section)
- `docs/AGENT_TOOLS_DOCUMENTATION.md` (Section 4)

### Sources Externes
- [rs-graph-llm GitHub](https://github.com/a-agmon/rs-graph-llm)
- [Tauri Events v2](https://v2.tauri.app/develop/calling-frontend/)
- [Tokio Tutorial - Async](https://tokio.rs/tokio/tutorial/async)
- [HULA Atlassian Paper](https://www.atlassian.com/blog/atlassian-engineering/hula-blog-autodev-paper-human-in-the-loop-software-development-agents)
- [Svelte 5 Runes Real-time](https://dev.to/polliog/real-world-svelte-5-handling-high-frequency-real-time-data-with-runes-3i2f)
- [serde_json Performance](https://purplesyringa.moe/blog/i-sped-up-serde-json-strings-by-20-percent/)
- [Tauri Memory Leak #12724](https://github.com/tauri-apps/tauri/issues/12724)
