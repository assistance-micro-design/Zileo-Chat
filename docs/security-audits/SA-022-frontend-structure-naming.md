# SA-022: Frontend Structure & Naming Audit

**Date**: 2026-02-26
**Type**: Quality audit
**Scope**: Frontend file structure, naming conventions, barrel exports, dead code
**Branch**: `security/audit-remediation-tdd`
**Status**: ALL PHASES DONE

## Context

Full verification of the frontend structure (files, directories) and analysis of naming pertinence (no ambiguity between names and their purposes). Covers 94 components, 16 stores, 24 type files, 6 services, 8 utils, 14 route files across 42 frontend directories.

---

## Findings Summary

| Severity | ID | Description |
|----------|----|-------------|
| HIGH | H-001 | Mixed naming conventions in `src/types/` and `src/lib/stores/` |
| HIGH | H-002 | Dead code: 3 unused components/files + unused helper functions |
| MEDIUM | M-001 | Incomplete barrel exports across 5 directories |
| MEDIUM | M-002 | Inventory meta counters outdated |
| MEDIUM | M-003 | Duplicate delete confirmation modal pattern |
| MEDIUM | M-004 | Types `index.ts` misleading JSDoc (`$lib/types` instead of `$types`) |
| MEDIUM | M-005 | `tool.ts` and `thinking.ts` contain unused helper functions |
| MEDIUM | M-006 | Missing barrel exports for `legal/` and `settings/` |
| MEDIUM | M-007 | `activity.service.ts` name no longer reflects actual purpose |
| MEDIUM | M-008 | `src/lib/validation/` directory with only 2 files |
| LOW | L-001 | `ChatContainer` in `agent/` directory (acceptable) |
| LOW | L-002 | Settings top-level loose files (provider-related) |
| LOW | L-003 | `ToolStatus` type duplication (resolves with H-002) |

---

## Detailed Findings

### H-001 -- Mixed Naming Conventions in `src/types/` and `src/lib/stores/`

**Impact**: A developer cannot predict the filename for a type without looking it up.

`src/types/` uses **4 different naming conventions** across 24 files:

| Convention | Files | Count |
|-----------|-------|-------|
| **kebab-case** | `background-workflow.ts`, `chat-block.ts`, `sub-agent.ts`, `user-question.ts` | 4 |
| **camelCase** | `customProvider.ts`, `importExport.ts` | 2 |
| **snake_case** | `function_calling.ts` | 1 |
| **simple lowercase** | `agent.ts`, `workflow.ts`, `llm.ts`, etc. | 17 |

Same issue in `src/lib/stores/`:

| Convention | Files | Count |
|-----------|-------|-------|
| **kebab-case** | `validation-settings.ts` | 1 |
| **camelCase** | `backgroundWorkflows.ts`, `executionBlocks.ts`, `userQuestion.ts` | 3 |
| **simple lowercase** | `agents.ts`, `workflows.ts`, etc. | 12 |

---

### H-002 -- Dead Code: 3 Unused Components/Files

| File | Evidence | Cause |
|------|----------|-------|
| `src/lib/components/ui/JsonViewer.svelte` | Only self-references (recursive), never imported by any other file | Created but never integrated |
| `src/lib/components/chat/ToolExecution.svelte` | Exported from barrel but never imported by any component | Replaced by `ToolCallBlock.svelte` in SA-019/P3 |
| `src/types/thinking.ts` | Never imported via `$types/thinking`; helper functions have zero callers | Types defined here are only used internally by `workflow.ts` |

Additionally, `ToolStatus` type is duplicated: defined in both `ToolExecution.svelte` (dead) and `streaming.ts` (used).

---

### M-001 -- Incomplete Barrel Exports

Several barrel `index.ts` files do not export all components in their directory.

**`src/lib/components/chat/index.ts`** -- Missing 6 of 12 components:
- `ThinkingBlock`, `ToolCallBlock`, `SubAgentBlock`, `ExecutionSpinner`, `TodoTasksBlock`, `PromptSelectorModal`

**`src/lib/components/ui/index.ts`** -- Missing 4 of 20 components:
- `MarkdownRenderer`, `JsonViewer` (dead), `ToastContainer`, `ToastItem`

