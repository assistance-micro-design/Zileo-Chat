# SA-016: Agent Page UX Remediation (TDD)

**Date:** 2026-02-21
**Reviewed:** 2026-02-22 (code audit vs plan)
**Status:** DONE (7 phases completed, Phase 8 cancelled)
**Branch:** `security/audit-remediation-tdd`
**Scope:** `src/lib/components/agent/`, `src/lib/components/chat/`, `src/lib/components/workflow/`, `src/lib/stores/streaming.ts`
**Phases:** 7 completed (Phase 8 cancelled - architectural complexity, moved to Out of Scope)
**Approach:** TDD - Red/Green/Refactor for logic, test-alongside for UI

---

## Table of Contents

1. [Summary](#1-summary)
2. [Problems Identified](#2-problems-identified)
3. [Phase 1 - Fix Double Scroll](#phase-1---fix-double-scroll-p2)
4. [Phase 2 - Filter Labels in Expanded Sidebar](#phase-2---filter-labels-in-expanded-sidebar-u4)
5. [Phase 3 - Remove Double Filtering](#phase-3---remove-double-filtering-p3)
6. [Phase 4 - Markdown in Streaming](#phase-4---markdown-in-streaming-p1)
7. [Phase 5 - Rename Discoverability](#phase-5---rename-discoverability-u1)
8. [Phase 6 - Informative Round Separators](#phase-6---informative-round-separators-u3)
9. [Phase 7 - Temporal Grouping of Workflows](#phase-7---temporal-grouping-of-workflows-u2)
10. [Additional Fixes](#additional-fixes)
11. [Dead Code Cleanup](#dead-code-cleanup)
12. [Out of Scope](#out-of-scope)
13. [Validation](#validation)
14. [Audit Review (2026-02-22)](#audit-review-2026-02-22)

---

## 1. Summary

Deep analysis of the Agent page revealed 11 problems (6 functional, 5 UX). This document defines the TDD remediation plan in 7 phases, ordered from quickest to longest. Phase 8 (Progressive Activity Display + Thinking in Chat) was cancelled due to architectural complexity and moved to Out of Scope.

**Additional fixes discovered during implementation:**
- Missing `rename_workflow` Tauri command (frontend called it but backend command did not exist)
- Space key not working in workflow rename input (event propagation issue)
- `ConfirmDeleteModal` not using standard `Modal` UI component

---

## 2. Problems Identified

| Prio | ID | Problem | Impact | Phase | Status |
|------|----|---------|--------|-------|--------|
| 1 | P6 | Fake streaming - no real token-by-token from LLM | Critical | ~~8~~ | **Cancelled** (Out of Scope) |
| 2 | P5 | Thinking not displayed in chat area | Critical | ~~8~~ | **Cancelled** (Out of Scope) |
| 3 | P1 | Streaming shows plain text, no Markdown | High | 4 | **DONE** |
| 4 | P2 | Double scroll (ChatContainer + MessageList) | High | 1 | **DONE** |
| 5 | U1 | Rename workflow only via double-click | High | 5 | **DONE** |
| 6 | U2 | No temporal grouping of workflows | High | 7 | **DONE** |
| 7 | P3 | Double filtering of workflows (store + local) | Medium | 3 | **DONE** |
| 8 | U4 | Filter tabs icon-only in expanded sidebar | Medium | 2 | **DONE** |
| 9 | U3 | Round separator non-informative | Medium | 6 | **DONE** |
| 10 | P4 | AgentSelector layout in header | Low | - | Out of scope |
| 11 | U5 | TokenDisplay too dense | Low | - | Out of scope |
| - | BF1 | Missing `rename_workflow` Tauri command | High | Fix | **DONE** |
| - | BF2 | Space key not working in rename input | Medium | Fix | **DONE** |
| - | BF3 | ConfirmDeleteModal not using standard Modal | Medium | Fix | **DONE** |

---

## Phase 1 - Fix Double Scroll (P2)

**Effort:** ~15 min
**Files:** `agent/ChatContainer.svelte`, `chat/MessageList.svelte`

### Problem

Both `ChatContainer.svelte` (`.messages-area`) and `MessageList.svelte` (`.message-list`) have `overflow-y: auto`. This creates nested scrollbars where the inner list scrolls inside an already scrollable container.

```
ChatContainer (.messages-area overflow-y: auto)
  --> MessageList (.message-list overflow-y: auto)   <-- double scroll
  --> Streaming bubble (inline)
```

### TDD

**RED - Test:**
No pure logic to unit test. Manual verification:
1. Load a workflow with 20+ messages
2. Confirm only ONE scrollbar is visible
3. Confirm scroll-to-bottom works on new messages
4. Confirm scroll-to-bottom works during streaming

**GREEN - Fix:**

`ChatContainer.svelte` - The `.messages-area` wrapper must keep `overflow-y: auto` because it also contains the streaming bubble below MessageList. MessageList must lose its own scroll.

Option: MessageList removes `overflow-y: auto` and `flex: 1`, becomes a simple vertical stack. ChatContainer's `.messages-area` remains the sole scroll container.

```diff
// MessageList.svelte - CSS
.message-list {
   display: flex;
   flex-direction: column;
   gap: var(--spacing-md);
-  flex: 1;
-  overflow-y: auto;
-  scroll-behavior: smooth;
   padding: var(--spacing-md);
}
```

The auto-scroll `$effect` in ChatContainer already targets `messagesContainer` (the `.messages-area` div), so it will continue to work.

The auto-scroll `$effect` in MessageList (line 65-69) must be removed or redirected to the parent, since MessageList no longer scrolls itself.

**REFACTOR:** Remove the `scrollToBottom()` function from MessageList (lines 56-60) and the `$effect` that calls it (lines 65-69). Remove the now-dead `autoScroll` prop. Auto-scroll responsibility moves entirely to ChatContainer. Verify `contain: strict` in performance mode (line 108-110) still works without `flex: 1`.

---

## Phase 2 - Filter Labels in Expanded Sidebar (U4)

**Effort:** ~20 min
**Files:** `workflow/ActivityFeed.svelte`

### Problem

The 5 filter tabs in ActivityFeed show only icons, even when the right sidebar is expanded to 320px. Users cannot tell what each icon means without hovering.

### TDD

**RED - Test:**
No pure logic. Manual verification:
1. Expand right sidebar
2. Confirm filter tabs show icon + label text
3. Collapse right sidebar
4. Confirm filter tabs show icon only
5. Confirm all 5 labels fit within 320px

**GREEN - Fix:**

ActivityFeed already receives `collapsed` prop (line 79, default `false`). Use it to conditionally render labels. Filter definitions are already imported from `$types/activity` as `ACTIVITY_FILTERS` with `id`, `icon`, `label` fields.

```svelte
<!-- workflow/ActivityFeed.svelte - filter tabs (lines 150-167) -->
{#each filters as filter}
  <button class="filter-tab" class:active={...} onclick={...}>
    <svelte:component this={filter.icon} size={14} />
    {#if !collapsed}
      <span class="filter-label">{filter.label}</span>
    {/if}
    {#if filter.count > 0}
      <span class="filter-badge">{filter.count}</span>
    {/if}
  </button>
{/each}
```

CSS: `.filter-label { font-size: 0.7rem; }` to keep tabs compact at 320px.

**REFACTOR:** Filter definitions already extracted in `ACTIVITY_FILTERS` from `$types/activity`. No refactor needed.

---

## Phase 3 - Remove Double Filtering (P3)

**Effort:** ~20 min
**Files:** `WorkflowSidebar.svelte`, potentially `workflowStore`

### Problem

`WorkflowSidebar.svelte` receives `workflows` (already from `$filteredWorkflows` store) then applies a second local filter:

```typescript
let filteredWorkflows = $derived(
  workflows.filter(w => w.name.toLowerCase().includes(searchFilter.toLowerCase()))
);
```

This means workflows are potentially filtered twice (once in store, once locally), or the store filter is bypassed entirely.

### TDD

**RED - Test (store unit test):**

> **NOTE (audit 2026-02-22):** These tests already exist in `src/lib/stores/__tests__/workflows.test.ts` lines 281-337. The store's `filteredWorkflows` derived already filters by `searchFilter`. No new tests needed for the store filter itself.

```typescript
// src/lib/stores/__tests__/workflows.test.ts (ALREADY EXISTS)
// Lines 281-337 cover filteredWorkflows with searchFilter
```

**GREEN - Fix:**

The store already filters by name (confirmed). The data flow is:
1. `WorkflowSidebar.searchFilter` (bindable, line 62) -> `handleSearchInput` -> `debouncedSearchChange` -> `onsearchchange` callback
2. `+page.svelte` line 453: `onsearchchange={(v) => workflowStore.setSearchFilter(v)}` propagates to store
3. Store `filteredWorkflows` (line 159-163) applies the filter
4. `+page.svelte` passes `workflows={$filteredWorkflows}` (already filtered) to WorkflowSidebar
5. WorkflowSidebar applies a SECOND local filter (lines 87-91) - redundant

**Fix:** Remove the local `filteredWorkflows` derived in WorkflowSidebar (lines 87-91). Change line 139 from `workflows={filteredWorkflows}` to `workflows={workflows}` when passing to WorkflowList.

**REFACTOR:** Ensure search lives in ONE place. Preferably the store (centralizes logic, testable). WorkflowSidebar becomes a pure UI that passes the search term up.

---

## Phase 4 - Markdown in Streaming (P1)

**Effort:** ~30 min
**Files:** `agent/ChatContainer.svelte`

### Problem

During streaming, content is displayed as plain text:
```svelte
<div class="streaming-content">
  {streamContent}
  <span class="cursor"></span>
</div>
```

While `MessageBubble.svelte` uses `<MarkdownRenderer>` for assistant messages. This means streaming shows raw markdown syntax (`**bold**`, `# headers`, code blocks) as plain text.

### TDD

**RED - Test:**
No pure logic (rendering concern). Manual verification:
1. Start a workflow that generates markdown
2. During streaming, confirm headers/bold/code render as formatted HTML
3. After completion, confirm the message bubble also renders markdown (already works)
4. Confirm the blinking cursor still appears at the end

**GREEN - Fix:**

Replace plain text with MarkdownRenderer:

```svelte
<!-- agent/ChatContainer.svelte -->
<div class="streaming-content">
  <MarkdownRenderer content={streamContent} />
  <span class="cursor"></span>
</div>
```

Import `MarkdownRenderer` from `$lib/components/ui/MarkdownRenderer.svelte` (NOT `chat/` - it lives in `ui/`).

Note: MarkdownRenderer uses `marked.parse()` + `DOMPurify.sanitize()` via `$derived`. The `marked` library handles partial/incomplete markdown gracefully. Test with:
- `"# Hello\n\nSome **bold**"` - should render properly
- `` "```python\nprint('hello')" `` - unclosed code block, should not break

> **ATTENTION (audit 2026-02-22) - Cursor positioning:** `<MarkdownRenderer>` generates block-level HTML. The `<span class="cursor">` after it will be on a new line, not inline after the last word. Two options:
> 1. Inject cursor via CSS `::after` pseudo-element on the last child of MarkdownRenderer's output
> 2. Append a cursor character to `streamContent` before passing to MarkdownRenderer and style it via CSS
>
> **ATTENTION (audit 2026-02-22) - CSS conflict:** `.streaming-content` has `white-space: pre-wrap` (line 169). This will interact poorly with HTML generated by MarkdownRenderer. Remove `white-space: pre-wrap` from `.streaming-content` after this change.

**REFACTOR:** Consider adding a `streaming` prop to MarkdownRenderer if it needs special handling for partial content (e.g., suppress unclosed code block errors).

---

## Phase 5 - Rename Discoverability (U1)

**Effort:** ~45 min
**Files:** `workflow/WorkflowItem.svelte`

### Problem

Workflow rename is only possible via double-click on the name. This is non-discoverable - users have no visual cue that renaming is possible.

### TDD

**RED - Test:**
Manual verification:
1. Hover over a workflow item
2. Confirm an edit (pencil) icon appears alongside the delete icon
3. Click the edit icon
4. Confirm inline editing activates (same as current double-click behavior)
5. Confirm double-click still works as before
6. Keyboard: confirm Tab or F2 on focused item starts edit

**GREEN - Fix:**

Add a pencil icon button next to the delete button, visible on hover:

```svelte
<!-- workflow/WorkflowItem.svelte - actions area -->
<!-- NOTE (audit 2026-02-22): Use $i18n() not $t() - project convention -->
<div class="item-actions">
  <button class="action-btn edit-btn" onclick={startEdit} title={$i18n('workflow_rename')}>
    <Pencil size={14} />
  </button>
  <button class="action-btn delete-btn" onclick={handleDelete} title={$i18n('workflow_delete')}>
    <X size={14} />
  </button>
</div>
```

CSS: Same hover-reveal pattern as the existing delete button (`opacity: 0` -> `opacity: 1` on `.workflow-item:hover`).

Add keyboard handler: F2 key triggers `startEdit()` when item is focused.

**REFACTOR:** Group action buttons in a container with consistent styling. Ensure the edit button does not trigger item selection (stop propagation, same as delete button).

---

## Phase 6 - Informative Round Separators (U3)

**Effort:** ~1h (was ~45 min, increased after audit)
**Files:** `workflow/ActivityFeed.svelte`, new utility `src/lib/utils/activityUtils.ts`

### Problem

Round separators in the activity feed show only the word "Round" with no useful information. Users cannot distinguish between rounds or understand what each round represents.

> **ATTENTION (audit 2026-02-22) - Data gap:** The current `FeedItem` separator type only has `messageId`:
> ```typescript
> type FeedItem =
>   | { kind: 'activity'; data: WorkflowActivityEvent }
>   | { kind: 'separator'; messageId: string };
> ```
> The `round`, `agentName`, and `count` fields must be **computed** from the activities grouped by `messageId`. This requires a new pure function `computeRoundMetadata()` not originally planned.
>
> Also note: `feedItems` key expression (`item.messageId` for separators) must be updated when the type changes. Both virtual scroll AND standard rendering paths must be updated.

### TDD

**RED - Test (pure functions):**
```typescript
// src/lib/utils/__tests__/activityUtils.test.ts
describe('computeRoundMetadata', () => {
  it('should compute round number, agent name, and count from activities', () => {
    const activities = [
      { metadata: { messageId: 'msg-1', agentName: 'Research Agent' } },
      { metadata: { messageId: 'msg-1' } },
      { metadata: { messageId: 'msg-2', agentName: 'Writer Agent' } },
    ];
    const rounds = computeRoundMetadata(activities);
    expect(rounds).toHaveLength(2);
    expect(rounds[0]).toEqual({ messageId: 'msg-1', round: 1, agentName: 'Research Agent', count: 2 });
    expect(rounds[1]).toEqual({ messageId: 'msg-2', round: 2, agentName: 'Writer Agent', count: 1 });
  });

  it('should handle activities without agent name', () => {
    const activities = [{ metadata: { messageId: 'msg-1' } }];
    const rounds = computeRoundMetadata(activities);
    expect(rounds[0].agentName).toBeUndefined();
  });
});

describe('formatRoundSeparator', () => {
  it('should format round with number and agent name', () => {
    const result = formatRoundSeparator(1, 'Research Agent', 3);
    expect(result).toBe('Round 1 - Research Agent (3 activities)');
  });

  it('should format round without agent name', () => {
    const result = formatRoundSeparator(2, undefined, 5);
    expect(result).toBe('Round 2 (5 activities)');
  });
});
```

**GREEN - Fix:**

1. Create `src/lib/utils/activityUtils.ts` with two pure functions:
   - `computeRoundMetadata(activities)` - groups by messageId, extracts round number/agent name/count
   - `formatRoundSeparator(roundNumber, agentName?, activityCount)` - formats the display string

2. In `workflow/ActivityFeed.svelte`, replace the current `feedItems` derived (lines 131-144) to use `computeRoundMetadata()` when building separators, enriching them with the computed metadata.

3. Update the `FeedItem` separator type:
```typescript
type FeedItem =
  | { kind: 'activity'; data: WorkflowActivityEvent }
  | { kind: 'separator'; messageId: string; round: number; agentName?: string; count: number };
```

4. Update the `{#each}` key expression for separators (currently uses `"sep-" + item.messageId`) in both the virtual scroll path (lines 196-200) AND standard rendering path.

```svelte
<!-- workflow/ActivityFeed.svelte - separator rendering -->
{#if item.kind === 'separator'}
  <div class="round-separator">
    <span class="round-label">{formatRoundSeparator(item.round, item.agentName, item.count)}</span>
  </div>
{/if}
```

**REFACTOR:** Consider adding a timestamp to the separator (time since workflow start).

---

## Phase 7 - Temporal Grouping of Workflows (U2)

**Effort:** ~1.5h
**Files:** `workflow/WorkflowList.svelte`, new utility `src/lib/utils/dateGrouping.ts`

### Problem

All workflows are listed in a flat list with no temporal context. Users cannot quickly find today's conversations vs older ones.

### TDD

**RED - Test (pure function, TDD mandatory):**
```typescript
// src/lib/utils/__tests__/dateGrouping.test.ts
describe('groupByDate', () => {
  const now = new Date('2026-02-21T14:00:00Z');

  it('should group today items', () => {
    const items = [{ id: '1', updated_at: '2026-02-21T10:00:00Z' }];
    const groups = groupByDate(items, 'updated_at', now);
    expect(groups[0].label).toBe('today');
    expect(groups[0].items).toHaveLength(1);
  });

  it('should group yesterday items', () => {
    const items = [{ id: '1', updated_at: '2026-02-20T10:00:00Z' }];
    const groups = groupByDate(items, 'updated_at', now);
    expect(groups[0].label).toBe('yesterday');
  });

  it('should group last 7 days items', () => {
    const items = [{ id: '1', updated_at: '2026-02-17T10:00:00Z' }];
    const groups = groupByDate(items, 'updated_at', now);
    expect(groups[0].label).toBe('last_7_days');
  });

  it('should group older items', () => {
    const items = [{ id: '1', updated_at: '2026-01-01T10:00:00Z' }];
    const groups = groupByDate(items, 'updated_at', now);
    expect(groups[0].label).toBe('older');
  });

  it('should handle mixed dates and sort within groups', () => {
    const items = [
      { id: '1', updated_at: '2026-02-21T08:00:00Z' },
      { id: '2', updated_at: '2026-02-21T12:00:00Z' },
      { id: '3', updated_at: '2026-02-20T10:00:00Z' },
      { id: '4', updated_at: '2026-01-15T10:00:00Z' },
    ];
    const groups = groupByDate(items, 'updated_at', now);
    expect(groups).toHaveLength(3); // today, yesterday, older
    expect(groups[0].label).toBe('today');
    expect(groups[0].items).toHaveLength(2);
    expect(groups[0].items[0].id).toBe('2'); // most recent first
  });

  it('should return empty array for empty input', () => {
    expect(groupByDate([], 'updated_at', now)).toEqual([]);
  });
});
```

**GREEN - Implementation:**

1. Create `src/lib/utils/dateGrouping.ts`:

```typescript
export type DateGroupLabel = 'today' | 'yesterday' | 'last_7_days' | 'older';

export interface DateGroup<T> {
  label: DateGroupLabel;
  items: T[];
}

export function groupByDate<T>(
  items: T[],
  dateField: keyof T,
  now?: Date
): DateGroup<T>[] {
  // Pure function: categorize items by date proximity
  // Sort items within each group by most recent first
  // Skip empty groups
}
```

2. Modify `WorkflowList.svelte`:
   - In expanded mode, replace the flat "Workflows" section with grouped sections
   - Running and recently completed sections remain at the top (as-is)
   - The remaining workflows get grouped by date
   - Each group gets a section header: "Today", "Yesterday", "Last 7 days", "Older"
   - i18n keys: `workflow.group.today`, `workflow.group.yesterday`, `workflow.group.last_7_days`, `workflow.group.older`

3. In collapsed mode: no change (compact items don't have space for headers)

**REFACTOR:** Labels use i18n. DateGroup utility is generic and reusable.

---

## Additional Fixes

Bugs discovered and fixed during implementation of Phases 1-7:

### BF1: Missing `rename_workflow` Tauri Command

**Problem:** `workflow.service.ts` called `invoke('rename_workflow')` but no corresponding Rust command existed. Error: "command rename_workflow not found".

**Fix:** Added `rename_workflow` command in `src-tauri/src/commands/workflow.rs` with:
- UUID validation via `validate_uuid_field()`
- Name validation via `Validator::validate_workflow_name()`
- Parameterized query via `serialize_for_query()`
- Registered in `main.rs` `generate_handler![]`

### BF2: Space Key Not Working in Workflow Rename Input

**Problem:** When editing a workflow name inline, pressing space triggered workflow selection instead of typing a space character. The `handleEditKeydown` handler in `WorkflowItem.svelte` did not call `stopPropagation()`, so the space keypress bubbled to the parent div's `handleKeydown` which intercepted it.

**Fix:** Added `event.stopPropagation()` at the end of `handleEditKeydown()` in `WorkflowItem.svelte`.

### BF3: ConfirmDeleteModal Not Using Standard Modal

**Problem:** `ConfirmDeleteModal.svelte` had custom styles duplicating the standard `Modal` UI component instead of using it.

**Fix:** Rewrote to use `Modal` from `$lib/components/ui/Modal.svelte` with `{#snippet body()}` and `{#snippet footer()}` slots. Only 3 scoped styles remain (`.confirm-text`, `.workflow-name`, `.delete-warning`).

---

## Dead Code Cleanup

**Status:** Deferred (not blocking, Phase 8 cancelled)
**Files:** `src/lib/components/chat/StreamingMessage.svelte`, `src/lib/components/chat/ToolExecution.svelte`, `src/lib/components/chat/ReasoningStep.svelte`, `src/lib/components/chat/index.ts`

### Analysis

> **CONFIRMED (audit 2026-02-22):** `StreamingMessage.svelte` is dead code - only exported from `chat/index.ts` (line 32), never imported by any component or page.
>
> `ToolExecution.svelte` and `ReasoningStep.svelte` are also dead code after StreamingMessage removal:
> - `ToolExecution.svelte` is exported from `chat/index.ts` (line 29), only imported by `StreamingMessage.svelte`
> - `ReasoningStep.svelte` is exported from `chat/index.ts` (line 31), only imported by `StreamingMessage.svelte`
> - Neither is used by `workflow/ActivityItem.svelte` or `workflow/ReasoningPanel.svelte` (separate implementations)

### Action (deferred)

Since Phase 8 was cancelled, these components remain as dead code. They can be safely deleted in a future cleanup:

1. Delete `src/lib/components/chat/StreamingMessage.svelte` (325 lines)
2. Delete `src/lib/components/chat/ToolExecution.svelte` (dead after StreamingMessage removal)
3. Delete `src/lib/components/chat/ReasoningStep.svelte` (dead after StreamingMessage removal)
4. Remove their exports from `src/lib/components/chat/index.ts`
5. Verify no imports remain: `grep -r "StreamingMessage\|ToolExecution\|ReasoningStep" src/ --include="*.svelte" --include="*.ts"`

---

## Out of Scope

| ID | Issue | Why Deferred |
|----|-------|--------------|
| P4 | AgentSelector vertical layout in header | Low impact, cosmetic. Header works at all breakpoints. |
| U5 | TokenDisplay too dense | Low impact. Information is useful for power users. Responsive already hides items <700px. |
| P5 | Thinking not displayed in chat area | Cancelled (was Phase 8). Architectural complexity: requires deep integration with streaming store, `isStillViewed()` guard issues, and triple ChatContainer modification risk. Thinking is available in the Activity Sidebar. |
| P6 | Fake streaming - progressive activity in chat | Cancelled (was Phase 8). Same architectural concerns as P5. The `isStillViewed()` guard in `workflowExecutor.service.ts` prevents streaming state from being properly reset, causing UI freezes. Would require rearchitecting the streaming/background workflow coordination. |
| Real streaming | True token-by-token from LLM provider | Requires deep changes to rig.rs adapter layer, orchestrator async streaming, and Tauri event pipeline. |

---

## Validation

### Per-Phase

Each phase must pass before proceeding to the next:

| Phase | Validation | Status |
|-------|------------|--------|
| 1 | Single scrollbar visible, auto-scroll works | **DONE** |
| 2 | Labels visible expanded, hidden collapsed | **DONE** |
| 3 | `npm run test` - store filter tests pass | **DONE** |
| 4 | Markdown renders during streaming | **DONE** |
| 5 | Edit icon visible on hover, F2 shortcut works | **DONE** |
| 6 | `npm run test` - separator format tests pass | **DONE** |
| 7 | `npm run test` - date grouping tests pass, groups visible in UI | **DONE** |
| BF1 | `rename_workflow` command works from UI | **DONE** |
| BF2 | Space key works in rename input | **DONE** |
| BF3 | Delete modal uses standard Modal design | **DONE** |

### Global

After all phases:

```bash
# Frontend
npm run lint && npm run check && npm run test

# Backend (rename_workflow command added)
cd src-tauri && cargo fmt --check && cargo clippy -- -D warnings && cargo test
```

### Manual Smoke Test

1. Create a new workflow
2. Execute with an agent that uses tools
3. After completion: see markdown-rendered response
4. Rename a workflow via edit icon (confirm space key works)
5. Rename a workflow via double-click
6. Rename a workflow via F2 shortcut
7. Delete a workflow (confirm standard modal design)
8. Verify workflow list groups by date
9. Verify activity sidebar shows labels when expanded
10. Verify single scrollbar in chat area

---

## Audit Review (2026-02-22)

Post-planning code audit comparing plan assumptions against actual codebase state.

### Risk Assessment Summary

| Phases | Confidence | Regression Risk | Implementable As-Is? | Outcome |
|--------|-----------|-----------------|----------------------|---------|
| 1-2 | 90% | Low | Yes (minor corrections) | **DONE** |
| 3-5 | 80% | Low-Medium | Yes (minor corrections) | **DONE** |
| 6 | 65% | Medium | No - missing metadata computation | **DONE** (computation added) |
| 7 | 75% | Low | Yes | **DONE** |
| 8 | 40-50% -> 70-80% | High -> Medium | No as-is -> Yes after split | **CANCELLED** (UI bugs, `isStillViewed()` issues) |

### Corrections Applied

| # | Issue | Correction |
|---|-------|------------|
| 1 | 6 incorrect file paths | All paths corrected to actual locations (`agent/`, `workflow/`, `ui/`) |
| 2 | `$t()` i18n syntax | Corrected to `$i18n()` (project convention) |
| 3 | Phase 3 tests "to write" | Noted as already existing (`workflows.test.ts` L281-337) |
| 4 | Phase 4 cursor positioning | Added attention block with 2 fix options |
| 5 | Phase 4 CSS `white-space: pre-wrap` conflict | Added attention block |
| 6 | Phase 6 missing metadata | Added `computeRoundMetadata()` function + tests + dual rendering path note |
| 7 | Phase 8 effort estimate | Corrected from ~4h to ~2-3 days |
| 8 | Phase 8 architecture | Split into 4 sub-phases (8a/8b/8c/8d) - ultimately cancelled |
| 9 | Phase 8 store imports | Changed to props-only pattern - ultimately cancelled |
| 10 | Dead code: ToolExecution + ReasoningStep | Confirmed dead, deferred for future cleanup |

### Phase 8 Cancellation Rationale

Phase 8 was implemented but caused critical UI bugs:
- Streaming area persisted after execution (spinner + "Processing your request..." remained)
- `isStillViewed()` guard in `workflowExecutor.service.ts` returned `false`, preventing `streamingStore.reset()` and `onAssistantMessage` from executing
- Attempted fix (using `streamingStore.getState().workflowId` as fallback check) made the entire UI unresponsive - could not switch workflows
- Decision: **Cancel Phase 8** and move P5/P6 to Out of Scope. The Activity Sidebar already provides thinking/tool visibility.

### ChatContainer Modifications (Phases 1 + 4 only)

`agent/ChatContainer.svelte` was modified in 2 phases (Phase 8 cancelled):
- **Phase 1**: Remove double scroll (CSS + scroll logic)
- **Phase 4**: Add MarkdownRenderer to streaming (template + CSS)
