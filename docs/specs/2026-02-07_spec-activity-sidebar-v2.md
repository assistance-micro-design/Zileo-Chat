# Specification - Activity Sidebar v2

## Metadata
- Date: 2026-02-07
- Branch: `feature/activity-sidebar-v2`
- Stack: Svelte 5.49 + Rust 1.93 + Tauri 2.9 + SurrealDB 2.5
- Complexity: **complex** (7 features, 3 priority tiers, full-stack changes)

## Context

**Request**: Improve the Activity Sidebar with 7 features organized by priority.

**Scope**:
- Included: Badge counts, expandable tool I/O, expandable reasoning, message grouping, token display, timestamps, export
- Excluded: Sub-agent tree view (descoped - too much backend work)

**Success Criteria**:
- [ ] Badge counts visible on filter tabs when count > 0
- [ ] Tool details expandable with lazy-loaded JSON viewer
- [ ] Reasoning steps expandable with full plain-text content
- [ ] Activities grouped by message_id with visual separators
- [ ] Token badges on reasoning steps and sub-agents
- [ ] Tooltip with absolute timestamp on duration
- [ ] Export button exports all activities as JSON via Tauri save dialog

---

## Current Architecture

### Data Flow
```mermaid
graph LR
    S[Streaming Store] -->|ActiveTool/Reasoning/SubAgent| C[Conversion Utils]
    DB[(SurrealDB)] -->|ToolExecution/ThinkingStep/SubAgent| AS[Activity Service]
    AS --> C
    C -->|WorkflowActivityEvent[]| ST[Activity Store]
    ST -->|allActivities| AF[ActivityFeed]
    ST -->|filteredActivities| AF
    AF --> AI[ActivityItem]
    AI --> AID[ActivityItemDetails]
```

### Key Files
| File | Role |
|------|------|
| `src/types/activity.ts` | ActivityMetadata, WorkflowActivityEvent, ActivityFilter |
| `src/lib/utils/activity.ts` | Conversion functions (streaming + historical) |
| `src/lib/stores/activity.ts` | Activity store with filtering |
| `src/lib/services/activity.service.ts` | Backend IPC (loadAll) |
| `src/lib/components/workflow/ActivityFeed.svelte` | Feed container with filter tabs + virtual scroll |
| `src/lib/components/workflow/ActivityItem.svelte` | Individual activity item (expand for tasks only) |
| `src/lib/components/workflow/ActivityItemDetails.svelte` | Task detail panel |
| `src/lib/components/agent/ActivitySidebar.svelte` | Sidebar wrapper (RightSidebar layout) |

### Data Lost in Conversions
| Source | Lost Fields |
|--------|------------|
| ToolExecution | `input_params`, `output_result`, `message_id` |
| ThinkingStep | Full `content` (truncated to 200 chars), `tokens`, `message_id` |
| SubAgentExecution | `result_summary`, `parent_agent_id` |
| ActiveReasoningStep | Full `content` (truncated to 200 chars) |

---

## Architecture - 7 Features

### Feature 1: Badge Counts on Filter Tabs

**Priority**: P1 | **Type**: Frontend only | **Impact**: Low

**Current State**: Counts exist in `ActivityFeed.svelte:102` as `$derived(countActivitiesByType(allActivities))` but only shown in `title` tooltip attribute (line 134).

**Proposed Change**: Add visible badge next to each filter icon when count > 0.

**Files Modified**:
- `src/lib/components/workflow/ActivityFeed.svelte` - Add badge span in filter tab button

**Implementation**:
```svelte
<!-- Inside {#each ACTIVITY_FILTERS as f (f.id)} -->
<button ...>
  <IconComponent size={16} />
  {#if counts[f.id] > 0}
    <span class="filter-badge">{counts[f.id]}</span>
  {/if}
</button>
```

**CSS**:
```css
.filter-badge {
  font-size: var(--font-size-xs);
  min-width: 16px;
  height: 16px;
  padding: 0 4px;
  border-radius: var(--radius-full);
  background: var(--color-accent);
  color: var(--color-bg-primary);
  display: flex;
  align-items: center;
  justify-content: center;
  line-height: 1;
}
```

