# SA-009 - Svelte Stores Quality Audit

**Date**: 2026-02-19
**Scope**: All 17 stores in `src/lib/stores/`
**Status**: Documented
**Summary**: 6 distinct patterns, 0 Svelte 5 runes usage, ~280 lines duplication, 6 error handling inconsistencies

---

## 1. Store Matrix

| # | Store | File | Lines | Pattern | Writable | Derived | get() | Error Handling | Types |
|---|-------|------|-------|---------|----------|---------|-------|----------------|-------|
| 1 | agentStore | agents.ts | 251 | CRUD Factory | 0 (factory) | 8 | 0 | getErrorMessage (factory) | $types/agent |
| 2 | promptStore | prompts.ts | 260 | CRUD Factory + ext | 0 (factory) | 9 | 0 | getErrorMessage (mixed) | $types/prompt |
| 3 | workflowStore | workflows.ts | 164 | Object Literal | 1 | 7 | 1 | manual | $types/workflow |
| 4 | activityStore | activity.ts | 223 | Object Literal | 1 | 6 | 2 | manual | $types/activity |
| 5 | streamingStore | streaming.ts | 762 | Object Literal | 1 | 14 | 2 | N/A (state only) | $types/streaming |
| 6 | tokenStore | tokens.ts | 257 | Object Literal | 1 | 4 | 0 | N/A (state only) | $types/workflow, $types/llm |
| 7 | backgroundWorkflowsStore | backgroundWorkflows.ts | 636 | Object Literal | 1 | 9 | 6 | manual | $types/streaming, $types/background-workflow |
| 8 | toastStore | toast.ts | 184 | Object Literal | 2 | 4 | 0 | N/A | $types/background-workflow |
| 9 | userQuestionStore | userQuestion.ts | 380 | Object Literal | 1 | 6 | 0* | getErrorMessage | $types/user-question |
| 10 | validationStore | validation.ts | 306 | Object Literal | 1 | 5 | 3 | manual | $types/validation, $types/sub-agent |
| 11 | validationSettingsStore | validation-settings.ts | 158 | Closure Function | 1 | 5 | 1 | manual | $types/validation |
| 12 | llmStore | llm.ts | 701 | Pure Functions | 0 | 0 | 0 | throws to caller | $types/llm, $types/customProvider |
| 13 | mcpStore | mcp.ts | 454 | Pure Functions | 0 | 0 | 0 | throws to caller | $types/mcp |
| 14 | theme | theme.ts | 82 | Object Literal | 1 | 0 | 1 | N/A | inline (Theme) |
| 15 | localeStore | locale.ts | 109 | Closure Function | 1 | 2 | 1 | N/A | $types/i18n |
| 16 | onboardingStore | onboarding.ts | 176 | Closure Function | 1 | 9 | 0 | N/A | $types/onboarding |
| **Total** | | | **5,103** | | **13** | **88** | **17** | | |

\* userQuestion uses subscribe/unsub hack instead of get() (see finding F6)

---

## 2. Pattern Taxonomy

| Pattern | Count | Stores |
|---------|-------|--------|
| **Object Literal + writable** | 9 | activity, backgroundWorkflows, streaming, theme, toast, tokens, userQuestion, validation, workflows |
| **Closure Function + writable** | 3 | locale, onboarding, validationSettings |
| **CRUD Factory** | 2 | agents, prompts |
| **Pure Functions (no store)** | 2 | llm, mcp |

### Observation

All 15 stores that use Svelte stores use the **Svelte 4 `writable/derived/get`** pattern. **Zero** stores use Svelte 5 runes (`$state`, `$derived`, `$effect`). This is valid (Svelte 5 supports legacy stores) but inconsistent with components that use runes.

---

## 3. Findings

### F1: Duplicated Chunk Processing (~280 lines)

**Severity**: MEDIUM (maintenance risk)

`streaming.ts` and `backgroundWorkflows.ts` both process the same `StreamChunk` types with near-identical logic:

| Chunk Type | streaming.ts | backgroundWorkflows.ts |
|------------|-------------|----------------------|
| token | handleToken (L197-210) | case 'token' (L157-160) |
| tool_start | handleToolStart (L215-227) | case 'tool_start' (L162-171) |
| tool_end | handleToolEnd (L232-241) | case 'tool_end' (L173-179) |
| reasoning | handleReasoning (L246-258) | case 'reasoning' (L181-190) |
| error | handleError (L263-269) | case 'error' (L192-194) |
| sub_agent_start | handleSubAgentStart (L274-290) | case 'sub_agent_start' (L196-210) |
| sub_agent_progress | handleSubAgentProgress (L295-308) | case 'sub_agent_progress' (L212-220) |
| sub_agent_complete | handleSubAgentComplete (L313-329) | case 'sub_agent_complete' (L223-236) |
| sub_agent_error | handleSubAgentError (L334-348) | case 'sub_agent_error' (L238-249) |
| task_create | handleTaskCreate (L353-368) | case 'task_create' (L251-263) |
| task_update | handleTaskUpdate (L373-382) | case 'task_update' (L265-271) |
| task_complete | handleTaskComplete (L387-394) | case 'task_complete' (L273-278) |

