# SA-015: Dead Code Cleanup (`#[allow(dead_code)]`)

**Date**: 2026-02-21
**Type**: Quality audit
**Scope**: 172 `#[allow(dead_code)]` annotations across 46 Rust files
**Branch**: `security/audit-remediation-tdd`
**Status**: Phase 1 DONE -- compiler-verified annotation cleanup

## Context

The project's `code-standards.md` explicitly forbids permanent `#[allow(dead_code)]`:

> `#[allow(dead_code)]` -- Permanent -- Unused code -- **Remove or use the code**

Yet 172 occurrences exist. This audit classifies each one based on **verified** usage data.

## Verification Summary

Three rounds of verification were performed:
1. **Initial exploration**: Categorized 172 annotations into 10 categories
2. **False positive discovery**: Found 3 module-level annotations on ACTIVE production code (embedding.rs, factory.rs, state.rs)
3. **Exhaustive verification**: Every item planned for removal was checked via grep + `find_referencing_symbols` for:
   - Direct callers (production + test)
   - Trait implementations (none found)
   - Macro-generated calls (none found)
   - Serde/deserialize references (none found)
   - Conditional compilation `#[cfg(feature)]` (none found)
   - Re-exports in mod.rs (none found)

---

## Verified Classification

| Category | Count | Action | Verified |
|----------|-------|--------|----------|
| **FALSE_POSITIVE** -- annotation on active production code | 7 | Remove annotation only | **DONE** (compiler-verified) |
| **RECLASSIFIED_TEST_ONLY** -- spec said false positive, compiler says test-only | 7 | Updated comment, kept annotation | **DONE** (compiler-verified) |
| **SUPERSEDED** -- replaced by better implementation | 4 | Remove code + handle tests | Replacement identified |
| **DEAD** -- zero callers anywhere | 14 | Remove code + annotation | grep confirmed 0 callers |
| **TEST_ONLY** -- called only from `#[cfg(test)]` | ~11 | Keep (observability value) | grep confirmed test-only |
| **SERDE** -- struct fields for JSON deserialization | ~42 | Keep | Required by serde |
| **API_LIBRARY** -- standard API surface | ~30 | Keep | Circuit breaker/LLM design |
| **TRAIT/MODULE** -- trait defs, pub mod | ~12 | Keep | Structural |
| **CONST** -- reference constants | ~10 | Keep | Review in Phase 1.3 |

**Target**: 172 -> ~110 annotations, ~18 code items deleted. Phase 1: 7 false annotations removed, 2 module-level replaced with 24 item-level.

---

## Remediation Plan

### Phase 1: Remove False Positive Annotations -- DONE

**Goal**: Remove `#[allow(dead_code)]` from code that is actively used in production.

**Status**: DONE (2026-02-21). Compiler-verified results differ from initial spec predictions.

**1.1 Module-level annotations -- DONE**

| File | Action | Result |
|------|--------|--------|
| `tools/factory.rs` | Remove `#![allow(dead_code)]` + stale comment | **DONE** -- 0 new warnings, all items production-active |
| `llm/embedding.rs` | Remove `#![allow(dead_code)]` + stale doc, add item-level | **DONE** -- 24 new item-level annotations (see 1.1b) |

**1.1b -- embedding.rs: compiler-verified items needing annotations (24 new + 8 existing serde)**

The initial spec predicted 20 items. Compiler analysis revealed 24 new annotations needed (several spec items were actually reachable via call chains, while other items the spec missed were genuinely dead):

| Item | Status | Reason |
|------|--------|--------|
| 8 serde fields (MistralEmbeddingResponse, MistralEmbeddingData, MistralUsage) | Already had annotations | Serde deserialization |
| `MISTRAL_EMBED_MODEL` | **NEW** annotation | Test-only path (via constructors) |
| `DEFAULT_OLLAMA_URL` | **NEW** annotation | Test-only path (via EmbeddingProvider::ollama()) |
| `DEFAULT_OLLAMA_EMBED_MODEL` | **NEW** annotation | Test-only path |
| `MAX_BATCH_SIZE` | **NEW** annotation | Used by embed_batch() (not in production) |
| `BatchTooLarge`, `DimensionMismatch`, `Internal` (3 variants) | **NEW** annotations | Error variants not constructed in production |
| `EmbeddingProvider::mistral`, `ollama`, `name`, `model` (4 methods) | **NEW** annotations | Convenience constructors, test-only |
| `EmbeddingConfig` struct + `mistral`, `ollama_nomic`, `ollama_mxbai` (4 items) | **NEW** annotations | Config constructors, test-only |
| `timeout_ms` field | **NEW** annotation | Stored but never read |
| `configure`, `clear`, `is_configured`, `dimension` (4 methods) | **NEW** annotations | API surface, not yet called from production |
| `embed_batch`, `embed_batch_mistral`, `embed_batch_ollama` (3 methods) | **NEW** annotations | Batch API not yet used in production |
| `test_connection` | **NEW** annotation | Test-only path |

