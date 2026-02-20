# SA-001: SurrealQL Injection Audit

**Date**: 2026-02-19
**Scope**: All `.rs` files under `src-tauri/src/`
**Status**: Documented (fixes pending)

## Summary

| Severity | Count | Pattern |
|----------|-------|---------|
| CRITICAL | 5 | Direct user/external input interpolation in WHERE/CONTENT |
| HIGH | 8 | `replace('\'', "''")` anti-pattern (ERR_SURREAL_004) |
| MEDIUM | 4 | Internal data in sensitive query positions |
| LOW | 18 | Validated UUIDs/constants in backtick record IDs |
| `.create().content()` | 0 | ERR_SURREAL_002 fully resolved |

## References

- **ERR_SURREAL_004** (`.claude/learning/errors.yml`): `replace('\'', "''")` is a broken escaping pattern
- **PAT_DB_002** (`.claude/learning/patterns.yml`): Correct pattern is `bind()` parameterized queries
- **SurrealDB rule** (`.claude/rules/surrealdb.md`): "Always use parameters for user input"

---

## CRITICAL Findings

### C1 - search_prompts: search term interpolation

- **File**: `commands/prompt.rs:304-307`
- **Code**:
  ```rust
  conditions.push(format!(
      "(string::lowercase(name) CONTAINS '{}' OR string::lowercase(description) CONTAINS '{}')",
      search_term, search_term
  ));
  ```
- **Source**: User input (`query: Option<String>` from frontend)
- **Exploitability**: HIGH - user types search query in UI, directly interpolated in WHERE
- **Fix**: Use `$search` bind parameter
  ```rust
  conditions.push("(string::lowercase(name) CONTAINS $search OR string::lowercase(description) CONTAINS $search)".to_string());
  params.push(("search".to_string(), serde_json::json!(search_term)));
  ```

### C2 - search_prompts: category interpolation

- **File**: `commands/prompt.rs:313`
- **Code**:
  ```rust
  conditions.push(format!("category = '{}'", cat.trim()));
  ```
- **Source**: User input (`category: Option<String>` from frontend)
- **Exploitability**: HIGH - user-selected category value
- **Fix**: Use `$category` bind parameter

### C3 - import_memories: content interpolation from file

- **File**: `commands/embedding.rs:446-453`
- **Code**:
  ```rust
  let sanitized_content = content.replace('\0', "").replace('\'', "''");
  let create_query = format!(
      "CREATE memory:`{}` CONTENT {{ type: '{}', content: '{}', metadata: {} }}",
      memory_id, memory_type, sanitized_content,
      serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".to_string())
  );
  ```
- **Source**: External file (JSON memory import - fully user-controlled)
- **Exploitability**: HIGH - arbitrary content from imported JSON file
- **Fix**: Use `CONTENT $data` bind with `serde_json::to_value` + `sanitize_for_surrealdb()`
- **Note**: `sanitize_for_surrealdb()` is missing here despite being used at other external data entry points

### C4 - regenerate_embeddings: type_filter interpolation

- **File**: `commands/embedding.rs:500-503`
- **Code**:
  ```rust
  Some(ref mtype) => format!(
      "SELECT meta::id(id) AS id, content FROM memory WHERE type = '{}'",
      mtype
  ),
  ```
- **Source**: User input (`type_filter: Option<String>` from frontend)
- **Exploitability**: HIGH - user-controlled parameter directly in WHERE
- **Fix**: Use `$type` bind parameter

### C5 - persist_workflow_metrics: model lookup interpolation

- **File**: `commands/streaming.rs:472-479`
- **Code**:
  ```rust
  let model_query = format!(
      "SELECT ... FROM llm_model WHERE api_name = '{}' AND provider = '{}'",
      model, provider_lower
  );
  ```
- **Source**: Internal (agent config from DB, originating from user model selection)
- **Exploitability**: MEDIUM - requires crafted model name stored in agent config
- **Fix**: Use `$api_name` and `$provider` bind parameters

---

## HIGH Findings - replace('\'', "''") Anti-Pattern

All instances use the broken escaping pattern documented in ERR_SURREAL_004.

### H1-H2 - Builtin model seeding (main.rs + models.rs)

