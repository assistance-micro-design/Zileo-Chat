# Plan d'Optimisation - Frontend Settings

## Metadata
- **Date**: 2025-12-07
- **Domaine**: frontend/settings
- **Stack**: SvelteKit 2.49.1, Svelte 5.45.6, Vite 5.4.21, Tauri 2.9.3
- **Impact estime**: Performance, Maintenabilite, DX

## Resume Executif

La page Settings (1657 lignes) est fonctionnelle et type-safe mais souffre de monolithicite (30+ variables d'etat, 22 fonctions async) et de duplication de code (~100 lignes de logique modal repetee 3x). Ce plan propose 13 optimisations priorisees : 5 quick wins (4h), 4 strategiques (23h), 2 nice-to-have (1.5h), et 2 differees post-v1.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src/routes/settings/+page.svelte` | 1657 | Tres elevee | Page monolithique, 30+ $state variables |
| `src/lib/stores/llm.ts` | 521 | Moyenne | Pure functions, pas de store wrapper |
| `src/lib/stores/mcp.ts` | 414 | Moyenne | Pure functions, pas de store wrapper |
| `src/lib/components/settings/agents/AgentForm.svelte` | 754 | Elevee | Recharge LLM/MCP deja charges par page |
| `src/lib/components/settings/memory/MemorySettings.svelte` | 1081 | Elevee | Composant isolé, validation minimale |

**Total domaine estime**: ~6000 lignes

### Patterns Identifies

**Positifs (a conserver)**:
- **Pure state management**: Fonctions immutables dans llm.ts, mcp.ts
- **CRUD Factory**: Pattern generique dans agents.ts, prompts.ts
- **Svelte 5 runes**: $state, $derived, $effect bien adoptes
- **Gestion erreurs**: try/catch systematique (~15/20 fonctions)
- **i18n**: Complet et consistent

**Problematiques (a ameliorer)**:
- **Duplication modal logic**: ~100L repetees 3x (MCP, Model, APIKey modals)
- **Duplication save/delete**: Pattern identique 7+ fois
- **3 patterns state**: Inconsistance (states, CRUD factory, isolated)
- **Lazy loading duplique**: AgentForm + page chargent memes donnees
- **Erreurs silencieuses**: catch vides dans AgentForm onMount

### Metriques Actuelles

```bash
# Page Settings
wc -l src/routes/settings/+page.svelte  # 1657 lignes
grep -c "$state" src/routes/settings/+page.svelte  # 30+ variables
grep -c "async function" src/routes/settings/+page.svelte  # 22 fonctions
```

## Best Practices (2024-2025)

### Sources Consultees
- [Svelte 5 Migration Guide](https://svelte.dev/docs/svelte/v5-migration-guide) - Officiel
- [Runes and Global state: do's and don'ts](https://mainmatter.com/blog/2025/03/11/global-state-in-svelte-5/) - Mainmatter
- [Settings UI Design Best Practices](https://www.setproduct.com/blog/settings-ui-design) - SetProduct
- [Tabs UX: Best Practices](https://www.eleken.co/blog-posts/tabs-ux) - Eleken
- [Vite 7 Release Notes](https://vite.dev/releases) - Officiel

### Patterns Recommandes
1. **$state prefere aux stores**: Fine-grained reactivity, bundles plus petits
2. **Max 2 niveaux nested tabs**: Au-dela = confusion UX
3. **Max 4-5 settings par groupe**: Surcharge cognitive sinon
4. **Lazy loading tabs**: Dynamic imports pour sections lourdes
5. **Optimistic updates**: UI reactive + fallback sur erreur

### Anti-Patterns a Eviter
1. **Export direct primitives $state**: Wrapper dans objet obligatoire
2. **Melanger stores et runes**: Choisir un pattern par scope
3. **use:enhance dans Tauri**: Pas de SSR, benefice limite

## Contraintes du Projet

- **Pattern CRUD Factory canonique**: Pour entites persistees (Source: ARCHITECTURE_DECISIONS.md Q20)
- **$types alias obligatoire**: Jamais `$lib/types` (Source: CLAUDE.md)
- **Tauri camelCase/snake_case**: Conversion automatique (Source: CLAUDE.md)
- **Cleanup obligatoire**: Stores event-driven (Source: ARCHITECTURE_DECISIONS.md)
- **i18n complet**: Toutes strings internationalisees (Source: CLAUDE.md)

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-1: Upgrade Vite 5.4.21 vers 7.2.2
- **Fichiers**: `package.json`
- **Changement**:
  ```bash
  npm install vite@7 @sveltejs/vite-plugin-svelte@4.0.4
  ```
- **Benefice**: Performance build +10-20%, securite (5.4 = maintenance only)
- **Risque regression**: Moyen (breaking changes possibles)
- **Validation**:
  ```bash
  npm run build
  npm run check
  ```

#### OPT-2: Extraire createModalController helper
- **Fichiers**:
  - Creer `src/lib/utils/modal.ts`
  - Modifier `src/routes/settings/+page.svelte`
- **Changement**: Factoriser la logique modal repetee
  ```typescript
  // src/lib/utils/modal.ts
  export function createModalController<T>() {
    let show = $state(false);
    let mode = $state<'create' | 'edit'>('create');
    let editing = $state<T | undefined>(undefined);

    return {
      get show() { return show; },
      get mode() { return mode; },
      get editing() { return editing; },
      openCreate() {
        mode = 'create';
        editing = undefined;
        show = true;
      },
      openEdit(item: T) {
        mode = 'edit';
        editing = item;
        show = true;
      },
      close() {
        show = false;
        editing = undefined;
      }
    };
  }
  ```
- **Benefice**: -100 lignes dupliquees, maintenabilite
- **Risque regression**: Faible
- **Validation**: Test manuel ouverture/fermeture modals

#### OPT-3: Extraire createAsyncHandler helper
- **Fichiers**:
  - Creer `src/lib/utils/async.ts`
  - Modifier `src/routes/settings/+page.svelte`
- **Changement**: Factoriser pattern try/catch/finally
  ```typescript
  // src/lib/utils/async.ts
  export function createAsyncHandler<T>(
    operation: () => Promise<T>,
    options: {
      onSuccess?: (result: T) => void;
      onError?: (error: unknown) => void;
      setLoading?: (loading: boolean) => void;
    }
  ): () => Promise<void> {
    return async () => {
      options.setLoading?.(true);
      try {
        const result = await operation();
        options.onSuccess?.(result);
      } catch (error) {
        options.onError?.(error);
      } finally {
        options.setLoading?.(false);
      }
    };
  }
  ```
- **Benefice**: -70 lignes repetees, pattern uniforme
- **Risque regression**: Faible
- **Validation**: `npm run check`

#### OPT-4: Minor dependencies upgrades
- **Fichiers**: `package.json`
- **Changement**:
  ```bash
  npm install lucide-svelte@0.556.0
  ```
- **Benefice**: Bug fixes, nouvelles icones
- **Risque regression**: Tres faible
- **Validation**: `npm run check`

#### OPT-5: Remplacer ComponentType par Component
- **Fichiers**: `src/lib/components/workflow/ActivityFeed.svelte`
- **Changement**:
  ```typescript
  // Avant
  import type { ComponentType } from 'svelte';

  // Apres
  import type { Component } from 'svelte';
  ```
- **Benefice**: Elimine deprecation warning Svelte 5
- **Risque regression**: Tres faible
- **Validation**: `npm run check`

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-6: Refactorer +page.svelte en composants containers
- **Fichiers**:
  - `src/routes/settings/+page.svelte` (modifier)
  - Creer `src/lib/components/settings/MCPSection.svelte` (~400L)
  - Creer `src/lib/components/settings/LLMSection.svelte` (~500L)
  - Creer `src/lib/components/settings/APIKeysSection.svelte` (~200L)
- **Changement**: Extraire chaque section majeure en composant autonome
  ```svelte
  <!-- +page.svelte reduit a ~500L -->
  <script lang="ts">
    import MCPSection from '$lib/components/settings/MCPSection.svelte';
    import LLMSection from '$lib/components/settings/LLMSection.svelte';
    import APIKeysSection from '$lib/components/settings/APIKeysSection.svelte';
    // ... navigation, theme, sections simples
  </script>

  <main>
    <Sidebar {activeSection} {sectionDefs} />
    <div class="content">
      <MCPSection />
      <LLMSection />
      <APIKeysSection />
      <!-- sections simples inline -->
    </div>
  </main>
  ```
- **Phases**:
  1. Extraire MCPSection avec son state et fonctions
  2. Extraire LLMSection avec son state et fonctions
  3. Extraire APIKeysSection
  4. Nettoyer page principale
- **Prerequis**: OPT-2, OPT-3 (helpers)
- **Risque regression**: Moyen (grande surface de changement)
- **Tests requis**:
  - Test mount/render chaque section
  - Test integration page complete
  - Test E2E navigation sections

#### OPT-7: Documenter patterns state management (Option B)
- **Fichiers**: `docs/ARCHITECTURE_DECISIONS.md`
- **Changement**: Documenter explicitement les 3 patterns acceptes
  ```markdown
  ### Q20bis: Patterns State Management Acceptes

  | Pattern | Cas d'usage | Exemple |
  |---------|-------------|---------|
  | CRUD Factory | Entites avec persistance DB | agents.ts, prompts.ts |
  | Pure Functions | Donnees complexes, selectors | llm.ts, mcp.ts |
  | Isolated Component | Sections autonomes | MemorySettings.svelte |
  ```
- **Benefice**: Clarifie decisions, reduit confusion
- **Risque regression**: Aucun (documentation)
- **Validation**: Review documentation

#### OPT-8: Lazy loading sections lourdes
- **Fichiers**: `src/routes/settings/+page.svelte`
- **Changement**: Import dynamique pour sections lourdes
  ```svelte
  <script lang="ts">
    import { onMount } from 'svelte';

    let MemorySettings: typeof import('$lib/components/settings/memory/MemorySettings.svelte').default;
    let AgentSettings: typeof import('$lib/components/settings/agents/AgentSettings.svelte').default;

    onMount(async () => {
      // Lazy load sections lourdes
      const [memoryModule, agentModule] = await Promise.all([
        import('$lib/components/settings/memory/MemorySettings.svelte'),
        import('$lib/components/settings/agents/AgentSettings.svelte')
      ]);
      MemorySettings = memoryModule.default;
      AgentSettings = agentModule.default;
    });
  </script>

  {#if activeSection === 'memory'}
    {#if MemorySettings}
      <MemorySettings />
    {:else}
      <LoadingSpinner />
    {/if}
  {/if}
  ```
- **Benefice**: Reduction bundle initial, chargement a la demande
- **Risque regression**: Faible
- **Validation**: Test navigation entre tabs

#### OPT-9: Cache donnees LLM/MCP au niveau store
- **Fichiers**:
  - `src/lib/stores/llm.ts`
  - `src/lib/stores/mcp.ts`
  - `src/lib/components/settings/agents/AgentForm.svelte`
- **Changement**: Ajouter cache avec invalidation
  ```typescript
  // llm.ts
  let cachedData: LLMData | null = null;
  let cacheTimestamp = 0;
  const CACHE_TTL = 30000; // 30 seconds

  export async function loadAllLLMData(forceRefresh = false): Promise<LLMData> {
    const now = Date.now();
    if (!forceRefresh && cachedData && (now - cacheTimestamp) < CACHE_TTL) {
      return cachedData;
    }
    cachedData = await invoke('get_llm_data');
    cacheTimestamp = now;
    return cachedData;
  }

  export function invalidateLLMCache(): void {
    cachedData = null;
  }
  ```
- **Benefice**: Evite chargements dupliques (page + AgentForm)
- **Risque regression**: Moyen (stale data si mauvaise invalidation)
- **Validation**: Test sequence create → edit → delete

### Nice to Have (Impact faible, Effort faible)

#### OPT-10: Ajouter confirmation sauvegarde API keys
- **Fichiers**: `src/routes/settings/+page.svelte`
- **Changement**: Modal confirmation avant invoke('save_api_key')
  ```typescript
  async function handleSaveApiKey(): Promise<void> {
    if (!settings.apiKey.trim()) {
      message = { type: 'error', text: t('api_key_empty') };
      return;
    }

    // Ajouter confirmation
    if (!confirm(t('api_key_confirm_save'))) {
      return;
    }

    // ... reste du code
  }
  ```
- **Benefice**: UX coherente avec delete operations
- **Risque regression**: Tres faible
- **Validation**: Test manuel

#### OPT-11: Remplacer erreurs silencieuses par warnings
- **Fichiers**: `src/lib/components/settings/agents/AgentForm.svelte`
- **Changement**:
  ```typescript
  onMount(async () => {
    try {
      const servers = await loadServers();
      mcpState = setServers(mcpState, servers);
    } catch (err) {
      // Avant: catch vide
      // Apres: warning visible
      console.warn('Failed to load MCP servers:', err);
      mcpState = setMCPError(mcpState, t('mcp_load_warning'));
    }
  });
  ```
- **Benefice**: Meilleur debugging, feedback utilisateur
- **Risque regression**: Tres faible
- **Validation**: Test avec serveur MCP indisponible

### Differe (Impact faible, Effort eleve)

#### OPT-12: Migration vers Superforms (Post-v1)
- **Raison du report**:
  - Effort 16h pour gain marginal
  - Tauri = pas de SSR (benefice principal de Superforms)
  - Validation actuelle fonctionne correctement
- **Reconsiderer si**: Ajout de nombreux formulaires complexes

#### OPT-13: Progressive Enhancement use:enhance (Post-v1)
- **Raison du report**:
  - Tauri = pas de SSR ni form actions server-side
  - Pattern actuel avec invoke() est idiomatique Tauri
  - Refactoring majeur sans benefice mesurable
- **Reconsiderer si**: Migration vers web app avec SSR

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| vite | 5.4.21 | 7.2.2 | Oui - voir migration guide |
| @sveltejs/vite-plugin-svelte | 4.0.0 | 4.0.4 | Non |
| lucide-svelte | 0.554.0 | 0.556.0 | Non |

### Nouvelles Dependencies (si justifie)

Aucune nouvelle dependance requise pour ce plan.

## Verification Non-Regression

### Tests Existants
- [x] `npm run check` - svelte-check + TypeScript strict
- [x] `npm run lint` - ESLint
- [x] `npm run test` - Vitest (scope: composants isoles)
- [x] `npm run build` - Build production

### Tests a Ajouter
- [ ] Test unitaire MCPSection.svelte (OPT-6)
- [ ] Test unitaire LLMSection.svelte (OPT-6)
- [ ] Test integration navigation sections (OPT-6)
- [ ] Test cache invalidation LLM/MCP (OPT-9)

### Benchmarks (si applicable)
```bash
# Avant optimisation
time npm run build
du -sh .svelte-kit/output

# Apres Vite upgrade (OPT-1)
time npm run build
du -sh .svelte-kit/output
# Attendu: -10-20% temps de build
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-1: Vite upgrade | 1h | Haut | P1 |
| OPT-2: Modal helper | 2h | Moyen | P1 |
| OPT-3: Async helper | 2h | Moyen | P1 |
| OPT-4: Minor deps | 15min | Bas | P1 |
| OPT-5: ComponentType | 10min | Bas | P1 |
| OPT-6: Refactor page | 8-16h | Haut | P2 |
| OPT-7: Doc patterns | 1h | Moyen | P2 |
| OPT-8: Lazy loading | 4h | Moyen | P2 |
| OPT-9: Cache data | 3h | Moyen | P2 |
| OPT-10: Confirm API | 1h | Bas | P3 |
| OPT-11: Warnings | 30min | Bas | P3 |

**Total Quick Wins (P1)**: ~5.5h
**Total Strategic (P2)**: ~16-24h
**Total Nice-to-have (P3)**: ~1.5h
**Total global**: ~23-31h

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Vite 7 breaking changes | Moyenne | Eleve | Test build complet, rollback si echec |
| Refactoring casse navigation | Faible | Moyen | Tests E2E manuels par section |
| Cache stale data | Moyenne | Moyen | Invalidation explicite sur mutations |
| Perte de fonctionnalite | Faible | Eleve | TypeScript strict + tests existants |

## Prochaines Etapes

1. [ ] **Valider ce plan** avec l'utilisateur
2. [ ] **Sprint 1 (Quick Wins P1)**: OPT-1 a OPT-5 (~5.5h)
3. [ ] **Sprint 2 (Strategic P2)**: OPT-6 (prerequis: OPT-2, OPT-3)
4. [ ] **Sprint 3 (Strategic P2)**: OPT-7, OPT-8, OPT-9
5. [ ] **Backlog**: OPT-10, OPT-11 (nice-to-have)

## References

### Code Analyse
- `src/routes/settings/+page.svelte` (1657L)
- `src/lib/stores/llm.ts` (521L)
- `src/lib/stores/mcp.ts` (414L)
- `src/lib/stores/factory/createCRUDStore.ts`
- `src/lib/components/settings/agents/AgentForm.svelte` (754L)
- `src/lib/components/settings/memory/MemorySettings.svelte` (1081L)

### Documentation Consultee
- `CLAUDE.md` - Conventions projet
- `docs/ARCHITECTURE_DECISIONS.md` - Decisions architecturales
- `docs/FRONTEND_SPECIFICATIONS.md` - Specifications UI
- `docs/DESIGN_SYSTEM.md` - Design system

### Sources Externes
- [Svelte 5 Migration Guide](https://svelte.dev/docs/svelte/v5-migration-guide)
- [Vite 7 Releases](https://vite.dev/releases)
- [Runes and Global state](https://mainmatter.com/blog/2025/03/11/global-state-in-svelte-5/)
- [Settings UI Design Best Practices](https://www.setproduct.com/blog/settings-ui-design)
- [Tabs UX Best Practices](https://www.eleken.co/blog-posts/tabs-ux)
- [SvelteKit Form Actions](https://svelte.dev/docs/kit/form-actions)
