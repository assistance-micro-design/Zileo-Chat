# SA-017: Settings Page Optimization

## Metadata
- **Date**: 2026-02-22
- **Domaine**: Settings page (toutes sections)
- **Branch**: security/audit-remediation-tdd
- **Stack**: SvelteKit 2.49.1 + Svelte 5.49.1 | Rust 1.93.0 + Tauri 2.10.2 | SurrealDB 2.5.0
- **Impact**: Performance scroll / Maintenabilite / Coherence code
- **Prerequis**: SA-010 (86% remedie)

## Resume Executif

Audit d'optimisation de la page Settings et toutes ses sections. L'analyse porte sur la **performance de scroll** (lenteurs signalees par l'utilisateur dans chaque section), la duplication de code, la qualite des composants, la coherence des patterns entre sections, et la connexion frontend/backend.

**Verdict global**: Le code fonctionne et est globalement sain (SA-010 deja 86% remedie). Les lenteurs de scroll sont causees par des **problemes CSS identifies** (containment manquant, box-shadow sur cards durant repaint, pointer-events timeout trop court, debounce manquant). Le backend est bien structure et securise.

## Scope Analyse

### Frontend (41 fichiers)
- 9 pages routes (`src/routes/settings/`)
- 29 composants (`src/lib/components/settings/`)
- 2 stores (`src/lib/stores/llm.ts`, `validation-settings.ts`)
- 7 types (`src/types/`)

### Backend (9 fichiers)
- 9 commandes (`src-tauri/src/commands/`)
- Modeles et DB associes

---

## Etat Actuel

### Coherence Frontend/Backend: PASS

| Verification | Status | Detail |
|---|---|---|
| Commandes exposees dans mod.rs | PASS | Toutes presentes |
| Parametres IPC (camelCase/snake_case) | PASS | Convention respectee |
| Types de retour synchronises | PASS | TS <-> Rust coherent |
| Gestion erreur Result<T, String> | PASS | Partout |
| Securite (injection, validation) | PASS | bind(), serialize_for_query(), validate_uuid_field() |
| Commandes orphelines | PASS | Aucune detectee |

### Duplication Verifiee

| Element | Fichiers | Lignes dupliquees | Verifie |
|---|---|---|---|
| Error-banner (template + CSS) | AgentSettings / PromptSettings | 28 lignes | Oui - identique sauf i18n key |
| Settings-header (template + CSS) | AgentSettings / PromptSettings | 38 lignes | Oui - identique sauf i18n keys |
| Delete confirmation modal | AgentSettings / PromptSettings | 22 lignes | Oui - identique sauf i18n keys |
| CRUD state management (handlers) | AgentSettings / PromptSettings | ~50 lignes | Oui - handleCreate/Edit/Delete/Close similaires |
| Validation functions backend | agent.rs / mcp.rs | ~30 lignes | Oui - validate_*_name() quasi-identiques |
| **Total duplication verifiee** | | **~168 lignes** | |

### Composants Trop Gros

| Composant | Lignes | Script | Template | CSS | Probleme |
|---|---|---|---|---|---|
| MemorySettings.svelte | 1082 | 298 | 118 | **440** | CSS massif, 4 cartes distinctes melangees |
| MemoryList.svelte | 880 | ~300 | ~200 | ~380 | List + 2 modals + export/import |
| ImportPanel.svelte | 808 | ~250 | ~200 | ~350 | List + form + preview + conflict |
| AgentForm.svelte | 794 | ~200 | ~250 | ~340 | Form complexe (tools, MCP, prompt) |

### Incoherences Inter-Sections

| Aspect | Agents | Prompts | Memory | MCP | LLM | Validation |
|---|---|---|---|---|---|---|
| Store pattern | CRUD factory | CRUD factory | State local | State wrapper | Pure functions | Store custom |
| Error display | Store + banner | Store + banner | Local message | State error | Local message | Absent |
| Form type | Modal | Modal | Modal | Modal | Modal | Inline |
| Lazy loading | Oui | Non | Oui | Non | Non | Non |
| Header pattern | SettingsHeader | SettingsHeader | Different | Different | Split view | N/A |

---

## Plan d'Optimisation

### Phase 0: Scroll Performance (PRIORITAIRE)

**Objectif**: Corriger les lenteurs de scroll signalees dans toutes les sections.

**Contexte**: Le layout a deja des optimisations (OPT-SCROLL-FIX): `pointer-events: none` pendant scroll, `contain: content`, `-webkit-overflow-scrolling: touch`, animations pausees pendant scroll. Mais des problemes subsistent.

#### PERF-1: Card `box-shadow` repeint a chaque frame de scroll (HIGH)

- **Fichier**: `src/styles/global.css:310`
- **Probleme**: `.card { box-shadow: var(--shadow-sm); }` applique une ombre double (`0 1px 3px + 0 1px 2px`) sur CHAQUE card. Dans les grilles settings (agents: 5-10 cards, prompts: 10-50 cards), le moteur de rendu repeint toutes les ombres visibles a chaque frame de scroll.
- **Fix**: Promouvoir les cards en GPU layer avec `transform: translateZ(0)` pour que l'ombre soit rasterisee une fois et composee par le GPU:
  ```css
  .card {
    box-shadow: var(--shadow-sm);
    transform: translateZ(0); /* GPU layer - shadow rasterized once */
  }
  ```
- **Impact**: HIGH sur sections avec beaucoup de cards (Agents, Prompts, LLM providers)
- **Non-regression**: Verifier visuellement que le rendu des cards ne change pas

#### PERF-2: `.prompt-grid` manque `contain: layout style` (HIGH)

- **Fichier**: `src/lib/components/settings/prompts/PromptList.svelte:264`
- **Probleme**: `.prompt-grid` n'a PAS `contain: layout style` alors que `.agent-grid` (AgentList:203), `.provider-grid` (LLMSection:468), et MCPSection:369 l'ont. Incoherence directe. Sans containment, un changement dans une card prompt force le recalcul layout de toute la grille.
- **Fix**:
  ```css
  .prompt-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: var(--spacing-lg);
    contain: layout style; /* OPT-SCROLL-5: Isolate layout recalculations */
  }
  ```
- **Impact**: HIGH - alignement avec les autres grilles
- **Non-regression**: Verifier layout de la grille prompts

#### PERF-3: Scroll timeout 100ms trop court + `:global(*)` couteux (MEDIUM)

- **Fichier**: `src/routes/settings/+layout.svelte:61,305`
- **Probleme**: Le timeout `pointer-events: none` est a 100ms. Le momentum scroll sur WebKit2GTK peut durer 300-500ms apres relachement. Resultat: les pointer-events reviennent trop tot, les hover states se recalculent pendant le scroll inertiel. De plus, `.content-area.is-scrolling :global(*) { pointer-events: none !important; }` force le navigateur a matcher TOUS les elements descendants.
- **Fix**:
  ```javascript
  // Augmenter de 100ms a 250ms
  scrollTimeout = setTimeout(() => {
    isScrolling = false;
  }, 250);
  ```
  ```css
  /* Supprimer le selecteur :global(*) - le parent suffit */
  .content-area.is-scrolling {
    pointer-events: none;
  }
  /* SUPPRIMER: .content-area.is-scrolling :global(*) { ... } */
  ```
- **Impact**: MEDIUM - reduit les repaints durant momentum scroll
- **Non-regression**: Verifier que les clics fonctionnent bien 250ms apres arret du scroll

#### PERF-4: Virtual table scroll sans pointer-events trick (MEDIUM)

- **Fichier**: `src/lib/components/settings/memory/MemoryList.svelte:762-770,810`
- **Probleme**: La virtual list a son propre scroll container (`.virtual-list-viewport overflow-y: scroll`). Le trick `pointer-events: none` du layout s'applique au scroll PRINCIPAL, mais PAS au scroll interne de la virtual list. Les `.virtual-row { transition: background-color 0.15s ease; }` (ligne 810) se declenchent a chaque row survol pendant le scroll interne.
- **Fix**: Ajouter le meme trick pointer-events sur le container virtual table + `contain: layout style` sur `.virtual-table-body`:
  ```css
  .virtual-table-body {
    height: 400px;
    overflow: hidden;
    contain: layout style;
  }
  ```
  Et dans le script, ajouter un listener scroll sur la virtual list qui desactive les pointer-events des rows.
  Alternative plus simple: supprimer la transition sur `.virtual-row` et utiliser un hover pur sans animation:
  ```css
  .virtual-row {
    /* AVANT: transition: background-color 0.15s ease; */
    /* APRES: pas de transition - hover instantane */
  }
  .virtual-row:hover {
    background: var(--color-bg-hover);
  }
  ```
- **Impact**: MEDIUM - section Memory seulement
- **Non-regression**: Verifier que le hover fonctionne sur les lignes

#### PERF-5: PromptList search sans debounce (MEDIUM)

- **Fichier**: `src/lib/components/settings/prompts/PromptList.svelte:115`
- **Probleme**: `oninput={(e) => (searchQuery = e.currentTarget.value)}` met a jour directement `searchQuery` qui declenche `$derived.by(filteredPrompts)` a chaque caractere. Sur 50+ prompts, chaque frappe cause ~5-10ms de filtre + re-render de la grille. Si l'utilisateur tape vite ou scrolle simultanement = jank.
- **Contexte**: MemoryList a deja un debounce 300ms (ligne 350-356). Incoherence.
- **Fix**: Ajouter debounce comme MemoryList:
  ```typescript
  let searchTimeout: ReturnType<typeof setTimeout>;
  function handleSearchInput(event: Event & { currentTarget: HTMLInputElement }): void {
    searchQuery = event.currentTarget.value;
    clearTimeout(searchTimeout);
    searchTimeout = setTimeout(() => {
      debouncedQuery = searchQuery;
    }, 300);
  }
  ```
  Et utiliser `debouncedQuery` dans le `$derived.by`.
- **Impact**: MEDIUM - section Prompts avec 10+ items
- **Non-regression**: Verifier que la recherche filtre correctement apres 300ms

#### PERF-6: Sidebar `transition: width` pendant scroll (LOW)

- **Fichier**: `src/styles/global.css:226`
- **Probleme**: `.sidebar { transition: width var(--transition-base); }` est permanent, pas uniquement lors du toggle collapse. Si le sidebar est en train de transitionner (rare), le layout de toute la page est recalcule a chaque frame.
- **Fix**: Aucun fix necessaire. La transition ne se produit que sur toggle (pas pendant scroll). Low priority.
- **Impact**: LOW - negligeable en pratique

#### PERF-7: `contain: content` sous-optimal sur `.content-area` (LOW)

- **Fichier**: `src/routes/settings/+layout.svelte:293`
- **Probleme**: `contain: content` est equivalent a `contain: layout style paint` mais le commentaire dit "instead of will-change". C'est correct et suffisant.
- **Verdict**: Pas de changement necessaire. Le containment est deja adequat.

**Gain Phase 0 estime**: Scroll nettement plus fluide dans toutes les sections. Les fixes PERF-1 a PERF-5 ciblent les causes reelles identifiees.

---

### Phase 1: Extraction Composants UI Partages (Quick Win)

**Objectif**: Eliminer la duplication verifiee entre sections.

#### OPT-1: Creer ErrorBanner.svelte

- **Fichier a creer**: `src/lib/components/ui/ErrorBanner.svelte`
- **Fichiers a modifier**: AgentSettings.svelte, PromptSettings.svelte
- **Changement**: Extraire le pattern error-banner en composant reutilisable
- **Interface**:
  ```typescript
  interface Props {
    message: string;
    onDismiss: () => void;
    dismissLabel?: string; // default: i18n('common_close')
  }
  ```
- **Gain**: -28 lignes par section utilisant le pattern (2 sections = -56 lignes)
- **Risque regression**: Faible - remplacement template 1:1
- **Non-regression**: Verifier visuellement l'affichage des erreurs dans Agents et Prompts

#### OPT-2: Creer SettingsSectionHeader.svelte

- **Fichier a creer**: `src/lib/components/settings/SettingsSectionHeader.svelte`
- **Fichiers a modifier**: AgentSettings.svelte, PromptSettings.svelte
- **Changement**: Extraire le header commun (titre + HelpButton + description + bouton Create)
- **Interface**:
  ```typescript
  interface Props {
    titleKey: string;
    descriptionKey: string;
    helpTitleKey: string;
    helpDescriptionKey: string;
    helpTutorialKey: string;
    createLabelKey: string;
    onCreate: () => void;
  }
  ```
- **Gain**: -38 lignes par section (2 sections = -76 lignes)
- **Risque regression**: Faible
- **Non-regression**: Verifier layout header dans Agents et Prompts

#### OPT-3: Creer DeleteConfirmModal.svelte

- **Fichier a creer**: `src/lib/components/ui/DeleteConfirmModal.svelte`
- **Fichiers a modifier**: AgentSettings.svelte, PromptSettings.svelte
- **Changement**: Extraire la modal de confirmation de suppression
- **Interface**:
  ```typescript
  interface Props {
    open: boolean;
    titleKey: string;
    confirmMessageKey: string;
    deleting: boolean;
    deletingLabelKey?: string;
    onConfirm: () => void;
    onCancel: () => void;
  }
  ```
- **Gain**: -22 lignes par section (2 sections = -44 lignes)
- **Risque regression**: Faible
- **Non-regression**: Tester suppression agent + suppression prompt

**Gain Phase 1**: ~176 lignes eliminees, 3 composants reutilisables crees

---

### Phase 2: Decomposition MemorySettings.svelte

**Objectif**: Reduire le composant de 1082 lignes en sous-composants coherents.

#### OPT-4: Extraire EmbeddingConfigCard.svelte

- **Fichier a creer**: `src/lib/components/settings/memory/EmbeddingConfigCard.svelte`
- **Source**: MemorySettings.svelte lignes ~335-394 (template) + CSS associe
- **Changement**: Encapsuler l'affichage et l'edition de la config embedding
- **Benefice**: Separation config vs stats vs test
- **Risque regression**: Moyen - logique d'etat a transmettre via props
- **Non-regression**: Verifier affichage config embedding, sauvegarde, edition

#### OPT-5: Extraire EmbeddingTestCard.svelte

- **Fichier a creer**: `src/lib/components/settings/memory/EmbeddingTestCard.svelte`
- **Source**: MemorySettings.svelte lignes ~396-468 (template) + CSS associe
- **Changement**: Isoler la fonctionnalite de test embedding
- **Benefice**: Composant autonome testable separement
- **Risque regression**: Faible - fonctionnalite isolee
- **Non-regression**: Tester l'envoi de texte de test et l'affichage du resultat

#### OPT-6: Extraire MemoryStatsCard.svelte

- **Fichier a creer**: `src/lib/components/settings/memory/MemoryStatsCard.svelte`
- **Source**: MemorySettings.svelte lignes ~470-539 (template) + CSS associe
- **Changement**: Isoler l'affichage des statistiques memoire
- **Benefice**: Composant purement presentationnel
- **Risque regression**: Faible - affichage seul
- **Non-regression**: Verifier affichage statistiques avec et sans donnees

**Gain Phase 2**: MemorySettings.svelte passe de ~1082 a ~500-600 lignes. 3 composants semantiquement clairs.

**Note d'honnetete**: Le CSS (440 lignes) sera distribue dans les sous-composants. Le total de lignes ne diminue pas significativement - c'est la **lisibilite et la maintenabilite** qui s'ameliorent, pas le volume.

---

### Phase 3: Centralisation Validation Backend

**Objectif**: Eliminer la duplication des fonctions de validation similaires.

#### OPT-7: Extraire validate_trimmed_name() dans validation helpers

- **Fichier a modifier**: `src-tauri/src/security/validation_helper.rs` (ou nouveau helper)
- **Fichiers a modifier**: `agent.rs`, `mcp.rs`
- **Changement**: Remplacer `validate_agent_name()` et `validate_mcp_server_display_name()` par un appel commun
- **Interface**:
  ```rust
  pub fn validate_trimmed_name(
      value: &str,
      field_name: &str,
      max_len: usize,
  ) -> Result<String, String>
  ```
- **Gain**: -30 lignes, logique de validation centralisee
- **Risque regression**: Faible - meme logique exacte, tests existants
- **Non-regression**: `cargo test` - tests de validation agent + MCP

#### OPT-8: Standardiser logging (mineur)

- **Fichier**: `embedding.rs:750`
- **Changement**: Remplacer `info!(query = %upsert_query, ...)` par `info!(operation = "upsert", memory_id = %id, ...)`
- **Benefice**: Ne pas logger les queries completes (securite)
- **Risque regression**: Nul
- **Non-regression**: Aucun test impacte

---

### Phase 4: Harmonisation Patterns (Nice to Have)

**Objectif**: Ameliorer la coherence entre sections sans sur-ingenierie.

#### OPT-9: Harmoniser gestion d'erreurs MemorySettings

- **Fichier**: `MemorySettings.svelte`
- **Changement**: Remplacer le pattern `message = { type, text }` local par le composant `ErrorBanner` (OPT-1) pour la coherence avec Agents/Prompts
- **Benefice**: UX coherente entre toutes les sections
- **Risque regression**: Faible
- **Non-regression**: Tester erreurs dans config embedding

#### OPT-10: Ajouter gestion erreur dans ValidationSettings

- **Fichier**: `ValidationSettings.svelte`
- **Changement**: Ajouter affichage d'erreur (actuellement absent)
- **Benefice**: Les erreurs de sauvegarde de config validation ne sont pas visibles par l'utilisateur
- **Risque regression**: Nul - ajout sans modification existante
- **Non-regression**: Provoquer une erreur de sauvegarde et verifier l'affichage

---

## Optimisations REJETEES (avec justification)

### REJ-1: CRUDContainer generique
- **Raison**: Over-engineering. Seulement 3 sections CRUD (Agents, Prompts, Memory). Chacune a ses specificites (Memory = embedding config + stats, pas un CRUD simple). Le cout de l'abstraction depasse le gain.
- **Reconsiderer si**: Une 4e+ section CRUD est ajoutee.

### REJ-2: Normalisation forcee de tous les stores
- **Raison**: Les stores utilisent des patterns differents pour des raisons valides:
  - Agents/Prompts: CRUD factory (entites simples avec persistence) - correct
  - Memory: state local (config embedding + stats + test != CRUD simple) - justifie
  - LLM: pure functions (logique de providers complexe, pas juste CRUD) - justifie
  - MCP: state wrapper (lifecycle serveurs != CRUD) - justifie
- **Reconsiderer si**: Des bugs de coherence d'etat apparaissent.

### REJ-3: Couche API centralisee complete
- **Raison**: Les appels `invoke()` sont simples et directs. Ajouter une couche intermediaire (`api/memory.ts`, `api/agent.ts`) ajoute de l'indirection sans gain fonctionnel. Le code fonctionne.
- **Reconsiderer si**: Des appels invoke() deviennent complexes (retry, cache, batching).

### REJ-4: QueryBuilder backend
- **Raison**: Les patterns UPDATE SET sont legèrement differents entre commandes. Un QueryBuilder ajouterait de la complexite pour un gain marginal. Les commandes sont deja bien securisees avec bind/serialize_for_query.
- **Reconsiderer si**: Nouvelles commandes CRUD avec patterns similaires.

### REJ-5: Refactoring lazy loading uniforme
- **Raison**: Le lazy loading est applique aux 2 sections les plus lourdes (Agents, Memory). Les autres sont legeres. Uniformiser n'apporte pas de gain mesurable.

### REJ-6: CSS utilities globales (grid-3, grid-4)
- **Raison**: Les grilles sont utilisees 2-3 fois dans des contextes differents. Des classes utilitaires globales ajouteraient du bruit CSS global pour un gain minimal.

---

## Dependencies

### Mises a Jour Recommandees (mineures)

| Package | Actuel | Recommande | Breaking Changes | Priorite |
|---|---|---|---|---|
| @sveltejs/kit | 2.49.1 | 2.53.0 | Non | Basse |
| svelte | 5.49.1 | 5.53.2 | Non | Basse |
| @lucide/svelte | 0.563.1 | 0.575.0 | Non | Basse |
| marked | 17.0.1 | 17.0.3 | Non | Basse |

**Note**: Aucune mise a jour critique. Le backend (Cargo.toml) est a jour.

---

## Verification Non-Regression

### Tests Existants
- `npm run test` - Stores agents/prompts/llm couverts
- `cargo test` - 932 tests, validation/securite couverts
- `npm run check` - TypeScript/Svelte type checking
- `npm run lint` - ESLint

### Tests a Ajouter
- Aucun test unitaire supplementaire requis pour Phases 1-3 (extraction sans changement logique)
- Pour OPT-10: Ajouter test de gestion d'erreur dans ValidationSettings si store existe

### Verification Manuelle
- [ ] **Scroll fluide** dans section Agents (grille de cards)
- [ ] **Scroll fluide** dans section Prompts (grille de cards)
- [ ] **Scroll fluide** dans section Memory (virtual table)
- [ ] **Scroll fluide** dans section LLM (providers + models)
- [ ] **Scroll fluide** dans section MCP
- [ ] Recherche prompts avec debounce (filtre apres 300ms)
- [ ] Hover sur virtual-row Memory fonctionne
- [ ] Clics fonctionnent apres arret du scroll (250ms timeout)
- [ ] Affichage erreur dans section Agents
- [ ] Affichage erreur dans section Prompts
- [ ] Suppression agent (confirmation modal)
- [ ] Suppression prompt (confirmation modal)
- [ ] Section Memory: config + stats + test embedding
- [ ] Section Validation: sauvegarde config

---

## Estimation

| Optimisation | Effort | Impact | Priorite |
|---|---|---|---|
| **PERF-1: Card GPU layer** | Trivial | Haut | **P0** |
| **PERF-2: Prompt grid contain** | Trivial | Haut | **P0** |
| **PERF-3: Scroll timeout + global(*)** | Faible | Moyen | **P0** |
| **PERF-4: Virtual row transition** | Faible | Moyen | **P0** |
| **PERF-5: Search debounce PromptList** | Faible | Moyen | **P0** |
| OPT-1: ErrorBanner | Faible | Moyen | P1 |
| OPT-2: SettingsSectionHeader | Faible | Moyen | P1 |
| OPT-3: DeleteConfirmModal | Faible | Moyen | P1 |
| OPT-4: EmbeddingConfigCard | Moyen | Haut | P2 |
| OPT-5: EmbeddingTestCard | Faible | Moyen | P2 |
| OPT-6: MemoryStatsCard | Faible | Moyen | P2 |
| OPT-7: validate_trimmed_name | Faible | Faible | P3 |
| OPT-8: Logging securise | Trivial | Faible | P3 |
| OPT-9: Error handling Memory | Faible | Moyen | P4 |
| OPT-10: Error handling Validation | Faible | Moyen | P4 |

---

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|---|---|---|---|
| `translateZ(0)` cree trop de GPU layers | Faible | Moyen | Applique uniquement sur `.card`, pas partout. Tester avec DevTools > Layers |
| Scroll timeout 250ms trop long = clic rate | Faible | Moyen | 250ms est le seuil standard. Ajustable si necessaire |
| Suppression `transition` virtual-row = UX degradee | Tres faible | Faible | Le hover instantane est plus reactif, pas degrade |
| Regression UI apres extraction composants | Faible | Moyen | Verification visuelle manuelle |
| Props drilling dans sous-composants Memory | Moyenne | Faible | Garder state dans parent, passer via props |
| Inconsistance i18n keys dans composants partages | Faible | Faible | Parametres i18n keys en props |

---

## Avertissements d'Honnetete (Metacognition)

1. **Le chiffre "42 problemes"** de l'analyse initiale inclut beaucoup de "nice to have" et de bruit. Les problemes reellement impactants sont ~10-12.

2. **La duplication verifiee est de ~168 lignes**, pas ~850 comme suggere initialement. Le gain reel est modeste.

3. **La decomposition de MemorySettings ne reduit pas le volume total** - elle distribue le code. Le gain est en lisibilite, pas en lignes.

4. **Le backend est sain**. Les optimisations backend (OPT-7, OPT-8) sont mineures et optionnelles.

5. **Certaines "incoherences" sont justifiees**: les sections ont des besoins differents (CRUD simple vs config complexe vs lifecycle). Forcer l'uniformite serait de l'over-engineering.

6. **Le code fonctionne**. Aucune de ces optimisations ne corrige un bug ou un probleme de performance. Ce sont des ameliorations de qualite de code pour faciliter la maintenance future.

---

## Statut d'Implementation

| Phase | Items | Statut | Commit | Date |
|-------|-------|--------|--------|------|
| Phase 0 | PERF-1 a PERF-5 | DONE | `7f2c37e` | 2026-02-22 |
| Phase 1 | OPT-1 a OPT-3 | DONE | `bc58204` | 2026-02-22 |
| Phase 2 | OPT-4 a OPT-6 | DONE | `c87bd85` | 2026-02-22 |
| Phase 3 | OPT-7, OPT-8 (N/A) | DONE | `0188ae8` | 2026-02-22 |
| Phase 4 | OPT-9, OPT-10 | DONE | `bde6149` | 2026-02-22 |

### Notes d'implementation

- **OPT-8 (N/A)**: Le pattern `info!(query = %upsert_query, ...)` mentionne dans l'audit n'existe pas dans le code actuel. Le logging dans `embedding.rs` est deja structure correctement (provider/model/dimension, pas de query loggee). Classe comme faux positif.
- **OPT-9**: En plus de l'utilisation d'ErrorBanner, correction d'un bug ou les erreurs de load/refresh/delete n'etaient jamais visibles (le message etait conditionne a `showConfigModal` qui etait ferme).
- **OPT-10**: Ajout de try/catch autour du `onMount` pour capturer les erreurs de `loadSettings()` qui pouvaient etre des rejections non gerees.

## Prochaines Etapes

1. [x] Valider ce plan
2. [x] **Phase 0: Scroll Performance** (PERF-1 a PERF-5) - PRIORITAIRE
3. [x] Phase 1: Extraction composants UI (OPT-1 a OPT-3)
4. [x] Phase 2: Decomposition MemorySettings (OPT-4 a OPT-6)
5. [x] Phase 3: Validation backend (OPT-7, OPT-8)
6. [x] Phase 4: Harmonisation (OPT-9, OPT-10)
7. [ ] Verification non-regression complete
8. [ ] Mettre a jour REMEDIATION-STATUS.md

---

## References

### Code Analyse
- `src/routes/settings/` - 9 pages routes
- `src/lib/components/settings/` - 29 composants
- `src-tauri/src/commands/` - 9 fichiers commandes
- `src/lib/stores/` - llm.ts, validation-settings.ts

### Documentation Consultee
- `docs/security-audits/SA-010-settings-forms-quality.md`
- `docs/security-audits/REMEDIATION-STATUS.md`
- `docs/FRONTEND_SPECIFICATIONS.md`
- `docs/ARCHITECTURE_DECISIONS.md`
- `.claude/learning/patterns.yml`
- `.claude/learning/errors.yml`

### Metacognition
- Biais verifies: ancrage, completion de pattern, remplissage de liste
- Confiance: Haute sur Phase 1-2, Moyenne sur Phase 3-4
- Alternatives rejetees documentees avec justification
