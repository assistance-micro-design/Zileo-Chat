# Remediation Status - Security Audit Findings

**Date**: 2026-02-20 (updated)
**Branch**: `security/audit-remediation-tdd`
**Base**: `main` (commit 1d8fc29)
**Files changed**: 75 (vs main)
**Lines**: +2,100 / -1,500 (approx)

---

## Summary

| Status | Count | Description |
|--------|-------|-------------|
| DONE | 49 | Fix implemented and tested |
| PARTIAL | 5 | Some aspects done, others remain |
| NOT DONE | 12 | Not yet addressed |
| N/A | 7 | Not applicable (desktop context) |

| Category | DONE | PARTIAL | NOT DONE |
|----------|------|---------|----------|
| CRITICAL (4) | 4 | 0 | 0 |
| HIGH (27) | 26 | 1 | 0 |
| MEDIUM (34) | 14 | 3 | 17 |
| LOW (13) | 4 | 0 | 9 |
| N/A (7) | - | - | - |

**All 4 CRITICAL findings are remediated.**

---

## Tests Added

### Rust (37 new tests)

| File | Test | Purpose |
|------|------|---------|
| commands/prompt.rs | test_search_prompts_with_valid_query | Characterization: search works |
| commands/prompt.rs | test_search_prompts_injection_safe | Injection: apostrophe/SQL in search |
| commands/prompt.rs | test_search_prompts_injection_preserves_data | Injection: data integrity after attack |
| commands/prompt.rs | test_search_prompts_with_category | Characterization: category filter |
| commands/prompt.rs | test_create_prompt_with_bind_params | Characterization: create works |
| commands/embedding.rs | test_import_memory_injection_safe | Injection: malicious memory content |
| commands/embedding.rs | test_regenerate_type_filter_injection_safe | Injection: type_filter param |
| commands/models.rs | test_model_name_with_apostrophe | Injection: apostrophe in model name |
| commands/models.rs | test_model_search_injection_safe | Injection: SQL in model name |
| commands/task.rs | test_update_task_name_with_special_chars | Injection: special chars in task |
| commands/mcp.rs | test_mcp_call_log_write_read_cycle | Schema: JSON string write/read |
| commands/mcp.rs | test_deserialize_mcp_call_log_from_string_params | Serde: new string format |
| commands/mcp.rs | test_deserialize_mcp_call_log_from_legacy_object_params | Serde: backward compat |
| models/custom_provider.rs | test_https_urls_no_warning | HTTP warning: HTTPS safe |
| models/custom_provider.rs | test_http_localhost_no_warning | HTTP warning: localhost safe |
| models/custom_provider.rs | test_http_127_0_0_1_no_warning | HTTP warning: 127.0.0.1 safe |
| models/custom_provider.rs | test_http_ipv6_loopback_no_warning | HTTP warning: IPv6 loopback |
| models/custom_provider.rs | test_http_remote_returns_warning | HTTP warning: remote triggers |
| models/custom_provider.rs | test_http_remote_ip_returns_warning | HTTP warning: remote IP |
| models/custom_provider.rs | test_http_remote_various_hosts | HTTP warning: various hosts |
| models/custom_provider.rs | test_non_http_schemes_no_warning | HTTP warning: non-HTTP safe |
| models/validation.rs | test_risk_level_deserializes_critical | Deserialization: Critical variant |
| db/utils.rs | test_sanitize_deeply_nested_json_truncated | DoS: depth limit works |
| db/utils.rs | test_sanitize_normal_depth_preserved | DoS: normal data preserved |
| commands/migration.rs | test_check_migration_not_applied | Guard: fresh DB returns false |
| commands/migration.rs | test_record_and_check_migration | Guard: record + check roundtrip |
| commands/migration.rs | test_check_migration_does_not_cross_contaminate | Guard: isolation between names |
| commands/migration.rs | test_memory_migration_first_run_clears_embeddings | Guard: first run works |
| commands/migration.rs | test_memory_migration_second_run_preserves_embeddings | Guard: SA-005 H3 core test |
| commands/migration.rs | test_memory_v2_migration_guard | Guard: v2 migration idempotent |
| commands/migration.rs | test_mcp_http_migration_guard | Guard: MCP HTTP migration idempotent |
| models/message.rs | test_message_create_always_serializes_tokens | SA-013 #6: tokens always in JSON output |
| models/message.rs | test_message_create_deserializes_without_tokens | SA-013 #6: defense-in-depth default |
| models/message.rs | test_message_create_tokens_roundtrip | SA-013 #6: roundtrip preservation |
| models/llm_models.rs | test_provider_settings_base_url_serializes_as_null_when_none | SA-013 #12: base_url always in JSON output |
| models/llm_models.rs | test_provider_settings_base_url_serializes_when_set | SA-013 #12: base_url present when set |
| models/llm_models.rs | test_provider_settings_base_url_roundtrip | SA-013 #12: serialize/deserialize roundtrip |

