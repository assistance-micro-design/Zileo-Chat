# SA-007 - Commands Rust Audit: Control Flow + Error Handling

| Field | Value |
|-------|-------|
| **Date** | 2026-02-19 |
| **Status** | Documented |
| **Scope** | `src-tauri/src/commands/` (21 files, 12,315 lines) |
| **Findings** | 13 oversized functions, 261 map_err calls, 6 cross-file duplication patterns |

---

## 1. Per-File Metrics Table

| File | Lines | Prod Fns | Fns >100L | Max Nesting | map_err | unwrap (prod) | expect (prod) |
|------|------:|:--------:|:---------:|:-----------:|--------:|:-------------:|:-------------:|
| **import_export.rs** | 1291 | 6 | **4** | 4 | 12 | 0 | 0 |
| **mcp.rs** | 1099 | 17 | 0 | 3 | 10 | 0 | 0 |
| **models.rs** | 1056 | 11 | **2** | 4 | 20 | 0 | 0 |
| **agent.rs** | 878 | 10 | **2** | 3 | 14 | 0 | 0 |
| **embedding.rs** | 793 | 12 | 0 | 4 | 16 | 0 | 0 |
| **streaming.rs** | 768 | 5 | **1** | 4 | 6 | 0 | 0 |
| **workflow.rs** | 706 | 5 | **2** | 4 | 24 | 0 | 0 |
| **validation.rs** | 678 | 11 | 0 | 3 | 19 | 0 | 0 |
| **task.rs** | 670 | 8 | 0 | 2 | 19 | 0 | 0 |
| **user_question.rs** | 589 | 6 | 0 | 1 | 10 | 0 | 0 |
| **memory.rs** | 568 | 6 | 0 | 2 | 10 | 0 | 0 |
| **tool_execution.rs** | 472 | 7 | **1** | 2 | 17 | 0 | 0 |
| **message.rs** | 404 | 5 | 0 | 1 | 12 | 0 | 0 |
| **migration.rs** | 382 | 4 | 0 | 1 | 9 | 0 | 0 |
| **thinking.rs** | 353 | 6 | 0 | 1 | 14 | 0 | 0 |
| **prompt.rs** | 351 | 7 | 0 | 1 | 15 | 0 | 0 |
| **custom_provider.rs** | 335 | 5 | **1** | 3 | 10 | 0 | 0 |
| **llm.rs** | 320 | 9 | 0 | 1 | 11 | 0 | 0 |
| **security.rs** | 272 | 9 | 0 | 1 | 8 | 0 | 0 |
| **sub_agent_execution.rs** | 151 | 2 | 0 | 0 | 5 | 0 | 0 |
| **mod.rs** | 179 | 0 | 0 | 0 | 0 | 0 | 0 |
| **TOTAL** | **12,315** | **155** | **13** | **4** | **261** | **0** | **0** |

---

## 2. Functions > 100 Lines (ranked)

| # | File | Function | Lines | Max Nesting |
|---|------|----------|------:|:-----------:|
| 1 | streaming.rs | `execute_workflow_streaming` | **616** | 4 |
| 2 | import_export.rs | `execute_import` | **506** | 4 |
| 3 | import_export.rs | `generate_export_file` | **261** | 4 |
| 4 | import_export.rs | `validate_import` | **225** | 3 |
| 5 | workflow.rs | `load_workflow_full_state` | **201** | 4 |
| 6 | import_export.rs | `prepare_export_preview` | **150** | 3 |
| 7 | workflow.rs | `execute_workflow` | **147** | 2 |
| 8 | models.rs | `test_provider_connection` | **117** | 4 |
| 9 | tool_execution.rs | `save_tool_execution` | **115** | 2 |
| 10 | agent.rs | `load_agents_from_db` | **111** | 3 |
| 11 | custom_provider.rs | `update_custom_provider` | **108** | 3 |
| 12 | agent.rs | `update_agent` | **104** | 2 |
| 13 | models.rs | `create_model` | **103** | 4 |

