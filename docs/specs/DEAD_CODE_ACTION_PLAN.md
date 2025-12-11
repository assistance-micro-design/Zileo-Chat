# Dead Code Action Plan

> **Version**: 1.0
> **Date**: 2025-12-11
> **Status**: Ready for Implementation
> **Supersedes**: DEAD_CODE_ANALYSIS.md (contains outdated analysis)

## Executive Summary

Analysis of 150 `#[allow(dead_code)]` annotations revealed that **many are erroneously applied to actively used code**. This document provides a corrected action plan categorized by actual code status.

| Category | Count | Action |
|----------|-------|--------|
| Cleanup (remove annotations) | 35+ | Remove `#[allow(dead_code)]` |
| Implement (connect existing) | 8 | Wire up existing code |
| Keep (intentional) | 60+ | No action needed |
| Delete (obsolete) | 5 | Remove code entirely |

---

## Category 1: CLEANUP - Remove Erroneous Annotations

These modules/functions are **actively used in production** but still have dead_code annotations that should be removed.

### 1.1 LLM Provider System (HIGH PRIORITY)

**Files to clean:**

| File | Symbol | Used In | Action |
|------|--------|---------|--------|
| `llm/manager.rs:86-87` | `impl ProviderManager` | state.rs, llm_agent.rs, context.rs, spawn_agent.rs, 12+ files | Remove `#[allow(dead_code)]` |
| `llm/circuit_breaker.rs:68-69` | `impl CircuitBreakerConfig` | manager.rs (circuit_breakers field) | Remove `#[allow(dead_code)]` |
| `llm/circuit_breaker.rs:148-149` | `impl CircuitBreaker` | manager.rs lines 110, 117, 147, 154 | Remove `#[allow(dead_code)]` |
| `llm/pricing.rs:16` | Module-level | streaming.rs:506 `calculate_cost()` | Remove module-level annotation |

**Evidence of usage:**
```rust
// src-tauri/src/commands/streaming.rs:506
let cost_usd = calculate_cost(
    report.metrics.tokens_input,
    ...
);

// src-tauri/src/llm/manager.rs:110
CircuitBreaker::new(
    CircuitBreakerConfig::for_llm_provider(),
    ...
)
```

### 1.2 Embedding System (HIGH PRIORITY)

**Files to clean:**

| File | Symbol | Used In | Action |
|------|--------|---------|--------|
| `llm/embedding.rs:44` | Module-level | state.rs, factory.rs, memory/tool.rs, commands/embedding.rs | Remove module-level annotation |
| `state.rs:46-47` | `embedding_service` field | commands/embedding.rs:171-172, factory.rs:85 | Remove field annotation |
| `state.rs:124-125` | `set_embedding_service()` | commands/embedding.rs, state.rs:287 | Remove method annotation |
| `state.rs:132-133` | `get_embedding_service()` | factory.rs:95, context.rs | Remove method annotation |

**Evidence of usage:**
```rust
// src-tauri/src/commands/embedding.rs:171
let service = EmbeddingService::with_provider(provider);

// src-tauri/src/tools/factory.rs:95
async fn get_embedding_service(&self) -> Option<Arc<EmbeddingService>> {
    self.embedding_service.read().await.clone()
}
```

### 1.3 Tool Factory (HIGH PRIORITY)

**Files to clean:**

| File | Symbol | Used In | Action |
|------|--------|---------|--------|
| `state.rs:39-40` | `tool_factory` field | main.rs, context.rs, spawn_agent.rs, llm_agent.rs | Remove field annotation |

**Evidence of usage:**
```rust
// src-tauri/src/main.rs - ToolFactory is created and stored in AppState
// src-tauri/src/agents/llm_agent.rs:133
pub fn with_factory(..., tool_factory: Arc<ToolFactory>, ...)
```

### 1.4 Streaming Events (MEDIUM PRIORITY)

**Files to clean:**

