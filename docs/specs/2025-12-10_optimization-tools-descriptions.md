# Plan d'Optimisation - Tool Descriptions

## Metadata
- **Date**: 2025-12-10
- **Domaine**: tools (descriptions des tools MCP)
- **Stack**: Rust 1.91.1 + Tauri 2.9.3 + Rig.rs 0.24.0
- **Impact estime**: Performance LLM / Maintenabilite / Coherence

## Resume Executif

Ce plan vise a ameliorer la qualite des descriptions des 7 outils MCP de Zileo-Chat-3 pour optimiser la comprehension et l'utilisation par les LLMs. L'analyse revele un ecart critique sur UserQuestionTool (90 chars vs 800+ pour les autres) et des opportunites de standardisation. Les gains attendus sont une meilleure precision des appels d'outils et une reduction des erreurs d'utilisation.

## Etat Actuel

### Analyse du Code

| Fichier | Complexite | Points d'attention |
|---------|------------|-------------------|
| `src-tauri/src/tools/user_question/tool.rs:505-569` | CRITIQUE | Description 90 chars, 0 exemples, circuit breaker non documente |
| `src-tauri/src/tools/todo/tool.rs:377-485` | Moyenne | Statuts valides non documentes dans description |
| `src-tauri/src/tools/spawn_agent.rs:550-650` | Bonne | Reference pour format ideal (850 chars) |
| `src-tauri/src/tools/delegate_task.rs:495-580` | Moyenne | 70% duplication avec SpawnAgent |
| `src-tauri/src/tools/memory/tool.rs:657-793` | Excellente | Model a suivre (420 chars, 4 exemples) |
| `src-tauri/src/tools/calculator/tool.rs:323-412` | Bonne | 450 chars, exhaustive |
| `src-tauri/src/tools/parallel_tasks.rs:612-690` | Bonne | 600 chars |

### Score Qualite des Descriptions

| Tool | Longueur | Format | Exemples | Constraints | Score |
|------|----------|--------|----------|-------------|-------|
| MemoryTool | 420 chars | 5/5 | 4 | Explicites | 19/20 |
| TodoTool | 380 chars | 4/5 | 4 | Partielles | 18/20 |
| CalculatorTool | 450 chars | 4/5 | 5 | Implicites | 17/20 |
| **UserQuestionTool** | **90 chars** | **2/5** | **0** | **Implicites** | **8/20** |
| SpawnAgentTool | 850 chars | 5/5 | 1 | Explicites | 20/20 |
| DelegateTaskTool | 750 chars | 4/5 | 2 | Explicites | 19/20 |
| ParallelTasksTool | 600 chars | 4/5 | 1 | Explicites | 18/20 |

### Patterns Identifies

- **Pattern reussi**: Sections structurees (USE THIS TOOL TO, OPERATIONS, BEST PRACTICES, EXAMPLES)
- **Pattern dynamique**: SpawnAgentTool utilise `format!()` pour injecter tools disponibles
- **Anti-pattern**: UserQuestionTool description d'une seule phrase
- **Duplication**: SpawnAgent et DelegateTask partagent 70% du texte

### Metriques Actuelles

```
Total lignes tools/: 14,097
Fichiers analyses: 24
Outils avec description complete: 6/7 (86%)
Outils avec exemples: 6/7 (86%)
Outils avec circuit breaker documente: 0/2 (0%)
```

## Best Practices (2024-2025)