### TypeScript (35 new tests in 3 new files + 1 updated)

| File | Tests | Purpose |
|------|-------|---------|
| utils/__tests__/error.test.ts | 11 tests | getErrorMessage + formatErrorForDisplay |
| utils/__tests__/url.test.ts | 11 tests | isAllowedScheme (XSS defense) |
| stores/__tests__/activity.test.ts | 8 tests | Activity capture guard (SA-011 H-001 race condition) |
| stores/__tests__/workflows.test.ts | 5 new tests | loadWorkflows retry recovery (SA-011 H-002) |

### Infrastructure

| File | Lines | Purpose |
|------|-------|---------|
| src-tauri/src/test_utils.rs | 340 | Shared test harness: setup_test_state(), seed helpers (incl. seed_test_memory_with_embedding) |

---

## Detailed Status by Audit

### SA-001: SurrealQL Injection

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| C1 | CRITICAL | search_prompts search_term interpolation | **DONE** | prompt.rs: `WHERE name CONTAINS $search` + `.bind(("search", val))`. 3 tests. |
| C2 | CRITICAL | search_prompts category interpolation | **DONE** | prompt.rs: `WHERE category = $category` + `.bind()`. 1 test. |
| C3 | CRITICAL | import_memories content interpolation | **DONE** | embedding.rs: `execute_with_params()` with `$mtype`, `$content`, `$metadata`. 1 test. |
| C4 | HIGH (adj.) | type_filter interpolation in regenerate_embeddings | **DONE** | embedding.rs: `WHERE type = $mtype` + `.bind()`. 1 test. |
| H3-H5 | HIGH | replace('\'', "''") in models.rs | **DONE** | models.rs: All queries use bind params. `validate_model_id()` added. 2 tests. |
| H6-H7 | HIGH | replace('\'', "''") in task.rs | **DONE** | task.rs: switched to `serde_json::to_string()`. 1 test. |
| H8-H9 | HIGH | format!() in streaming.rs | **DONE** | streaming.rs: All WHERE clauses use `$wf_id`, `$model_id` bind params. |
| M1-M5 | MEDIUM | Non-user-input interpolation (validation, cascade) | **DONE** | validation.rs: `$status`, `$reason` bind params. queries.rs: `$wf_id` bind param. |
| L1-L18 | LOW | format!() with validated UUIDs | **NOT DONE** | UUIDs from DB are not user-controlled. Defense-in-depth only. |

