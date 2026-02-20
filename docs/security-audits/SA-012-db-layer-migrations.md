# SA-012: DB Layer & Migrations Audit

**Date**: 2026-02-19
**Scope**: `src-tauri/src/db/` (all files) + schema compliance + ERR_SURREAL_* verification
**Status**: Documented (fixes pending)

## Summary

| Category | Count | Details |
|----------|-------|---------|
| Architecture | 5 files | client.rs, schema.rs, queries.rs, utils.rs, mod.rs |
| Tables defined | 17 | All SCHEMAFULL with DEFINE TABLE OVERWRITE |
| Tables SCHEMALESS | 2 | `prompt`, `settings` (not in schema.rs) |
| ERR_SURREAL compliant | 7/10 | 001, 003, 005, 007, 008, 009, 010 |
| ERR_SURREAL partial | 2/10 | 002 (update method), 006 (low sanitize coverage) |
| ERR_SURREAL violations | 1/10 | 004 (superseded by SA-001) |
| TYPE object risks | 4 fields | Dynamic keys potentially dropped |
| SELECT * violations | 2 queries | validation.rs lines 128, 163 |
| Dead table | 1 | agent_state (defined but never written) |

---

## 1. DB Architecture Overview

### 1.1 File Structure

| File | Size | Purpose |
|------|------|---------|
| `client.rs` | 655 lines | DBClient struct, all query methods |
| `schema.rs` | 403 lines | SCHEMA_SQL const + MCP migration |
| `queries.rs` | 153 lines | Centralized query constants + cascade delete |
| `utils.rs` | 152 lines | `sanitize_for_surrealdb()` |
| `mod.rs` | 48 lines | Module re-exports |

### 1.2 Client API

The `DBClient` provides a well-layered API:

| Method | Purpose | Parameterized? | Used For |
|--------|---------|----------------|----------|
| `query<T>()` | SELECT with deserialization | No (raw string) | Simple queries |
| `query_json()` | SELECT as JSON | No (raw string) | SDK workaround |
| `query_with_params<T>()` | SELECT with bind params | Yes | Parameterized reads |
| `query_json_with_params()` | SELECT as JSON with bind | Yes | Safe JSON reads |
| `execute()` | Write, no result | No (raw string) | Simple writes |
| `execute_with_params()` | Write with bind, no result | Yes | Safe writes |
| `create()` | CREATE with $data bind | Yes | Record creation |
| `update()` | UPDATE via SDK `.content()` | SDK-handled | **See finding F1** |
| `delete()` | DELETE with backtick ID | No (format!) | Record deletion |
| `transaction()` | Multi-query transaction | No | Batch operations |
| `transaction_with_params()` | Parameterized transaction | Yes | Safe batch ops |

### 1.3 Query Helpers

- `queries::workflow` - Centralized SELECT constants (SELECT_BASE, SELECT_LIST, SELECT_BASIC)
- `queries::cascade` - Workflow cascade delete with `join_all` parallelism
- `tools::utils::QueryBuilder` - Fluent query builder (non-parameterized)
- `tools::utils::ParamQueryBuilder` - Parameterized query builder (safe)

---

## 2. Schema Analysis

### 2.1 DEFINE OVERWRITE Compliance (PAT_DB_003)

**Status: COMPLIANT**

All 17 tables and all fields use `DEFINE TABLE OVERWRITE` and `DEFINE FIELD OVERWRITE`. The schema is fully idempotent.

**Exception**: The inline MCP HTTP migration in `client.rs:78-81` uses `REMOVE FIELD IF EXISTS` + `DEFINE FIELD` (without OVERWRITE). This works but is inconsistent with the OVERWRITE pattern used everywhere else.

```rust
// client.rs:78-81
let mcp_http_migration = r#"
    REMOVE FIELD IF EXISTS command ON TABLE mcp_server;
    DEFINE FIELD command ON mcp_server TYPE string ASSERT $value IN ['docker', 'npx', 'uvx', 'http'];
"#;
```

### 2.2 Tables Not In Schema (SCHEMALESS)

Two tables are used in the codebase but not defined in `schema.rs`:

| Table | Used In | Risk |
|-------|---------|------|
| `prompt` | `commands/prompt.rs`, `commands/import_export.rs` | No ASSERT constraints, no field validation |
| `settings` | `commands/embedding.rs`, `tools/validation_helper.rs`, `state.rs` | Stores config objects (embedding, validation) |

These tables are SCHEMALESS by default, meaning:
- No field type validation
- No ASSERT constraints
- Dynamic keys are preserved (ERR_SURREAL_001 does not apply)
- No indexes defined

