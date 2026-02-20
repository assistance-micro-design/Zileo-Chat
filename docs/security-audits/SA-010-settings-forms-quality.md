# SA-010: Settings Components & Forms Quality Audit

**Date:** 2026-02-19
**Scope:** 23 components in `src/lib/components/settings/`
**Status:** Documented
**Total lines audited:** ~9,600

---

## Executive Summary

| Category | Finding |
|----------|---------|
| try/catch blocks | 30 total, 5 different error patterns, only 1 uses `getErrorMessage` |
| Template duplication | 3 severe hotspots (~650 lines extractable) |
| CSS duplication | 9 repeated CSS patterns across files |
| Accessibility gaps | 7 distinct issues across 10+ files |
| Code standard violations | 5 files use `console.error/warn`, 1 empty catch |
| Estimated extractable lines | ~850 / 9,600 (~9% reduction) |

---

## 1. Error Handling Inconsistency (30 try/catch blocks)

### Pattern Distribution

| Pattern | Files | Count | Correct? |
|---------|-------|-------|----------|
| `getErrorMessage(e)` | CustomProviderForm | 1 | Yes |
| Raw `` `${err}` `` interpolation | APIKeysSection, LLMSection, MCPSection, ImportPanel, ExportPanel | 12 | No |
| `String(err)` | MemoryForm, MemoryList, MemorySettings, ValidationSettings | 10 | No |
| `console.error` / `console.warn` | AgentForm, PromptSettings, MemorySettings, ValidationSettings, ImportExportSettings | 5 | **Violation** |
| Empty `catch {}` | AgentForm:251 | 1 | **Violation** |
| No catch (try/finally) | AgentSettings:108 | 1 | Risky |

### Specific Violations

| File | Line | Issue |
|------|------|-------|
| AgentForm.svelte | 251 | `catch {}` - error silently swallowed |
| AgentForm.svelte | 156, 165 | `console.warn` in production |
| PromptSettings.svelte | 115 | `console.error` in production |
| ValidationSettings.svelte | 111 | `console.error` in production |
| MemorySettings.svelte | 143 | `console.error` in production |
| ImportExportSettings.svelte | 67 | `console.error` in production |
| APIKeysSection.svelte | 73 | Hardcoded English error ("API key cannot be empty") |

### Proposed Extraction: `withAsyncAction()`

```typescript
// $lib/utils/async-action.ts
type AsyncActionOptions<T> = {
  action: () => Promise<T>;
  onSuccess?: (result: T) => void;
  onError?: (message: string) => void;
  loadingState?: { set: (v: boolean) => void };
};

async function withAsyncAction<T>(opts: AsyncActionOptions<T>): Promise<T | undefined> {
  opts.loadingState?.set(true);
  try {
    const result = await opts.action();
    opts.onSuccess?.(result);
    return result;
  } catch (e) {
    const message = getErrorMessage(e);
    opts.onError?.(message);
  } finally {
    opts.loadingState?.set(false);
  }
}
```

**Impact:** Standardizes all 30 try/catch blocks. Eliminates ~200 lines of boilerplate. Forces `getErrorMessage` usage everywhere.

---

## 2. Template Duplication

### 2.1 ValidationSettings.svelte (849 lines) -- SEVERE

**9 near-identical info-card blocks** across 3 modes (auto/manual/selective), each rendering the same 3 entity types (sub-agents, local tools, MCP servers).

| Mode | Lines | Entity blocks |
|------|-------|---------------|
| Auto | 237-293 | 3 info-cards (read-only with "auto_approved" status) |
| Manual | 306-363 | 3 info-cards (read-only with "requires_approval" status) |
| Selective | 374-443 | 3 info-cards (with checkboxes for per-item toggle) |

**Extraction:** Data-driven loop + `ValidationEntityCard.svelte` component.

```svelte
{#each entitySections as section (section.key)}
  <ValidationEntityCard
    title={section.title}
    items={section.items}
    mode={localMode}
    statusLabel={section.statusLabel}
    bind:selectedItems={section.bindTarget}
  />
{/each}
```

**Estimated reduction:** ~300 lines (from 849 to ~550)

### 2.2 ImportPreview.svelte (412 lines) -- SEVERE

**4 identical summary cards** (lines 129-177) + **4 identical entity list blocks** (lines 182-312).

Each entity section (agents, MCP, models, prompts) has:
- Summary Card with count badge
- Detail Card with "select all" checkbox + `{#each}` entity list

**Extraction:** `EntityPreviewSection.svelte` component looped over entity types.

**Estimated reduction:** ~150 lines (from 412 to ~260)

### 2.3 ExportPreview.svelte (388 lines) -- SEVERE

**4 identical collapsible entity sections** (agents:65-96, MCP:99-158, models:161-197, prompts:200-233).

Each section: Card with header button + chevron icon + collapsible `{#each}` body. 4 separate `expandedX` state variables with identical toggle logic.

**Extraction:** `CollapsibleEntitySection.svelte` or data-driven `{#each entitySections}`.