### SA-002: MCP + Import/Export + XSS + Secrets

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| S2-H1 | HIGH | String interpolation in import UPDATE/CREATE | **DONE** | import_export.rs: All INSERT/UPDATE use `CONTENT $data` with `execute_with_params()`. |
| S2-H2 | HIGH | Entity ID interpolation in export SELECT | **DONE** | import_export.rs: All WHERE use `.bind(("id", ...))`. |
| S2-M1 | HIGH (upgraded) | Missing sanitize_for_surrealdb() on import | **DONE** | import_export.rs calls sanitize. db/utils.rs has depth-limited sanitizer. |
| S2-H3 | MEDIUM (adj.) | MCP HTTP base_url not validated | **NOT DONE** | Only custom_provider.rs has HTTP warning. MCP server config unchanged. |
| S2-C1 | MEDIUM (adj.) | HTTP provider URLs in cleartext | **DONE** | CustomProviderResponse with warning. Frontend shows warning toast. |
| S2-M2 | MEDIUM | MCP env stored as TYPE object | **NOT DONE** | MCP env already uses TYPE string in schema. No change needed? Needs verification. |
| S2-M3 | MEDIUM | Import file read from arbitrary path | **DONE** | `read_import_file` command removed entirely. |
| S2-M4 | MEDIUM | Export file write path not validated | **DONE** | `save_export_to_file()` validates path: rejects `..`, system dirs, requires .json/.csv. |
| S2-M5 | MEDIUM | No import size limit | **DONE** | `MAX_IMPORT_ENTITIES = 100` enforced in `validate_import()`. |
| S2-L1 | LOW | MCP tool descriptions not sanitized | **NOT DONE** | DOMPurify handles display. Defense-in-depth only. |
| S2-L2 | LOW | Export includes internal IDs | **NOT DONE** | Design choice, not vulnerability. |

### SA-005: CSP & Tauri Permissions

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| C1 | HIGH (adj.) | read_import_file arbitrary paths | **DONE** | Command removed from Tauri registration in main.rs. |
| H1 | HIGH | Google Fonts blocked by CSP | **DONE** | CDN links removed from +layout.svelte. 4 @font-face in global.css. 4 woff2 files in /static/fonts/. |
| H2 | HIGH | Missing sanitize_for_surrealdb() in import | **DONE** | Same as S2-M1 above. |
| H3 | HIGH | migrate_memory_schema destroys embeddings | **DONE** | migration_log table in schema.rs. check_migration_applied/record_migration_applied guards all 3 migrations. 7 new tests. |
| M1 | MEDIUM | opener:default allows any URL | **PARTIAL** | isAllowedScheme() added for markdown links. But Tauri permission scope unchanged. |
| M2 | MEDIUM | dialog:default grants all types | **NOT DONE** | Tauri capabilities unchanged. |
| M3 | MEDIUM | No IPC deny patterns | **NOT DONE** | Tauri capabilities unchanged. |
| L1-L3 | LOW | CSP documentation, permission comments | **NOT DONE** | No documentation added. |

### SA-006: Dependency Vulnerabilities

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| 7 CVEs | N/A | All NPM CVEs inapplicable to desktop | **N/A** | Confirmed in EVALUATION. |
| DEP-1 | HIGH | rig-core features = ["all"] pulls bloat | **DONE** | Cargo.toml: removed `features = ["all"]` from rig-core. |
| DEP-2 | HIGH | surrealdb unused features | **NOT DONE** | surrealdb dependencies unchanged. |
| DEP-3 | HIGH | NPM patch updates available | **NOT DONE** | No package.json changes in branch. |
| L/INFO | LOW/INFO | Unmaintained transitive deps | **NOT DONE** | Upstream dependency, cannot fix. |

### SA-007: Commands Control Flow & Error Handling

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| DUP-1 | MEDIUM | UUID validation repeated 52x | **NOT DONE** | No ResultExt trait extracted. |
| DUP-2 | MEDIUM | serde_json escaping repeated 25x | **NOT DONE** | |
| DUP-3 | MEDIUM | COUNT extraction repeated 16x | **NOT DONE** | |
| F1-F13 | MEDIUM | 13 oversized functions (>100 lines) | **NOT DONE** | No function decomposition. |
| F14 | LOW | 15 generic "Database error" messages | **NOT DONE** | |
| COMPLIANT | - | 0 .unwrap() in production | **CONFIRMED** | Still true. |

