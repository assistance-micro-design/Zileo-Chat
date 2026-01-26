# Remaining Specifications - Deferred Frontend Optimizations

> **Status**: DEFERRED - Post-v1 implementation
> **Last Updated**: 2026-01-25
> **Sources**: OPT-FA (11/14 done), OPT-MSG (8/8 done)

## Summary

This document consolidates deferred frontend optimization items from OPT-FA and OPT-MSG phases. All items are low priority and can be implemented post-v1.

---

## OPT-FA Deferred Items (3 remaining)

### OPT-FA-10: Migrate Streaming to Class Runes (OPTIONAL)

**Status**: DEFERRED | **Effort**: 6h | **Priority**: Low

Migrate streaming store to Svelte 5 class-based runes for fine-grained reactivity.

```typescript
// src/lib/stores/streaming.svelte.ts
export class StreamingManager {
  content = $state('');
  isStreaming = $state(false);
  tools = $state<ToolState[]>([]);
  activeTools = $derived(this.tools.filter(t => t.status === 'running'));
}
```

**Risk**: HIGH - Requires complete migration of all consumers

---

### OPT-FA-14: Add $inspect Debug Helpers

**Status**: DEFERRED | **Effort**: 0.25h | **Priority**: Low

Add Svelte 5 `$inspect()` for development debugging.

```typescript
if (import.meta.env.DEV) {
  $inspect(streaming.content).with((type, value) => {
    console.log('[Streaming]', type, value?.length, 'chars');
  });
}
```

**Risk**: NONE (dev only)

---

### OPT-FA-15: Implement Retry Logic

**Status**: DEFERRED | **Effort**: 4h | **Priority**: Medium

Add retry wrapper with exponential backoff for critical operations.

```typescript
// src/lib/utils/retry.ts
export async function withRetry<T>(
  operation: () => Promise<T>,
  config: { maxRetries?: number; initialDelay?: number }
): Promise<T>
```

**Risk**: MEDIUM - Integration with existing error handling

---

## OPT-MSG Deferred Items (1 remaining)

### OPT-MSG-7: Virtual Scroll MessageList

**Status**: DEFERRED | **Effort**: 2h | **Priority**: P3

Implement virtual scrolling for 500+ messages in MessageList.

**Current State**: CSS containment works well until ~200 messages

**Implementation**:
```svelte
import SvelteVirtualList from '@humanspeak/svelte-virtual-list';

<SvelteVirtualList items={messages} defaultEstimatedItemHeight={120}>
  {#snippet renderItem(message)}
    <MessageBubble {message} />
  {/snippet}
</SvelteVirtualList>
```

**Challenge**: Auto-scroll to bottom must be preserved

**Risk**: MEDIUM - Scroll behavior changes

---

## Completed Items Reference

### OPT-FA Completed (11/14)

| Item | Description | Commit |
|------|-------------|--------|
| OPT-FA-1 | Modal duplication fix | Quick Wins |
| OPT-FA-2 | plugin-dialog update | Quick Wins |
| OPT-FA-3 | Error handling | Quick Wins |
| OPT-FA-4 | Debounce search | Quick Wins |
| OPT-FA-5 | localStorage service | Quick Wins |
| OPT-FA-6 | Vitest update | Quick Wins |
| OPT-FA-7 | Derived stores (28->14) | 75043fa |
| OPT-FA-8 | WorkflowExecutor | 2e37267 |
| OPT-FA-9 | PageState interface | 435594c |
| OPT-FA-11 | Lazy modals | ddda635 |
| OPT-FA-12 | @lucide/svelte | 2396077 |
| OPT-FA-13 | Activity memoization | 399abca |

### OPT-MSG Completed (7/8)

| Item | Description | Commit |
|------|-------------|--------|
| OPT-MSG-1 | TokenDisplay conditional animations | b78be09 |
| OPT-MSG-2 | formatDuration utility (complete) | b78be09, OPT-MSG-2 |
| OPT-MSG-3 | iconMap const | b78be09 |
| OPT-MSG-4 | activity-icons.ts | b78be09 |
| OPT-MSG-5 | Virtual scroll ActivityFeed | 0af6d5b |
| OPT-MSG-6 | Overflow fixes + ActivityItemDetails | 02c0157 |
| OPT-MSG-8 | Accessibility task-details (role="region") | Verified in code |

---

## Documentation Updated

- `docs/REMAINING_TASKS.md`: OPT-FA and OPT-MSG sections
- `docs/FRONTEND_SPECIFICATIONS.md`: Utilities, components, optimizations
- `docs/TECH_STACK.md`: Package versions

---

## Archived Files (to be removed)

Original specs and task reports archived in git history:
- `docs/specs/2025-12-10_optimization-frontend-agent.md`
- `docs/specs/2025-12-11_optimization-frontend-messages-area.md`
