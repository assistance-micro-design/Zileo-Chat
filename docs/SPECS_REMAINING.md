# Remaining Specifications - OPT-FA Deferred Items

> **Status**: DEFERRED - Post-v1 implementation
> **Source**: docs/specs/2025-12-10_optimization-frontend-agent.md
> **Created**: 2025-12-11
> **Completed Items**: 11/14 (OPT-FA-1 to OPT-FA-9, OPT-FA-11 to OPT-FA-13)

## Summary

The OPT-FA (Frontend/Agent) optimization plan identified 14 items. 11 have been implemented and verified in code. The remaining 3 items are documented here for future implementation.

## Deferred Items

### OPT-FA-10: Migrate Streaming to Class Runes (OPTIONAL)

**Status**: DEFERRED
**Effort**: 6h
**Priority**: Low (optional optimization)
**Category**: Strategic

**Description**: Migrate the streaming store from traditional Svelte store pattern to Svelte 5 class-based runes pattern for fine-grained reactivity.

**Current State**:
- `src/lib/stores/streaming.ts` uses writable/derived stores
- 14 derived stores after consolidation (OPT-FA-7)

**Target State**:
```typescript
// src/lib/stores/streaming.svelte.ts
export class StreamingManager {
  content = $state('');
  isStreaming = $state(false);
  workflowId = $state<string | null>(null);
  tools = $state<ToolState[]>([]);
  subAgents = $state<SubAgentState[]>([]);

  // Fine-grained derived
  activeTools = $derived(this.tools.filter(t => t.status === 'running'));

  // Methods
  handleToken(chunk: TokenChunk) {
    this.content += chunk.content;
  }

  reset() {
    this.content = '';
    this.isStreaming = false;
    this.tools = [];
  }
}

export const streaming = new StreamingManager();
```

**Implementation Steps**:
1. Create `streaming.svelte.ts` with class-based state
2. Migrate chunk handlers as class methods
3. Export singleton instance
4. Migrate consumer components one by one
5. Remove old `streaming.ts`

**Risk**: HIGH - Requires complete migration of all consumers
**Prerequisite**: OPT-FA-7 (done)

---

### OPT-FA-14: Add $inspect Debug Helpers

**Status**: DEFERRED
**Effort**: 0.25h
**Priority**: Low (developer experience)
**Category**: Nice to Have

**Description**: Add Svelte 5 `$inspect()` helpers for debugging streaming state in development mode.

**Target Implementation**:
```typescript
// In streaming.ts or streaming.svelte.ts
if (import.meta.env.DEV) {
  $inspect(streaming.content).with((type, value) => {
    console.log('[Streaming]', type, value?.length, 'chars');
  });

  $inspect(streaming.tools).with((type, tools) => {
    console.log('[Tools]', type, tools?.length, 'active');
  });
}
```

**Benefits**:
- Real-time state debugging
- No production overhead
- Native Svelte 5 debugging pattern

**Risk**: NONE (dev only)

---

### OPT-FA-15: Implement Retry Logic

**Status**: DEFERRED
**Effort**: 4h
**Priority**: Medium (reliability)
**Category**: Strategic

**Description**: Add retry wrapper for critical frontend operations (save message, execute workflow) with exponential backoff.

**Target Implementation**:
```typescript
// src/lib/utils/retry.ts
interface RetryConfig {
  maxRetries: number;      // Default: 3
  initialDelay: number;    // Default: 1000ms
  maxDelay: number;        // Default: 30000ms
  multiplier: number;      // Default: 2
}

export async function withRetry<T>(
  operation: () => Promise<T>,
  config: Partial<RetryConfig> = {}
): Promise<T> {
  const { maxRetries = 3, initialDelay = 1000, maxDelay = 30000, multiplier = 2 } = config;

  let delay = initialDelay;
  let lastError: Error;

  for (let attempt = 0; attempt <= maxRetries; attempt++) {
    try {
      return await operation();
    } catch (error) {
      lastError = error as Error;
      if (attempt === maxRetries) break;

      await new Promise(r => setTimeout(r, delay));
      delay = Math.min(delay * multiplier, maxDelay);
    }
  }

  throw lastError!;
}
```

**Usage in WorkflowExecutorService**:
```typescript
// src/lib/services/workflowExecutor.service.ts
const saveResult = await withRetry(
  () => MessageService.save(userMessage),
  { maxRetries: 2, initialDelay: 500 }
);
```

**Risk**: MEDIUM - Requires careful integration with existing error handling
**Prerequisite**: OPT-FA-3 (done - error handling)

---

## References

### Completed OPT-FA Items

| Item | Description | Commit |
|------|-------------|--------|
| OPT-FA-1 | Modal duplication fix | Quick Wins batch |
| OPT-FA-2 | @tauri-apps/plugin-dialog update | Quick Wins batch |
| OPT-FA-3 | Error handling in message.service | Quick Wins batch |
| OPT-FA-4 | Debounce search input | Quick Wins batch |
| OPT-FA-5 | Typed localStorage service | Quick Wins batch |
| OPT-FA-6 | Vitest update | Quick Wins batch |
| OPT-FA-7 | Consolidated derived stores | 75043fa |
| OPT-FA-8 | WorkflowExecutor service | 2e37267 |
| OPT-FA-9 | PageState interface | 435594c |
| OPT-FA-11 | Lazy load modals | ddda635 |
| OPT-FA-12 | lucide-svelte migration | 2396077 |
| OPT-FA-13 | Memoize activity filtering | 399abca |

### Documentation Updated

- `docs/TECH_STACK.md`: Package versions, OPT-FA notes
- `docs/FRONTEND_SPECIFICATIONS.md`: Services, patterns, utilities
- `docs/REMAINING_TASKS.md`: OPT-FA section added

### Original Spec

Full specification archived in git history:
- `docs/specs/2025-12-10_optimization-frontend-agent.md` (to be removed)

### Task Reports

Task completion reports archived in git history:
- `docs/taches/2025-12-11_opt-fa-quick-wins.md`
- `docs/taches/2025-12-11_OPT-FA-7_consolidate-derived-stores.md`
- `docs/taches/2025-12-11_OPT-FA-8_workflow-executor-service.md`
- `docs/taches/2025-12-11_OPT-FA-9_aggregate-pagestate-interface.md`
- `docs/taches/2025-12-11_OPT-FA-12_lucide-migration.md`
- `docs/taches/2025-12-11_OPT-FA-13-memoize-activity-filtering.md`

All archived files to be removed after documentation sync.