| # | File:Line | Variable | Source |
|---|-----------|----------|--------|
| H1 | `main.rs:152-153` | `model.name`, `model.api_name` | Internal (builtin constants) |
| H2 | `commands/models.rs:363-364` | `model.name`, `model.api_name` | Internal (builtin constants) |
| H6 | `commands/models.rs:930-931` | `model.name`, `model.api_name` | Internal (builtin sync) |

- **Exploitability**: LOW (hardcoded values) but sets bad precedent
- **Fix**: Rewrite to use `CONTENT $data` bind pattern for entire model object

### H3-H5 - update_model / update_provider_settings

| # | File:Line | Variable | Source |
|---|-----------|----------|--------|
| H3 | `commands/models.rs:444` | `name` | User input (update_model) |
| H4 | `commands/models.rs:471` | `api_name` | User input (update_model) |
| H5 | `commands/models.rs:700` | `base_url` | User input (provider settings) |

- **Exploitability**: HIGH - direct user input with broken escaping
- **Fix**: Build SET clause with bind parameters
  ```rust
  set_parts.push("name = $name".to_string());
  params.push(("name".to_string(), serde_json::json!(name)));
  ```

### H7-H9 - update_task string fields

| # | File:Line | Variable | Source |
|---|-----------|----------|--------|
| H7 | `commands/task.rs:364` | `name` | User input |
| H8 | `commands/task.rs:371` | `description` | User input |
| H9 | `commands/task.rs:375` | `agent_assigned` | User input |

- **Exploitability**: HIGH - direct user input with broken escaping
- **Fix**: Same bind parameter pattern as H3-H5

---

## MEDIUM Findings

| # | File:Line | Pattern | Source | Fix |
|---|-----------|---------|--------|-----|
| M1 | `commands/models.rs:448-450` | `WHERE provider = '{}' AND api_name = '{}' AND id != '{}'` | DB record + user input | Bind all 3 values |
| M2 | `commands/models.rs:492` | `UPDATE llm_model:\`{}\` SET {set_parts}` | Validated ID + set_parts (some with replace) | Resolved by fixing H3-H6 |
| M3 | `commands/streaming.rs:535-549` | `UPDATE workflow:\`{}\` SET ... model_id = '{}'` | Validated UUID + model_id from DB | Bind `$model_id` |
| M4 | `db/queries.rs:115` | `DELETE {} WHERE workflow_id = '{}'` | Internal (from cascade with validated UUID) | Bind `$wf_id` |

---

## LOW Findings - Validated UUIDs/Constants

All use `Validator::validate_uuid()` (uuid::Uuid::parse_str - charset `[0-9a-fA-F-]`) or `validate_provider_name()` (charset `[a-z0-9-]`). Not exploitable.

| # | File:Line | Context |
|---|-----------|---------|
| L1 | `commands/message.rs:316` | `DELETE message:\`{validated_id}\`` |
| L2 | `commands/thinking.rs:266` | `DELETE thinking_step:\`{validated_id}\`` |
| L3 | `commands/tool_execution.rs:378` | `DELETE tool_execution:\`{validated_id}\`` |
| L4 | `commands/agent.rs:529` | `DELETE agent:\`{validated_id}\`` |
| L5 | `commands/memory.rs:245` | `DELETE memory:\`{validated_id}\`` |
| L6 | `commands/memory.rs:212-216` | `SELECT ... WHERE meta::id(id) = '{validated_id}'` |
| L7 | `commands/prompt.rs:278` | `DELETE prompt:\`{prompt_id}\`` (internal UUID) |
| L8 | `commands/task.rs:460` | `UPDATE task:\`{validated_id}\` SET status = $status` |
| L9 | `commands/embedding.rs:54,574` | `SELECT config FROM settings:\`{CONSTANT}\`` |
| L10 | `commands/embedding.rs:309` | `UPDATE memory:\`{memory_id}\`` (validated) |
| L11 | `commands/embedding.rs:541` | `UPDATE memory:\`{id}\` SET embedding = {json}` (DB iteration) |
| L12 | `commands/custom_provider.rs:152` | `CREATE custom_provider:\`{name}\`` (validated `[a-z0-9-]`) |
| L13 | `commands/custom_provider.rs:320` | `DELETE custom_provider:\`{name}\`` (validated) |
| L14 | `commands/models.rs:538` | `DELETE llm_model:\`{id}\`` (validated) |
| L15 | `commands/workflow.rs:357` | `WHERE meta::id(id) = '{validated_id}'` |
| L16 | `commands/streaming.rs:107` | `WHERE meta::id(id) = '{validated_id}'` |
| L17 | `db/client.rs:257` | `DELETE {table}:\`{uuid}\`` (internal) |
| L18 | `tools/utils.rs:61` | `DELETE {table}:\`{id}\`` (internal) |

