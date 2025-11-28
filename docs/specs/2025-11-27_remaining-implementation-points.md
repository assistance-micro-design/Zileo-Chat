# Points Restants - Post Workflow Persistence Phases 1-6

> **Date**: 2025-11-27
> **Contexte**: Phases 1-6 du spec `workflow-persistence-streaming` completees
> **Source**: Analyse de `WORKFLOW_ORCHESTRATION.md` et `FRONTEND_SPECIFICATIONS.md`

---

## Resume Executif

Apres completion des 6 phases du workflow persistence/streaming, il reste des fonctionnalites documentees dans les specs mais non encore implementees. Ce document liste ces points par priorite.

---

## Etat Memory Tool (Verifie)

Les composants Memory sont **largement implementes**:

| Composant | Fichier | Status |
|-----------|---------|--------|
| MemorySettings | `src/lib/components/settings/memory/MemorySettings.svelte` | COMPLET |
| MemoryList | `src/lib/components/settings/memory/MemoryList.svelte` | COMPLET |
| MemoryForm | `src/lib/components/settings/memory/MemoryForm.svelte` | COMPLET |

### Fonctionnalites Memory Implementees

- [x] Selection provider embedding (Mistral, Ollama)
- [x] Selection modele embedding par provider
- [x] Configuration chunking (size, overlap, strategy)
- [x] Test embedding avec preview vecteur
- [x] Statistiques (total, with/without embeddings, by type)
- [x] Table avec filtrage par type
- [x] Recherche textuelle (CONTAINS)
- [x] CRUD complet (Add, View, Edit, Delete)
- [x] Export JSON/CSV
- [x] Import JSON
- [x] Regeneration embeddings

### Gaps Mineurs Memory (Optionnels)

- [ ] Pagination (actuellement charge toutes les memories)
- [ ] Recherche vectorielle semantique (backend stub, utilise CONTAINS)
- [ ] Pie chart distribution par type (affiche nombres seulement)
- [ ] UI pour purge par criteres (backend existe: `clear_memories_by_type`)

---

## Points Restants - WORKFLOW_ORCHESTRATION.md

| # | Point | Description | Priorite | Effort |
|---|-------|-------------|----------|--------|
| 1 | Dry-Run Mode | Simulation sans execution pour valider plan avant lancement | MEDIUM | 4h |
| 2 | Gantt Chart | Visualisation bottlenecks execution workflow | LOW | 8h |
| 3 | Batch Processing | Regrouper operations similaires MCP (`read_files_batch`) | MEDIUM | 3h |
| 4 | Cache LRU | Cache resultats MCP deterministes (context7 docs) | LOW | 4h |
| 5 | Timeouts Adaptatifs | Timeouts bases sur historique P95 | LOW | 3h |
| 6 | Metrics Avancees | `parallelization_ratio`, `speedup_factor` dans WorkflowMetrics | LOW | 2h |

---

## Points Restants - FRONTEND_SPECIFICATIONS.md

### Section Settings (Page Settings)

| # | Point | Description | Priorite | Effort |
|---|-------|-------------|----------|--------|
| 7 | Prompt Library | Prompt Selector complet avec preview, variables auto-detectees, formulaire dynamique | MEDIUM | 6h |
| 8 | Validation Settings | Configuration globale: mode default, selective config, risk thresholds, timeout | HIGH | 4h |

> **Note**: Points Directory Management (11, 12, 13) reportes - a implementer plus tard.

### Section Agent (Page Agent)

| # | Point | Description | Priorite | Effort |
|---|-------|-------------|----------|--------|
| 9 | Message Queue System | Queue indicator, view modal, reorder, edit messages en attente | HIGH | 6h |
| 10 | Queue Visual Feedback | Badge compteur, toast notification quand message ajoute | MEDIUM | 2h |
| 11 | Validation Request UI | Modal approve/reject/approve-all avec details operation | HIGH | 5h |
| 12 | Risk Badges | Affichage niveau risque (low/medium/high) dans validation | MEDIUM | 1h |
| 13 | Audit Trail | Log des validations avec export CSV/JSON | LOW | 3h |
| 14 | Token Display Complet | Speed (tk/s), warning states (75%/90%/100%) | MEDIUM | 2h |
| 15 | SubAgents Visualization | Kanban cards avec progress bars, tools expandable | LOW | 6h |
| 16 | Keyboard Shortcuts | Ctrl+Enter (approve), Esc (reject), Ctrl+D (toggle mode) | MEDIUM | 2h |

