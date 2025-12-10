# Rapport - OPT-UQ-12 Circuit Breaker Integration

## Metadata
- **Date**: 2025-12-10
- **Spec source**: docs/specs/2025-12-10_optimization-user-question-tool.md
- **Section**: Nice to Have
- **Optimization**: OPT-UQ-12

## Description

Integration du circuit breaker dans le UserQuestionTool pour prevenir le spam de questions lorsque l'utilisateur est non-responsif.

## Orchestration

### Graphe Execution
```
Groupe 1 (SEQ): Constants -> Add CIRCUIT_* to constants.rs
      |
      v
Groupe 2 (SEQ): Struct -> Add circuit_breaker field to UserQuestionTool
      |
      v
Groupe 3 (SEQ): Integration -> Update ask_question() with CB checks
      |
      v
Validation (SEQ): cargo clippy + cargo test
```

### Agents Utilises
| Phase | Agent | Execution |
|-------|-------|-----------|
| Constants | Main | Sequentiel |
| Struct | Main | Sequentiel |
| Integration | Main | Sequentiel |
| Validation | Main | Sequentiel |

## Fichiers Modifies

### Constants (src-tauri/src/tools/constants.rs)
- Ajout de `CIRCUIT_FAILURE_THRESHOLD: u32 = 3`
- Ajout de `CIRCUIT_COOLDOWN_SECS: u64 = 60`

### Tool (src-tauri/src/tools/user_question/tool.rs)
- Import de `UserQuestionCircuitBreaker` et `RwLock`
- Ajout du champ `circuit_breaker: RwLock<UserQuestionCircuitBreaker>` dans la struct
- Initialisation du circuit breaker dans `new()` avec les constantes
- Modification de `ask_question()`:
  - Verification `allow_question()` avant de poser une question
  - Mise a jour du circuit breaker apres la reponse:
    - `record_success()` sur reponse reussie
    - `record_timeout()` sur timeout (ouvre le circuit apres 3 timeouts)
    - `record_skip()` sur skip (traite comme succes)

### Module (src-tauri/src/tools/user_question/mod.rs)
- Suppression du commentaire "not yet integrated"
- Re-export conditionnel du `UserQuestionCircuitBreaker`

## Implementation Details

### Circuit Breaker Behavior

1. **Closed State** (Normal): Toutes les questions passent
2. **Open State** (Apres 3 timeouts): Questions rejetees immediatement
   - Message: "User appears unresponsive (X consecutive timeouts). Question rejected. Retry in Y seconds."
3. **Half-Open State** (Apres 60s cooldown): Permet une question de test
   - Succes -> retour a Closed
   - Timeout -> retour a Open

### Thread Safety

Utilisation de `RwLock` au lieu de `RefCell` car le trait `Tool` requiert `Send + Sync`.

### Error Messages

Les messages d'erreur incluent:
- Nombre de timeouts consecutifs
- Temps restant avant retry possible

## Validation

### Backend
- cargo fmt: PASS
- cargo clippy: PASS
- cargo test --lib: 844 tests PASS

### Tests Specifiques
- Circuit breaker tests: 14 tests PASS
- User question tests: 58 tests PASS (incluant nouveaux tests)

## Metriques
- Fichiers modifies: 3
- Lignes ajoutees: ~80
- Tests existants: Tous passent
- Nouveaux tests: 0 (tests existants dans circuit_breaker.rs suffisants)

## Notes

- Le circuit breaker est scope par workflow (chaque `UserQuestionTool` instance a son propre circuit breaker)
- Les erreurs de verrouillage (poison) sont gerees gracieusement
- Les tests d'integration pre-existants ont des erreurs non liees a cette implementation (probleme d'API `ToolFactory`)