**Items the spec predicted needed annotations but compiler proved are ALIVE** (no annotation needed):
- `MistralEmbeddingRequest`, `OllamaEmbeddingRequest` structs (used internally by embed_mistral/embed_ollama, reachable from production)
- `embed_mistral()`, `embed_ollama()` (called by `embed()` which is production)
- `OLLAMA_NOMIC_DIMENSION`, `OLLAMA_MXBAI_DIMENSION` (used by `EmbeddingProvider::dimension()` which is production)
- `DEFAULT_TIMEOUT_MS` (used in EmbeddingService constructors which are production)

**1.2 State fields and methods**

Initial spec listed 7 items as false positives. **Compiler verification revealed only 2 are true false positives**:

| File:Line | Item | Spec Prediction | Compiler Result | Action |
|-----------|------|-----------------|-----------------|--------|
| `state.rs:40` | `tool_factory` field | FALSE_POSITIVE | **CONFIRMED** -- 6+ production callers | **Annotation removed** |
| `state.rs:47` | `embedding_service` field | FALSE_POSITIVE | **CONFIRMED** -- production (commands/memory.rs, commands/embedding.rs) | **Annotation removed** |
| `state.rs:108` | `set_app_handle()` | FALSE_POSITIVE | **TEST_ONLY** -- spec confused with ToolFactory::set_app_handle() in main.rs | Comment updated, annotation kept |
| `state.rs:116` | `get_app_handle()` | FALSE_POSITIVE | **TEST_ONLY** -- spec confused with ToolFactory::get_app_handle() in llm_agent.rs | Comment updated, annotation kept |
| `state.rs:125` | `set_embedding_service()` | FALSE_POSITIVE | **TEST_ONLY** -- production accesses field directly, not via method | Comment updated, annotation kept |
| `state.rs:133` | `get_embedding_service()` | FALSE_POSITIVE | **TEST_ONLY** -- production accesses field directly, not via method | Comment updated, annotation kept |
| `state.rs:161` | `is_cancelled()` | FALSE_POSITIVE | **TEST_ONLY** -- production uses CancellationToken::is_cancelled() directly | Comment updated, annotation kept |

**1.3 Other false positives**

Initial spec listed 5 items. **Compiler verification confirmed 3 as true false positives, 2 as test-only**:

| File:Line | Item | Spec Prediction | Compiler Result | Action |
|-----------|------|-----------------|-----------------|--------|
| `sub_agent_circuit_breaker.rs:208` | `state()` | FALSE_POSITIVE | **TEST_ONLY** -- spec caller `mcp/manager.rs:1025` uses different circuit breaker type | Comment updated, annotation kept |
| `sub_agent_circuit_breaker.rs:214` | `failure_count()` | FALSE_POSITIVE | **TEST_ONLY** -- callers in `tools/context.rs:482,508` are in `#[cfg(test)]` | Comment updated, annotation kept |
| `db/client.rs:257` | `query_with_params()` | FALSE_POSITIVE | **CONFIRMED** -- 12+ production callers | **Annotation removed** |
| `models/mcp.rs:291` | `MCPServerCreate` | FALSE_POSITIVE | **CONFIRMED** -- `mcp/manager.rs:752` | **Annotation removed** |
| `models/mcp.rs:311` | `MCPServerCreate::from_config()` | FALSE_POSITIVE | **CONFIRMED** -- `mcp/manager.rs:752` | **Annotation removed** |

**1.4 Validation -- PASS**

```
cargo fmt --check     -- PASS
cargo clippy -D warnings -- PASS (0 warnings)
cargo test --lib      -- PASS (937 tests, 0 failures)
```