### Sources Consultees
- [MCP Best Practices | Peter Steinberger](https://steipete.me/posts/2025/mcp-best-practices)
- [Tools - Model Context Protocol](https://modelcontextprotocol.io/specification/2025-06-18/server/tools)
- [How to implement tool use - Claude Docs](https://docs.claude.com/en/docs/agents-and-tools/tool-use/implement-tool-use)
- [Introducing Structured Outputs | OpenAI](https://openai.com/index/introducing-structured-outputs-in-the-api/)
- [Less is More: Optimizing Function Calling for LLM](https://arxiv.org/html/2411.15399v1)
- [15 Best Practices for Building MCP Servers](https://thenewstack.io/15-best-practices-for-building-mcp-servers-in-production/)
- [MCP tool descriptions best practices](https://www.merge.dev/blog/mcp-tool-description)

### Patterns Recommandes

1. **Information critique en premier**: Les LLMs ne lisent pas toujours l'integralite
2. **Description 3-4 phrases minimum**: Pour outils complexes
3. **Exemples concrets integres**: Directement dans le texte de description
4. **Parametres documentes**: required/optional explicite, valeurs par defaut
5. **Workflow explicite**: Documenter les prerequis et l'ordre d'execution
6. **Messages d'erreur actionnables**: Codes machine-readable + explications
7. **Nommage descriptif**: Eviter `val`, `data` - preferer `transaction_value`

### Anti-Patterns a Eviter

1. **Descriptions vagues**: `"Updates data"` - Trop court
2. **Parametres sans description**: Manque le champ description dans le schema
3. **Statut required/optional non documente**: LLM ne sait pas quoi omettre
4. **Trop d'outils exposes**: Surcharge cognitive, baisse de performance
5. **Limites non documentees**: LLM ne connait pas les contraintes

## Contraintes du Projet

- **Format Raw String**: Utiliser `r#"..."#` pour descriptions multi-lignes (Rust)
- **Sections CAPS**: USE THIS TOOL TO, OPERATIONS, BEST PRACTICES, EXAMPLES
- **Longueur max**: ~1500 caracteres (reference: SpawnAgentTool)
- **Langue**: Anglais uniquement (LLMs optimises pour anglais)
- **No Breaking Changes**: Noms operations et parametres inchanges
- **Source: CLAUDE.md section Tool Development Patterns**

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-TD-1: Enrichir UserQuestionTool description
- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:505-569`
- **Changement**: Passer de 90 a 800+ caracteres avec structure complete
- **Benefice**: LLM comprendra comment utiliser checkbox/text/mixed
- **Risque regression**: Faible (texte uniquement)
- **Effort**: 1h
- **Validation**: Test manuel avec prompt utilisant UserQuestionTool

**Description proposee**:
```rust
r#"Asks the user a question and waits for their response with configurable input types.

USE THIS TOOL WHEN:
- You need user input to proceed (clarification, choice, confirmation)
- A decision cannot be made autonomously
- User preferences or validation is required

IMPORTANT CONSTRAINTS:
- Timeout: 5 minutes (returns error if no response)
- Circuit breaker: After 3 consecutive timeouts, tool blocks for 60 seconds
- Maximum 20 options for checkbox type
- Question length: max 2000 characters

QUESTION TYPES:
- checkbox: Multiple choice with predefined options (user selects one or more)
- text: Free-form text input with optional placeholder
- mixed: Both options AND text input available

OPERATIONS:
- ask: Present question to user and wait for response

BEST PRACTICES:
- Keep questions clear and concise
- Provide meaningful option labels for checkbox type
- Use context parameter to explain why you're asking
- Handle timeout errors gracefully (circuit may be open)

EXAMPLES:
1. Checkbox question:
   {"operation": "ask", "question": "Which database should we use?", "questionType": "checkbox", "options": [{"id": "pg", "label": "PostgreSQL"}, {"id": "mysql", "label": "MySQL"}]}

2. Text input:
   {"operation": "ask", "question": "What should be the API endpoint name?", "questionType": "text", "textPlaceholder": "e.g., /api/v1/users"}

3. Mixed (options + text):
   {"operation": "ask", "question": "Select a template or describe custom:", "questionType": "mixed", "options": [{"id": "basic", "label": "Basic template"}], "textPlaceholder": "Custom description..."}"#
```

#### OPT-TD-2: Documenter circuit breaker UserQuestionTool
- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:505-569`
- **Changement**: Ajouter section IMPORTANT CONSTRAINTS avec circuit breaker
- **Benefice**: LLM saura pourquoi l'outil peut etre temporairement bloque
- **Risque regression**: Faible
- **Effort**: 0.5h (inclus dans OPT-TD-1)
- **Validation**: Verification presence dans description

#### OPT-TD-3: Ajouter exemples JSON a UserQuestionTool
- **Fichiers**: `src-tauri/src/tools/user_question/tool.rs:505-569`
- **Changement**: 3-4 exemples couvrant checkbox, text, mixed
- **Benefice**: LLM aura des modeles concrets a suivre
- **Risque regression**: Faible
- **Effort**: 0.5h (inclus dans OPT-TD-1)
- **Validation**: Verification exemples valides syntaxiquement

#### OPT-TD-4: TodoTool - lister statuts valides
- **Fichiers**: `src-tauri/src/tools/todo/tool.rs:377-485`
- **Changement**: Ajouter "Valid statuses: pending, in_progress, completed, blocked"
- **Benefice**: LLM ne tentera pas de statuts invalides
- **Risque regression**: Faible
- **Effort**: 0.5h
- **Validation**: `cargo test` passe

**Modification proposee** (dans section OPERATIONS):
```rust
// Ajouter apres "- update_status: Change task status"
"- update_status: Change task status. Valid values: pending, in_progress, completed, blocked"
```

#### OPT-TD-5: Ameliorer descriptions enum operation
- **Fichiers**: Tous les `tool.rs` (7 fichiers)
- **Changement**: Remplacer `"description": "The operation to perform"` par descriptions explicites
- **Benefice**: LLM comprend directement les operations disponibles
- **Risque regression**: Faible
- **Effort**: 1h
- **Validation**: Verification JSON Schema valide

**Avant**:
```json
"operation": {
  "type": "string",
  "enum": ["ask"],
  "description": "Operation to perform"
}
```

**Apres**:
```json
"operation": {
  "type": "string",
  "enum": ["ask"],
  "description": "Operation: 'ask' presents question to user and waits for response"
}
```

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-TD-6: Template commun sub-agent tools
- **Fichiers**: `spawn_agent.rs`, `delegate_task.rs`, `parallel_tasks.rs`
- **Changement**: Extraire sections communes dans helper function
- **Phases**:
  1. Identifier sections identiques (COMMUNICATION PATTERN, PROMPT BEST PRACTICES)
  2. Creer `fn sub_agent_common_sections() -> String` dans `tools/utils.rs`
  3. Refactorer les 3 tools pour utiliser le helper
- **Prerequis**: OPT-TD-1 a OPT-TD-5 completes
- **Risque regression**: Moyen (changement structure)
- **Effort**: 2h
- **Tests requis**: `cargo test` + verification descriptions generees

#### OPT-TD-7: Injection dynamique des constantes
- **Fichiers**: `memory/tool.rs`, `todo/tool.rs`, `user_question/tool.rs`
- **Changement**: Utiliser `format!()` avec constantes de `tools/constants.rs`
- **Phases**:
  1. Identifier constantes pertinentes (MAX_CONTENT_LENGTH, VALID_TYPES, etc.)
  2. Modifier descriptions pour utiliser format!()
  3. Tester generation
- **Prerequis**: Aucun
- **Risque regression**: Moyen
- **Effort**: 3h
- **Tests requis**: `cargo test` + verification valeurs injectees

**Exemple**:
```rust
fn description(&self) -> String {
    format!(r#"Manages persistent memory (max content: {} chars).

USE THIS TOOL TO:
...

MEMORY TYPES: {:?}
"#,
        memory::MAX_CONTENT_LENGTH,
        memory::VALID_TYPES
    )
}
```

#### OPT-TD-8: Guidelines CLAUDE.md
- **Fichiers**: `CLAUDE.md`
- **Changement**: Ajouter section "Tool Description Guidelines"
- **Contenu**:
  - Structure obligatoire (sections CAPS)
  - Longueur recommandee (300-800 chars basic, 800-1500 sub-agent)
  - Exemples minimum (2-4 par tool)
  - Checklist avant merge
- **Prerequis**: OPT-TD-1 a OPT-TD-5 comme exemples
- **Risque regression**: Faible (documentation)
- **Effort**: 1h
- **Validation**: Review manuelle

### Nice to Have (Impact faible, Effort faible)

#### OPT-TD-9: Documenter timeouts/limits dans chaque tool
- **Fichiers**: Tous les `tool.rs`
- **Changement**: Ajouter section LIMITS avec timeouts et max values
- **Benefice**: LLM anticipe les contraintes
- **Risque regression**: Faible
- **Effort**: 1h

#### OPT-TD-10: Section ERRORS avec codes
- **Fichiers**: Tools complexes (Memory, Todo, UserQuestion)
- **Changement**: Ajouter section documentant codes d'erreur possibles
- **Benefice**: LLM peut anticiper et gerer les erreurs
- **Risque regression**: Faible
- **Effort**: 2h

### Differe (Impact faible, Effort eleve)

| Optimisation | Raison du report |
|--------------|------------------|
| OPT-TD-11: Centraliser descriptions dans tools/descriptions.rs | Refactoring majeur, benefice marginal |
| OPT-TD-12: Auto-generer docs depuis Rust | Complexite elevee, risque de bugs |
| OPT-TD-13: i18n des descriptions | LLMs performent bien en anglais, ROI faible |

## Dependencies

### Mises a Jour Recommandees

Aucune mise a jour de dependance requise pour ce plan d'optimisation.
Les descriptions sont du texte pur, pas de dependance sur crates externes.

### Nouvelles Dependencies

Aucune nouvelle dependance requise.

## Verification Non-Regression

### Tests Existants
- [x] `cargo test` - Couvre validations parametres (ne teste pas descriptions)
- [x] `cargo clippy` - Verification qualite code
- [x] `npm run check` - Frontend (non impacte)

### Tests a Ajouter
Aucun test unitaire possible pour les descriptions (texte libre).
Validation manuelle recommandee:

- [ ] Test 1: Prompt avec UserQuestionTool checkbox - verifier format reponse
- [ ] Test 2: Prompt avec UserQuestionTool text - verifier placeholder utilise
- [ ] Test 3: Prompt avec TodoTool update_status - verifier statuts valides utilises

### Benchmarks
```bash
# Avant optimisation - noter taux de succes appels tools
# Compter erreurs "invalid operation" ou "missing parameter" dans logs

# Apres optimisation
# Meme mesure, comparer reduction erreurs
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-TD-1 | 1h | Haut | P1 |
| OPT-TD-2 | 0.5h | Haut | P1 (inclus OPT-TD-1) |
| OPT-TD-3 | 0.5h | Haut | P1 (inclus OPT-TD-1) |
| OPT-TD-4 | 0.5h | Moyen | P1 |
| OPT-TD-5 | 1h | Moyen | P1 |
| OPT-TD-6 | 2h | Moyen | P2 |
| OPT-TD-7 | 3h | Haut | P2 |
| OPT-TD-8 | 1h | Moyen | P2 |
| OPT-TD-9 | 1h | Faible | P3 |
| OPT-TD-10 | 2h | Faible | P3 |

**Total P1 (Quick Wins)**: ~3h
**Total P2 (Strategic)**: ~6h
**Total P3 (Nice to Have)**: ~3h

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle | Tres faible | Eleve | Descriptions = texte, pas de logique |
| Description trop longue | Faible | Moyen | Limiter a 1500 chars max |
| LLM moins performant apres changement | Faible | Moyen | Test A/B avant deploy |
| Duplication avec docs externes | Moyenne | Faible | Single source = code Rust |

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer OPT-TD-1 (UserQuestionTool - quick win critique)
3. [ ] Executer OPT-TD-4 (TodoTool statuts)
4. [ ] Executer OPT-TD-5 (descriptions enum)
5. [ ] Mesurer impact qualitatif
6. [ ] Continuer avec P2 si resultats positifs

## References

### Code Analyse
- `src-tauri/src/tools/user_question/tool.rs` - Description critique
- `src-tauri/src/tools/todo/tool.rs` - Statuts manquants
- `src-tauri/src/tools/spawn_agent.rs` - Reference qualite
- `src-tauri/src/tools/memory/tool.rs` - Reference qualite
- `src-tauri/src/tools/constants.rs` - Constantes a injecter

### Documentation Consultee
- `docs/AGENT_TOOLS_DOCUMENTATION.md`
- `docs/API_REFERENCE.md`
- `CLAUDE.md` section Tool Development Patterns

### Sources Externes
- MCP Specification 2025-06-18
- Claude Tool Use Documentation
- OpenAI Structured Outputs Guide
- Research: Less is More (LLM Function Calling Optimization)