**Risk**: LOW - These tables store internal config data, not user input. But lack of schema means no DB-level validation.

### 2.3 TYPE object Fields (ERR_SURREAL_001 Risk)

| Table.Field | Sub-fields Defined? | Dynamic Keys? | Risk |
|-------------|---------------------|---------------|------|
| `agent_state.config` | No | Yes (unknown structure) | **HIGH** - but table is dead |
| `agent_state.metrics` | No | Yes (unknown structure) | **HIGH** - but table is dead |
| `memory.metadata` | Yes (tags, priority, agent_source) | Constrained to 3 sub-fields | LOW - explicit sub-fields |
| `validation_request.details` | No | Yes (serde_json::Value) | **MEDIUM** - dynamic operation details |
| `mcp_call_log.params` | No | Yes (MCP tool params) | **HIGH** - arbitrary MCP params |
| `mcp_call_log.result` | No (array\|object) | Yes (MCP tool results) | **HIGH** - arbitrary MCP results |
| `agent.llm` | Yes (provider, model, temperature, max_tokens) | Constrained to 4 sub-fields | LOW - explicit sub-fields |

**Findings**:
- **F2**: `mcp_call_log.params` and `mcp_call_log.result` store dynamic MCP data in `TYPE object`/`TYPE array|object`. Dynamic keys in params will be silently dropped. Should be `TYPE string` with JSON serialization.
- **F3**: `validation_request.details` stores dynamic operation details as `TYPE object`. Dynamic keys will be dropped. Should be `TYPE string`.
- `agent_state` table has TYPE object fields but the table is never written to (dead table), so no runtime risk.

### 2.4 Indexes

All indexes use `DEFINE INDEX OVERWRITE`. Index coverage is comprehensive:

| Table | Index Count | Notes |
|-------|-------------|-------|
| workflow | 0 | No indexes (small table, no frequent filters) |
| message | 2 | workflow_id + timestamp (documented as write-heavy trade-off) |
| memory | 4 | HNSW vector + workflow + type/workflow composite + type/created composite |
| task | 4 | workflow + status + priority + agent |
| mcp_server | 2 | UNIQUE id + UNIQUE name |
| mcp_call_log | 2 | workflow_id + server_name |
| llm_model | 3 | UNIQUE id + provider + UNIQUE provider/api_name |
| provider_settings | 1 | UNIQUE provider |
| custom_provider | 1 | UNIQUE name |
| agent | 3 | UNIQUE id + name + llm.provider |
| tool_execution | 4 | workflow + message + agent + type |
| thinking_step | 3 | workflow + message + agent |
| sub_agent_execution | 3 | workflow + parent + status |
| user_question | 3 | workflow + status + workflow/status composite |
| validation_request | 0 | No indexes (queried by status but low volume) |

**Note**: `validation_request` has no indexes. If volume grows, `status` and `workflow_id` indexes would help.

---

## 3. ERR_SURREAL Compliance

### ERR_SURREAL_001: Dynamic keys in SCHEMAFULL

**Status: PARTIALLY COMPLIANT**

| Implementation | Status |
|----------------|--------|
| `mcp_server.env` | FIXED - `TYPE string DEFAULT '{}'` |
| `tool_execution.input_params` | FIXED - `TYPE string` |
| `tool_execution.output_result` | FIXED - `TYPE option<string>` |
| `user_question.options` | FIXED - `TYPE string DEFAULT '[]'` |
| `user_question.selected_options` | FIXED - `TYPE string DEFAULT '[]'` |
| `mcp_call_log.params` | **VIOLATION** - `TYPE object` with dynamic keys |
| `mcp_call_log.result` | **VIOLATION** - `TYPE array\|object` with dynamic data |
| `validation_request.details` | **VIOLATION** - `TYPE object` with dynamic keys |
| `agent.llm` | OK - `TYPE object` with explicit sub-field DEFINEs |
| `memory.metadata` | OK - `TYPE object` with explicit sub-field DEFINEs |

### ERR_SURREAL_002: SDK .create().content()

**Status: MOSTLY COMPLIANT**

- `.create().content()`: **0 violations** - All create operations use `db.create()` method which internally uses raw `CREATE ... CONTENT $data` with bind.
- `.update().content()`: **1 violation** at `client.rs:232` - The `update()` method uses `self.db.update(id).content(data)` SDK pattern. However, this method is `#[allow(dead_code)]` and not currently called anywhere.

### ERR_SURREAL_003: meta::id(id) for clean UUIDs

**Status: COMPLIANT**

Extensively used across the codebase (80+ occurrences). All SELECT queries use `meta::id(id) AS id`.