**`src/types/index.ts`** -- Missing 9 of 24 type files:
- `background-workflow.ts` (3 importers), `chat-block.ts` (8 importers), `customProvider.ts` (5 importers), `i18n.ts` (4 importers), `services.ts` (2 importers), `sub-agent.ts` (4 importers), `thinking.ts` (0 importers -- dead), `tool.ts` (0 direct importers), `user-question.ts` (3 importers)

**`src/lib/utils/index.ts`** -- Missing 3 of 8 utils:
- `duration.ts` (3 importers), `dateGrouping.ts` (1 importer + tests), `url.ts` (security-sensitive)

**`src/lib/stores/index.ts`** -- Missing 3 stores:
- `backgroundWorkflows.ts` (2 importers), `executionBlocks.ts` (2 importers), `toast.ts` (3 importers)

---

### M-002 -- Inventory Meta Counters Outdated

**File**: `.claude/registry/inventory.yml`

| Counter | Current value | Actual |
|---------|--------------|--------|
| `total_components` | 88 | 94 |
| `total_stores` | 17 | 16 unique files + factory + chunkProcessor |

The inventory entries themselves are correct. Only the `meta:` summary counters are wrong.

---

### M-003 -- Duplicate Delete Confirmation Modal

Two components with inverted naming for the same UI pattern:

| Component | Path | Purpose |
|-----------|------|---------|
| `ConfirmDeleteModal` | `src/lib/components/workflow/ConfirmDeleteModal.svelte` | Workflow deletion (hard-coded `workflowName` prop) |
| `DeleteConfirmModal` | `src/lib/components/ui/DeleteConfirmModal.svelte` | Generic deletion (i18n keys, reusable) |

The word order is inconsistent. `ui/DeleteConfirmModal` is the generalized reusable version (SA-017/OPT-3).

---

### M-004 -- Types `index.ts` Misleading JSDoc

**File**: `src/types/index.ts`, line 31

```typescript
// Current (WRONG):
import type { Workflow, Agent, Message } from '$lib/types';

// Should be:
import type { Workflow, Agent, Message } from '$types';
```

Directly contradicts the project convention (ERR_TS_001).

---

### M-005 -- Unused Helper Functions in `tool.ts` and `thinking.ts`

These helpers are well-designed and ready to use, but never integrated into the components that display the corresponding data.

**`src/types/tool.ts`**: 8 exports, but helper functions have zero callers:
- `formatToolDuration`, `getToolTypeDisplay`, `getToolIdentifier`, `createToolExecutionFromWorkflow`
- `ActiveToolExecution` and `ToolExecutionStatus` types also have zero callers

**`src/types/thinking.ts`**: Helper functions with zero callers:
- `formatThinkingDuration`, `truncateThinkingContent`, `groupThinkingStepsByMessage`

**Action**: Integrate these helpers into the components that need them (`ToolCallBlock.svelte`, `ThinkingBlock.svelte`, etc.) to improve formatting consistency and reduce inline logic duplication.

---

### M-006 -- Missing Barrel Exports for `legal/` and `settings/`

Two component directories have no `index.ts` barrel export:
- `src/lib/components/legal/` (contains `LegalModal.svelte`)
- `src/lib/components/settings/` (top-level, contains 5 loose `.svelte` files)

Every other component directory has a barrel export.

---

### M-007 -- `activity.service.ts` Name No Longer Reflects Purpose

**File**: `src/lib/services/activity.service.ts`

After the ActivitySidebar removal (SA-019/P4), this service now serves message/sub-agent enrichment, not "activity" display. The name `activity.service.ts` is misleading.

**Recommendation**: Rename to `subAgentExecution.service.ts` or `messageEnrichment.service.ts` to match actual purpose.

---

### M-008 -- `src/lib/validation/` Directory With Only 2 Files

**Files**: `schemas.ts` and `invoke.ts`

A directory with only 2 files creates unnecessary directory proliferation. These could be merged into `src/lib/utils/` as `utils/validation-schemas.ts` and `utils/validation-invoke.ts`, or kept as-is if growth is expected.

---