**streaming.ts**: 12 named handler functions with registry pattern
**backgroundWorkflows.ts**: 1 big switch/case `updateExecutionFromChunk`

Both produce structurally identical state updates on different state shapes (`StreamingState` vs `WorkflowStreamState`).

**Recommendation**: Extract a shared chunk-to-state-delta mapper, or have backgroundWorkflows delegate to streaming's handlers.

---

### F2: Error Handling Inconsistency

**Severity**: LOW (code quality)

6 stores use manual error extraction instead of the project-standard `getErrorMessage()`:

| Store | Location | Current | Should Use |
|-------|----------|---------|-----------|
| activity.ts | L93 | `e instanceof Error ? e.message : String(e)` | `getErrorMessage(e)` |
| workflows.ts | L82 | `e instanceof Error ? e.message : String(e)` | `getErrorMessage(e)` |
| validation.ts | L189, L212 | `error instanceof Error ? error.message : String(error)` | `getErrorMessage(error)` |
| validation-settings.ts | L71, L89, L105 | `err instanceof Error ? err.message : String(err)` | `getErrorMessage(err)` |
| backgroundWorkflows.ts | implicit | (no error state updates) | N/A |
| locale.ts | L47 | `console.warn(...)` | Debatable (warning, not error) |

Additionally:
- `validation.ts` L142: uses `console.warn` which violates code standards

---

### F3: Module-Level Mutable State

**Severity**: LOW (testing/predictability)

Stores with `let` variables at module scope, outside the Svelte store subscription model:

| Store | Variables | Purpose |
|-------|-----------|---------|
| backgroundWorkflows.ts | `unlisteners`, `isInitialized`, `cleanupTimer`, `onChunkForViewed`, `onCompleteForViewed`, `onUserQuestion` (6) | Event listeners + callbacks |
| validation.ts | `unlistener`, `isInitialized` (2) | Event listener |
| llm.ts | `llmCache`, `filteredModelsCache` (2) | Response caching |
| mcp.ts | `mcpCache` (1) | Response caching |