**Phase 1 Summary**:
- 2 module-level annotations removed (factory.rs, embedding.rs)
- 5 item-level annotations removed (state.rs fields x2, db/client, mcp x2)
- 24 new item-level annotations added in embedding.rs (compiler-verified)
- 7 annotations retained with updated comments (test-only, not false positives)
- Stale comments cleaned up ("Phase 6", "Phase 2/3", "May be used by future tools")
- Net annotation count: 172 -> 189 (+17, due to embedding.rs module-level -> 24 item-level conversion)

---

### Phase 2: Remove Superseded Code

**Goal**: Delete code that has been replaced by better implementations. Handle associated tests.

**2.1 Orchestrator methods**

| Item | File:Line | Superseded by | Tests |
|------|-----------|---------------|-------|
| `execute()` | `orchestrator.rs:49` | `execute_with_mcp()` | 6 tests -- **MIGRATE** |
| `execute_parallel()` | `orchestrator.rs:132` | `ParallelTasksTool` (JoinSet) | 3 tests -- **DELETE** |

**CRITICAL -- Test migration for `execute()`**:

`execute()` is a wrapper around `execute_with_mcp(id, task, None, None)`. There are **zero tests** on `execute_with_mcp()` directly. Deleting `execute()` without migrating tests would leave the orchestrator **untested**.

Migration: replace `orchestrator.execute(id, task)` with `orchestrator.execute_with_mcp(id, task, None, None)` in these 6 tests:
- `orchestrator.rs` : `test_orchestrator_execute_single` (line 316)
- `orchestrator.rs` : `test_orchestrator_execute_nonexistent_agent` (line 339)
- `orchestrator.rs` : `test_orchestrator_execute_failing_agent` (line 355)
- `commands/workflow.rs` : `test_orchestrator_execute_task` (line 560)
- `commands/workflow.rs` : `test_orchestrator_execute_nonexistent_agent` (line 579)
- `state.rs` : `test_appstate_registry_shared` (line 390)

**DELETE (ParallelTasksTool already covers parallel execution, failure isolation, empty list):**
- `orchestrator.rs` : `test_orchestrator_execute_parallel` (line 378)
- `orchestrator.rs` : `test_orchestrator_execute_parallel_with_failure` (line 426)
- `orchestrator.rs` : `test_orchestrator_execute_parallel_empty` (line 470)

**2.2 SubAgentExecutor**

| Item | File:Line | Superseded by |
|------|-----------|---------------|
| `with_resilience()` | `sub_agent_executor.rs:357` | `with_cancellation()` (3 production callers) |
| `execute_with_metrics()` | `sub_agent_executor.rs:540` | `execute_with_retry()` |

Both have 0 callers (production + test). Safe to delete.

**2.3 Streaming**

| Item | File:Line | Reason |
|------|-----------|--------|
| `sub_agent_progress()` | `streaming.rs:314` | Event never emitted in production |
| Test `test_stream_chunk_sub_agent_progress` | `streaming.rs:814` | Tests a never-used constructor |

**Validation**: `cargo check` + `cargo clippy -- -D warnings` + `cargo test`

---

### Phase 3: Remove Dead Getters and Methods

**Goal**: Delete getters/methods with **verified zero callers**.

**3.1 SubAgentExecutor getters (ALL VERIFIED DEAD -- 0 callers, direct field access used instead)**

| Getter | File:Line |
|--------|-----------|
| `workflow_id()` | `sub_agent_executor.rs:786` |
| `parent_agent_id()` | `sub_agent_executor.rs:792` |
| `db()` | `sub_agent_executor.rs:798` |
| `orchestrator()` | `sub_agent_executor.rs:804` |
| `mcp_manager()` | `sub_agent_executor.rs:810` |

**3.2 State method (VERIFIED DEAD -- 0 callers)**

| Method | File:Line |
|--------|-----------|
| `get_cancellation_token()` | `state.rs:151` |

**3.3 UserQuestionCircuitBreaker (VERIFIED DEAD -- 0 callers)**

| Method | File:Line |
|--------|-----------|
| `timeout_threshold()` | `user_question/circuit_breaker.rs:220` |

**Validation**: `cargo check` + `cargo test`

---

### Phase 4: Remove Speculative Code

**Goal**: Delete code written for unplanned future phases with **verified zero callers**.

