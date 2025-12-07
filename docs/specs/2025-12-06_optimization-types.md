# Plan d'Optimisation - Types

## Metadata
- **Date**: 2025-12-06
- **Domaine**: types (synchronisation TypeScript/Rust)
- **Stack**: TypeScript 5.9.3, Rust 1.91.1, serde 1.0.228, Tauri 2.9.4
- **Impact estime**: Maintenabilite (principal), Securite (secondaire)

## Resume Executif

Le systeme de types de Zileo-Chat-3 est mature (7.5/10) avec 25 fichiers TypeScript et 19 fichiers Rust synchronises manuellement. L'optimisation principale vise a eliminer le risque de desynchronisation silencieuse via l'implementation de **specta/tauri-specta** pour la generation automatique de types. Les quick wins incluent la standardisation de la nullabilite et la centralisation des constantes dupliquees.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src/types/agent.ts` | 188 | Moyenne | AVAILABLE_TOOLS duplique avec Rust |
| `src/types/task.ts` | 208 | Moyenne | Constants inline |
| `src/types/sub-agent.ts` | 300 | Haute | Types complexes bien structures |
| `src/types/llm.ts` | 227 | Moyenne | Constraints non typees (temperature, max_tokens) |
| `src/types/user-question.ts` | 40 | Basse | Synchronisation incomplete avec Rust |
| `src-tauri/src/models/agent.rs` | 274 | Moyenne | KNOWN_TOOLS duplique avec TS |
| `src-tauri/src/models/serde_utils.rs` | 275 | Haute | Custom deserializers complexes |
| `src-tauri/src/models/llm_models.rs` | 612 | Haute | Le plus gros fichier modele |
| `src-tauri/src/models/sub_agent.rs` | 369 | Haute | SubAgentOperationType sans equivalent TS |

**Total**: ~2500 lignes TypeScript + ~4500 lignes Rust

### Patterns Identifies

- **Pattern 1**: Enums Rust avec `#[serde(rename_all = "snake_case")]` vers union types TS
  - Fichiers: tous les modeles avec enums
  - Status: Bien implemente

- **Pattern 2**: Structs Create/Update/Summary pour CRUD
  - Fichiers: `agent.ts/rs`, `workflow.ts/rs`, `task.ts/rs`, `memory.ts/rs`
  - Status: Coherent et bien documente

- **Pattern 3**: Custom deserializers pour SurrealDB Thing type
  - Fichiers: `serde_utils.rs`
  - Status: Necessaire mais complexe

- **Pattern 4**: Alias `$types` obligatoire pour imports
  - Fichiers: tous les composants Svelte
  - Status: Respecte systematiquement

### Metriques Actuelles

```
Fichiers TypeScript types: 25
Fichiers Rust models: 19
Commandes Tauri: 99
Enums synchronises: 12
Custom deserializers: 4
Constantes dupliquees: 3 (TOOLS, defaults, limits)
```

## Best Practices (2024-2025)

### Sources Consultees