These variables are invisible to Svelte's reactivity system. Changes to them don't trigger re-renders. This is intentional for event listeners and caches, but makes testing harder (can't reset via store.set()).

---

### F4: userQuestion.ts subscribe/unsub Hack

**Severity**: LOW (readability)

Lines 184-189 and 226-231 use a verbose pattern to read state synchronously:

```typescript
getQuestionsForWorkflow(workflowId: string): UserQuestion[] {
    let result: UserQuestion[] = [];
    const unsub = store.subscribe((s) => {
        result = s.pendingQuestions.filter((q) => q.workflowId === workflowId);
    });
    unsub();
    return result;
}
```

Same pattern at L226-231 (submitResponse) and L282-287 (skipQuestion).

This is functionally identical to `get(store).pendingQuestions.filter(...)` but is 5 lines instead of 1. The `get()` import from `svelte/store` is already used by other stores.

---

### F5: llm.ts and mcp.ts Are Not Stores

**Severity**: INFO (architectural)

These two modules export only pure functions and async IPC wrappers. They have:
- No `writable` or `derived`
- No subscription model
- Manual caching via module-level variables

Components using them must manage their own `$state` + error handling. This is a **fundamentally different pattern** from the other 15 stores. The inventory classifies them as `type: pure_functions`.

**Impact**: Consumers (settings pages) must duplicate loading/error/state patterns that the other stores centralize. The tradeoff is flexibility: components can compose these functions freely without being coupled to a single state shape.

---

### F6: Naming Inconsistencies

**Severity**: LOW (DX/discoverability)

#### Store Instance Names

| Convention | Stores | Example |
|------------|--------|---------|
| `xStore` | 13 | `agentStore`, `workflowStore`, `streamingStore` |
| bare name | 1 | `theme` (not `themeStore`) |

#### Derived Store Prefixes

| Pattern | Example | Stores |
|---------|---------|--------|
| No prefix | `isLoading`, `error`, `formMode` | agents |
| Store-specific prefix | `workflowsLoading`, `workflowsError` | workflows |
| Feature prefix | `activityLoading`, `activityError` | activity |
| Abbreviated | `isValidating`, `validationError` | validation |

This causes naming collisions in `index.ts` which requires manual aliasing (L39-46).

---

### F7: Deprecated/Legacy Code

**Severity**: LOW (cleanup)

| Store | Symbol | Status |
|-------|--------|--------|
| agents.ts | `AgentState` interface | `@deprecated Use agentStore instead` |
| agents.ts | `createInitialAgentState()` | `@deprecated Use agentStore instead` |
| agents.ts | `agentCount` derived | `@deprecated Use agents.length instead` |
| prompts.ts | `promptCount` derived | `@deprecated Use prompts.length instead` |
| tokens.ts | `isTokenStreaming` derived | `@deprecated Use streamingStore.isStreaming instead` |

---

### F8: CRUD Factory Coverage

**Severity**: INFO

Only 2 of 17 stores use `createCRUDStore`. Evaluation of other stores:

| Store | Has List | Has Get | Has Create | Has Update | Has Delete | Has Form | Candidate? |
|-------|----------|---------|------------|------------|------------|----------|------------|
| workflowStore | list | - | - | - | - | - | **No** (read-only list) |
| validationSettingsStore | - | load | - | update | - | - | **No** (singleton, no list) |
| tokenStore | - | - | - | update | - | - | **No** (derived metrics) |
| activityStore | load | - | - | - | - | - | **No** (read-only) |

**Conclusion**: Factory coverage is correct. No additional stores would benefit from it.

---

### F9: No get() in derived() (ERR_SVELTE_005)

**Severity**: NONE (no violations found)

All 17 `get()` calls in stores are in imperative methods (action handlers, sync getters), never inside `derived()` callbacks. No ERR_SVELTE_005 violations.

---

### F10: Potential Circular Dependencies

**Severity**: NONE (properly mitigated)

Dependency graph:
```
backgroundWorkflows -> toastStore, validationSettings
userQuestion -> backgroundWorkflowsStore, toastStore
activity -> streamingStore
streaming -> tokenStore
toast -> (no store deps)
```

The `backgroundWorkflows <-> streaming` and `backgroundWorkflows <-> userQuestion` relationships use callback injection (`setForwardCallbacks`) to avoid circular imports. This is correct.

---

## 4. Recommendations

### Priority 1: Fix Error Handling (F2)

Replace manual error extraction in 4 stores with `getErrorMessage()`:
- `activity.ts` L93
- `workflows.ts` L82
- `validation.ts` L189, L212
- `validation-settings.ts` L71, L89, L105

Remove `console.warn` in `validation.ts` L142.

**Effort**: ~30 min | **Impact**: Consistency

### Priority 2: Extract Shared Chunk Processor (F1)

Create `src/lib/stores/utils/chunkProcessor.ts` with a generic chunk-to-delta mapper. Both `streaming.ts` and `backgroundWorkflows.ts` would consume it.

**Effort**: ~2h | **Impact**: -280 lines duplication, single source of truth for chunk handling

### Priority 3: Simplify userQuestion.ts (F4)

Replace 3 subscribe/unsub hacks with `get(store)`:

```typescript
// Before (5 lines)
let result: UserQuestion[] = [];
const unsub = store.subscribe((s) => {
    result = s.pendingQuestions.filter(...);
});
unsub();
return result;

// After (1 line)
return get(store).pendingQuestions.filter(...);
```

**Effort**: ~15 min | **Impact**: Readability

### Priority 4: Remove Deprecated Code (F7)

Remove `AgentState`, `createInitialAgentState()`, `agentCount`, `promptCount`, `isTokenStreaming`. Check for references first with `find_referencing_symbols`.

**Effort**: ~30 min | **Impact**: Cleanup

### Priority 5: Standardize Derived Store Naming (F6)

Adopt consistent prefix convention. Two reasonable options:
- **Option A**: Always prefix with feature name: `agentLoading`, `agentError`, `workflowLoading`, `workflowError`
- **Option B**: No prefix (rely on import aliasing at consumer site)

Option A avoids collision issues in `index.ts` barrel exports.

**Effort**: ~2h (rename + update all consumers) | **Impact**: DX

### Not Recommended

- **Migrate to Svelte 5 runes**: All 15 stores work correctly with Svelte 4 patterns. Migration would be large (5,103 lines) with no functional benefit. Svelte 5 `$state` in modules (`.svelte.ts`) is stable but the ecosystem convention is still forming.
- **Force llm/mcp into store pattern**: Their pure-function pattern is intentional and gives consumers more flexibility.

---

## 5. Summary Statistics

| Metric | Value |
|--------|-------|
| Total stores | 17 |
| Total lines | 5,103 |
| Svelte 5 runes ($state/$derived/$effect) | **0** |
| Svelte 4 writable stores | 13 |
| Derived stores | 88 |
| get() calls | 17 |
| ERR_SVELTE_005 violations | 0 |
| CRUD factory users | 2/17 |
| Inconsistent error handling | 6 stores |
| Duplicated logic | ~280 lines (streaming/backgroundWorkflows) |
| Deprecated symbols | 5 |
| Circular dependency risks | 0 (mitigated by callbacks) |
| console.warn violations | 2 (locale, validation) |