### L-001 -- ChatContainer in `agent/` Directory

`ChatContainer.svelte` is in `src/lib/components/agent/` but imports heavily from `chat/` components. Its placement in `agent/` is defensible as a page-level composition component specific to the agent page.

**No action required.**

---

### L-002 -- Settings Top-Level Loose Files

5 `.svelte` files at `src/lib/components/settings/` top-level:
- `APIKeysSection.svelte`, `CustomProviderForm.svelte`, `LLMSection.svelte`, `MCPSection.svelte`, `SettingsSectionHeader.svelte`

The first 3 are provider/LLM-related and could benefit from a `providers/` subdirectory.

---

### L-003 -- `ToolStatus` Type Duplication

`ToolStatus` is defined identically in:
- `src/lib/stores/streaming.ts` (line 38) -- **used**
- `src/lib/components/chat/ToolExecution.svelte` (line 36) -- **dead code**

Resolves automatically when `ToolExecution.svelte` is removed (H-002).

---

## Remediation Plan

### Phase 1: Quick Fixes (Trivial effort) -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| M-004 | Fix JSDoc: replace `'$lib/types'` with `'$types'` | `src/types/index.ts` | **DONE** |
| M-002 | Update inventory meta counters to accurate values | `.claude/registry/inventory.yml` | **DONE** |

**Changes**: JSDoc line 30 `$lib/types` → `$types`. Meta counters: `total_components` 88→94, `total_stores` 17→18.

---

### Phase 2: Dead Code Removal + Helper Integration -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| H-002a | Remove `JsonViewer.svelte` (0 importers, only self-references) | `src/lib/components/ui/JsonViewer.svelte` | **DONE** |
| H-002b | Remove `ToolExecution.svelte` + its barrel export (replaced by `ToolCallBlock` in SA-019/P3) | `src/lib/components/chat/ToolExecution.svelte`, `src/lib/components/chat/index.ts` | **DONE** |
| M-005a | Integrated `formatToolDuration` into `ToolCallBlock.svelte` (replaces inline logic). Removed unused: `getToolTypeDisplay`, `getToolIdentifier`, `createToolExecutionFromWorkflow`, `ActiveToolExecution`, `ToolExecutionStatus` | `src/types/tool.ts`, `src/lib/components/chat/ToolCallBlock.svelte` | **DONE** |
| M-005b | Integrated `truncateThinkingContent` into `ThinkingBlock.svelte` (replaces inline logic). Removed unused: `formatThinkingDuration`, `createActiveThinkingStep`, `ActiveThinkingStep`, `groupThinkingStepsByMessage`, `calculateTotalThinkingTokens`, `calculateTotalThinkingDuration` | `src/types/thinking.ts`, `src/lib/components/chat/ThinkingBlock.svelte` | **DONE** |
| L-003 | `ToolStatus` duplication resolved: only `streaming.ts` definition remains | `src/lib/stores/streaming.ts` | **DONE** |

**Changes**: 2 dead components deleted, 2 barrel exports removed, 2 helpers integrated into components, 11 unused exports removed from type files. Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

### Phase 3: Naming Normalization -- DONE

**Convention choisie**: **kebab-case** (aligns with SvelteKit route naming, already used by 4 files).

| ID | Action | From | To | Importers updated | Status |
|----|--------|------|----|-------------------|--------|
| H-001a | Rename type file | `src/types/customProvider.ts` | `src/types/custom-provider.ts` | 4 (llm.ts, ModelForm, LLMSection, AgentForm) | **DONE** |
| H-001b | Rename type file | `src/types/importExport.ts` | `src/types/import-export.ts` | 5 + 1 barrel (ConflictResolver, MCPFieldEditor, MCPEnvEditor, ImportPanel, ImportPreview, index.ts) | **DONE** |
| H-001c | Rename type file | `src/types/function_calling.ts` | `src/types/function-calling.ts` | 1 barrel (index.ts) | **DONE** |
| H-001d | Rename store file | `src/lib/stores/backgroundWorkflows.ts` | `src/lib/stores/background-workflows.ts` | 3 (userQuestion.ts, workflowExecutor.service.ts, +page.svelte) | **DONE** |
| H-001e | Rename store file | `src/lib/stores/executionBlocks.ts` | `src/lib/stores/execution-blocks.ts` | 3 + 1 test (workflowExecutor.service.ts, +page.svelte, execution-blocks.test.ts) | **DONE** |
| H-001f | Rename store file | `src/lib/stores/userQuestion.ts` | `src/lib/stores/user-question.ts` | 3 (UserQuestionModal, +page.svelte, stores/index.ts) | **DONE** |
| H-001g | Rename store file | `src/lib/stores/validation-settings.ts` | Already kebab-case | N/A | **DONE** |