**Validation**:
- [ ] Badge visible when count > 0
- [ ] Badge hidden when count = 0
- [ ] Badge updates reactively when activities change
- [ ] "All" tab shows total count

---

### Feature 2: Expandable Tool Details (Input/Output)

**Priority**: P1 | **Type**: Full-stack | **Impact**: Medium

**Design**: Expand an ActivityItem of type `tool_*` to show tool input/output. Data is lazy-loaded from DB via a new `get_tool_execution` command. Works only for historical data (post-streaming) since ActiveTool doesn't carry I/O during streaming.

#### 2a. Backend: New `get_tool_execution` command

**File**: `src-tauri/src/commands/tool_execution.rs`

**Signature**:
```rust
/// Loads a single tool execution by ID.
///
/// # Arguments
/// * `execution_id` - The tool execution UUID
///
/// # Returns
/// The full ToolExecution record including input_params and output_result
#[tauri::command]
#[instrument(name = "get_tool_execution", skip(state), fields(execution_id = %execution_id))]
pub async fn get_tool_execution(
    execution_id: String,
    state: State<'_, AppState>,
) -> Result<ToolExecution, String>
```

**Implementation pattern** (follows existing tool_execution.rs commands):
```rust
// Input validation (ERR_TAURI_001: all existing tool_execution commands use this)
let validated_id = Validator::validate_uuid(&execution_id, "execution_id")?;

let query = format!(
    r#"SELECT
        meta::id(id) AS id,
        workflow_id, message_id, agent_id, tool_type, tool_name,
        server_name, input_params, output_result, success,
        error_message, duration_ms, iteration, created_at
    FROM tool_execution
    WHERE meta::id(id) = '{}'"#,
    validated_id
);

// ERR_SURREAL_003: meta::id(id) AS id for clean UUID
let result: Option<ToolExecution> = db.query_json(&query).await
    .map_err(|e| format!("Failed to get tool execution: {}", e))?
    .into_iter().next();

result.ok_or_else(|| format!("Tool execution not found: {}", execution_id))
```

**Registration**: Add to `src-tauri/src/main.rs` in the tool execution commands block.

#### 2b. Frontend: Store execution_id in ActivityMetadata

**File**: `src/types/activity.ts`

Add field to `ActivityMetadata`:
```typescript
/** Original execution ID for lazy-loading details */
executionId?: string;
```

**File**: `src/lib/utils/activity.ts`

Modify `toolExecutionToActivity()` to preserve `exec.id`:
```typescript
metadata.executionId = exec.id;
```

#### 2c. Frontend: New JsonViewer component

**File**: `src/lib/components/ui/JsonViewer.svelte` (NEW)

**Props**:
```typescript
interface Props {
  data: unknown;
  maxDepth?: number;     // Default: 3
  collapsed?: boolean;   // Default: true (start collapsed)
}
```

**Behavior**:
- Recursive tree of key-value pairs
- Collapse/expand per node (click key to toggle)
- Icons: ChevronRight (collapsed) / ChevronDown (expanded)
- Values colored by type: string (green), number (blue), boolean (orange), null (gray)
- Max depth to prevent stack overflow on deeply nested JSON
- Truncate string values > 200 chars with "..." and expand on click

**CSS**: Use existing design tokens (mono font for keys, xs font-size, tertiary bg)

#### 2d. Frontend: Expand tool details in ActivityItem

**File**: `src/lib/components/workflow/ActivityItem.svelte`

**Changes**:
1. Add local state: `let isToolExpanded = $state(false)`
2. Add derived: `const isToolWithDetails = $derived(activity.type.startsWith('tool_') && activity.metadata?.executionId)`
3. Show chevron button for tools (same pattern as tasks)
4. On expand: lazy-load via `invoke('get_tool_execution', { executionId: activity.metadata.executionId })`
5. Cache result in local state to avoid re-fetching