---

## sanitize_for_surrealdb() Coverage Gap

| Caller | File | External Data | Has Sanitize |
|--------|------|---------------|--------------|
| user_question tool | `tools/user_question/tool.rs:214` | Yes (agent) | YES |
| MCP call log | `mcp/manager.rs:953` | Yes (MCP) | YES |
| Memory import | `commands/embedding.rs:446` | Yes (file) | **NO** |

---

## Priority Fix Order

1. **P0** (CRITICAL user input): C1, C2, C4 - bind params in prompt.rs and embedding.rs
2. **P0** (CRITICAL external data): C3 - rewrite import_memories with `$data` bind + sanitize
3. **P1** (HIGH user input): H3, H4, H5, H7, H8, H9 - bind params in models.rs and task.rs
4. **P1** (HIGH + MEDIUM): C5, M1, M3 - bind params in streaming.rs and models.rs
5. **P2** (defense-in-depth): M4 - bind params in queries.rs cascade
6. **P3** (code hygiene): H1, H2, H6 - refactor builtin model seeding to use bind
7. **P4** (optional): L* - convert to bind during normal maintenance

## Estimation

- P0 fixes: ~2h (4 files, straightforward bind parameter conversion)
- P1 fixes: ~2h (3 files, SET clause refactoring to use params)
- P2-P3: ~1h (cleanup during P0/P1 work)
- Total: ~5h for complete remediation

---

## Code Verification (2026-02-19)

**Methodology**: 4 exploration agents read the actual code. thinking-mcp bias checks (sycophancy, anchoring, adversarial reframe, confidence assessment) applied to prevent inflated or deflated ratings.

### Severity Adjustments

| Finding | Original | Adjusted | Justification |
|---------|----------|----------|---------------|
| C1 | CRITICAL | **CONFIRMED CRITICAL** | `format!()` with `search_term` in WHERE CONTAINS. Verified at `prompt.rs:304-313`. Direct user input from search box. |
| C2 | CRITICAL | **CONFIRMED CRITICAL** | `format!()` with `category` in WHERE =. Verified at `prompt.rs:313`. Direct user input. |
| C3 | CRITICAL | **CONFIRMED CRITICAL** | Content interpolated in CREATE CONTENT with broken `replace` escaping. Verified at `embedding.rs:446-453`. External file data. |
| C4 | CRITICAL | **ADJUSTED HIGH** | `type_filter` interpolated in WHERE =. Verified at `embedding.rs:500-503`. Input comes from frontend dropdown, not free-form user text. Still exploitable but requires more deliberate action than C1-C3. |
| C5 | CRITICAL | **ADJUSTED MEDIUM** | Model name lookup in WHERE. Verified at `streaming.rs:472-479`. Data originates from agent config stored in DB, not direct user input. Requires a pre-existing compromised agent config to exploit. |
| H1-H9 | HIGH | **CONFIRMED HIGH** | `replace('\'', "''")` anti-pattern verified across models.rs and task.rs. Known broken escaping pattern (ERR_SURREAL_004). |
| M1-M4 | MEDIUM | **CONFIRMED MEDIUM** | Defense-in-depth concerns verified. |
| L1-L18 | LOW | **CONFIRMED LOW** | All use `Validator::validate_uuid()` which restricts charset to `[0-9a-fA-F-]`. Not exploitable. |

### Summary After Verification

| Severity | Original Count | Adjusted Count | Change |
|----------|---------------|----------------|--------|
| CRITICAL | 5 | 3 | -2 (C4->HIGH, C5->MEDIUM) |
| HIGH | 8 | 9 | +1 (C4 moved here) |
| MEDIUM | 4 | 5 | +1 (C5 moved here) |
| LOW | 18 | 18 | No change |