**Changes**: 6 files renamed (3 types + 3 stores), 1 test file renamed, 16 import paths updated, 2 barrel re-exports updated, 6 JSDoc `@module` tags updated. Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

### Phase 4: Modal Consolidation -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| M-003 | Consolidated `workflow/ConfirmDeleteModal.svelte` into `ui/DeleteConfirmModal.svelte`. Added optional `itemName` and `warningMessageKey` props. Updated `+page.svelte` caller with `deleting` state. Deleted `ConfirmDeleteModal.svelte` + barrel export. | `src/lib/components/ui/DeleteConfirmModal.svelte`, `src/routes/agent/+page.svelte`, `src/lib/components/workflow/index.ts` | **DONE** |

**Changes**: 1 dead component deleted, 1 barrel export removed, 2 optional props added to generic modal (`itemName`, `warningMessageKey`), 1 caller updated with proper `deleting` state management. Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

### Phase 5: Barrel Export Completion -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| M-001a | Add missing exports to `src/lib/components/chat/index.ts` | `ThinkingBlock`, `ToolCallBlock`, `SubAgentBlock`, `ExecutionSpinner`, `TodoTasksBlock`, `PromptSelectorModal` | **DONE** |
| M-001b | Add missing exports to `src/lib/components/ui/index.ts` | `MarkdownRenderer`, `ToastContainer`, `ToastItem` (skip dead `JsonViewer`) | **DONE** |
| M-001c | Add missing exports to `src/types/index.ts` | `background-workflow`, `chat-block`, `custom-provider`, `i18n`, `services`, `sub-agent`, `thinking`, `tool`, `user-question` | **DONE** |
| M-001d | Add missing exports to `src/lib/utils/index.ts` | `duration`, `dateGrouping`, `url` | **DONE** |
| M-001e | Add missing exports to `src/lib/stores/index.ts` | `background-workflows`, `execution-blocks`, `toast` | **DONE** |
| M-006a | Create `src/lib/components/legal/index.ts` barrel | Export `LegalModal` | **DONE** |
| M-006b | Create `src/lib/components/settings/index.ts` barrel | Export `APIKeysSection`, `CustomProviderForm`, `LLMSection`, `MCPSection`, `SettingsSectionHeader` | **DONE** |
| fix | Remove duplicate `RiskLevel` from `sub-agent.ts` | Import from `./validation` instead of re-declaring (avoids barrel conflict) | **DONE** |

**Changes**: 5 barrel files updated (chat +6, ui +3, types +9, utils +3, stores +3), 2 barrel files created (legal, settings), 1 duplicate type removed (`RiskLevel` in `sub-agent.ts`). `thinking.ts` and `tool.ts` included despite audit note (both have active importers after Phase 2 integration). Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

### Phase 6: Service & Directory Cleanup -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| M-007 | Renamed `activity.service.ts` to `sub-agent-execution.service.ts`. Renamed export `ActivityService` to `SubAgentExecutionService`. Updated 2 importers (`message.service.ts`, `services/index.ts`). | `src/lib/services/sub-agent-execution.service.ts` | **DONE** |
| M-008 | Merged `src/lib/validation/` into `src/lib/utils/`: `schemas.ts` -> `validation-schemas.ts`, `invoke.ts` -> `validation-invoke.ts`. Updated `utils/index.ts` barrel. Deleted `validation/` directory. Zero external importers affected. | `src/lib/utils/validation-schemas.ts`, `src/lib/utils/validation-invoke.ts` | **DONE** |
| BUG | Fixed pre-existing bug: SubAgentBlocks missing on reload. `load_message_blocks` only queries `tool_execution` + `thinking_step`, never `sub_agent_execution`. Added `appendSubAgentBlocks()` in `+page.svelte` to rebuild SubAgent ChatBlocks from enriched message data. Modified `MessageService.loadWithSubAgents` to return raw executions. | `src/routes/agent/+page.svelte`, `src/lib/services/message.service.ts` | **DONE** |