**New component**: `src/lib/components/workflow/ToolDetailsPanel.svelte` (NEW)

**Props**:
```typescript
interface Props {
  executionId: string;
}
```

**Behavior**:
- On mount: `invoke('get_tool_execution', { executionId })`
- Loading state with spinner
- Error state: use `getErrorMessage(e)` from `$lib/utils/error` (PAT_ERR_001)
- Display: Input section + Output section, each with JsonViewer
- Styled like ActivityItemDetails (border-left accent, tertiary bg, slideDown)

**Validation**:
- [ ] Chevron appears for historical tool activities (not streaming)
- [ ] Click expands with loading spinner
- [ ] JSON viewer shows input_params and output_result
- [ ] Collapse/expand works on JSON nodes
- [ ] Second click collapses (no re-fetch)
- [ ] Error handling for failed loads

---

### Feature 3: Expandable Reasoning Step Details

**Priority**: P1 | **Type**: Frontend + types | **Impact**: Medium

**Design**: Expand an ActivityItem of type `reasoning` to show full plain text content. Data is available from both streaming (ActiveReasoningStep.content) and historical (ThinkingStep.content). NO markdown rendering - plain text only.

#### 3a. Types: Add content to ActivityMetadata

**File**: `src/types/activity.ts`

Add field to `ActivityMetadata`:
```typescript
/** Full content text for reasoning steps */
content?: string;
```

#### 3b. Conversions: Preserve full content

**File**: `src/lib/utils/activity.ts`

**activeReasoningToActivity()**: Add `metadata.content = step.content`

**thinkingStepToActivity()**: Add `metadata.content = step.content`

Keep the truncated `description` for list display. The full `content` is in metadata for expand.

#### 3c. Frontend: Add tokens to conversion

**File**: `src/lib/utils/activity.ts`

**thinkingStepToActivity()**: Add token mapping:
```typescript
// ThinkingStep.tokens = tokens generated (output), not input tokens
if (step.tokens) {
  metadata.tokens = { input: 0, output: step.tokens };
}
```

#### 3d. Frontend: Expand reasoning in ActivityItem

**File**: `src/lib/components/workflow/ActivityItem.svelte`

**Changes**:
1. Add local state: `let isReasoningExpanded = $state(false)`
2. Add derived: `const isReasoningWithContent = $derived(activity.type === 'reasoning' && activity.metadata?.content)`
3. Show chevron button for reasoning (same pattern as tasks/tools)
4. On expand: show content from `activity.metadata.content`

**New component**: `src/lib/components/workflow/ReasoningDetailsPanel.svelte` (NEW)

**Props**:
```typescript
interface Props {
  content: string;
}
```

**Behavior**:
- Plain text display (NO markdown)
- `white-space: pre-wrap; word-break: break-word` (same as chat ReasoningStep)
- Styled like ActivityItemDetails (border-left accent, tertiary bg, slideDown)
- Max-height with scroll for very long content (300px)

**Validation**:
- [ ] Chevron appears for reasoning activities (both streaming and historical)
- [ ] Click shows full plain text content
- [ ] NO markdown rendering
- [ ] Long content scrollable within detail panel
- [ ] Content matches what was in the original ThinkingStep/ActiveReasoningStep

---

### Feature 4: Groupement by message_id (Rounds)

**Priority**: P2 | **Type**: Frontend + types | **Impact**: Medium

**Design**: Group activities by `message_id` (1 round = 1 assistant response). Visual separators between rounds. Works for historical data (which has message_id) and streaming (derive from order).

#### 4a. Types: Add messageId to ActivityMetadata

**File**: `src/types/activity.ts`

Add field to `ActivityMetadata`:
```typescript
/** Message ID for grouping by round */
messageId?: string;
```

#### 4b. Conversions: Preserve message_id

**File**: `src/lib/utils/activity.ts`

**toolExecutionToActivity()**: Add `metadata.messageId = exec.message_id`