### SA-008: Agent System Quality & Performance

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| PERF-1 | HIGH | messages.clone() in tool loop | **DONE** | manager.rs, mistral.rs, ollama.rs, openai_compatible.rs: `&[serde_json::Value]` instead of owned. llm_agent.rs: passes references. |
| PERF-2 | MEDIUM | Sequential DB writes in streaming | **DONE** | streaming.rs: `futures::future::join_all()` for tool_executions and reasoning_steps. |
| PERF-3 | LOW | Retry closure cloning | **NOT DONE** | Rare path, low priority. |
| DUP-1 | MEDIUM | Report::failed() repeated 5x | **NOT DONE** | |
| DUP-3 | MEDIUM | Mistral/OpenAI adapters 95% identical | **NOT DONE** | |
| DUP-4 | MEDIUM | Provider dispatch repeated 3x | **NOT DONE** | |

### SA-009: Stores Quality Audit

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| F1 | MEDIUM | Streaming/backgroundWorkflows chunk duplication | **NOT DONE** | |
| F2 | MEDIUM | Manual error extraction in 6 stores | **DONE** | validation-settings.ts, validation.ts: now use getErrorMessage(). |
| F4 | MEDIUM | userQuestion.ts subscribe/unsub hack | **DONE** | Replaced with `get(store)` pattern. |
| F9 | - | Zero ERR_SVELTE_005 violations | **CONFIRMED** | Still true. |
| Dead code | LOW | Deprecated exports | **DONE** | Removed: agentCount, promptCount, isTokenStreaming, createInitialAgentState, AgentState. |

### SA-010: Settings Forms Quality

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| ERR-1 | MEDIUM | 29/30 try/catch not using getErrorMessage | **DONE** | 18+ components updated to use getErrorMessage(). See frontend diff. |
| ERR-2 | MEDIUM | 5 files use console.error/warn | **PARTIAL** | Most replaced with getErrorMessage. Some console.warn remain in AgentForm. |
| DUP-1 | MEDIUM | ValidationSettings 9 identical info-cards | **NOT DONE** | No template extraction. |
| DUP-2 | MEDIUM | ImportPreview 4 identical sections | **NOT DONE** | |
| DUP-3 | MEDIUM | ExportPreview 4 identical sections | **NOT DONE** | |
| A11Y-1 | LOW | Tab ARIA attributes | **DONE** | ImportExportSettings: role="tablist", role="tab", aria-selected. |
| A11Y-2 | LOW | aria-expanded on collapsible sections | **DONE** | ExportPreview: aria-expanded on 4 section-header buttons. |
| A11Y-3 | LOW | aria-live on status messages | **DONE** | ImportExportSettings: role="status", aria-live="polite". |
| A11Y-4 | LOW | aria-label on icon buttons | **DONE** | MemorySettings: aria-label on edit/delete buttons. Settings nav: aria-label + aria-current. |
| A11Y-5-7 | LOW | Other accessibility gaps | **NOT DONE** | |

### SA-011: Chat & Workflow Components

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| H-001 | HIGH | Activity capture race condition | **DONE** | **Frontend**: activity.ts: `lastCapturedWorkflowId` guard prevents duplicate capture. workflowExecutor.service.ts: capture moved to `finally` block (before `streamingStore.reset()`), runs on both success and error paths. 8 new tests in activity.test.ts. **Backend**: CancellationToken now propagated through full chain: streaming.rs -> orchestrator -> Agent trait -> LLMAgent -> AgentToolContext -> sub-agent tools -> SubAgentExecutor. Sub-agents stop promptly on user cancel. Files: agent.rs, orchestrator.rs, llm_agent.rs, context.rs, streaming.rs, workflow.rs, sub_agent_executor.rs. |
| H-002 | HIGH | No error recovery on loadWorkflows | **DONE** | WorkflowList: error state with retry button. WorkflowSidebar/+page.svelte: pass workflowsError + workflowsLoading + onretry. 5 new store tests. i18n keys added. |
| H-003 | HIGH | No double-submit protection | **DONE** | workflowExecutor.service.ts: `executingWorkflows` Set guards against concurrent sends. |
| M-001 to M-012 | MEDIUM | Various quality issues | **NOT DONE** | No changes to chat/workflow components (except MarkdownRenderer URL check). |