---

## 3. Error Handling Analysis

### 3.1 Global Stats

- **261 total `.map_err()` calls** across 20 files
- **173 `.unwrap_or()` / `.unwrap_or_default()` calls** (safe fallbacks, primarily in import_export.rs: 105)
- **52 bare `.unwrap()` in test code only** -- 0 in production
- **19 `.expect()` in test code only** -- 0 in production (all in `setup_test_state`)

### 3.2 Message Prefix Consistency

| Pattern | Count | Consistent? |
|---------|------:|:-----------:|
| `"Failed to X: {}"` | ~200 | YES -- dominant pattern |
| `"Invalid X: {}"` | ~35 | YES -- for validation |
| `"Database error: {}"` | 15 | NO -- too generic (models.rs: 7, task.rs: 8) |
| `"X failed: {}"` / misc | ~11 | NO -- inconsistent phrasing |

**Problem**: `"Database error: {}"` appears 15 times in models.rs + task.rs. These should say "Failed to list models:", "Failed to update task status:", etc. to be debuggable.

### 3.3 ResultExt Trait Candidates

The 261 `.map_err(|e| format!("Failed to X: {}", e))` calls have three dominant sub-patterns:

| Pattern | Occurrences | Refactor |
|---------|:-----------:|----------|
| `Validator::validate_uuid(&id).map_err(\|e\| format!("Invalid X: {}", e))` | **52** | `validate_uuid_field(value, "workflow_id")` |
| `state.db.query(...).await.map_err(\|e\| format!("Failed to X: {}", e))` | ~100 | `db.query_ctx(query, "load workflow")` |
| `serde_json::to_string(&v).map_err(\|e\| format!("Failed to serialize X: {}", e))` | ~25 | `serialize_for_db(val, "name")` |

A `ResultExt` trait with `.ctx("operation context")` could eliminate ~175 of the 261 calls:

```rust
trait ResultExt<T> {
    fn ctx(self, operation: &str) -> Result<T, String>;
}
impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn ctx(self, op: &str) -> Result<T, String> {
        self.map_err(|e| format!("Failed to {}: {}", op, e))
    }
}
```

---

## 4. Cross-File Structural Duplication

### 4.1 Duplicated Patterns (ranked by occurrences)

| # | Pattern | Occurrences | Files | Refactor |
|---|---------|:-----------:|:-----:|----------|
| 1 | **UUID validation boilerplate** (`Validator::validate_uuid + map_err + warn!`) | **52** | 10 | Extract `validate_uuid_field()` helper |
| 2 | **serde_json::to_string + map_err for DB** | **~25** | 5 | Extract `serialize_for_db()` helper |
| 3 | **COUNT GROUP ALL result extraction** (`.first().and_then(\|v\| v.get("count"))...unwrap_or(0)`) | **16** | 8 | Extract `extract_count()` helper |
| 4 | **`"Database error: {}"` generic messages** | **15** | 2 | Replace with contextual messages |
| 5 | **query_json + serde_json::from_value + collect + map_err** (deserialization chain) | **12+** | 5 | Extract `db.query_typed::<T>(query)` |
| 6 | **SELECT field lists** repeated 2-4x per entity within file | **~18** | 6 | `const X_FIELDS: &str` per entity |
| 7 | **setup_test_state** (identical temp_dir + DB + schema + MCP manager) | **5** | 5 | Extract to `test_utils::setup()` |
| 8 | **clear_workflow_X** (validate UUID, count, delete, return count) | **4** | 4 | Generic `clear_workflow_records(table)` |
| 9 | **Validator::validate_provider + map_err** (identical block) | **4** | 1 | Extract `validate_provider_input()` |
| 10 | **ProviderInfo struct literal** (identical construction) | **3** | 1 | `ProviderInfo::from_custom()` |

### 4.2 import_export.rs: Special Case

This file has the worst structural duplication in the codebase. The 4 entity types (agent, mcp_server, model, prompt) each have nearly identical code blocks for:

- Preview loading (4x identical loops)
- Export serialization (4x identical JSON construction)
- Conflict detection (4x identical query + check)
- Import execution (4x identical create/update logic)

An `EntityHandler<T>` trait or macro would cut this file from 1291 lines to ~500.

### 4.3 streaming.rs: Monolith Function

`execute_workflow_streaming` at 616 lines handles:

1. Input validation + DB loading (~70 lines)
2. History construction + system prompt (~60 lines)
3. LLM execution with cancellation + tokio::select! (~150 lines)
4. Response persistence + token tracking (~80 lines)
5. Tool execution persistence (~60 lines)
6. Thinking step persistence (~50 lines)
7. Result building + event emission (~50 lines)
8. Error recovery + cleanup (~90 lines)

Each of these is a natural extraction point.

### 4.4 workflow.rs: Deserialization Boilerplate

The `query_json -> into_iter().map(serde_json::from_value).collect::<Result<Vec<T>, _>>().map_err(...)` pattern appears 5+ times in workflow.rs alone and 12+ times across all files. A generic `fn parse_json_results<T: DeserializeOwned>(results: Vec<Value>, ctx: &str) -> Result<Vec<T>, String>` would eliminate all of these.

### 4.5 user_question.rs: Unused Helper

`validate_question_pending` exists as a helper function but `skip_question` duplicates its logic inline instead of calling it.

---

## 5. Additional Findings

### 5.1 Dead Code

- `load_agents_from_db` in agent.rs (111 lines) is marked `#[allow(dead_code)]` -- should be removed or used.

### 5.2 SQL Injection Surface

- `search_prompts` in prompt.rs interpolates `search_term` directly into SurrealQL string with `CONTAINS '{}'` without parameterized query. Follow-up to SA-001.

### 5.3 Silent Error Swallowing

- `sub_agent_execution.rs:118`: `.unwrap_or_default()` on a DB query result silently ignores database failures.

### 5.4 Clippy Suppressions

- `#[allow(clippy::too_many_arguments)]` on 3 files (tool_execution, message, thinking) -- inherent to Tauri command parameter passing pattern.

---

## 6. Top 10 Refactors by Impact

| # | Refactor | Effort | Lines Saved | Files Touched |
|---|----------|:------:|:-----------:|:-------------:|
| **1** | **`ResultExt` trait** with `.ctx("operation")` | S | ~175 map_err -> 1-liner | 20 |
| **2** | **Decompose `execute_workflow_streaming`** into 6-8 sub-functions | M | 0 (same total) but -616 line fn | 1 |
| **3** | **`EntityHandler` trait for import_export.rs** | L | ~700 | 1 |
| **4** | **`validate_uuid_field()` helper** | S | ~150 | 10 |
| **5** | **`extract_count()` DB helper** | S | ~50 | 8 |
| **6** | **Shared `test_utils::setup_test_state()`** | S | ~160 | 5 |
| **7** | **`clear_workflow_records()` generic** | S | ~120 | 4 |
| **8** | **`const X_FIELDS` per entity** for SELECT queries | S | ~60 | 6 |
| **9** | **Fix `"Database error: {}"` messages** to contextual | S | 0 (debuggability++) | 2 |
| **10** | **`db.query_typed::<T>()`** generic deserialization | M | ~80 | 5 |

**Effort key**: S = < 1h, M = 1-3h, L = 3-8h

---

## 7. Compliance Summary

| Rule | Status |
|------|--------|
| No `.unwrap()` in production | **PASS** (0 in prod, 52 in tests) |
| No `.expect()` in production | **PASS** (0 in prod, 19 in tests) |
| All commands return `Result<T, String>` | **PASS** |
| No `println!` (use tracing) | **PASS** |
| Consistent error messages | **PARTIAL** -- 15x generic "Database error" |
| No dead code | **FAIL** -- `load_agents_from_db` with `#[allow(dead_code)]` |
| Functions < 100 lines | **FAIL** -- 13 functions exceed, 1 at 616 lines |