**thinkingStepToActivity()**: Add `metadata.messageId = step.message_id`

**subAgentExecutionToActivity()**: SubAgentExecution doesn't have message_id directly. Skip for now.

**activeToolToActivity()**: Streaming tools don't have message_id. Skip.

**activeReasoningToActivity()**: Streaming reasoning doesn't have message_id. Skip.

#### 4c. Frontend: Group rendering in ActivityFeed

**File**: `src/lib/components/workflow/ActivityFeed.svelte`

**Changes**:
1. Compute groups from `activities` based on `metadata.messageId`
2. Activities without messageId form their own group (streaming activities)
3. Render a separator between groups (thin horizontal line + optional round label)

**Group computation** (in script):
```typescript
interface ActivityGroup {
  messageId: string | null;
  activities: WorkflowActivityEvent[];
}

const groupedActivities = $derived.by(() => {
  const groups: ActivityGroup[] = [];
  let currentGroup: ActivityGroup | null = null;

  for (const activity of activities) {
    const msgId = activity.metadata?.messageId ?? null;
    if (!currentGroup || currentGroup.messageId !== msgId) {
      currentGroup = { messageId: msgId, activities: [] };
      groups.push(currentGroup);
    }
    currentGroup.activities.push(activity);
  }
  return groups;
});
```

**Separator CSS**:
```css
.round-separator {
  display: flex;
  align-items: center;
  gap: var(--spacing-sm);
  padding: var(--spacing-xs) var(--spacing-md);
  color: var(--color-text-tertiary);
  font-size: var(--font-size-xs);
}
.round-separator::before,
.round-separator::after {
  content: '';
  flex: 1;
  height: 1px;
  background: var(--color-border);
}
```

**Virtual scroll integration**: When virtual scroll is active (20+ items), grouping uses a flat list with separators injected as discriminated union items. The `renderItem` snippet conditionally renders `ActivityItem` or a separator div based on item type. Confirmed: `@humanspeak/svelte-virtual-list` v0.3.10 supports heterogeneous item heights via ResizeObserver and conditional rendering in `renderItem(item, index)`.

```typescript
/** Discriminated union for virtual list items with separators */
type FeedItem =
  | { kind: 'activity'; data: WorkflowActivityEvent }
  | { kind: 'separator'; messageId: string };

/** Flatten groups into a FeedItem[] for virtual scroll */
const flatFeedItems = $derived.by(() => {
  const items: FeedItem[] = [];
  for (let i = 0; i < groupedActivities.length; i++) {
    const group = groupedActivities[i];
    if (i > 0 && group.messageId) {
      items.push({ kind: 'separator', messageId: group.messageId });
    }
    for (const activity of group.activities) {
      items.push({ kind: 'activity', data: activity });
    }
  }
  return items;
});
```

```svelte
<!-- In virtual scroll renderItem snippet -->
{#snippet renderItem(item)}
  {#if item.kind === 'separator'}
    <div class="round-separator">Round</div>
  {:else}
    <ActivityItem activity={item.data} />
  {/if}
{/snippet}
```

**Validation**:
- [ ] Historical activities grouped by message_id
- [ ] Visual separator between groups
- [ ] Streaming activities (no message_id) displayed without separators
- [ ] Works with virtual scroll (separators as list items)

---

### Feature 5: Display Tokens

**Priority**: P2 | **Type**: Frontend only | **Impact**: Low

**Design**: Show compact token badge "142 tok" next to duration for reasoning steps and sub-agents.

#### 5a. Conversions already partially done

From Feature 3c, reasoning steps already get `metadata.tokens`. Sub-agents already map tokens in both streaming and historical conversions.

#### 5b. Frontend: Token badge in ActivityItem

**File**: `src/lib/components/workflow/ActivityItem.svelte`

**Changes**: Add token badge after duration display:

```svelte
{#if activity.metadata?.tokens}
  {@const totalTokens = activity.metadata.tokens.input + activity.metadata.tokens.output}
  {#if totalTokens > 0}
    <span class="token-badge">{formatTokenCount(totalTokens)} tok</span>
  {/if}
{/if}
```

