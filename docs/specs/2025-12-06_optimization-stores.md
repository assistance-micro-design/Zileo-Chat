# Plan d'Optimisation - Stores (State Management)

## Metadata
- **Date**: 2025-12-06
- **Domaine**: stores
- **Stack**: SvelteKit 2.49.0 + Svelte 5.43.14 (writable/derived stores natifs)
- **Impact estime**: Performance / Maintenabilite / Stabilite

## Resume Executif

Les stores de Zileo-Chat-3 constituent une architecture solide avec 14 stores couvrant ~4,230 lignes de code. Cependant, trois axes d'amelioration majeurs ont ete identifies: (1) risques de memory leaks dans les stores event-driven, (2) duplication de code significative (~400 lignes extractibles), et (3) patterns inconsistants entre stores. Ce plan propose 10 optimisations priorisees pour renforcer la stabilite et la maintenabilite sans ajouter de nouvelles fonctionnalites.

---

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Pattern | Complexite | Points d'attention |
|---------|--------|---------|-----------|-------------------|
| streaming.ts | 752 | Writable + Events | Tres haute | processChunk() 161 lignes, memory leak risk |
| llm.ts | 522 | Pure Functions | Haute | Pas de store reactif central |
| mcp.ts | 415 | Pure Functions | Haute | Meme pattern que llm.ts |
| workflows.ts | 378 | Hybrid | Haute | Duplication pure + reactive |
| validation.ts | 295 | Writable + Events | Haute | Memory leak risk (unlistener) |
| prompts.ts | 286 | Writable + Derived | Moyenne | Pattern similaire a agents.ts |
| agents.ts | 281 | Writable + Derived | Moyenne | Modele de reference |
| tokens.ts | 256 | Writable + Derived | Moyenne | Calculs derives complexes |
| userQuestion.ts | 253 | Writable + Events | Moyenne | Memory leak risk (unlistener) |
| activity.ts | 223 | Writable + Derived | Moyenne | Deduplication couteuse |
| onboarding.ts | 176 | Factory Pattern | Basse | Correct |
| validation-settings.ts | 158 | Factory Pattern | Basse | Correct |
| locale.ts | 109 | Factory Pattern | Basse | Correct |
| theme.ts | 90 | Factory Pattern | Basse | toggle() inefficace |

**Total: 14 stores, ~4,230 lignes**

### Patterns Identifies

1. **Writable + Derived (Moderne)**: agents.ts, tokens.ts, prompts.ts
   - Store interne writable + facade objet avec actions
   - Derived stores pour consommation optimisee
   - **Recommande comme pattern canonique**

2. **Pure Functions + Factory (Stateless)**: llm.ts, mcp.ts
   - Fonctions async pour Tauri IPC
   - Reducers purs sans effets de bord
   - Pas de single source of truth

3. **Event-Driven avec Listeners**: streaming.ts, validation.ts, userQuestion.ts
   - Tauri event listeners avec UnlistenFn
   - **Risque de memory leaks si cleanup non appele**

4. **Hybrid (Pure + Reactive)**: workflows.ts
   - Deux implementations paralleles (duplication)
   - Pure functions + reactive store separement

5. **Custom Factory (Persistence)**: theme.ts, locale.ts, onboarding.ts
   - localStorage integre
   - Pattern simple et efficace

### Metriques Actuelles

| Metrique | Valeur | Evaluation |
|----------|--------|------------|
| Lignes totales | ~4,230 | Moderee |
| Ratio duplication | ~12% | A reduire |
| Couverture tests | ~25% | Faible (4/14 stores) |
| Memory leak risk | HIGH | 3 stores concernes |
| Documentation | BON | JSDoc present partout |
| Type safety | BON | TypeScript strict |

---

## Best Practices (2024-2025)

### Sources Consultees