| File | Symbol | Used In | Action |
|------|--------|---------|--------|
| `models/streaming.rs:381-393` | `task_create()` | tools/todo/tool.rs:144 | Remove method annotation |
| `models/streaming.rs:411-426` | `task_update()` | tools/todo/tool.rs:196 | Remove method annotation |

**Evidence of usage:**
```rust
// src-tauri/src/tools/todo/tool.rs:144
self.emit_task_event(StreamChunk::task_create(

// src-tauri/src/tools/todo/tool.rs:196
self.emit_task_event(StreamChunk::task_update(
```

### 1.5 Cancellation System (MEDIUM PRIORITY)

**Files to clean:**

| File | Symbol | Used In | Action |
|------|--------|---------|--------|
| `state.rs:168-178` | `request_cancellation()` | commands/streaming.rs:702 | Remove method annotation |
| `state.rs:180-184` | `clear_cancellation()` | Internal cleanup | Remove method annotation |
| `state.rs:136-143` | `create_cancellation_token()` | commands/streaming.rs | Remove method annotation |

**Evidence of usage:**
```rust
// src-tauri/src/commands/streaming.rs:702
state.request_cancellation(&validated_id).await;
```

---

## Category 2: IMPLEMENT - Wire Up Existing Code

These modules are **fully implemented with tests** but never used in production. They need to be connected.

### 2.1 Query Builders (MEDIUM PRIORITY - Security)

**File**: `src-tauri/src/tools/utils.rs`
**Status**: Complete implementation with 5 tests, never used
**Effort**: 2-3 hours per migration

| Symbol | Lines | Implementation Status |
|--------|-------|----------------------|
| `QueryBuilder` | 130-193 | Complete with `select()`, `where_eq()`, `order_by()`, `limit()`, `build()` |
| `ParamQueryBuilder` | 203-280 | Complete with parameterized queries |

**Current Problem:**
```rust
// CURRENT (vulnerable to injection)
let query = format!("SELECT * FROM memory WHERE type = '{}'", memory_type);

// RECOMMENDED (use existing QueryBuilder)
let query = QueryBuilder::new("memory")
    .select(&["*"])
    .where_eq("type", memory_type)
    .build();
```

**Action Items:**
1. Create tracking issue for migration
2. Prioritize user-input-facing queries
3. Migrate incrementally (5-10 queries per PR)

### 2.2 Sub-Agent Progress Streaming (LOW PRIORITY - UX)

**File**: `src-tauri/src/models/streaming.rs:288-303`
**Status**: Method exists, tested, never emitted
**Effort**: 2-3 hours

**Current State:**
```rust
// Method exists but is never called during execution
pub fn sub_agent_progress(
    agent_id: &str,
    workflow_id: &str,
    status: &str,
    progress: u8,
) -> Self { ... }
```

**Action Items:**
1. Add progress emission to `tools/sub_agent_executor.rs`
2. Handle in frontend `src/lib/stores/streaming.ts`
3. Display progress indicator in agent panel

### 2.3 Cancellation Token Helpers (LOW PRIORITY)

**File**: `src-tauri/src/state.rs`
**Status**: Methods exist, tested, not used in execution loop

| Symbol | Lines | Purpose |
|--------|-------|---------|
| `get_cancellation_token()` | 150-161 | Get token for workflow |
| `is_cancelled()` | 163-167 | Check cancellation status |

**Current State:**
- `request_cancellation()` is used to REQUEST cancellation
- `get_cancellation_token()` and `is_cancelled()` are meant for CHECKING in execution loops
- Execution loops don't currently check for cancellation

**Action Items:**
1. Add cancellation checks to `agents/llm_agent.rs` execution loop
2. Add cancellation checks to `tools/sub_agent_executor.rs`

---

## Category 3: KEEP - Intentional Dead Code

These items should **retain `#[allow(dead_code)]`** with explanatory comments.

### 3.1 Serde Deserialization Fields (REQUIRED)

| File | Lines | Reason |
|------|-------|--------|
| `llm/embedding.rs:322-357` | MistralEmbeddingResponse fields | Required for API JSON parsing |
| `llm/mistral.rs:*` | Response struct fields | Required for API JSON parsing |
| `models/mcp.rs:*` | MCP protocol fields | Required for protocol compliance |