**Estimated reduction:** ~120 lines (from 388 to ~270)

### 2.4 AgentSettings / PromptSettings -- Structural Twins

| Element | AgentSettings lines | PromptSettings lines |
|---------|-------|-------|
| Header + title + add button | 173-186 | 173-186 |
| Error banner + dismiss | 188-194 | 188-194 |
| List component | 196 | 196 |
| Create/edit modal | 198-215 | 198-218 |
| Delete confirmation modal | 217-240 | 220-243 |
| CSS (.settings-header, .error-banner, etc.) | 248-296 | 251-301 |

**Extraction:** `CRUDSettingsLayout.svelte` wrapper component handling header, error banner, list, form modal, and delete confirmation with slots/snippets for customization.

**Estimated reduction:** ~150 lines combined (merging ~600 lines into ~450)

### 2.5 Three-State Loading Pattern (7 instances)

```svelte
{#if loading}
  <Card><StatusIndicator /> Loading...</Card>
{:else if items.length === 0}
  <div class="empty-state">No items</div>
{:else}
  {#each items as item}...{/each}
{/if}
```

Found in: LLMSection (x2), MCPSection, AgentList, PromptList, MemoryList, MemorySettings.

**Extraction:** `AsyncList.svelte` or `{#snippet}` pattern.

**Estimated reduction:** ~50 lines total

---

## 3. CSS Duplication

| CSS Pattern | Files Sharing It | Lines Each |
|-------------|-----------------|------------|
| `.settings-section` + `.section-title` + `.section-header-row` | LLMSection, MCPSection | ~15 |
| `.loading-state` (flex center + gap) | AgentList, PromptList, MemoryList, MemorySettings, ExportPanel | ~5 |
| `.empty-state` (flex column center) | AgentList, PromptList, MemoryList, MemorySettings, MCPSection, LLMSection | ~8 |
| `.error-banner` + `.dismiss-btn` | AgentSettings, PromptSettings | ~15 |
| `.message` success/error toast | LLMSection, MCPSection, MemoryList, MemorySettings, ValidationSettings, ImportExportSettings, ExportPanel | ~10 |
| `.modal-actions` (flex end gap) | AgentSettings, PromptSettings, MemorySettings | ~5 |
| `.confirm-text` | AgentSettings, PromptSettings | ~3 |
| `.checkbox-group` + `.checkbox-item` | ValidationSettings, AgentForm | ~12 |
| `.slider` + `.slider-label` + `.slider-help` | MemoryForm, MemorySettings | ~25 |

**Total duplicated CSS:** ~250 lines across all instances

**Extraction options:**
1. Shared CSS classes in `app.css` or a `settings.css` partial
2. Component extraction (preferred -- the CSS follows the component boundaries)

---

## 4. Accessibility Issues

### 4.1 Tab Pattern Without ARIA (ImportExportSettings.svelte)

Lines 143-156: Tab buttons with no `role="tablist"`, `role="tab"`, or `aria-selected`.

```svelte
<!-- Current -->
<div class="tab-bar">
  <button class:active onclick={...}>Export</button>
  <button class:active onclick={...}>Import</button>
</div>

<!-- Required -->
<div class="tab-bar" role="tablist">
  <button role="tab" aria-selected={activeTab === 'export'} onclick={...}>Export</button>
  <button role="tab" aria-selected={activeTab === 'import'} onclick={...}>Import</button>
</div>
```

### 4.2 Missing `aria-expanded` (ExportPreview.svelte)

Lines 67, 101, 163, 202: Collapsible section buttons lack `aria-expanded`.

### 4.3 Missing `aria-live` for Dynamic Messages

Toast/message regions in LLMSection, MCPSection, MemoryList, MemorySettings, ValidationSettings, ImportExportSettings, ExportPanel have no `aria-live="polite"` -- screen readers won't announce success/error messages.

### 4.4 Missing `aria-label` on Search/Filter Inputs

PromptList and MemoryList have search inputs without `aria-label` (the Input component may provide a `label` prop, but visual labels should be verified).

### 4.5 No Focus Management After Modal Operations

All files using Modal: focus is not explicitly returned to the trigger element after modal close.

### 4.6 Hardcoded English String

APIKeysSection.svelte:73 -- `"API key cannot be empty"` bypasses the `t()` i18n system.

### 4.7 Icon-Only Buttons Without `aria-label`

MCPFieldEditor.svelte: "Clear all sensitive" action link has no `aria-label`.

---

## 5. Validation Patterns Summary

| Component | Method | Quality | Issues |
|-----------|--------|---------|--------|
| CustomProviderForm | `isValid` derived | Good | None |
| AgentForm | `validate()` + errors record | Good | 10+ state vars |
| MemoryForm | Imperative checks | Adequate | `String(err)` in error |
| PromptForm | `isValid` derived | Minimal | No max length enforcement |
| ImportPanel | `canProceed` derived | Good | Complex derived chain |
| MCPEnvEditor | `allRequiredFilled` derived | Good | None |
| ValidationSettings | `hasChanges` tracking | Minimal | Manual tracking |
| APIKeysSection | Inline `trim()` check | Poor | Hardcoded English |