**Helper** (in `src/lib/utils/activity.ts`):
```typescript
export function formatTokenCount(tokens: number): string {
  if (tokens >= 1000) {
    return `${(tokens / 1000).toFixed(1)}k`;
  }
  return String(tokens);
}
```

**CSS**:
```css
.token-badge {
  font-size: var(--font-size-xs);
  color: var(--color-text-tertiary);
  font-family: var(--font-mono);
  white-space: nowrap;
}
```

**Validation**:
- [ ] Token badge visible on reasoning steps (when tokens available)
- [ ] Token badge visible on sub-agent items (when tokens available)
- [ ] No token badge on tool items
- [ ] Format: "142 tok", "1.5k tok"

---

### Feature 6: Timestamps

**Priority**: P2 | **Type**: Frontend only | **Impact**: Low

**Design**: Tooltip with absolute timestamp on the existing duration display. Simplest approach.

**File**: `src/lib/components/workflow/ActivityItem.svelte`

**Changes**: Add `title` attribute to the duration element:

```svelte
<span class="duration" title={formatAbsoluteTimestamp(activity.timestamp)}>
  {formatDuration(activity.duration)}
</span>
```

**Helper** (in `src/lib/utils/activity.ts`):
```typescript
export function formatAbsoluteTimestamp(timestamp: number): string {
  return new Date(timestamp).toLocaleString();
}
```

**Validation**:
- [ ] Hovering over duration shows absolute timestamp in tooltip
- [ ] Format: locale-specific date+time string

---

### Feature 7: Export Activity Log

**Priority**: P3 | **Type**: Frontend only | **Impact**: Low

**Design**: Export button in sidebar header. Serialize allActivities as JSON via Tauri save dialog.

#### 7a. i18n Keys

**File**: `src/lib/i18n/en.ts` (and `fr.ts`)

New keys:
```typescript
activity_export: 'Export',
activity_export_title: 'Save Activity Export',
activity_export_success: 'Exported {count} activities',
activity_export_error: 'Export failed: {error}',
activity_export_empty: 'No activities to export',
```

#### 7b. Frontend: Export button in ActivitySidebar

**File**: `src/lib/components/agent/ActivitySidebar.svelte`

**Changes**:
1. Add export button in header (next to HelpButton)
2. On click: serialize `allActivities` to JSON, open save dialog

```typescript
import { save } from '@tauri-apps/plugin-dialog';
import { invoke } from '@tauri-apps/api/core';

async function handleExport() {
  if (allActivities.length === 0) {
    // Show toast: no activities
    return;
  }

  const filename = `zileo-activity-${new Date().toISOString().slice(0, 10)}.json`;
  const filePath = await save({
    defaultPath: filename,
    filters: [{ name: 'JSON', extensions: ['json'] }],
    title: $i18n('activity_export_title')
  });

  if (!filePath) return;

  const content = JSON.stringify(allActivities, null, 2);
  await invoke('save_export_to_file', { path: filePath, content });
}
```

**Note**: `save_export_to_file` command already exists (used by ExportPanel).

**Icon**: Download from `@lucide/svelte`

**Validation**:
- [ ] Export button visible in sidebar header
- [ ] Click opens save dialog with default filename
- [ ] JSON file contains all activities
- [ ] Empty state handled (toast message)
- [ ] File saved correctly

---

## Types Synchronized

### ActivityMetadata - New Fields

**TypeScript** (`src/types/activity.ts`):
```typescript
export interface ActivityMetadata {
  // ... existing fields ...

  /** Full content text for reasoning steps */
  content?: string;
  /** Message ID for grouping by round */
  messageId?: string;
  /** Original execution ID for lazy-loading details */
  executionId?: string;
}
```

**No Rust changes needed** - ActivityMetadata is frontend-only. The backend models (ToolExecution, ThinkingStep) already have these fields; we're just preserving them through conversions.

### New Rust Command