**Exceptions**: 2 queries in `commands/validation.rs` use `SELECT *`:
- Line 128: `SELECT * FROM validation_request WHERE status = 'pending'`
- Line 163: `SELECT * FROM validation_request WHERE workflow_id = '{}'`

These return `ValidationRequest` struct which handles deserialization, but the `id` field will contain the Thing type format (`validation_request:uuid`) instead of clean UUID.

### ERR_SURREAL_004: String escaping

**Status: MOSTLY COMPLIANT (SA-001 overlap)**

The SA-001 audit already documented 8 HIGH findings for `replace('\'', "''")` anti-pattern. The DB layer itself does not have this issue - `import_export.rs` correctly uses `serde_json::to_string()` for name escaping.

### ERR_SURREAL_005: ORDER BY field in SELECT

**Status: COMPLIANT**

All ORDER BY queries include the sort field in the SELECT list. The centralized `queries::workflow` constants ensure consistency.

### ERR_SURREAL_006: Null character sanitization

**Status: PARTIALLY COMPLIANT**

`sanitize_for_surrealdb()` is only called in 2 locations:

| Call Site | Data Source | Risk |
|-----------|-------------|------|
| `mcp/manager.rs:953` | MCP tool call results | External (MCP servers) |
| `tools/user_question/tool.rs:214` | User question data | Agent-generated |

**Missing sanitization** at these external data entry points:

| Location | Data Source | Risk |
|----------|-------------|------|
| `commands/import_export.rs` (execute_import) | Imported JSON file | **HIGH** - fully user-controlled |
| `commands/embedding.rs` (import_memories) | Imported JSON file | **HIGH** - fully user-controlled |
| `commands/prompt.rs` (create_prompt) | User input from frontend | MEDIUM - UI-mediated |
| `commands/agent.rs` (create_agent, update_agent) | User config from frontend | LOW - structured data |
| `commands/models.rs` (create_model) | User input from frontend | LOW - mostly numeric/enum |

The `db.create()` method in `client.rs` does NOT sanitize automatically - it only does `serde_json::to_value()` + bind.

### ERR_SURREAL_007: datetime fields via CONTENT

**Status: COMPLIANT**

The `memory` table's `expires_at` field correctly uses:
1. `#[serde(skip_serializing)]` on `MemoryCreate.expires_at` and `MemoryCreateWithEmbedding.expires_at`
2. Separate `set_expires_at_if_present()` helper with `<datetime>` cast in UPDATE

Other `option<datetime>` fields (`workflow.completed_at`, `task.completed_at`, `sub_agent_execution.completed_at`, `user_question.answered_at`) are set via UPDATE queries with `time::now()` or direct value assignment, not via JSON CONTENT.

### ERR_SURREAL_008: vector::distance::cosine

**Status: COMPLIANT**

All vector search uses `vector::similarity::cosine()` (3 occurrences in `tools/memory/helpers.rs:345-356`).

### ERR_SURREAL_009: GROUP ALL for aggregates

**Status: COMPLIANT**

All aggregate queries (`count()`, `math::min()`, `math::max()`) include `GROUP ALL` (18 occurrences verified).

### ERR_SURREAL_010: query_json_with_params on CREATE

**Status: COMPLIANT**

Write operations use `execute_with_params()` (no result deserialization), not `query_json_with_params()`.

---

## 4. Query Pattern Analysis

### 4.1 Parameterized vs Interpolated

| Pattern | Count | Examples |
|---------|-------|---------|
| `$param` bind parameters | ~25 | `tools/utils.rs`, `tools/memory/helpers.rs`, `tools/todo/tool.rs` |
| `format!()` with validated UUID | ~40 | Most commands use `Validator::validate_uuid()` then format |
| `format!()` with `serde_json::to_string()` | ~8 | `import_export.rs` name comparisons |
| `format!()` with unvalidated input | ~5 | **SA-001 findings** (prompt search, import) |

### 4.2 Cascade Delete Injection Risk

```rust
// queries.rs:115
let query = format!("DELETE {} WHERE workflow_id = '{}'", table, workflow_id);
```

`table` is from `CASCADE_DELETE_TABLES` constant (code-controlled). `workflow_id` comes from a parameter but is passed from validated contexts. **Risk: LOW** - but should use bind parameter for defense-in-depth.

### 4.3 Error Handling Consistency

All DB operations use `.map_err(|e| ...)` with `tracing::error!()`. Pattern is consistent:
- `client.rs` methods: `anyhow::Result` with error logging
- Command layer: `.map_err(|e| format!("..."))` to convert to `String`
- Tool layer: `.map_err(|e| ToolError::DatabaseError(...))`