- [Specta Documentation](https://specta.dev/docs/tauri-specta/v2) - Dec 2025
- [GitHub - specta-rs/tauri-specta](https://github.com/specta-rs/tauri-specta) - Dec 2025
- [ts-rs GitHub](https://github.com/Aleph-Alpha/ts-rs) - Dec 2025
- [TauRPC Documentation](https://docs.rs/crate/taurpc/latest) - Dec 2025
- [Tauri Type Safety Patterns](https://www.gramigna.dev/blog/tauri-type-safety/) - 2025
- [Calling Rust from Frontend | Tauri](https://v2.tauri.app/develop/calling-rust/) - Dec 2025

### Patterns Recommandes

1. **Generation automatique de types avec Specta**
   - Utiliser `#[derive(specta::Type)]` sur les structs Rust
   - Export automatique vers TypeScript lors du build
   - Support natif Tauri 2.x via tauri-specta
   - Benefice: Single source of truth, zero desynchronisation

2. **Validation runtime avec Zod**
   - Creer schemas Zod correspondant aux interfaces TypeScript
   - Valider les reponses IPC avant utilisation
   - Benefice: Detection erreurs runtime, meilleure DX

3. **Conventions de nommage Tauri**
   - Rust: `snake_case` pour parametres
   - TypeScript: `camelCase` pour parametres
   - Tauri gere la conversion automatiquement
   - Benefice: Deja en place dans le projet

4. **Error handling avec thiserror**
   - Types d'erreur derives avec `#[derive(serde::Serialize)]`
   - Erreurs typees cote TypeScript
   - Benefice: Meilleur debugging, erreurs explicites

### Anti-Patterns a Eviter

1. **any/unknown overuse**
   - Probleme: Perte de type safety
   - Detection: `Record<string, unknown>` utilise 6+ fois dans les types
   - Alternative: Types specialises (MemoryMetadata, ValidationDetails)

2. **Desynchronisation silencieuse**
   - Probleme: Modifier Rust sans mettre a jour TypeScript
   - Detection: Pas de CI check pour synchronisation
   - Alternative: specta + CI workflow

3. **Nullabilite inconsistante**
   - Probleme: Melange `undefined` et `null` sans convention
   - Detection: `workflow_id?: string | null` vs `agentAssigned?: string`
   - Alternative: Convention stricte documentee

## Contraintes du Projet

Les contraintes suivantes sont NON-NEGOCIABLES (source: CLAUDE.md, ARCHITECTURE_DECISIONS.md):

1. **Alias $types obligatoire**
   - JAMAIS utiliser `$lib/types/` ou chemins relatifs
   - Configure dans tsconfig.json et svelte.config.js

2. **Conversion Tauri snake_case/camelCase**
   - Rust: `workflow_id`, TypeScript: `workflowId`
   - Tauri gere automatiquement, ne pas forcer manuellement

3. **Custom deserializers pour SurrealDB**
   - `deserialize_thing_id` requis pour IDs
   - `deserialize_workflow_status` pour enums stockes comme strings
   - Necessaires tant que SurrealDB SDK 2.x est utilise

4. **Types Create/Update separes**
   - `AgentConfig` (lecture complete)
   - `AgentConfigCreate` (creation sans ID)
   - `AgentConfigUpdate` (mise a jour, tous optionnels)

## Plan d'Optimisation

### Quick Wins (Impact haut/moyen, Effort faible)

#### OPT-1: Audit et synchronisation user-question types
- **Fichiers**: `src/types/user-question.ts`, `src-tauri/src/models/user_question.rs`
- **Changement**: Aligner les types TS avec Rust (QuestionOption, UserQuestionResponse manquants cote Rust)
- **Benefice**: Maintenabilite - feature active avec types incomplets
- **Risque regression**: Faible - ajout de champs, pas de modification
- **Validation**:
  ```bash
  npm run check
  cargo test
  ```
- **Effort estime**: 1h

#### OPT-2: Standardiser convention nullabilite
- **Fichiers**: Tous les fichiers `src/types/*.ts`
- **Changement**:
  - Champs DB nullable: `T | null`
  - Champs optionnels non-DB: `T | undefined` (via `?:`)
  - Documenter la convention dans CLAUDE.md
- **Benefice**: Coherence, moins d'erreurs runtime
- **Risque regression**: Faible - changements de type checking
- **Validation**:
  ```bash
  npm run check  # Detectera incompatibilites
  npm run lint
  ```
- **Effort estime**: 2h

#### OPT-3: Centraliser constantes dupliquees
- **Fichiers**:
  - `src/types/agent.ts` (AVAILABLE_TOOLS, BASIC_TOOLS)
  - `src-tauri/src/models/agent.rs` (KNOWN_TOOLS)
  - `src-tauri/src/tools/constants.rs` (diverses constantes)
- **Changement**:
  - Creer `src/lib/constants/tools.ts` avec toutes les constantes outils
  - Documenter que Rust est source de verite pour les outils backend
  - Exporter depuis `src/types/index.ts`
- **Benefice**: Single source of truth cote frontend
- **Risque regression**: Faible - reorganisation imports
- **Validation**:
  ```bash
  npm run check
  npm run build  # Verifier imports resolus
  ```
- **Effort estime**: 1h

#### OPT-4: Documenter custom deserializers
- **Fichiers**: `src-tauri/src/models/serde_utils.rs`, `CLAUDE.md`
- **Changement**:
  - Ajouter exemples d'usage dans serde_utils.rs
  - Documenter quand utiliser chaque deserializer
  - Ajouter section dediee dans CLAUDE.md
- **Benefice**: Onboarding facilite, moins d'erreurs
- **Risque regression**: Aucun - documentation seulement
- **Validation**: Review manuelle
- **Effort estime**: 1h

### Optimisations Strategiques (Impact haut, Effort eleve)

#### ~~OPT-5: Implementer specta + tauri-specta~~ [DIFFERE]

**Status**: Differe en Phase 9 (Post-v1)
**Raison**: tauri-specta v2.0.0-rc.21 (Janvier 2025) est incompatible avec Tauri 2.9.x stable.
- Derniere release: 11 mois sans version stable
- Issue connue: "broken until tauri-apps/tauri#12371"
- Documentation pinne sur Tauri 2.0.0-beta.22

**Condition de reactivation**: Quand tauri-specta 2.0 stable sort avec support Tauri 2.9+

**Fichiers** (pour reference future):
- `src-tauri/Cargo.toml` (nouvelles deps)
- `src-tauri/src/main.rs` (export config)
- Tous les fichiers `src-tauri/src/models/*.rs` (derive macro)
- `src/types/generated.ts` (nouveau fichier genere)

**Effort estime**: 8-10h (quand disponible)

### Nice to Have (Impact moyen, Effort moyen)

#### OPT-6: Specialiser Record<string, unknown>
- **Fichiers**:
  - `src/types/memory.ts` (MemoryMetadata)
  - `src/types/validation.ts` (ValidationDetails)
  - `src/types/tool.ts` (ToolInputParams, ToolOutputResult)
- **Changement**:
  ```typescript
  // Avant
  metadata: Record<string, unknown>;

  // Apres
  interface MemoryMetadata {
    source?: string;
    relevance_score?: number;
    tags?: string[];
    embedding_model?: string;
  }
  ```
- **Benefice**: Meilleur IntelliSense, type safety accrue
- **Risque regression**: Faible - types plus stricts
- **Validation**: npm run check
- **Effort estime**: 3h

#### OPT-7: Ajouter validation Zod runtime
- **Fichiers**:
  - `package.json` (ajouter zod)
  - `src/lib/validation/schemas.ts` (nouveau)
  - `src/types/validated.ts` (nouveau)
- **Changement**:
  1. Installer zod: `npm install zod`
  2. Creer schemas pour types critiques (LLMConfig, AgentConfig)
  3. Wrapper invoke() avec validation
- **Benefice**: Detection erreurs runtime, meilleure DX
- **Risque regression**: Faible - ajout de validation, pas de modification
- **Validation**: npm run test
- **Effort estime**: 4h

### Differe (Non prioritaire)

| Item | Description | Raison du report |
|------|-------------|------------------|
| OPT-5 | specta + tauri-specta | tauri-specta v2 en RC, incompatible Tauri 2.9.x (Dec 2025) |

**Note**: OPT-5 sera reactivee quand tauri-specta 2.0 stable sortira avec support Tauri 2.9+.

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| TypeScript | 5.9.3 | 5.9.3 | Non - deja a jour |
| serde | 1.0.228 | 1.0.228 | Non - deja a jour |
| serde_json | 1.0.145 | 1.0.145 | Non - deja a jour |
| specta | Absent | 2.x | N/A - nouvelle dep |
| tauri-specta | Absent | 2.x | N/A - nouvelle dep |
| zod | Absent | 3.x | N/A - nouvelle dep |

### Nouvelles Dependencies (si OPT-5 et OPT-7 implementes)

| Package/Crate | Raison | Impact |
|---------------|--------|--------|
| specta | Generation types TS depuis Rust | +minimal compile time |
| tauri-specta | Integration Tauri pour specta | +minimal compile time |
| zod | Validation runtime TypeScript | +12kb bundle (tree-shakeable) |

## Verification Non-Regression

### Tests Existants
- [x] `npm run check` - svelte-check + TypeScript strict (couvre tous les types TS)
- [x] `npm run lint` - ESLint avec TypeScript plugin
- [x] `cargo test` - Tests unitaires backend (serialisation modeles)
- [x] `cargo clippy` - Detection types morts
- [x] `npm run build` - Build production (valide imports)

### Tests a Ajouter
- [ ] Test snapshot comparant types exportes vs attendus (pour OPT-5)
- [ ] Test de serialisation round-trip pour modeles critiques
- [ ] CI check pour detecter desynchronisation (si specta implemente)
- [ ] Tests unitaires pour schemas Zod (si OPT-7 implemente)

### Benchmarks (si applicable)
```bash
# Temps de build actuel (baseline)
time npm run build
time cargo build --release

# Apres OPT-5 (specta)
time npm run build  # Devrait etre similaire
time cargo build --release  # +5-10% pour generation types
```

## Estimation

| Optimisation | Effort | Impact | Priorite | Status |
|--------------|--------|--------|----------|--------|
| OPT-1: Sync user-question | 1h | Haut | P1 | Complete (Phase 3) |
| OPT-2: Standardiser nullabilite | 2h | Moyen | P1 | Complete (Phase 3) |
| OPT-3: Centraliser constantes | 1h | Moyen | P1 | Complete (Phase 3) |
| OPT-4: Documenter deserializers | 1h | Faible | P2 | Complete (Phase 3) |
| ~~OPT-5: Implementer specta~~ | 8-10h | Haut | - | **DIFFERE** |
| OPT-6: Specialiser Record<> | 3h | Moyen | P3 | Complete (Phase 8) |
| OPT-7: Validation Zod | 4h | Moyen | P3 | Complete (Phase 8) |

**Total Quick Wins (P1)**: 4-5h - COMPLETE
**Total Strategique (P2)**: 1h - COMPLETE (OPT-5 differe)
**Total Nice to Have (P3)**: 7h - COMPLETE

**Total restant**: 0h (OPT-5 differe)

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Desync types apres OPT-5 | Moyenne | Eleve | Comparer types generes vs manuels avant migration complete |
| Breaking changes nullabilite | Faible | Moyen | npm run check detecte toutes les incompatibilites |
| Imports casses OPT-3 | Faible | Faible | Build echoue immediatement si import invalide |
| Specta incompatible SurrealDB types | Faible | Eleve | Garder custom deserializers, specta pour types IPC seulement |

## Prochaines Etapes

1. [x] Valider ce plan avec l'utilisateur
2. [x] Executer OPT-1 (sync user-question) - Complete Phase 3
3. [x] Executer OPT-2 (nullabilite) - Complete Phase 3
4. [x] Executer OPT-3 (constantes) - Complete Phase 3
5. [x] Executer OPT-4 (deserializers docs) - Complete Phase 3
6. [x] OPT-5 (specta) - **DIFFERE** (tauri-specta incompatible Tauri 2.9.x)
7. [x] OPT-6, OPT-7 - Complete Phase 8
8. [ ] Surveiller sortie tauri-specta 2.0 stable pour reactivation OPT-5

## References

### Code Analyse
- `src/types/` - 25 fichiers TypeScript
- `src-tauri/src/models/` - 19 fichiers Rust
- `src-tauri/src/models/serde_utils.rs` - Custom deserializers
- `src/types/index.ts` - Point d'entree exports

### Documentation Consultee
- `CLAUDE.md` - Sections Type Synchronization, Parameter Naming
- `docs/ARCHITECTURE_DECISIONS.md` - Decisions types et serialisation
- `docs/API_REFERENCE.md` - Signatures commandes Tauri
- `tsconfig.json` - Configuration TypeScript strict

### Sources Externes
- [Specta Documentation](https://specta.dev/docs/tauri-specta/v2)
- [GitHub - specta-rs/tauri-specta](https://github.com/specta-rs/tauri-specta)
- [ts-rs GitHub](https://github.com/Aleph-Alpha/ts-rs)
- [TauRPC Documentation](https://docs.rs/crate/taurpc/latest)
- [Tauri v2 - Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/)
- [TypeScript 5.9 Release Notes](https://devblogs.microsoft.com/typescript/announcing-typescript-5-9/)