### Section Performance/UX

| # | Point | Description | Priorite | Effort |
|---|-------|-------------|----------|--------|
| 17 | Virtual Scrolling | Pour listes >100 messages (`@sveltejs/svelte-virtual-list`) | LOW | 3h |
| 18 | E2E Tests Complets | Playwright tests documentes section 11 FRONTEND_SPECIFICATIONS | MEDIUM | 8h |

---

## Resume par Priorite

### HIGH (Implementation prioritaire)

| # | Point | Effort Estime |
|---|-------|---------------|
| 8 | Validation Settings (global config) | 4h |
| 9 | Message Queue System | 6h |
| 11 | Validation Request UI (modal) | 5h |

**Total HIGH**: ~15h

### MEDIUM (Amelioration UX significative)

| # | Point | Effort Estime |
|---|-------|---------------|
| 1 | Dry-Run Mode | 4h |
| 3 | Batch Processing MCP | 3h |
| 7 | Prompt Library | 6h |
| 10 | Queue Visual Feedback | 2h |
| 12 | Risk Badges | 1h |
| 14 | Token Display Complet | 2h |
| 16 | Keyboard Shortcuts | 2h |
| 18 | E2E Tests | 8h |

**Total MEDIUM**: ~28h

### LOW (Nice-to-have)

| # | Point | Effort Estime |
|---|-------|---------------|
| 2 | Gantt Chart | 8h |
| 4 | Cache LRU | 4h |
| 5 | Timeouts Adaptatifs | 3h |
| 6 | Metrics Avancees | 2h |
| 13 | Audit Trail | 3h |
| 15 | SubAgents Visualization | 6h |
| 17 | Virtual Scrolling | 3h |

**Total LOW**: ~29h

---

## Recommandation Implementation

### Phase 7A: Validation System (HIGH) - ~15h

```
1. Validation Settings dans Page Settings
   - Mode globale (auto/manual/selective)
   - Configuration selective par type operation
   - Risk thresholds configuration
   - Timeout settings

2. Validation Request UI dans Page Agent
   - Modal avec details operation
   - Boutons Approve/Reject/Approve All
   - Risk badges (low/medium/high)
   - Keyboard shortcuts (Ctrl+Enter, Esc)

3. Message Queue System
   - Queue indicator quand workflow running
   - View modal pour voir/reordonner/editer queue
   - Toast notifications
```

### Phase 7B: UX Enhancements (MEDIUM) - ~28h

```
1. Prompt Library complet
2. Token Display avec vitesse et warnings
3. Batch Processing pour MCP
4. Dry-Run Mode
5. E2E Tests Playwright
```

### Phase 7C: Polish (LOW) - ~29h

```
1. Gantt Chart visualization
2. Cache LRU pour MCP
3. Virtual scrolling listes longues
4. SubAgents Kanban cards
5. Audit trail export
```

---

## Dependances

```
Phase 7A: Validation System
    ├── Validation Settings (Settings page)
    │   └── Pre-requis: Aucun
    ├── Validation Request UI (Agent page)
    │   └── Pre-requis: Validation Settings
    └── Message Queue System
        └── Pre-requis: Aucun (independant)

Phase 7B: UX Enhancements
    ├── Token Display
    │   └── Pre-requis: Streaming (Phase 2) DONE
    ├── Prompt Library
    │   └── Pre-requis: Aucun
    └── E2E Tests
        └── Pre-requis: Phase 7A complete
```

---

## Notes Techniques

### Validation System Backend

Les commandes backend existent deja (`src-tauri/src/commands/validation.rs`):
- `create_validation_request`
- `list_pending_validations`
- `list_workflow_validations`
- `approve_validation`
- `reject_validation`
- `delete_validation`

Il faut principalement le **frontend UI** pour les utiliser.

### Message Queue

Necessite:
1. Store Svelte pour gerer la queue (`src/lib/stores/messageQueue.ts`)
2. Composant QueueIndicator
3. Modal QueueManager
4. Integration dans ChatInput pour detecter workflow running

### Keyboard Shortcuts

Utiliser le pattern existant dans l'application ou ajouter:
```typescript
// Dans +page.svelte ou composant validation
function handleKeydown(e: KeyboardEvent) {
  if (e.ctrlKey && e.key === 'Enter') approveValidation();
  if (e.key === 'Escape') rejectValidation();
  if (e.ctrlKey && e.key === 'd') toggleValidationMode();
}
```

---

**Version**: 1.0
**Status**: Ready for Implementation