**Example:**
```rust
#[allow(dead_code)] // Required for serde deserialization
pub struct MistralEmbeddingResponse {
    pub id: String,        // Present in JSON but not accessed
    pub object: String,    // Present in JSON but not accessed
    pub data: Vec<...>,    // Actually used
}
```

### 3.2 Test Infrastructure (REQUIRED)

| File | Purpose |
|------|---------|
| `commands/validation.rs:524` | Test utilities |
| `commands/memory.rs:593` | Test state setup |
| `commands/agent.rs:549` | Test state setup |
| `commands/task.rs:554` | Test utilities |

### 3.3 Builder Pattern Methods (API COMPLETENESS)

| File | Reason |
|------|--------|
| `models/function_calling.rs:46,177,243` | Standard builder pattern for future extensibility |
| `agents/llm_agent.rs:88,109,131` | Multiple constructor variants |
| `models/memory.rs:90,101` | Memory creation helpers |

### 3.4 Resilience Infrastructure (FUTURE USE)

| File | Symbol | Status |
|------|--------|--------|
| `tools/sub_agent_executor.rs:349` | `with_resilience()` | Prepared for OPT-SA-* |
| `llm/retry.rs:65` | `RetryConfig` impl | Custom retry available |
| `mcp/mod.rs:74-88` | Various modules | Phase 2-3 preparation |

---

## Category 4: DELETE - Obsolete Code

Code that should be **removed entirely**.

### 4.1 Duplicate Validation Logic

**File**: `src-tauri/src/security/validation.rs`
**Issue**: Some validation functions duplicate logic in `commands/validation.rs`

**Investigation Required:**
1. Compare `security/validation.rs` with `commands/validation.rs`
2. Identify truly duplicate functions
3. Consolidate into single location

### 4.2 Outdated Phase Comments

Throughout codebase, there are comments like:
```rust
// Phase 2 preparation
// Phase 3 - will be used later
```

**Action**: Remove phase references after this cleanup, as phases are no longer relevant.

---

## Implementation Priority

### Sprint 1 (Immediate - 1 day)

Remove erroneous `#[allow(dead_code)]` from actively used code:

1. **llm/manager.rs** - ProviderManager impl
2. **llm/circuit_breaker.rs** - CircuitBreaker and Config impl
3. **llm/pricing.rs** - Module-level annotation
4. **llm/embedding.rs** - Module-level annotation
5. **state.rs** - embedding_service, tool_factory, cancellation methods
6. **models/streaming.rs** - task_create, task_update

**Expected Result**: ~35 fewer `#[allow(dead_code)]` annotations

### Sprint 2 (Near-term - 3 days)

Connect existing but unused code:

1. Migrate 10 critical queries to use `QueryBuilder`
2. Add sub-agent progress emission
3. Add cancellation checks to execution loops

### Sprint 3 (Maintenance)

1. Consolidate duplicate validation code
2. Remove outdated phase comments
3. Add explanatory comments to intentional dead_code

---

## Verification Script

After Sprint 1, run this command to verify annotation count reduced:

```bash
cd src-tauri && grep -r "#\[allow(dead_code)\]" src/ | wc -l
# Expected: ~115 (down from 150)
```

---

## Appendix: Files to Modify (Sprint 1)

| File | Current Annotations | Remove | Keep |
|------|---------------------|--------|------|
| `llm/manager.rs` | 2 | 2 | 0 |
| `llm/circuit_breaker.rs` | 5 | 3 | 2 |
| `llm/pricing.rs` | 1 (module) | 1 | 0 |
| `llm/embedding.rs` | 9 | 4 | 5 |
| `state.rs` | 8 | 6 | 2 |
| `models/streaming.rs` | 6 | 2 | 4 |
| `tools/factory.rs` | 0 | 0 | 0 |
| **Total** | 31 | 18 | 13 |

---

*Document generated: 2025-12-11*
*Based on actual code analysis, not documentation*