---

## 5. Dead Code / Technical Debt

### 5.1 Dead Table: agent_state

The `agent_state` table is defined in schema (lines 41-47) with `TYPE object` fields but:
- No CREATE/INSERT/UPSERT/UPDATE queries target it
- Referenced only in `workflow_agent` relation and schema definition
- Superseded by the `agent` table

**Recommendation**: Remove from schema or document as intentionally preserved for future use.

### 5.2 Dead Method: DBClient::update()

`client.rs:226-239` - Uses `.update().content()` SDK pattern (ERR_SURREAL_002 risk). Marked `#[allow(dead_code)]`. Never called.

**Recommendation**: Remove or rewrite to use raw query with bind.

### 5.3 Unused QueryBuilder

`tools/utils::QueryBuilder` (non-parameterized, lines 131-199) is `#[allow(dead_code)]`. The safer `ParamQueryBuilder` exists alongside it.

**Recommendation**: Remove `QueryBuilder`, keep only `ParamQueryBuilder`.

---

## 6. Findings Summary

### HIGH Priority

| ID | Finding | File:Line | ERR Reference |
|----|---------|-----------|---------------|
| F1 | `DBClient::update()` uses `.update().content()` SDK pattern | `db/client.rs:232` | ERR_SURREAL_002 |
| F2 | `mcp_call_log.params` TYPE object drops dynamic MCP params | `db/schema.rs:163` | ERR_SURREAL_001 |
| F3 | `mcp_call_log.result` TYPE array\|object drops dynamic MCP results | `db/schema.rs:164` | ERR_SURREAL_001 |
| F4 | `sanitize_for_surrealdb()` missing at import entry points | `commands/import_export.rs`, `commands/embedding.rs` | ERR_SURREAL_006 |

### MEDIUM Priority

| ID | Finding | File:Line | ERR Reference |
|----|---------|-----------|---------------|
| F5 | `validation_request.details` TYPE object drops dynamic keys | `db/schema.rs:107` | ERR_SURREAL_001 |
| F6 | `SELECT *` in validation queries returns Thing-format IDs | `commands/validation.rs:128,163` | ERR_SURREAL_003 |
| F7 | Cascade delete uses string interpolation for workflow_id | `db/queries.rs:115` | Defense-in-depth |
| F8 | MCP migration uses DEFINE FIELD without OVERWRITE | `db/client.rs:80` | PAT_DB_003 |

### LOW Priority

| ID | Finding | File:Line | Notes |
|----|---------|-----------|-------|
| F9 | `agent_state` table is dead (never written) | `db/schema.rs:41-47` | Technical debt |
| F10 | `prompt` and `settings` tables are SCHEMALESS | Not in schema.rs | No DB-level validation |
| F11 | `DBClient::update()` dead code with SDK anti-pattern | `db/client.rs:226-239` | Remove or rewrite |
| F12 | Non-parameterized `QueryBuilder` unused alongside safe `ParamQueryBuilder` | `tools/utils.rs:131-199` | Remove dead code |

---

## 7. Cross-Reference with SA-001

SA-001 (SurrealQL Injection) documented 5 CRITICAL + 8 HIGH findings in the command layer. This audit (SA-012) focuses on the DB infrastructure layer and confirms:

- The DB client layer itself is well-designed with parameterized query support
- The gaps are in the **calling code** (commands), not the DB layer
- `sanitize_for_surrealdb()` exists but is under-deployed
- The `ParamQueryBuilder` exists but is underutilized

---

## 8. Recommendations

1. **F2/F3**: Change `mcp_call_log.params` to `TYPE string` and `mcp_call_log.result` to `TYPE string`. Serialize with `serde_json::to_string()`, deserialize on read. This matches the pattern already used for `tool_execution.input_params`/`output_result`.

2. **F4**: Add `sanitize_for_surrealdb()` call inside `DBClient::create()` method (centralized). This would protect ALL create operations automatically without requiring each caller to remember.

3. **F5**: Change `validation_request.details` to `TYPE string`. Serialize with `serde_json::to_string()`.

4. **F6**: Replace `SELECT *` with explicit field selection using `meta::id(id) AS id` in validation queries.

5. **F7**: Refactor cascade delete to use bind parameters: `DELETE $table WHERE workflow_id = $wf_id`.

6. **F8**: Change MCP migration to use `DEFINE FIELD OVERWRITE`.

7. **F9/F11/F12**: Remove dead code (agent_state table, update() method, QueryBuilder).

8. **F10**: Consider adding `prompt` and `settings` table definitions to schema.rs for validation consistency.