**`get_tool_execution`** - only new backend addition. Follows existing patterns.

---

## Implementation Plan

### Phase 1: Types & Conversions (Foundation)
**Objective**: Extend ActivityMetadata and update conversion functions to preserve data.

**Tasks**:
1. **Types**: Add `content`, `messageId`, `executionId` to ActivityMetadata (`src/types/activity.ts`)
2. **Conversions**: Update `toolExecutionToActivity()` - add `executionId`, `messageId` (`src/lib/utils/activity.ts`)
3. **Conversions**: Update `thinkingStepToActivity()` - add `content`, `tokens`, `messageId` (`src/lib/utils/activity.ts`)
4. **Conversions**: Update `activeReasoningToActivity()` - add `content` (`src/lib/utils/activity.ts`)
5. **Conversions**: Update `subAgentExecutionToActivity()` - no message_id available, skip
6. **Utility**: Add `formatTokenCount()` and `formatAbsoluteTimestamp()` to `src/lib/utils/activity.ts`

**Dependencies**: None
**Validation**: `npm run check` passes

### Phase 2: Backend Command
**Objective**: Add `get_tool_execution` Rust command.

**Tasks**:
1. **Command**: Add `get_tool_execution` to `src-tauri/src/commands/tool_execution.rs`
2. **Registration**: Add to `src-tauri/src/main.rs`

**Dependencies**: None (parallel with Phase 1)
**Validation**: `cargo clippy -- -D warnings && cargo test`

### Phase 3: Badge Counts (Feature 1)
**Objective**: Show visible badge counts on filter tabs.

**Tasks**:
1. **UI**: Add badge span in filter tab buttons (`ActivityFeed.svelte`)
2. **CSS**: Badge styles (pill, accent bg, xs font)

**Dependencies**: None
**Validation**: Badge appears when count > 0, hidden when 0

### Phase 4: Expandable Reasoning (Feature 3)
**Objective**: Expand reasoning ActivityItem to show full content.

**Tasks**:
1. **Component**: Create `ReasoningDetailsPanel.svelte` (plain text display)
2. **ActivityItem**: Add reasoning expand logic (state, derived, chevron, conditional render)

**Dependencies**: Phase 1 (content in metadata)
**Validation**: Reasoning items expandable with full text, no markdown

### Phase 5: Expandable Tool Details (Feature 2)
**Objective**: Expand tool ActivityItem with lazy-loaded I/O.

**Tasks**:
1. **Component**: Create `JsonViewer.svelte` (recursive collapsible JSON tree)
2. **Component**: Create `ToolDetailsPanel.svelte` (loading + JsonViewer for input/output)
3. **ActivityItem**: Add tool expand logic (state, derived, chevron, lazy-load, cache)

**Dependencies**: Phase 1 (executionId), Phase 2 (backend command)
**Validation**: Tool items expandable, JSON viewer works, lazy-load with cache

### Phase 6: Tokens + Timestamps (Features 5 & 6)
**Objective**: Add token badges and timestamp tooltips.

**Tasks**:
1. **ActivityItem**: Add token badge after duration
2. **ActivityItem**: Add title tooltip on duration element

**Dependencies**: Phase 1 (tokens in metadata)
**Validation**: Tokens shown for reasoning/sub-agents, tooltip on hover

### Phase 7: Message Grouping (Feature 4)
**Objective**: Group activities by message_id with separators.

**Tasks**:
1. **ActivityFeed**: Compute grouped activities from messageId
2. **ActivityFeed**: Render separators between groups
3. **Virtual scroll**: Handle separators as list items in virtual scroll mode

**Dependencies**: Phase 1 (messageId in metadata)
**Validation**: Groups visible with separators, works with virtual scroll

### Phase 8: Export (Feature 7)
**Objective**: Export activity log as JSON.

**Tasks**:
1. **i18n**: Add export-related keys (en + fr)
2. **ActivitySidebar**: Add export button + handler using save dialog

**Dependencies**: None
**Validation**: Export saves valid JSON file, empty state handled