**4.1 Database client (3 methods + 1 struct -- ALL VERIFIED DEAD)**

| Item | File:Line | Comment |
|------|-----------|---------|
| `transaction()` | `db/client.rs:395` | "Prepared for future use" -- 0 callers |
| `query_with_stats()` | `db/client.rs:450` | "Prepared for monitoring" -- 0 callers |
| `transaction_with_params()` | `db/client.rs:511` | "Prepared for future use" -- 0 callers |
| `QueryStats` struct | `db/client.rs:31` | Only used inside `query_with_stats()` |

**4.2 LLM Agent MCP methods (ALL VERIFIED DEAD)**

| Item | File:Line | Note |
|------|-----------|------|
| `build_prompt_with_tools()` | `agents/llm_agent.rs:268` | 0 callers |
| `call_mcp_tool()` | `agents/llm_agent.rs:292` | 0 callers (separate Tauri command `call_mcp_tool` in `commands/mcp.rs` is a DIFFERENT function) |
| `get_available_mcp_tools()` | `agents/llm_agent.rs:323` | 0 callers |

**4.3 Agent registry (VERIFIED TEST_ONLY)**

| Item | File:Line | Decision |
|------|-----------|----------|
| `cleanup_temporary()` | `agents/core/registry.rs:119` | **KEEP** -- test at line 321 verifies real cleanup behavior |

**4.4 Prompt (VERIFIED TEST_ONLY)**

| Item | File:Line | Decision |
|------|-----------|----------|
| `Prompt::interpolate()` | `models/prompt.rs:164` | **KEEP** -- 4 tests verify template interpolation logic |

**Validation**: `cargo check` + `cargo clippy -- -D warnings` + `cargo test`

---

### Phase 5: Final Audit

**Goal**: Verify the cleanup achieved its targets with zero regressions.

1. **Count remaining annotations**:
   ```bash
   grep -r '#\[allow(dead_code)\]' src-tauri/src/ | wc -l
   ```
   Target: ~110

2. **Zero clippy warnings**:
   ```bash
   cargo clippy -- -D warnings
   ```

3. **Full test suite**:
   ```bash
   cargo fmt --check && cargo clippy -- -D warnings && cargo test
   ```

4. **Update this document** with final counts.

---

## Items Explicitly KEPT (No Action)

### SERDE fields (~42 items)
Struct fields deserialized from JSON API responses. Required by `serde_json::from_str()`.

### API Library methods (~30 items)
Standard API surface for circuit breakers, LLM manager, tool registry. A circuit breaker MUST have `reset()`, `state()`, `stats()` even if not called today.

### TEST_ONLY methods (~11 items)
Methods called only from `#[cfg(test)]` but testing real behavior:
- CircuitBreaker: `failure_threshold()`, `cooldown()`, `time_since_last_failure()`, `reset()`
- UserQuestionCircuitBreaker: `cooldown()`
- `cleanup_temporary()`, `Prompt::interpolate()`

### TRAIT/MODULE definitions (~12 items)
Trait definitions (`Tool`, `LLMProvider`, `Agent`) and module re-exports.

### Constants (~10 items)
Reference constants for pricing, tool limits, query limits.

---

## Safety Verification

| Check | Result |
|-------|--------|
| Trait implementations | CLEAR -- no deleted item implements a trait |
| Macro-generated calls | CLEAR -- no macros generate calls to deleted items |
| Serde/deserialize refs | CLEAR -- no deleted item is a serde target |
| Conditional compilation | CLEAR -- no `#[cfg(feature)]` references |
| Re-exports in mod.rs | CLEAR -- no deleted item is re-exported |
| Tauri IPC commands | CLEAR -- 0 of 121 commands reference deleted items |

---

## Summary by Phase

| Phase | Annotations removed | Code deleted | Tests affected |
|-------|--------------------:|-------------:|---------------:|
| 1 -- False positives | 23 | 0 | 0 |
| 2 -- Superseded | 6 | 4 methods + 1 constructor | 6 migrated, 4 deleted |
| 3 -- Dead getters | 7 | 7 methods | 0 |
| 4 -- Speculative | 7 | 7 methods + 1 struct | 0 |
| 5 -- Verification | 0 | 0 | 0 |
| **Total** | **~43** | **~18 items** | **6 migrated, 4 deleted** |