- [Svelte 5 Migration Guide](https://svelte.dev/docs/svelte/v5-migration-guide) - Official Svelte docs
- [Mainmatter: Runes and Global State](https://mainmatter.com/blog/2025/03/11/global-state-in-svelte-5/) - March 2025
- [Loopwerk: Refactoring Stores to Runes](https://www.loopwerk.io/articles/2025/svelte-5-stores/) - 2025
- [SvelteKit State Management](https://svelte.dev/docs/kit/state-management) - Official docs
- [Tauri Store Plugin](https://v2.tauri.app/plugin/store/) - v2 docs

### Patterns Recommandes (Svelte 5)

1. **Runes pour nouveau code**: `$state`, `$derived`, `$effect` offrent 2-3x les performances des stores
2. **Migration bottom-up**: Migrer composants enfants d'abord, puis parents
3. **$derived > $effect**: 90% des cas utilisent $derived, pas $effect
4. **Context pour SSR**: setContext/getContext pour etat global SSR-safe
5. **Coexistence stores + runes**: Migration progressive possible

### Anti-Patterns a Eviter

1. **Module-level $state avec SSR**: Fuite de donnees entre requetes
2. **$effect pour valeurs derivees**: Utiliser $derived a la place
3. **subscribe/unsubscribe pour lecture unique**: Utiliser get() a la place
4. **Giant switch statements**: Extraire en handlers separes

---

## Contraintes du Projet

Decisions existantes a respecter (source: CLAUDE.md, ARCHITECTURE_DECISIONS.md):

| Contrainte | Description | Source |
|------------|-------------|--------|
| Pas de librairie externe | Redux, Zustand, Pinia exclus par design | ARCHITECTURE_DECISIONS.md |
| Tauri naming convention | snake_case Rust -> camelCase JS (auto) | CLAUDE.md |
| $types alias obligatoire | Jamais $lib/types ou ../types | CLAUDE.md |
| Type sync manuel | Rust <-> TypeScript non auto-genere | CLAUDE.md |
| Cleanup lifecycle | Event listeners doivent etre nettoyes | Code patterns |

---

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-1: Mise a jour Svelte 5.43.14 -> 5.45.6

- **Fichiers**: `package.json`
- **Changement**: `npm update svelte@5.45.6`
- **Benefice**: Bug fixes, nouvelles fonctionnalites (fork API), performances
- **Risque regression**: Aucun (zero breaking changes documentes)
- **Effort**: 10 minutes
- **Validation**: `npm run check && npm run test && npm run build`

#### OPT-2: Creer utilitaire standardise de gestion d'erreurs

- **Fichiers**: Nouveau `src/lib/utils/error.ts`
- **Changement**:
  ```typescript
  /**
   * Extracts error message from unknown error type.
   * Standardizes error handling across all stores.
   */
  export function getErrorMessage(error: unknown): string {
    if (error instanceof Error) return error.message;
    return String(error);
  }
  ```
- **Benefice**: Pattern uniforme, messages d'erreur preserves
- **Risque regression**: Aucun (nouvelle fonction)
- **Effort**: 30 minutes (creation + application dans 8 stores)
- **Validation**: Tests unitaires pour la fonction

#### OPT-3: Completer exports barrel dans index.ts

- **Fichiers**: `src/lib/stores/index.ts`
- **Changement**: Ajouter exports manquants
  ```typescript
  export * from './validation';
  export * from './validation-settings';
  ```
- **Benefice**: Imports consistants depuis `$lib/stores`
- **Risque regression**: Aucun
- **Effort**: 15 minutes
- **Validation**: `npm run check`

#### OPT-4: Corriger theme.ts toggle() subscription gaspillee

- **Fichiers**: `src/lib/stores/theme.ts:53-59`
- **Changement**:
  ```typescript
  // Avant (inefficace)
  toggle: (): void => {
    let currentTheme: Theme = 'light';
    const unsubscribe = subscribe((value) => {
      currentTheme = value;
    });
    unsubscribe();
    // ...
  }

  // Apres (optimal)
  toggle: (): void => {
    const currentTheme = get(store);
    const nextTheme: Theme = currentTheme === 'light' ? 'dark' : 'light';
    // ...
  }
  ```
- **Benefice**: Elimine subscription/unsubscription inutile
- **Risque regression**: Faible
- **Effort**: 10 minutes
- **Validation**: Test manuel du toggle theme

---

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-5: Corriger risques de memory leaks (CRITIQUE)

- **Fichiers**:
  - `src/lib/stores/streaming.ts:194`
  - `src/lib/stores/validation.ts:94`
  - `src/lib/stores/userQuestion.ts:71`
- **Changement**:
  ```typescript
  // Avant (module-level, risque accumulation)
  let unlisteners: UnlistenFn[] = [];

  // Apres (instance-level avec safety check)
  const createEventStore = () => {
    let unlisteners: UnlistenFn[] = [];
    let isInitialized = false;

    return {
      async init() {
        if (isInitialized) {
          console.warn('Store already initialized, cleaning up first');
          await this.cleanup();
        }
        isInitialized = true;
        // ... setup listeners
      },
      async cleanup() {
        for (const unlisten of unlisteners) {
          unlisten();
        }
        unlisteners = [];
        isInitialized = false;
      }
    };
  };
  ```
- **Benefice**: Prevention des fuites memoire, calls multiples securises
- **Risque regression**: Moyen (modifier pattern existant)
- **Effort**: 2 heures
- **Tests requis**:
  - [ ] Multiple init() calls n'accumulent pas de listeners
  - [ ] cleanup() supprime correctement tous les listeners
  - [ ] Component unmount declenche cleanup
- **Validation**: `npm run test` + test manuel streaming long

#### OPT-6: Supprimer duplication workflows.ts

- **Fichiers**: `src/lib/stores/workflows.ts`
- **Changement**:
  - Supprimer pure functions (lignes 48-237)
  - Garder uniquement workflowStore reactif (lignes 242-378)
  - Marquer exports deprecated si utilises ailleurs
- **Phases**:
  1. Grep usages des pure functions dans le projet
  2. Migrer usages vers workflowStore
  3. Supprimer pure functions
  4. Mettre a jour tests
- **Benefice**: -190 lignes, elimination confusion patterns
- **Risque regression**: Moyen (changement d'API potentiel)
- **Effort**: 1.5 heures
- **Tests requis**:
  - [ ] Tests workflows existants passent
  - [ ] workflowStore maintient meme comportement
- **Validation**: `npm run test -- workflows`

#### OPT-7: Refactorer processChunk() en handlers separes

- **Fichiers**: `src/lib/stores/streaming.ts:205-366`
- **Changement**:
  ```typescript
  // Avant: 161-line switch statement
  function processChunk(chunk: StreamChunk): void {
    store.update((s) => {
      switch (chunk.chunk_type) {
        case 'token': return { ...s, content: s.content + chunk.content };
        case 'tool_start': return { ...s, tools: [...s.tools, {...}] };
        // ... 11 cases total
      }
    });
  }

  // Apres: Handlers extraits
  type ChunkHandler = (state: StreamingState, chunk: StreamChunk) => StreamingState;

  const chunkHandlers: Record<string, ChunkHandler> = {
    token: (s, c) => ({ ...s, content: s.content + (c.content ?? ''), tokensReceived: s.tokensReceived + 1 }),
    tool_start: (s, c) => ({ ...s, tools: [...s.tools, { name: c.tool ?? 'unknown', status: 'running', startedAt: Date.now() }] }),
    tool_end: (s, c) => ({ ...s, tools: s.tools.map(t => t.name === c.tool && t.status === 'running' ? { ...t, status: 'completed', duration: c.duration } : t) }),
    // ... autres handlers
  };

  function processChunk(chunk: StreamChunk): void {
    const handler = chunkHandlers[chunk.chunk_type];
    if (handler) {
      store.update((s) => handler(s, chunk));
    }
  }
  ```
- **Benefice**: Testabilite, lisibilite, extensibilite
- **Risque regression**: Moyen (refactoring logique complexe)
- **Effort**: 2 heures
- **Tests requis**:
  - [ ] Chaque handler teste individuellement
  - [ ] Processing complet fonctionne toujours
  - [ ] Types inconnus geres gracieusement
- **Validation**: `npm run test -- streaming`

#### OPT-8: Extraire factory CRUD pour agents/prompts

- **Fichiers**:
  - Nouveau `src/lib/stores/factory/createCRUDStore.ts`
  - Refactorer `src/lib/stores/agents.ts`
  - Refactorer `src/lib/stores/prompts.ts`
- **Changement**:
  ```typescript
  interface CRUDStoreConfig<T, TCreate, TUpdate, TSummary> {
    name: string;
    commands: {
      list: string;
      get: string;
      create: string;
      update: string;
      delete: string;
    };
    idField: keyof TSummary;
  }

  function createCRUDStore<T, TCreate, TUpdate, TSummary>(
    config: CRUDStoreConfig<T, TCreate, TUpdate, TSummary>
  ) {
    const initialState = { items: [], selectedId: null, loading: false, error: null, formMode: null, editing: null };
    const store = writable(initialState);

    return {
      subscribe: store.subscribe,
      async loadItems() { /* generique */ },
      async create(data: TCreate) { /* generique */ },
      async update(id: string, data: TUpdate) { /* generique */ },
      async delete(id: string) { /* generique */ },
      select(id: string | null) { /* generique */ },
      openCreateForm() { /* generique */ },
      openEditForm(id: string) { /* generique */ },
      closeForm() { /* generique */ },
      // ...
    };
  }

  // Usage
  export const agentStore = createCRUDStore<AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary>({
    name: 'agent',
    commands: { list: 'list_agents', get: 'get_agent_config', create: 'create_agent', update: 'update_agent', delete: 'delete_agent' },
    idField: 'id'
  });
  ```
- **Benefice**: -200 lignes duplication, maintenance simplifiee
- **Risque regression**: Moyen (changement de structure)
- **Prerequis**: OPT-2 (error utility)
- **Effort**: 4 heures
- **Tests requis**:
  - [ ] Factory cree store avec API correcte
  - [ ] Tests agents passent avec nouveau store
  - [ ] Tests prompts passent avec nouveau store
- **Validation**: `npm run test`

---

### Nice to Have (Impact faible, Effort faible)

#### OPT-9: Auditer derived stores inutilises

- **Fichiers**: Tous les stores
- **Changement**: Verifier usage dans composants de:
  - `agentCount` (agents.ts:246)
  - `hasRunningTools` (streaming.ts:669)
  - Autres derived stores peu utilises
- **Benefice**: Reduction code mort
- **Risque regression**: Faible
- **Effort**: 1 heure
- **Validation**: Grep + review manuelle

#### OPT-10: Documenter pattern canonique

- **Fichiers**: `docs/ARCHITECTURE_DECISIONS.md`
- **Changement**: Ajouter decision sur pattern store recommande
  - Pattern agent model = canonique
  - Pure functions (llm, mcp) = acceptable pour async-only
  - Hybrid (workflows) = deprecie
- **Benefice**: Guide pour futurs developpements
- **Risque regression**: Aucun
- **Effort**: 30 minutes

---

### Differe (Impact variable, Effort eleve)

| Optimisation | Raison du report | Conditions de reprise |
|--------------|------------------|----------------------|
| Svelte 5 runes migration | Effort 8-16h, necessite planification | Phase 7+ ou nouveau projet |
| Tauri channels vs events | Requiert changements backend | Quand latence streaming devient critique |
| Race conditions AbortController | Complexe, cas edge rares | Si bugs utilisateurs reportes |
| @tauri-apps/plugin-store | Nice-to-have, localStorage suffit | Quand secrets/config critiques |

---

## Dependencies

### Mises a Jour Recommandees

| Package | Actuel | Recommande | Breaking Changes |
|---------|--------|------------|------------------|
| svelte | ^5.43.14 | 5.45.6 | Non - corrections bugs |
| @tauri-apps/api | ^2.9.0 | 2.9.1 | Non - corrections mineures |

### Nouvelles Dependencies

Aucune nouvelle dependance requise pour ce plan.

---

## Verification Non-Regression

### Tests Existants

| Fichier | Scope |
|---------|-------|
| `src/lib/stores/__tests__/workflows.test.ts` | Workflows CRUD |
| `src/lib/stores/__tests__/streaming.test.ts` | Streaming state |
| `src/lib/stores/__tests__/agents.test.ts` | Agents CRUD |
| `src/lib/stores/__tests__/llm.test.ts` | LLM state |

**Couverture actuelle**: ~25% (4/14 stores)

### Tests a Ajouter

Pour OPT-5 (memory leaks):
- [ ] Test init() appele plusieurs fois n'accumule pas listeners
- [ ] Test cleanup() libere tous les listeners
- [ ] Test store fonctionne apres cleanup + re-init

Pour OPT-7 (processChunk):
- [ ] Test handler 'token' ajoute contenu
- [ ] Test handler 'tool_start' ajoute tool
- [ ] Test handler 'tool_end' complete tool
- [ ] Test handler inconnu ne crash pas

Pour OPT-8 (CRUD factory):
- [ ] Test factory retourne store avec API complete
- [ ] Test loadItems() appelle bon command
- [ ] Test create() appelle command + reload

### Benchmarks

```bash
# Avant optimisation (baseline)
npm run build
# Taille bundle: X KB

# Apres optimisation
npm run build
# Taille bundle: Y KB (objectif: <= X)

# Performance streaming (si applicable)
# Mesurer tokens/sec avant et apres OPT-7
```

---

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-1: Update Svelte | 10min | Medium | P1 |
| OPT-2: Error utility | 30min | Medium | P1 |
| OPT-3: Index exports | 15min | Low | P1 |
| OPT-4: theme toggle | 10min | Low | P1 |
| OPT-5: Memory leaks | 2h | HIGH | P0 |
| OPT-6: Workflows dedup | 1.5h | HIGH | P2 |
| OPT-7: processChunk | 2h | HIGH | P2 |
| OPT-8: CRUD factory | 4h | HIGH | P2 |
| OPT-9: Audit derived | 1h | Low | P3 |
| OPT-10: Document pattern | 30min | Low | P3 |

**Total estime**: ~12h (P0-P2)

---

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle | Moyenne | Eleve | Tests existants + nouveaux tests |
| Performance degradee | Faible | Moyen | Benchmarks avant/apres |
| Breaking changes Svelte | Tres faible | Eleve | Version mineure uniquement |
| Memory leak non detecte | Moyenne | Eleve | Tests lifecycle explicites |

---

## Prochaines Etapes

1. [ ] **Valider ce plan** avec l'utilisateur
2. [ ] **Executer OPT-5** (P0 - memory leaks) en priorite
3. [ ] **Executer OPT-1,2,3,4** (P1 - quick wins) en batch
4. [ ] **Mesurer impact** avec tests et benchmarks
5. [ ] **Executer OPT-6,7** (P2 - strategic) sequentiellement
6. [ ] **Executer OPT-8** (P2 - CRUD factory) si gains confirmes
7. [ ] **Executer OPT-9,10** (P3 - cleanup)

---

## References

### Code Analyse
- `src/lib/stores/*.ts` (14 fichiers, ~4,230 lignes)
- `src/lib/stores/__tests__/*.test.ts` (4 fichiers)

### Documentation Consultee
- `CLAUDE.md` - Patterns etablis
- `docs/ARCHITECTURE_DECISIONS.md` - Decisions architecturales
- `docs/FRONTEND_SPECIFICATIONS.md` - Specifications frontend
- `docs/TECH_STACK.md` - Stack technique

### Sources Externes
- [Svelte 5 Migration Guide](https://svelte.dev/docs/svelte/v5-migration-guide)
- [Mainmatter: Runes and Global State](https://mainmatter.com/blog/2025/03/11/global-state-in-svelte-5/)
- [Loopwerk: Refactoring Stores to Runes](https://www.loopwerk.io/articles/2025/svelte-5-stores/)
- [SvelteKit State Management](https://svelte.dev/docs/kit/state-management)
- [Tauri Store Plugin](https://v2.tauri.app/plugin/store/)