**Two validation styles coexist:**
1. **Declarative** (`$derived` returning boolean) -- CustomProviderForm, PromptForm, MCPEnvEditor
2. **Imperative** (`validate()` populating errors object) -- AgentForm

Both are valid, but they should be documented as accepted patterns. The imperative style is better for complex forms with per-field error messages.

---

## 6. Proposed Extraction Plan (Priority Order)

### P1: `withAsyncAction()` utility

**Files:** All 14 files with try/catch blocks
**Impact:** Standardizes error handling, eliminates `console.error` violations, forces `getErrorMessage`
**Reduction:** ~200 lines
**Complexity:** Low -- pure utility function

### P2: ValidationSettings entity loop

**File:** ValidationSettings.svelte (849 lines)
**Impact:** Eliminates 9 duplicated info-card blocks
**Reduction:** ~300 lines
**Complexity:** Medium -- needs `ValidationEntityCard.svelte` component

### P3: Import/Export entity loops

**Files:** ImportPreview.svelte, ExportPreview.svelte, ImportPanel.svelte (complete step)
**Impact:** Eliminates 16 duplicated entity blocks
**Reduction:** ~270 lines
**Complexity:** Medium -- data-driven loop refactoring

### P4: `CRUDSettingsLayout.svelte` wrapper

**Files:** AgentSettings.svelte, PromptSettings.svelte (+ future settings pages)
**Impact:** Eliminates structural twin, provides reusable CRUD layout
**Reduction:** ~150 lines
**Complexity:** Medium -- snippet-based composition

### P5: Shared CSS extraction

**Files:** 10+ files with duplicated CSS
**Impact:** Consistency, reduces CSS bloat
**Reduction:** ~250 lines CSS
**Complexity:** Low -- move to shared stylesheet or components

### P6: Accessibility fixes

**Files:** 10+ files
**Impact:** WCAG compliance, screen reader support
**Reduction:** 0 (adds ~30 lines of attributes)
**Complexity:** Low -- attribute additions

---

## 7. File-by-File Summary

| File | Lines | try/catch | Template Dup | CSS Dup | A11y Issues |
|------|-------|-----------|--------------|---------|-------------|
| APIKeysSection | 261 | 2 (raw err) | None | Low | Hardcoded EN |
| CustomProviderForm | 175 | 1 (correct) | None | None | Good |
| LLMSection | 585 | 5 (raw err) | Loading x2 | High | No aria-live |
| MCPSection | 414 | 5 (raw err) | Loading x1 | High | No aria-live |
| ValidationSettings | 849 | 3 (String+console) | **9 blocks** | Medium | Good (role=group) |
| AgentForm | 771 | 3 (empty+console) | Low | Medium | Good |
| AgentList | 283 | 0 | Loading x1 | Medium | No aria-label btns |
| AgentSettings | 296 | 1 (no catch) | **Twin** | High | No aria-label |
| PromptForm | 228 | 0 | None | None | No aria-describedby |
| PromptList | 363 | 0 | Loading x1 | Medium | No aria-label search |
| PromptSettings | 301 | 1 (console) | **Twin** | High | No aria-label |
| MemoryForm | 291 | 1 (String) | None | Slider dup | Good |
| MemoryList | 874 | 6 (String) | Loading x1 | Medium | Partial (has title) |
| MemorySettings | 1082 | 5 (String+console) | Model opts x2 | Slider dup | Good |
| ImportExportSettings | 249 | 1 (console) | None | None | **No tab ARIA** |
| ImportPanel | 808 | 2 (raw err) | **8 entity blocks** | None | No step ARIA |
| ImportPreview | 412 | 0 | **8 entity blocks** | None | No section labels |
| ExportPanel | 632 | 3 (raw err) | Step footer x3 | Low | No step ARIA |
| ExportPreview | 388 | 0 | **4 entity blocks** | None | **No aria-expanded** |
| EntitySelector | 276 | 0 | None | None | No root label |
| ConflictResolver | 446 | 0 | 3 radio blocks | None | Good |
| MCPFieldEditor | 317 | 0 | None | None | No aria-label link |
| MCPEnvEditor | 271 | 0 | None | None | Good |

---

## 8. Reduction Estimates

| Extraction | Lines Saved | Files Affected |
|------------|-------------|----------------|
| `withAsyncAction()` | ~200 | 14 |
| ValidationSettings entity loop | ~300 | 1 (+1 new component) |
| Import/Export entity loops | ~270 | 3 |
| CRUDSettingsLayout | ~150 | 2 (+1 new component) |
| CSS deduplication | ~250 | 10+ |
| **Total** | **~1,170** | **~20** |

**From 9,600 lines to ~8,430 lines (~12% reduction)**

Note: The bias check corrected an initial overestimate of 40% down to this evidence-based 12%. The real value is not just line reduction but **consistency**: one error handling pattern, one loading state pattern, one entity list pattern.