**Changes**: 1 service file renamed + export renamed, 2 validation files moved to utils/, 1 directory deleted, 1 barrel updated, 1 pre-existing bug fixed (sub-agent blocks persistence on reload). Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

### Phase 7: Settings Structure -- DONE

| ID | Action | Files | Status |
|----|--------|-------|--------|
| L-002 | Created `src/lib/components/settings/providers/` subdirectory. Moved `APIKeysSection.svelte`, `CustomProviderForm.svelte`, `LLMSection.svelte` into it. Created `providers/index.ts` barrel. Updated `settings/index.ts` re-export paths. Updated `src/routes/settings/providers/+page.svelte` import paths. | `src/lib/components/settings/providers/` | **DONE** |

**Changes**: 3 components moved to `providers/` subdirectory, 1 barrel file created (`providers/index.ts`), 1 barrel updated (`settings/index.ts` re-export paths), 1 route page updated (import paths). `LLMSection` relative import of `CustomProviderForm` unchanged (same directory). Validation: `npm run lint` + `npm run check` + `npm run test` (260 tests) all pass.

---

## Verification Checklist

After each phase:
- [ ] `npm run lint` passes
- [ ] `npm run check` passes
- [ ] `npm run test` passes
- [ ] No broken imports (verified by `svelte-check`)
- [ ] Inventory updated if component counts changed

Final:
- [ ] All file names follow kebab-case convention
- [ ] All barrel exports are complete
- [ ] No dead code remains
- [ ] No naming ambiguity between components

---

## Ordre d'implementation

```
Phase 1 (Quick Fixes)
  |
  v
Phase 2 (Dead Code + Integration helpers)
  |
  v
Phase 3 (Naming Normalization) ---> Phase 4 (Modal Consolidation) [independant]
  |
  v
Phase 5 (Barrel Exports) ---------> Phase 6 (Service & Directory Cleanup) [independant]
                                       |
                                       v
                                     Phase 7 (Settings Structure) [optionnel]
```

| Ordre | Phase | Prerequis | Justification |
|-------|-------|-----------|---------------|
| 1 | **Phase 1**: Quick Fixes | Aucun | Trivial, pas de risque de regression, corrige les references documentaires |
| 2 | **Phase 2**: Dead Code + Integration helpers | Phase 1 | Supprime les fichiers morts AVANT de renommer ou d'exporter, integre les helpers dans les composants actifs |
| 3 | **Phase 3**: Naming Normalization | Phase 2 | Les fichiers restants (apres purge) sont renommes en kebab-case. Les imports sont mis a jour. |
| 4 | **Phase 4**: Modal Consolidation | Aucun (independant) | Peut etre fait en parallele de Phase 3, mais logiquement apres Phase 2 (suppression du dead code) |
| 5 | **Phase 5**: Barrel Exports | Phase 2 + Phase 3 | Les barrels doivent refleter les noms finaux (kebab-case) et ne pas exporter du dead code |
| 6 | **Phase 6**: Service & Directory Cleanup | Phase 5 | Les barrels sont a jour, on peut deplacer/renommer les fichiers restants sans casser les re-exports |
| 7 | **Phase 7**: Settings Structure (optionnel) | Phase 6 | Reorganisation fine, derniere etape pour ne pas interferer avec les renommages precedents |

**Regles**:
- Phase 2 avant Phase 5 : eviter d'exporter du dead code
- Phase 3 avant Phase 5 : les barrels utilisent les noms definitifs
- Phase 4 est independante mais beneficie de Phase 2 (plus de `ToolExecution.svelte` dans les exports)
- Phase 6 apres Phase 5 : les barrels sont stables, on peut reorganiser les directories
- Phase 7 en dernier : impact localise, optionnel