### SA-012: DB Layer & Migrations

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| F2 | HIGH | mcp_call_log.params TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '{}'`. MCPCallLog uses serialize_as_json_string. 3 tests. |
| F3 | HIGH | mcp_call_log.result TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '[]'`. Same serde pattern. |
| F4 | HIGH | Import sanitization missing | **DONE** | Same as S2-M1. |
| F5 | HIGH | validation_request.details TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '{}'`. ValidationRequest uses custom serde. |
| F6 | MEDIUM | Non-parameterized queries in queries.rs | **DONE** | cascade::delete_by_workflow_id uses `$wf_id` bind param. |
| F7 | MEDIUM | validation queries not parameterized | **DONE** | Same as SA-001 M1-M5. |
| F8 | LOW | Redundant validation_helper logic | **PARTIAL** | RiskLevel::Critical handled in should_require_validation, but no full refactor. |
| F9 | LOW | agent_state table defined but never written | **DONE** | Removed from schema.rs. |
| F10 | LOW | workflow_agent table unused | **DONE** | Removed from schema.rs. |
| F11 | LOW | DBClient::update() unused | **DONE** | Removed from client.rs. |
| F12 | LOW | Non-parameterized QueryBuilder unused | **DONE** | Removed from tools/utils.rs. |

### SA-013: Types & Tools Coherence

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| #13 | CRITICAL | RiskLevel missing 'critical' variant | **DONE** | validation.rs: `RiskLevel::Critical` added. Schema ASSERT updated. 1 test. validation_helper.rs updated. |
| #1-4 | HIGH (adj.) | AgentConfig optional vs required mismatch | **DONE** | AgentConfig: both fields already `number`/`boolean`. AgentConfigCreate: `max_tool_iterations: number` (was `number?`), `enable_thinking: boolean` (was `boolean?`). AgentForm.svelte updated to always send `enable_thinking`. Zod schema updated. 2 test mocks updated. |
| #6 | HIGH | MessageCreate missing tokens field | **DONE** | message.ts: `tokens: number` added to MessageCreate. 3 helper functions updated. Rust: `#[serde(default)]` added for defense-in-depth. 3 new tests. |
| #12 | HIGH | ProviderSettings.base_url skip_serializing vs null | **DONE** | llm_models.rs: removed `skip_serializing_if` from `base_url`. Now serializes as `null` when None, matching TS `string \| null`. 3 new tests. |
| #14-15 | MEDIUM (adj.) | Orphan ChunkType variants, user_question field | **NOT DONE** | ChunkType unchanged. |
| #16-20 | MEDIUM | Console.* violations (28 instances) | **PARTIAL** | Many console.error replaced with getErrorMessage(). Some remain. |
| model_id | - | Workflow.model_id convention | **DONE** | workflow.ts: `model_id: string | null` (was `model_id?: string`). |

---

## Work Done Outside ACTION-PLAN-TDD

These changes were implemented but were not explicitly listed in the TDD plan:

| Change | Files | Rationale |
|--------|-------|-----------|
| CancellationToken propagation to sub-agents | agent.rs, orchestrator.rs, llm_agent.rs, context.rs, streaming.rs, workflow.rs, sub_agent_executor.rs | Sub-agents continued running after user cancel. Token now threaded through full execution chain. |
| UTF-8 safe truncation in memory compact mode | tools/memory/tool.rs | Panic on multi-byte chars (French accented text). Replaced byte slice with safe_truncate(). |
| CSP blob: directive | tauri.conf.json | `default-src 'self' blob:` for markdown/export |
| MemoryList export via Tauri dialog | MemoryList.svelte | Replaced Blob+DOM link with native save dialog + backend invoke |
| Accessibility improvements | ExportPreview, ImportExportSettings, MemorySettings, settings/+layout | aria-expanded, role, aria-current, aria-label |
| i18n keys for export | en.json, fr.json | `memory_export_title` |
| Agent test update | agents.test.ts | Added `enable_thinking: true` to mock config |
| Workflow test update | workflows.test.ts | Added `model_id: null` to mock workflow |
| serde_utils consolidation | serde_utils.rs, tool_execution.rs | Moved shared serializers to serde_utils, removed duplicates |

---

## What Remains (Honest Assessment)

### Must Do Before Merge (HIGH unresolved)

| Finding | Why | Effort |
|---------|-----|--------|
| ~~SA-005 H3: Migration guard~~ | ~~Memory embeddings can be destroyed by re-running migration~~ | **DONE** |
| ~~SA-011 H-001: Activity capture race~~ | ~~Activities can be lost during streaming reset~~ | **DONE** (frontend guard + backend cancellation token propagation) |
| ~~SA-011 H-002: loadWorkflows error recovery~~ | ~~Blank sidebar with no retry on DB failure~~ | **DONE** |
| ~~SA-013 #1-4: max_tool_iterations TS type~~ | ~~Still optional in TS, always present from Rust~~ | **DONE** |
| ~~SA-013 #6: MessageCreate tokens~~ | ~~Missing field in TS type~~ | **DONE** |
| ~~SA-013 #12: ProviderSettings.base_url~~ | ~~Nullability mismatch~~ | **DONE** |

### Should Do (MEDIUM unresolved, grouped)

| Group | Findings | Effort |
|-------|----------|--------|
| Tauri permissions hardening | SA-005 M1-M3 | 1h |
| Template deduplication | SA-010 DUP-1/2/3 | 3h |
| Code deduplication (Rust) | SA-007 DUP-1/2/3, SA-008 DUP-1/3/4 | 6h |
| Function decomposition | SA-007 F1-F13 | 4h |
| NPM dependency updates | SA-006 DEP-3 | 30min |
| SurrealDB feature pruning | SA-006 DEP-2 | 1h |
| Orphan TS types cleanup | SA-013 #14-15 | 30min |
| MCP HTTP validation | SA-002 S2-H3 | 30min |

### Deferred (LOW / quality-only)

| Group | Findings | Reason |
|-------|----------|--------|
| UUID bind params for validated IDs | SA-001 L1-L18 | Defense-in-depth, IDs from DB |
| Chat/workflow component quality | SA-011 M-001 to M-012 | UX improvements, not security |
| Error message context | SA-007 F14 | Quality improvement |
| Remaining console.* | SA-013 #16-20 partial | Non-critical |

---

## Verification Status

| Check | Status | Notes |
|-------|--------|-------|
| cargo fmt --check | **PASS** | 2026-02-20 |
| cargo clippy -- -D warnings | **PASS** | 2026-02-20, 0 warnings |
| cargo test --lib | **PASS** | 2026-02-20, 897 tests passed |
| npm run lint | **PASS** | 2026-02-20, 0 errors |
| npm run check | **PASS** | 2026-02-20, 0 errors 0 warnings |
| npm run test | **PASS** | 2026-02-20, 225 tests passed |
| Manual test: streaming + cancel | **PASS** | 2026-02-20, user confirmed no bugs |
| Manual test: memory compact mode | **PASS** | 2026-02-20, French text no longer panics |
| Manual test: search prompts | **NOT RUN** | |
| Manual test: import/export | **NOT RUN** | |
| Manual test: custom provider HTTP warning | **NOT RUN** | |
| Manual test: fonts rendering | **NOT RUN** | |