---

## Estimation

| Phase | Files | Effort | Notes |
|-------|-------|--------|-------|
| 1. Types & Conversions | 2 | Low | Simple field additions |
| 2. Backend Command | 2 | Low | Follows existing get_* pattern |
| 3. Badge Counts | 1 | Low | ~20 lines of code |
| 4. Expandable Reasoning | 2 | Low-Med | New component + ActivityItem changes |
| 5. Expandable Tool Details | 3 | Med-High | JsonViewer is most complex new component |
| 6. Tokens + Timestamps | 1 | Low | ~15 lines each |
| 7. Message Grouping | 1 | Medium | Virtual scroll integration tricky |
| 8. Export | 2 | Low | Reuses existing save pattern |

**Parallelizable**: Phase 1 + Phase 2 in parallel. Phase 3 independent. Phase 6 + 8 independent.

---

## Risk Analysis

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| JsonViewer performance on large JSON (50KB) | Medium | Medium | Max depth limit, lazy rendering, truncate long strings |
| Virtual scroll + separators conflict | Medium | Medium | Treat separators as special items with different height |
| Token data missing for reasoning | Low | Low | Graceful fallback (hide badge when no tokens) |
| message_id not available for streaming activities | Expected | Low | Don't group streaming activities, only historical |
| Large export files | Low | Low | JSON.stringify handles well; warn if > 10MB |

---

## Testing

### Frontend (Vitest)
- `formatTokenCount()` - unit test for formatting
- `formatAbsoluteTimestamp()` - unit test
- Updated conversion functions - verify new metadata fields populated
- `countActivitiesByType()` - already tested, no changes needed

### Backend (Rust)
- `get_tool_execution` - compile test (follows existing pattern)

---

## New Files Summary

| File | Type | Description |
|------|------|-------------|
| `src/lib/components/ui/JsonViewer.svelte` | Component | Recursive collapsible JSON tree viewer |
| `src/lib/components/workflow/ToolDetailsPanel.svelte` | Component | Lazy-loaded tool I/O display |
| `src/lib/components/workflow/ReasoningDetailsPanel.svelte` | Component | Full plain-text reasoning content |

## Modified Files Summary

| File | Changes |
|------|---------|
| `src/types/activity.ts` | +3 fields in ActivityMetadata |
| `src/lib/utils/activity.ts` | Update 4 conversion functions + 2 new utility functions |
| `src/lib/components/workflow/ActivityFeed.svelte` | Badge counts + message grouping |
| `src/lib/components/workflow/ActivityItem.svelte` | Expand for tools + reasoning + tokens + timestamps |
| `src/lib/components/agent/ActivitySidebar.svelte` | Export button |
| `src-tauri/src/commands/tool_execution.rs` | New get_tool_execution command |
| `src-tauri/src/main.rs` | Register new command |
| `src/lib/i18n/en.ts` | Export keys |
| `src/lib/i18n/fr.ts` | Export keys |

---

## References
- Discovery: 4 parallel agents (architecture, backend, types, i18n)
- Existing patterns: ActivityItemDetails (slideDown), ReasoningStep (collapse/expand), ToolExecutionPanel (badge)
- Known errors avoided: ERR_SURREAL_002 (raw queries), ERR_SURREAL_003 (meta::id), ERR_TAURI_001 (camelCase IPC), ERR_TAURI_002 (main.rs registration), ERR_TS_001 ($types alias), ERR_TS_002 (nullability)
- Patterns applied: PAT_RUST_001 (Tauri command), PAT_ERR_001 (getErrorMessage), PAT_ERR_002 (.map_err), PAT_DB_002 (parameterized query convention)
- Inventory verified: `save_export_to_file` (reused), `@tauri-apps/plugin-dialog` v2.6.0 (reused), `Validator::validate_uuid` (reused)
- Virtual scroll verified: `@humanspeak/svelte-virtual-list` v0.3.10 supports heterogeneous item heights + conditional rendering (confirmed via npm/GitHub)
