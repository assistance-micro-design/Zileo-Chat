# Rapport - OPT-FA-13: Memoize Activity Filtering

## Metadata
- **Date**: 2025-12-11
- **Spec source**: docs/specs/2025-12-10_optimization-frontend-agent.md
- **Complexity**: Low (0.5h estimated)
- **Category**: Nice to Have

## Orchestration

### Execution Flow
```
Single Agent (Sequential):
  1. Analyze current filtering logic
  2. Update ActivityFeed component
  3. Update ActivitySidebar component
  4. Update +page.svelte
  5. Validation
```

### Files Modified

| File | Change |
|------|--------|
| `src/lib/components/workflow/ActivityFeed.svelte` | Removed local `filterActivities()` call, added `allActivities` prop for counts |
| `src/lib/components/agent/ActivitySidebar.svelte` | Added `allActivities` prop passthrough |
| `src/routes/agent/+page.svelte` | Import and pass `$allActivities` from store |

## Problem Analysis

### Before (Duplication)
```
+page.svelte
  -> passes $filteredActivities (store-level filtering)
  -> ActivitySidebar
     -> ActivityFeed
        -> filterActivities(activities, filter)  // DUPLICATE FILTERING
        -> countActivitiesByType(activities)     // WRONG COUNTS (from filtered data)
```

### After (Single Source of Truth)
```
+page.svelte
  -> passes $filteredActivities (pre-filtered for display)
  -> passes $allActivities (unfiltered for counts)
  -> ActivitySidebar
     -> ActivityFeed
        -> uses activities directly (already filtered)
        -> countActivitiesByType(allActivities)  // CORRECT COUNTS
```

## Implementation Details

### ActivityFeed.svelte Changes

**Removed**:
- `filterActivities` import from `$lib/utils/activity`
- Local `filteredActivities` derived store

**Added**:
- `allActivities` prop for accurate filter counts
- Direct use of `activities` prop (pre-filtered)

**Updated**:
- `counts` derived now uses `allActivities` instead of `activities`
- `showEmptyState` derived uses `activities.length` directly

### Key Code Changes

```typescript
// Before
const filteredActivities = $derived(filterActivities(activities, filter));
const counts = $derived(countActivitiesByType(activities));  // Wrong counts

// After
const counts = $derived(countActivitiesByType(allActivities));  // Correct counts
// activities prop is already filtered, used directly in template
```

## Validation

### Frontend
- Lint: PASS (0 errors)
- TypeCheck: PASS (0 errors, 0 warnings)

## Benefits

1. **Single Source of Truth**: Filtering logic centralized in `activity.ts` store
2. **Correct Counts**: Filter tab badges show accurate totals from all activities
3. **Reduced Computation**: No duplicate filtering in component
4. **Better Maintainability**: Filter logic in one place (store derived)

## Spec Reference

From `docs/specs/2025-12-10_optimization-frontend-agent.md`:

> **OPT-FA-13: Memoize Activity Filtering**
> - Files: `src/lib/components/agent/WorkflowSidebar.svelte:64-68`, `src/lib/components/agent/ActivitySidebar.svelte`
> - Change: Centralize filtering logic in a derived store
> - Benefit: Avoids duplication, single source of truth
> - Risk: Low
> - Effort: 0.5h

Note: The spec incorrectly referenced WorkflowSidebar (which filters workflows, not activities). The actual duplication was in ActivityFeed.svelte.
