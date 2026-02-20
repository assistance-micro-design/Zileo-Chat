# SA-011: Chat & Workflow Components Quality Audit

**Date:** 2026-02-19
**Status:** Documented
**Scope:** `src/lib/components/chat/` (10 files, ~47KB), `src/lib/components/workflow/` (20 files, ~130KB), related stores and data flow
**Findings:** 0 CRITICAL, 3 HIGH, 12 MEDIUM, 26 LOW

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Chat Components Map](#2-chat-components-map)
3. [Workflow Components Map](#3-workflow-components-map)
4. [Data Flow Architecture](#4-data-flow-architecture)
5. [Fragility Points](#5-fragility-points)
6. [Findings Detail](#6-findings-detail)
7. [UX Impact Analysis](#7-ux-impact-analysis)
8. [Recommendations](#8-recommendations)

---

## 1. Executive Summary

The chat and workflow components form the core UI of Zileo-Chat-3. This audit covers **30 Svelte 5 components** totaling ~7,900 lines (chat: ~1,580 / workflow: ~6,320) and their interaction with **11 stores** and **~12 Tauri commands**.

**Overall quality: Good.** Chat components are exemplary (excellent a11y, i18n, reactivity). Workflow components are solid but accumulate minor issues (hardcoded values, a11y gaps, large derivations). The real risks live in the **store orchestration layer** where event routing, callback forwarding, and activity capture timing create fragility.

### Severity Distribution

| Severity | Count | Category |
|----------|-------|----------|
| HIGH | 3 | Data flow fragility (capture timing, missing retry, double-submit) |
| MEDIUM | 12 | Error handling gaps, large derivations, a11y holes |
| LOW | 26 | Hardcoded values, minor a11y, style inconsistencies |

---

## 2. Chat Components Map

### Component Metrics

| Component | Lines | Script | Template | Style | Functions | $state | $derived | $effect | Complexity |
|-----------|-------|--------|----------|-------|-----------|--------|----------|---------|------------|
| ChatInput | 257 | 115 | 39 | 103 | 4 | 2 | 1 | 0 | Low |
| MessageBubble | 193 | 68 | 28 | 97 | 2 | 1 | 1 | 0 | Low |
| MessageList | 153 | 70 | 24 | 59 | 1 | 1 | 1 | 1 | Low |
| MessageListSkeleton | 113 | 48 | 20 | 45 | 0 | 0 | 1 | 0 | None |
| MessageMetrics | 157 | 47 | 51 | 59 | 0 | 0 | 1 | 0 | Low |
| PromptSelectorModal | 433 | 167 | 98 | 168 | 5 | 5 | 4 | 0 | Medium |
| ReasoningStep | 152 | 50 | 32 | 70 | 1 | 0 | 0 | 0 | Low |
| StreamingMessage | 325 | 83 | 94 | 148 | 0 | 0 | 4 | 0 | Low |
| ToolExecution | 152 | 72 | 23 | 57 | 0 | 0 | 2 | 0 | Low |
| index.ts | 34 | 34 | - | - | 0 | 0 | 0 | 0 | None |
| **TOTAL** | **1,969** | **754** | **409** | **806** | **13** | **9** | **15** | **1** | - |

### Chat Props & Store Access

| Component | Props | Store Imports | invoke() | listen() |
|-----------|-------|---------------|----------|----------|
| ChatInput | `value`, `placeholder`, `disabled`, `loading`, `onsend` | i18n | 0 | 0 |
| MessageBubble | `message`, `isUser?` | i18n | 0 | 0 |
| MessageList | `messages`, `autoScroll?`, `performanceThreshold?` | none | 0 | 0 |
| MessageListSkeleton | `count?` | i18n | 0 | 0 |
| MessageMetrics | `message` | none | 0 | 0 |
| PromptSelectorModal | `open`, `onclose?`, `onselect?` | promptStore | 0 | 0 |
| ReasoningStep | `step`, `expanded`, `stepNumber?` | i18n | 0 | 0 |
| StreamingMessage | `content`, `tools?`, `reasoning?`, `isStreaming?`, `showTools?`, `showReasoning?` | none (types from streaming) | 0 | 0 |
| ToolExecution | `tool`, `status`, `duration?`, `error?` | i18n | 0 | 0 |

**Key observation:** Chat components are **pure prop-driven**. No direct store mutations, no invoke() calls, no listen() events. All data flows through the parent page. This is excellent architecture.

### Chat Conditional Rendering

| Component | Max {#if} Depth | Max {#each} Depth | Complex Conditions |
|-----------|-----------------|--------------------|--------------------|
| ChatInput | 2 | 0 | No |
| MessageBubble | 3 | 0 | No |
| MessageList | 3 | 1 (keyed) | No |
| MessageListSkeleton | 2 | 1 (keyed) | No |
| MessageMetrics | 2 | 1 | Chain of `||` |
| PromptSelectorModal | 4 | 2 | Filter + search |
| ReasoningStep | 2 | 0 | No |
| StreamingMessage | 3 | 2 (both keyed) | No |
| ToolExecution | 2 | 0 | No |

**Deepest nesting:** PromptSelectorModal at 4 levels (list/detail view toggle > variable form > each variables > empty states). Acceptable for a modal with search + filter + detail view.

### Chat Quality Assessment

**Strengths:**
- All `{#each}` blocks use keys (`message.id`, `step.timestamp`, `tool.name + tool.startedAt`)
- Excellent a11y: `role="log"`, `aria-live="polite"`, `aria-busy`, `aria-expanded`, `aria-controls`
- i18n coverage complete including plural forms
- Performance: CSS containment, `content-visibility`, `prefers-reduced-motion`
- Zero `$effect` cascades (only 1 effect total: scroll-to-bottom)
- Zero `any` types, zero `@ts-ignore`

**Issues found:** 2 (see [Findings Detail](#6-findings-detail))

---

## 3. Workflow Components Map

### Component Metrics

| Component | Lines | Script | Template | Style | Functions | $state | $derived | $effect | Complexity |
|-----------|-------|--------|----------|-------|-----------|--------|----------|---------|------------|
| ActivityFeed | 481 | 145 | 84 | 252 | 1 | 0 | 4 | 0 | Low |
| ActivityItem | 388 | 153 | 59 | 176 | 5 | 3 | 8 | 0 | Medium |
| ActivityItemDetails | 145 | 50 | 25 | 70 | 1 | 0 | 0 | 0 | Very Low |
| AgentSelector | 218 | 142 | 32 | 44 | 3 | 0 | 5 | 0 | Low |
| ConfirmDeleteModal | 274 | 84 | 52 | 138 | 4 | 1 | 0 | 1 | Low |
| MetricsBar | 130 | 58 | 35 | 37 | 2 | 0 | 0 | 0 | Very Low |
| NewWorkflowModal | 450 | 113 | 87 | 250 | 4 | 4 | 0 | 1 | Low |
| ReasoningDetailsPanel | 86 | 32 | 8 | 46 | 0 | 0 | 0 | 0 | None |
| ReasoningPanel | 458 | 169 | 84 | 205 | 3 | 1 | 6 | 0 | Medium |
| SubAgentActivity | 466 | 113 | 104 | 249 | 2 | 1 | 4 | 0 | Medium |
| TokenDisplay | 582 | 114 | 111 | 357 | 3 | 0 | 3 | 0 | Low |
| ToolDetailsPanel | 171 | 54 | 29 | 88 | 0 | 3 | 0 | 0 | Low |
| ToolExecutionPanel | 460 | 188 | 72 | 200 | 3 | 1 | 4 | 0 | Medium |
| UserQuestionModal | 323 | 113 | 77 | 133 | 4 | 2 | 0 | 1 | Medium |
| ValidationModal | 287 | 101 | 68 | 118 | 3 | 1 | 0 | 0 | Low |
| WorkflowItemCompact | 167 | 68 | 21 | 78 | 2 | 0 | 1 | 0 | Very Low |
| WorkflowItem | 271 | 123 | 41 | 107 | 5 | 3 | 0 | 1 | Low |
| WorkflowList | 236 | 82 | 83 | 71 | 0 | 0 | 3 | 0 | Low |
| index.ts | 40 | 40 | - | - | 0 | 0 | 0 | 0 | None |
| **TOTAL** | **5,633** | **1,942** | **1,072** | **2,619** | **45** | **20** | **38** | **4** | - |

### Workflow Props & Store Access

| Component | Store Imports | invoke() | listen() |
|-----------|---------------|----------|----------|
| ActivityFeed | none (types only) | 0 | 0 |
| ActivityItem | none | 0 | 0 |
| ActivityItemDetails | none | 0 | 0 |
| AgentSelector | none | 0 | 0 |
| ConfirmDeleteModal | none | 0 | 0 |
| MetricsBar | none | 0 | 0 |
| NewWorkflowModal | none | 0 | 0 |
| ReasoningDetailsPanel | none | 0 | 0 |
| ReasoningPanel | none | 0 | 0 |
| SubAgentActivity | streaming types | 0 | 0 |
| TokenDisplay | none | 0 | 0 |
| ToolDetailsPanel | none | **1** (`get_tool_execution`) | 0 |
| ToolExecutionPanel | none | 0 | 0 |
| UserQuestionModal | **userQuestionStore** (4 exports) | 0 | 0 |
| ValidationModal | none | 0 | 0 |
| WorkflowItemCompact | none | 0 | 0 |
| WorkflowItem | none | 0 | 0 |
| WorkflowList | none | 0 | 0 |

**Key observation:** Like chat components, workflow components are mostly **prop-driven**. Only 2 exceptions: ToolDetailsPanel (lazy-loads via invoke) and UserQuestionModal (direct store access). Good architecture.

### Workflow Reactivity Summary

| Pattern | Count | Notes |
|---------|-------|-------|
| $state | 20 | Mostly local UI state (expansion, editing, form inputs) |
| $derived | 38 | Heavy use for filtering, aggregation, status mapping |
| $effect | 4 | State reset on prop change (modals), edit sync |
| $effect writing to $state | 4 | All legitimate (modal reset, not cascading) |
| Potential $effect -> $derived | 0 | No violations found |
| Cascade risk | 0 | No $effect chains detected |

**Large derivations flagged:**
- `ReasoningPanel.displaySteps` (~40 lines): merges active + historical steps, maps fields, sorts
- `ToolExecutionPanel.displayExecutions` (~60 lines): merges 3 data sources, maps, filters

### Workflow State Machines

| Component | States | Transitions | Pattern |
|-----------|--------|-------------|---------|
| ConfirmDeleteModal | `idle -> deleting -> closed` | via `isDeleting` flag | Simple boolean |
| NewWorkflowModal | `idle -> submitting -> closed/error` | via `isSubmitting` + `error` | Form state |
| UserQuestionModal | `idle -> answering -> submitting -> closed` | via store state | Store-driven |
| ValidationModal | `pending -> approved/rejected` | via callback handlers | Callback-based |
| ToolDetailsPanel | `loading -> loaded/error` | via `loading`/`error`/`execution` | Tri-state |
| WorkflowItem | `viewing -> editing -> viewing` | via `editing` flag | Toggle |

**No formal state machine library used.** All state transitions are ad-hoc boolean/enum states. This works at current complexity but may not scale if more states are added.

---

## 4. Data Flow Architecture

### Complete Event Chain

```
USER INPUT                    STORES                          BACKEND
-----------                   ------                          -------

ChatInput.onsend()
    |
    v
+page.svelte
    |-- MessageService.save()  --------------------------------> save_message
    |-- streamingStore.start()
    |-- tokenStore.startStreaming()
    |-- bgWorkflowsStore.register()
    |-- WorkflowService.executeStreaming() --------------------> execute_workflow_streaming
                                                                     |
                                                                     | (emits events)
                                                                     v
                              backgroundWorkflowsStore  <---- listen('workflow_stream')
                                  |                     <---- listen('workflow_complete')
                                  |
                        +---------+---------+
                        |                   |
                  (if viewed)          (always)
                        |                   |
                        v                   v
                  streamingStore     executions Map
                        |           (background state)
                        |
            +-----------+-----------+
            |           |           |
            v           v           v
    StreamingMessage  TokenDisplay  ActivityFeed
    (content+tools)   (token count) (via capture)
```

### Store Dependency Graph

```
agent/+page.svelte (ORCHESTRATOR)
    |
    +-- workflowStore -----------> invoke('load_workflows')
    |       |
    |       +-- WorkflowService -> invoke('create_workflow')
    |                           -> invoke('rename_workflow')
    |                           -> invoke('delete_workflow')
    |                           -> invoke('execute_workflow_streaming')
    |                           -> invoke('cancel_workflow_streaming')
    |
    +-- agentStore --------------> invoke('list_agents')
    |                           -> invoke('get_agent_config')
    |
    +-- streamingStore ----------> (no direct invoke, callback-driven)
    |       |
    |       +-- receives from backgroundWorkflowsStore.onChunkForViewed()
    |
    +-- backgroundWorkflowsStore -> listen('workflow_stream')    [OWNS]
    |       |                    -> listen('workflow_complete')   [OWNS]
    |       |
    |       +-- forwards to streamingStore (viewed only)
    |       +-- forwards to userQuestionStore (all workflows)
    |       +-- forwards to toastStore (completions)
    |
    +-- activityStore -----------> ActivityService.loadAll()
    |                           -> captureStreamingActivities() [TIMING CRITICAL]
    |
    +-- validationStore ---------> listen('validation_required') [OWNS]
    |                           -> invoke('approve_validation')
    |                           -> invoke('reject_validation')
    |
    +-- userQuestionStore -------> invoke('submit_user_response')
    |                           -> invoke('skip_question')
    |
    +-- tokenStore --------------> (no invoke, state-only)
    |
    +-- toastStore --------------> (no invoke, UI notifications)
    |
    +-- promptStore -------------> invoke('list_prompts')
                                -> invoke('get_prompt')
```

### Initialization Order (Critical)

```
onMount in agent/+page.svelte:

1. workflowStore.loadWorkflows()              // Populate sidebar
2. agentStore.loadAgents()                     // Load agent list
3. backgroundWorkflowsStore.init()             // Setup event listeners
4. backgroundWorkflowsStore.setForwardCallbacks({
     onChunkForViewed,                         // -> streamingStore
     onCompleteForViewed,                      // -> streamingStore
     onUserQuestion                            // -> userQuestionStore
   })
5. validationStore.init()                      // Setup validation listener
6. userQuestionStore.init()                     // Init question queue
7. selectWorkflow(lastWorkflowId)              // Restore last view
```

**Risk:** Steps 3 and 4 MUST happen in this order. If callbacks are set before init, they may be overwritten. If init happens but callbacks aren't set, the viewed workflow gets no streaming updates.

---

## 5. Fragility Points

### P0 - HIGH (Fix before next release)

#### H-001: Activity Capture Race Condition
- **Location:** `activityStore.captureStreamingActivities()`
- **Risk:** Must be called AFTER streaming completes but BEFORE `streamingStore.reset()`. If order is wrong, activities from the last streaming session are lost.
- **Impact:** User loses tool execution history and reasoning steps for the completed workflow.
- **Fix:** Add a guard (e.g., `capturedForWorkflow` set) to prevent duplicate captures, and ensure the orchestrator calls capture before reset.

#### H-002: No Error Recovery on loadWorkflows
- **Location:** `workflowStore.loadWorkflows()` in `+page.svelte`
- **Risk:** If the initial load fails (DB unavailable, migration issue), the sidebar shows an empty list with no retry mechanism. User has no way to recover without restarting the app.
- **Impact:** Complete loss of workflow navigation.
- **Fix:** Show error state with retry button in WorkflowList. Add `workflowStore.retry()` or automatic retry with exponential backoff.

#### H-003: No Double-Submit Protection on Send
- **Location:** `agent/+page.svelte` send handler
- **Risk:** User can click Send multiple times before the first `execute_workflow_streaming` starts. No button disable during the invoke() call. Could trigger concurrent streams for the same workflow.
- **Impact:** Duplicate messages, confused streaming state, potential backend errors.
- **Fix:** Disable Send button immediately when `handleSend()` is called, re-enable on error or streaming start.

### P1 - MEDIUM

#### M-001: Unhandled Clipboard Error
- **File:** `MessageBubble.svelte:53-58`
- **Code:** `navigator.clipboard.writeText()` not wrapped in try/catch
- **Impact:** Silent failure if clipboard API unavailable (e.g., insecure context, permission denied)
- **Fix:** Wrap in try/catch, show error toast

#### M-002: Console.error Without User Feedback
- **File:** `PromptSelectorModal.svelte:132-133`
- **Code:** `console.error('Failed to load prompt:', e)` in catch block
- **Impact:** User sees loading spinner forever if prompt load fails
- **Fix:** Set error state, show error message in modal

#### M-003: Large Derivation in ReasoningPanel
- **File:** `ReasoningPanel.svelte` - `displaySteps` derivation (~40 lines)
- **Impact:** Hard to maintain, hard to test, merges active + historical data with field mapping and sorting
- **Fix:** Extract to utility function `mergeAndSortReasoningSteps()`

#### M-004: Large Derivation in ToolExecutionPanel
- **File:** `ToolExecutionPanel.svelte` - `displayExecutions` derivation (~60 lines)
- **Impact:** Same as M-003, merges 3 data sources
- **Fix:** Extract to utility function `mergeToolExecutions()`

#### M-005: Validation Timeout Missing
- **Location:** `validationStore`
- **Risk:** If user ignores a validation request, the backend workflow hangs indefinitely waiting for approval
- **Fix:** Auto-reject after configurable timeout (e.g., 5 minutes) with warning toast

#### M-006: UserQuestionModal console.warn
- **File:** `UserQuestionModal.svelte`
- **Code:** Uses `console.warn()` instead of proper logging
- **Fix:** Remove or replace with appropriate handling

#### M-007: ActivityItem Expansion State
- **File:** `ActivityItem.svelte`
- **Code:** 3 separate boolean states (`isTaskExpanded`, `isReasoningExpanded`, `isToolExpanded`)
- **Impact:** Could allow multiple sections expanded simultaneously (may be intended but wastes space)
- **Fix:** Consider single `expandedSection: 'none' | 'task' | 'reasoning' | 'tool'` enum

#### M-008: setTimeout for Focus Management
- **Files:** `NewWorkflowModal.svelte`, `WorkflowItem.svelte`
- **Code:** `setTimeout(() => ref?.focus(), 0/50)`
- **Impact:** Non-deterministic timing, may fail on slower devices
- **Fix:** Use Svelte's `tick()` or `onMount` callback

#### M-009: TokenDisplay Progress Bar Accessibility
- **File:** `TokenDisplay.svelte`
- **Code:** Has `role="progressbar"` but visual warning states (75%, 90%, 100%) not reflected in ARIA attributes
- **Fix:** Add `aria-valuetext` describing current level ("Warning: 78% of context used")

#### M-010: backgroundWorkflowsStore Cleanup Timer
- **Location:** `backgroundWorkflows.ts:45`
- **Risk:** Cleanup runs after 10 minutes, could theoretically clean up a still-running execution
- **Fix:** Add `status !== 'running'` guard before cleanup

#### M-011: WorkflowItem State Sync During Edit
- **File:** `WorkflowItem.svelte`
- **Code:** Effect syncs editName only when `!editing` - misses external renames while user is editing
- **Impact:** If workflow is renamed externally (e.g., by another session) while user edits, the old name persists
- **Fix:** Minor - acceptable tradeoff, document behavior

#### M-012: ToolDetailsPanel No Retry
- **File:** `ToolDetailsPanel.svelte`
- **Code:** If `get_tool_execution` invoke fails, no retry mechanism
- **Fix:** Add retry button in error state

### P2 - LOW (26 items)

#### Hardcoded Values (14 instances)

| File | Value | Should Be |
|------|-------|-----------|
| SubAgentActivity | `max-height: 300px` | CSS variable |
| ReasoningDetailsPanel | `max-height: 300px` | CSS variable |
| ReasoningPanel | `previewLength: 200` | Prop with default |
| ToolExecutionPanel | `max-height: 200px` | CSS variable |
| ToolDetailsPanel | `maxDepth={3}` | Prop with default |
| TokenDisplay | `75%/90%/100%` thresholds | Constants or props |
| MetricsBar | `toFixed(4)` cost precision | Utility constant |
| NewWorkflowModal | `max-height: 200px` agent list | CSS variable |
| SubAgentActivity | `slice(0, 100)` task description | Constant |
| ConfirmDeleteModal | `.icon-wrapper` 40px | CSS variable |
| WorkflowItemCompact | Hardcoded animation timing | CSS variable |
| ActivityFeed | `defaultEstimatedItemHeight={72}` | Measured or CSS var |
| ActivityItemDetails | Priority key pattern | Validated mapping |
| ValidationModal | Risk level mapping | Constants file |

#### Accessibility Gaps (8 instances)

| File | Gap | Severity |
|------|-----|----------|
| MessageMetrics | No aria-labels on metrics section | LOW |
| ToolExecution | Missing aria-label on container | LOW |
| ActivityFeed | Filter tabs aria-controls too generic | LOW |
| ActivityItem | Toggle buttons missing aria-controls | LOW |
| AgentSelector | Label missing aria-label | LOW |
| MetricsBar | role="status" but no aria-live | LOW |
| NewWorkflowModal | Agent selector buttons lack aria-label | LOW |
| WorkflowList | Section headers not semantic headings | LOW |

#### Style Inconsistencies (4 instances)

| File | Issue |
|------|-------|
| AgentSelector | `--border-radius-sm` vs `--radius-sm` inconsistency |
| WorkflowItemCompact | `pulse-ring` animation potentially distracting |
| SubAgentActivity | Manual string truncation instead of utility |
| WorkflowList | Similar rendering code for 3 workflow categories |

---

## 6. Findings Detail

### Chat Components: Findings Summary

| ID | File | Finding | Severity |
|----|------|---------|----------|
| C-001 | MessageBubble | Unhandled clipboard error | MEDIUM |
| C-002 | PromptSelectorModal | console.error without user feedback | MEDIUM |

**Strengths (no issues):**
- ChatInput: Excellent a11y (aria-labels, keyboard hints, type="button")
- MessageList: Exemplary (role="log", aria-live, CSS containment, keyed each)
- StreamingMessage: Best-in-class (aria-busy, native details/summary, plural i18n)
- ReasoningStep: Clean (aria-expanded, aria-controls, bindable props)
- MessageListSkeleton: Correct (role="presentation")

### Workflow Components: Findings Summary

| ID | File | Finding | Severity |
|----|------|---------|----------|
| W-001 | ReasoningPanel | Large derivation (~40 lines) | MEDIUM |
| W-002 | ToolExecutionPanel | Large derivation (~60 lines) | MEDIUM |
| W-003 | NewWorkflowModal | setTimeout for focus | MEDIUM |
| W-004 | WorkflowItem | setTimeout for focus | MEDIUM |
| W-005 | UserQuestionModal | console.warn in production | MEDIUM |
| W-006 | ActivityItem | 3 booleans instead of enum | MEDIUM |
| W-007 | TokenDisplay | Progress bar ARIA incomplete | MEDIUM |
| W-008 | ToolDetailsPanel | No retry on load failure | MEDIUM |
| W-009-W-022 | Various | Hardcoded values (14 instances) | LOW |
| W-023-W-030 | Various | Accessibility gaps (8 instances) | LOW |
| W-031-W-034 | Various | Style inconsistencies (4 instances) | LOW |

### Store/Command: Findings Summary

| ID | Location | Finding | Severity |
|----|----------|---------|----------|
| S-001 | activityStore | Capture race condition with streamingStore.reset() | HIGH |
| S-002 | workflowStore | No error recovery / retry on loadWorkflows() | HIGH |
| S-003 | +page.svelte | No double-submit protection on Send | HIGH |
| S-004 | validationStore | No timeout on pending validations | MEDIUM |
| S-005 | backgroundWorkflowsStore | Cleanup timer could hit running execution | MEDIUM |

---

## 7. UX Impact Analysis

### From End-User Perspective

| Scenario | Current Behavior | Risk | Impact |
|----------|------------------|------|--------|
| DB unavailable at startup | Empty sidebar, no error message | HIGH | User thinks app is broken, must restart |
| Double-click Send | Two concurrent executions attempted | HIGH | Confusing output, possible backend error |
| Copy message on restricted clipboard | Silent failure | MEDIUM | User thinks copy worked, pastes nothing |
| Long workflow, switch back | Activities may be lost if capture timing wrong | HIGH | Missing tool execution history |
| Validation request ignored | Backend hangs indefinitely | MEDIUM | Workflow stuck forever, user confused |
| Prompt load fails | Spinner forever in modal | MEDIUM | User must close and reopen modal |
| Tool detail load fails | Error shown, no retry | MEDIUM | User must collapse and re-expand |

### Most Frustrating Scenarios (End-User Priority)

1. **Blank sidebar on error** - No way to recover without app restart
2. **Lost activities after workflow switch** - Invisible data loss
3. **Stuck validation** - Workflow hangs with no timeout or notification
4. **Silent clipboard failure** - False sense of success

---

## 8. Recommendations

### Phase 1: Quick Wins (1-2 hours)

| ID | Action | Files |
|----|--------|-------|
| FIX-001 | Add try/catch to clipboard copy with toast feedback | MessageBubble.svelte |
| FIX-002 | Replace console.error with error state in prompt modal | PromptSelectorModal.svelte |
| FIX-003 | Replace console.warn with proper handling | UserQuestionModal.svelte |
| FIX-004 | Disable Send button during execution | +page.svelte |
| FIX-005 | Replace setTimeout with tick() for focus | NewWorkflowModal.svelte, WorkflowItem.svelte |

### Phase 2: Robustness (2-4 hours)

| ID | Action | Files |
|----|--------|-------|
| FIX-006 | Add error state + retry button to workflow list | WorkflowList.svelte, workflowStore |
| FIX-007 | Add capture guard in activityStore | activity.ts |
| FIX-008 | Add validation timeout (5 min auto-reject) | validationStore |
| FIX-009 | Add running guard to cleanup timer | backgroundWorkflows.ts |
| FIX-010 | Add retry button to ToolDetailsPanel error state | ToolDetailsPanel.svelte |

### Phase 3: Maintainability (3-5 hours)

| ID | Action | Files |
|----|--------|-------|
| FIX-011 | Extract displaySteps to utility | ReasoningPanel.svelte + new util |
| FIX-012 | Extract displayExecutions to utility | ToolExecutionPanel.svelte + new util |
| FIX-013 | Consolidate ActivityItem expansion to enum | ActivityItem.svelte |
| FIX-014 | Extract hardcoded values to CSS variables | Multiple (14 files) |
| FIX-015 | Add missing aria-labels | Multiple (8 files) |

### Phase 4: Future Considerations

- Consider a formal state machine library if workflow states grow more complex
- Consider extracting the callback forwarding pattern into a documented utility
- Consider integration tests for the 8-step workflow execution chain
- Consider adding aria-valuetext to TokenDisplay for screen reader context

---

## Appendix: Store Reliability Matrix

| Store | Owns Events | Error Handling | Callback-Driven | Race Risk | Health |
|-------|-------------|----------------|-----------------|-----------|--------|
| workflowStore | No | Yes | No | None | SAFE |
| streamingStore | No | Yes | Yes (bgWf) | View switch | DEPENDENT |
| backgroundWorkflowsStore | Yes (2) | Yes | Yes (forwards) | Cleanup timer | CRITICAL HUB |
| activityStore | No | Yes | No | Capture timing | TIMING SENSITIVE |
| validationStore | Yes (1) | Yes | No | None | SAFE |
| userQuestionStore | No | Yes | Yes (bgWf) | Queue FIFO | SAFE |
| tokenStore | No | N/A | No | None | SAFE |
| toastStore | No | N/A | No | Dismiss order | ORDER SENSITIVE |
| agentStore | No | Yes (factory) | No | None | SAFE |
| promptStore | No | Yes | No | None | SAFE |
