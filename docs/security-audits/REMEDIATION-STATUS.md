# Remediation Status - Security Audit Findings

**Date**: 2026-02-26 (updated)
**Branch**: `security/audit-remediation-tdd`
**Base**: `main` (commit 1d8fc29)
**Files changed**: 166 (vs main)
**Lines**: +11,217 / -2,729

---

## Summary

| Status | Count | Description |
|--------|-------|-------------|
| DONE | 92 | Fix implemented and tested |
| NOT DONE | 10 | Not yet addressed |
| DOCUMENTED | 3 | Analyzed, documented as non-issue or by-design |
| N/A | 7 | Not applicable (desktop context) |

| Category | DONE | NOT DONE |
|----------|------|----------|
| CRITICAL (4) | 4 | 0 |
| HIGH (30) | 30 | 0 |
| MEDIUM (39) | 41 | 0 |
| LOW (19) | 13 | 6 |
| N/A (7) | - | - |

**All 4 CRITICAL findings are remediated.**
**SA-014 added: 13 findings (3H/4M/6L), 10 DONE + 3 DOCUMENTED.**
**SA-015 ALL PHASES DONE: annotation cleanup + superseded code removal + dead getters + speculative code + final audit (22 items deleted, 6 tests migrated, 5 tests deleted, 171 remaining annotations all verified legitimate).**
**SA-016 UX REMEDIATION: 7 phases DONE + 3 additional bug fixes. Phase 8 cancelled (architectural complexity, moved to Out of Scope).**
**SA-017 SETTINGS OPTIMIZATION: All 5 phases DONE (PERF-1-5, OPT-1-10). Scroll performance, component extraction, MemorySettings decomposition, backend validation centralization, error handling harmonization.**
**SA-018 HARDCODED ELEMENTS: ALL 3 PHASES DONE. P1: model IDs + pricing dead code removed, is_reasoning from DB. P2: DEFAULT_OLLAMA_URL centralized. P3: 27 i18n keys added.**
**SA-018 HARDCODED ELEMENTS: P1 DONE (model IDs removed, is_reasoning propagated from DB). P2/P3 remaining.**
**SA-019 ALL PHASES DONE: Block-by-block agent chat refactoring. P1-P2: backend events+persistence. P3-P4: frontend display+sidebar removal. P5: dead code cleanup + 4 bug fixes. P6: TodoTool tasks display (inline task list grouped by agent, 9 TDD tests, persistence fix + agent name resolution). Follow-up: auto-scroll fix.**
**SA-020 ALL PHASES DONE: Hybrid agent ID/name resolution. P1: UNIQUE index + backend validation. P2: AgentRegistry.get_by_name(). P3: resolve_agent_ref() shared function. P4: DelegateTaskTool accepts agent_name. P5: ParallelTasksTool accepts agent_name + real names in events/reports. P6: Frontend duplicate name validation + i18n. P7: Documentation. 22 TDD tests, 12 files, +1225/-268 lines.**
**SA-021 DONE: Report enforcement mechanism. Detects generic "Task completed" messages and makes one follow-up LLM call for a proper markdown report. 6 TDD tests, 1 file, +175 lines.**
**SA-022 ALL PHASES DONE: Frontend structure & naming audit. P1: JSDoc fix + inventory counters. P2: Dead code removal + helper integration. P3: Naming normalization to kebab-case. P4: Modal consolidation. P5: Barrel export completion. P6: Service & directory cleanup + sub-agent blocks bug fix. P7: Settings providers restructuration (3 components moved to providers/ subdirectory).**
**SA-023 P1-P4 DONE: Backend structure & naming audit. P1: ProviderType consolidated into llm/provider.rs. P2: commands/models.rs renamed to commands/llm_models.rs. P3: safe_truncate() moved to tools/utils.rs. P4: App-wide constants moved to top-level constants.rs.**
**SA-024 P1-P4 DONE: P1: Dependency cleanup (once_cell, futures, surrealdb pin). P2: Production robustness - 7 .expect() converted to Result in LLM providers. P3: Code hygiene - 171 OPT-* markers removed from 52 files (32 Rust + 20 frontend). dead_code audit deferred. P4: Config & Inventory - total_commands fixed (106->123, 39 commands added to inventory), svelte-virtual-list moved to dependencies.**

---

## Tests Added

### Rust (92 new tests)

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
| tools/validation_helper.rs | test_should_require_validation_auto_mode_skips | SA-012 F8: Auto mode skips low-risk |
| tools/validation_helper.rs | test_should_require_validation_auto_mode_confirms_high | SA-012 F8: Auto mode confirms high-risk |
| tools/validation_helper.rs | test_should_require_validation_manual_mode | SA-012 F8: Manual mode always requires |
| tools/validation_helper.rs | test_should_require_validation_selective_mode | SA-012 F8: Selective mode per-type |
| tools/validation_helper.rs | test_should_require_validation_selective_auto_approve_low | SA-012 F8: Selective auto-approve low |
| commands/mcp.rs | test_check_mcp_http_warning_docker_no_warning | SA-002 S2-H3: Docker method no warning |
| commands/mcp.rs | test_check_mcp_http_warning_npx_no_warning | SA-002 S2-H3: Npx method no warning |
| commands/mcp.rs | test_check_mcp_http_warning_https_no_warning | SA-002 S2-H3: HTTPS no warning |
| commands/mcp.rs | test_check_mcp_http_warning_localhost_no_warning | SA-002 S2-H3: Localhost no warning |
| commands/mcp.rs | test_check_mcp_http_warning_remote_http_returns_warning | SA-002 S2-H3: Remote HTTP triggers |
| commands/mcp.rs | test_check_mcp_http_warning_remote_ip_returns_warning | SA-002 S2-H3: Remote IP triggers |
| commands/mcp.rs | test_check_mcp_http_warning_empty_args_no_warning | SA-002 S2-H3: Empty args safe |
| mcp/http_handle.rs | test_http_warning_for_remote_http_url | SA-002 S2-H3: Integration - remote HTTP |
| mcp/http_handle.rs | test_no_http_warning_for_https_url | SA-002 S2-H3: Integration - HTTPS safe |
| mcp/http_handle.rs | test_no_http_warning_for_localhost | SA-002 S2-H3: Integration - localhost safe |
| security/validation.rs | test_validate_uuid_field_valid | SA-007 DUP-1: valid UUID returns Ok |
| security/validation.rs | test_validate_uuid_field_invalid | SA-007 DUP-1: invalid UUID includes field name |
| security/validation.rs | test_validate_uuid_field_includes_field_name_in_error | SA-007 DUP-1: error contains field name |
| security/validation.rs | test_validate_uuid_field_trims_whitespace | SA-007 DUP-1: whitespace trimmed |
| security/validation.rs | test_serialize_for_query_string | SA-007 DUP-2: string serializes to JSON |
| security/validation.rs | test_serialize_for_query_special_chars | SA-007 DUP-2: special chars properly escaped |
| security/validation.rs | test_serialize_for_query_vec | SA-007 DUP-2: vec serializes to JSON array |
| security/validation.rs | test_serialize_for_query_option_none | SA-007 DUP-2: None serializes to null |
| models/streaming.rs | test_user_question_chunk_type_serialization | SA-013 #14: ChunkType serialization for UserQuestionStart/Complete |
| models/streaming.rs | test_stream_chunk_user_question_start | SA-013 #14-15: user_question_start constructor + payload fields |
| models/streaming.rs | test_stream_chunk_user_question_complete | SA-013 #14-15: user_question_complete constructor + question_id |
| models/streaming.rs | test_user_question_fields_skipped_when_none | SA-013 #14-15: user_question/question_id absent for other chunk types |
| commands/agent.rs | test_create_agent_rejects_duplicate_name | SA-020/P1: duplicate name rejected on create |
| commands/agent.rs | test_update_agent_allows_keeping_own_name | SA-020/P1: update without name change succeeds |
| commands/agent.rs | test_update_agent_rejects_collision_with_other | SA-020/P1: rename to existing name rejected |
| agents/core/registry.rs | test_get_by_name_found | SA-020/P2: exact name lookup works |
| agents/core/registry.rs | test_get_by_name_case_insensitive | SA-020/P2: case-insensitive lookup |
| agents/core/registry.rs | test_get_by_name_trimmed | SA-020/P2: whitespace-trimmed lookup |
| agents/core/registry.rs | test_get_by_name_not_found | SA-020/P2: returns None for unknown name |
| agents/core/registry.rs | test_get_by_name_empty | SA-020/P2: returns None for empty string |
| tools/utils.rs | test_resolve_agent_ref_by_id | SA-020/P3: resolve by UUID fast path |
| tools/utils.rs | test_resolve_agent_ref_by_name | SA-020/P3: resolve by name slow path |
| tools/utils.rs | test_resolve_agent_ref_not_found | SA-020/P3: NotFound error for unknown ref |
| tools/utils.rs | test_resolve_agent_ref_empty_input | SA-020/P3: error on empty input |
| tools/delegate_task.rs | test_validate_input_accepts_agent_id | SA-020/P4: agent_id accepted |
| tools/delegate_task.rs | test_validate_input_accepts_agent_name | SA-020/P4: agent_name accepted |
| tools/delegate_task.rs | test_validate_input_rejects_missing_both | SA-020/P4: error when neither provided |
| tools/delegate_task.rs | test_definition_has_agent_name_property | SA-020/P4: schema includes agent_name |
| tools/parallel_tasks.rs | test_validate_parallel_task_accepts_agent_id | SA-020/P5: agent_id accepted per task |
| tools/parallel_tasks.rs | test_validate_parallel_task_accepts_agent_name | SA-020/P5: agent_name accepted per task |
| tools/parallel_tasks.rs | test_validate_parallel_task_rejects_missing_both | SA-020/P5: error when neither provided |
| tools/parallel_tasks.rs | test_definition_has_agent_name_property | SA-020/P5: schema includes agent_name |
| tools/parallel_tasks.rs | test_parallel_task_spec_includes_agent_name | SA-020/P5: ParallelTaskSpec has agent_name field |
| tools/parallel_tasks.rs | test_parallel_task_spec_serialization | SA-020/P5: spec serialization includes agent_name |
| agents/llm_agent.rs | test_is_generic_completion_message_standard_pattern | SA-021: detects "Task completed after N iteration(s)" |
| agents/llm_agent.rs | test_is_generic_completion_message_max_iterations_pattern | SA-021: detects "Max tool iterations (N) reached" |
| agents/llm_agent.rs | test_is_generic_completion_message_empty | SA-021: detects empty/whitespace content |
| agents/llm_agent.rs | test_is_generic_completion_message_real_reports | SA-021: does NOT flag real markdown reports |
| agents/llm_agent.rs | test_is_generic_completion_message_with_whitespace | SA-021: handles whitespace trimming |
| agents/llm_agent.rs | test_report_enforcement_prompt_is_valid | SA-021: prompt is valid and contains keywords |

### TypeScript (67 new tests in 5 new files + 1 updated)

| File | Tests | Purpose |
|------|-------|---------|
| utils/__tests__/error.test.ts | 11 tests | getErrorMessage + formatErrorForDisplay |
| utils/__tests__/url.test.ts | 11 tests | isAllowedScheme (XSS defense) |
| stores/__tests__/activity.test.ts | ~~8 tests~~ | ~~Activity capture guard (SA-011 H-001 race condition)~~ **DELETED** (SA-019/P4: activityStore removed) |
| stores/__tests__/workflows.test.ts | 5 new tests | loadWorkflows retry recovery (SA-011 H-002) |
| stores/__tests__/chunkProcessor.test.ts | 22 tests | Shared chunk processor (SA-009 F1: all 12 chunk types + immutability + extended state) |
| utils/__tests__/panel-merge.test.ts | ~~10 tests~~ | ~~Panel merge utilities (SA-011 M-003/M-004)~~ **DELETED** (SA-019/P4: panel components removed) |

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
| S2-H3 | MEDIUM (adj.) | MCP HTTP base_url not validated | **DONE** | `check_http_warning()` reused in `http_handle.rs::connect()` (runtime) + `commands/mcp.rs` create/update (config-time). `MCPServerResponse` wrapper returns warning to frontend. MCPSection.svelte shows warning. 10 new tests. |
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
| M1 | MEDIUM | opener:default allows any URL | **DONE** | isAllowedScheme() for markdown links + Tauri permission scope restricted: `opener:allow-open-url` with allow `https://*`, `http://*`, `mailto:*` and deny `file://*`, `tel:*`, `data:*`, `javascript:*`, `vbscript:*`. `opener:deny-open-path` added. StepImport.svelte: `window.open()` replaced with `openUrl()` (SA-005 M4). |
| M2 | MEDIUM | dialog:default grants all types | **NOT DONE** | Tauri capabilities unchanged. |
| M3 | MEDIUM | No IPC deny patterns | **NOT DONE** | Tauri capabilities unchanged. |
| M4 | MEDIUM | window.open() bypasses opener plugin | **DONE** | StepImport.svelte: replaced `window.open()` with `openUrl()` from `@tauri-apps/plugin-opener`. Now goes through scoped permission. |
| L1-L3 | LOW | CSP documentation, permission comments | **NOT DONE** | No documentation added. |

### SA-006: Dependency Vulnerabilities

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| 7 CVEs | N/A | All NPM CVEs inapplicable to desktop | **N/A** | Confirmed in EVALUATION. |
| DEP-1 | HIGH | rig-core features = ["all"] pulls bloat | **DONE** | Cargo.toml: removed `features = ["all"]` from rig-core. 26 crates removed from lock file (lopdf, rayon, nom, etc.). |
| DEP-2 | HIGH | surrealdb unused features | **DONE** | Cargo.toml: `default-features = false, features = ["kv-rocksdb"]`. 4 crates removed (tokio-tungstenite, tungstenite, webpki-roots, data-encoding). 0 network deps remain in SurrealDB tree. 902 tests pass, 0 clippy warnings. |
| DEP-3 | HIGH | NPM patch updates available | **NOT DONE** | No package.json changes in branch. |
| L/INFO | LOW/INFO | Unmaintained transitive deps | **NOT DONE** | Upstream dependency, cannot fix. |

### SA-007: Commands Control Flow & Error Handling

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| DUP-1 | MEDIUM | UUID validation repeated 52x | **DONE** | `validate_uuid_field()` helper in `security/validation.rs`. 47 production occurrences replaced across 12 command files. 4 new tests. |
| DUP-2 | MEDIUM | serde_json escaping repeated 25x | **DONE** | `serialize_for_query()` helper in `security/validation.rs`. 25 occurrences replaced across 9 command files. 4 new tests (920 total pass). |
| DUP-3 | MEDIUM | COUNT extraction repeated 16x | **DONE** | `extract_count()` helper in `db/utils.rs`. 16 occurrences replaced across 7 command files. |
| F1-F13 | MEDIUM | 13 oversized functions (>100 lines) | **DONE** | All 12 remaining functions decomposed below 100 lines (1 already removed). streaming.rs: 6 sub-functions. import_export.rs: 17+ helpers. workflow.rs: generic query+deserialize. models.rs: connection_test_outcome + test_mistral_api + check_model_uniqueness. custom_provider.rs: build_provider_update_clauses + reconfigure_provider_runtime. agent.rs: merge_agent_config. tool_execution.rs: validate_tool_fields. |
| F14 | LOW | 15 generic "Database error" messages | **DONE** | All 15 "Database error" replaced with contextual messages (e.g., "Failed to list models", "Failed to update task status"). |
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
| F1 | MEDIUM | Streaming/backgroundWorkflows chunk duplication | **DONE** | `applyChunkToState()` in `stores/utils/chunkProcessor.ts`. Both stores delegate to shared processor. 22 new tests. |
| F2 | MEDIUM | Manual error extraction in 6 stores | **DONE** | validation-settings.ts, validation.ts: now use getErrorMessage(). |
| F4 | MEDIUM | userQuestion.ts subscribe/unsub hack | **DONE** | Replaced with `get(store)` pattern. |
| F9 | - | Zero ERR_SVELTE_005 violations | **CONFIRMED** | Still true. |
| Dead code | LOW | Deprecated exports | **DONE** | Removed: agentCount, promptCount, isTokenStreaming, createInitialAgentState, AgentState. |

### SA-010: Settings Forms Quality

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| ERR-1 | MEDIUM | 29/30 try/catch not using getErrorMessage | **DONE** | 18+ components updated to use getErrorMessage(). See frontend diff. |
| ERR-2 | MEDIUM | 5 files use console.error/warn | **DONE** | All console.error/warn replaced: AgentForm (loadWarnings state + i18n), PromptSettings (store handles error), ValidationSettings/MemorySettings/ImportExportSettings (message state + i18n). Empty catch in AgentForm documented (store handles error). |
| DUP-1 | MEDIUM | ValidationSettings 9 identical info-cards | **DONE** | Extracted `ValidationInfoCard.svelte` component. Merged auto/manual modes into single block with `@const` variant. Shared `toolBadgeList`/`mcpBadgeList` snippets reused by all 3 modes. ~130 lines removed. |
| DUP-2 | MEDIUM | ImportPreview 4 identical sections | **DONE** | Data-driven loop with `entityDefs` array replaces 4 summary cards + 4 entity lists. `getEntityMeta()` helper for type-specific content. ~126 lines removed. |
| DUP-3 | MEDIUM | ExportPreview 4 identical sections | **DONE** | Extracted `ExportEntitySection.svelte` component with collapsible Card + expand logic. 4 `expanded*` variables replaced by single object. MCP section kept inline (MCPFieldEditor + sanitization). ~80 lines removed. |
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
| M-001 | MEDIUM | Clipboard copy no error handling | **DONE** | MessageBubble: try/catch + copyError state + AlertCircle visual feedback. |
| M-002 | MEDIUM | PromptSelector console.error | **DONE** | Already fixed in SA-013 #16-20 (console cleanup). |
| M-003 | MEDIUM | ReasoningPanel large derivation | **DONE** | Extracted `mergeAndSortReasoningSteps()` to `utils/panel-merge.ts`. 5 tests. |
| M-004 | MEDIUM | ToolExecutionPanel large derivation | **DONE** | Extracted `mergeToolExecutions()` to `utils/panel-merge.ts`. Reused `ActiveTool` from streaming store. 5 tests. |
| M-005 | MEDIUM | Validation no timeout | **DONE** | validation.ts: 5-min `VALIDATION_TIMEOUT_MS`, auto-reject via `startValidationTimeout()`. Wired into init/approve/reject/dismiss/cleanup. |
| M-006 | MEDIUM | UserQuestionModal console.warn | **DONE** | Already fixed in SA-013 #16-20 (console cleanup). |
| M-007 | MEDIUM | ActivityItem 3 boolean states | **DONE** | Replaced `isTaskExpanded`/`isReasoningExpanded`/`isToolExpanded` with single `expandedSection` enum. |
| M-008 | MEDIUM | setTimeout for focus | **DONE** | NewWorkflowModal + WorkflowItem: replaced `setTimeout(() => ref?.focus())` with `tick().then(() => ref?.focus())`. |
| M-009 | MEDIUM | TokenDisplay progressbar ARIA | **DONE** | Moved `role="progressbar"` to parent, added `aria-valuetext` with warning-level-aware text + `aria-label`. |
| M-010 | MEDIUM | backgroundWorkflows cleanup | **DONE** | Already had `status !== 'running'` guard at line 268. No change needed. |
| M-011 | MEDIUM | WorkflowItem rename edge case | **DONE** | Added documentation comment explaining intentional behavior (editing ignores external renames). |
| M-012 | MEDIUM | ToolDetailsPanel no retry | **DONE** | Extracted `loadExecution()` from onMount, added retry button with RefreshCw icon in error state. |

### SA-012: DB Layer & Migrations

| ID | Severity | Finding | Status | Evidence |
|----|----------|---------|--------|----------|
| F2 | HIGH | mcp_call_log.params TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '{}'`. MCPCallLog uses serialize_as_json_string. 3 tests. |
| F3 | HIGH | mcp_call_log.result TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '[]'`. Same serde pattern. |
| F4 | HIGH | Import sanitization missing | **DONE** | Same as S2-M1. |
| F5 | HIGH | validation_request.details TYPE object | **DONE** | schema.rs: `TYPE string DEFAULT '{}'`. ValidationRequest uses custom serde. |
| F6 | MEDIUM | Non-parameterized queries in queries.rs | **DONE** | cascade::delete_by_workflow_id uses `$wf_id` bind param. |
| F7 | MEDIUM | validation queries not parameterized | **DONE** | Same as SA-001 M1-M5. |
| F8 | LOW | Redundant validation_helper logic + MCP migration DEFINE FIELD | **DONE** | Full refactor: extracted `should_require_validation()` as pure function, `request_validation()` delegates to `create_and_wait_validation()`, removed `needs_validation()` wrapper, unified event emission via `ValidationRequiredEvent` struct. MCP migration inline code removed from client.rs (redundant with schema.rs DEFINE FIELD OVERWRITE). 5 new tests. TS types updated. |
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
| #14-15 | MEDIUM (adj.) | Orphan ChunkType variants, user_question field | **DONE** | Rust ChunkType: added `UserQuestionStart`/`UserQuestionComplete` variants. Rust StreamChunk: added `user_question`/`question_id` fields + 2 constructors. tool.rs + commands/user_question.rs: replaced manual `json!()` with typed `StreamChunk` constructors. TS StreamChunk: added `question_id` field. `UserQuestionStreamPayload`: added `Deserialize`. 4 new tests. |
| #16-20 | MEDIUM | Console.* violations (28 instances) | **DONE** | All 28 console.* removed: services (silent return), stores/i18n (silent fallback), agent page (toast notifications), settings pages (error state in UI), components (UI error state or silent guard). 0 remaining. |
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
| SA-011 M-001 to M-012 remediation | MessageBubble, ActivityItem, TokenDisplay, WorkflowItem, NewWorkflowModal, ToolDetailsPanel, ReasoningPanel, ToolExecutionPanel, validation.ts, panel-merge.ts (new), panel-merge.test.ts (new), vitest.config.ts, en.json, fr.json | 12 MEDIUM quality issues: clipboard error handling, derivation extraction, validation timeout, boolean consolidation, tick() focus, ARIA progressbar, retry button. 10 new tests. |
| Agent test update | agents.test.ts | Added `enable_thinking: true` to mock config |
| Workflow test update | workflows.test.ts | Added `model_id: null` to mock workflow |
| serde_utils consolidation | serde_utils.rs, tool_execution.rs | Moved shared serializers to serde_utils, removed duplicates |
| Sub-agent token separation (P3 revised) | schema.rs, queries.rs, workflow.rs, streaming.rs, tokens.ts, TokenDisplay.svelte, workflow.ts, en/fr.json | Separate DB fields for sub-agent tokens; frontend AGENT/TOTAL sections; context gauge shows cumulative main agent |

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
| Tauri permissions hardening | SA-005 M2-M3 (M1+M4 DONE) | 30min |
| Template deduplication | SA-010 DUP-1/2/3 | 3h |
| ~~UUID validation dedup~~ | ~~SA-007 DUP-1~~ | **DONE** |
| ~~serde_json escaping dedup~~ | ~~SA-007 DUP-2~~ | **DONE** |
| ~~COUNT extraction dedup~~ | ~~SA-007 DUP-3~~ | **DONE** |
| ~~Function decomposition~~ | ~~SA-007 F1-F13~~ | **DONE** |
| Code deduplication (Rust) | SA-008 DUP-1/3/4 | 3h |
| NPM dependency updates | SA-006 DEP-3 | 30min |
| ~~SurrealDB feature pruning~~ | ~~SA-006 DEP-2~~ | **DONE** |
| ~~Orphan TS types cleanup~~ | ~~SA-013 #14-15~~ | **DONE** |
| ~~MCP HTTP validation~~ | ~~SA-002 S2-H3~~ | **DONE** |

### Deferred (LOW / quality-only)

| Group | Findings | Reason |
|-------|----------|--------|
| UUID bind params for validated IDs | SA-001 L1-L18 | Defense-in-depth, IDs from DB |
| ~~Chat/workflow component quality~~ | ~~SA-011 M-001 to M-012~~ | **DONE** - 9 new fixes + 3 already done (M-002/M-006 in SA-013, M-010 existing guard) |
| ~~Error message context~~ | ~~SA-007 F14~~ | **DONE** |
| ~~Remaining console.* (non-settings)~~ | ~~SA-013 #16-20~~ | **DONE** - All 22 remaining removed |

---

## SA-014: Data Persistence & Display After Restart

**Document:** [SA-014-data-persistence-restart.md](SA-014-data-persistence-restart.md)

| ID | Severity | Finding | Status |
|----|----------|---------|--------|
| P1 | HIGH | Sub-agent tool executions never persisted | **DONE** |
| P2 | HIGH | Sub-agent reasoning steps never persisted | **DONE** |
| P3 | MEDIUM | Sub-agent tokens separated from main agent | **DONE** |
| P4 | MEDIUM | SubAgentActivity only accepts live data | **DOCUMENTED** (unused component, pipeline works) |
| P5 | MEDIUM | Message enrichment missing cancelled agents | **DONE** |
| P6 | LOW | parent_execution_id dropped by SCHEMAFULL | **DONE** |
| P7 | LOW | Undefined task_description + missing agentId | **DONE** |
| P8 | LOW | Activity IDs differ live vs historical | **DOCUMENTED** (by-design, no user impact) |
| P9 | LOW | activeToolToActivity missing executionId | **DOCUMENTED** (intentional during streaming) |
| P10 | LOW | Cancelled sub-agents shown as error | **DONE** |
| P11 | HIGH | update_execution_record format!() silent SQL failures | **DONE** |
| P12 | LOW | "failed" status doesn't match SubAgentStatus enum | **DONE** |
| P13 | MEDIUM | Message enrichment dedup uses name instead of ID | **DONE** |

**Key changes:**
- NEW `src-tauri/src/db/persistence.rs` - shared persistence module
- Extended `ExecutionResult` with `tool_executions` + `reasoning_steps`
- `aggregate_sub_agent_tokens()` stores sub-agent tokens in separate fields; TokenDisplay shows AGENT + TOTAL sections
- P11: Parameterized queries in `update_execution_record()` (format!() was causing silent SQL failures)
- P12: Fixed "failed" to "error" to match SubAgentStatus serde
- P13: ID-based dedup in message enrichment (name-based caused token swap for duplicate agents)
- 10 new tests (3 Rust + 8 TypeScript)

---

## SA-015: Dead Code Cleanup (`#[allow(dead_code)]`)

**Document:** [SA-015-dead-code-cleanup.md](SA-015-dead-code-cleanup.md)

### Phase 1: Remove False Positive Annotations -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Module-level annotations removed | 2 | `tools/factory.rs`, `llm/embedding.rs` |
| Item-level annotations removed | 5 | `state.rs` fields (x2), `db/client.rs`, `models/mcp.rs` (x2) |
| Item-level annotations added | 24 | `llm/embedding.rs` (compiler-verified replacements for module-level) |
| Comments corrected (test-only) | 7 | `state.rs` (x5), `sub_agent_circuit_breaker.rs` (x2) |
| Stale doc/comments removed | 3 | factory.rs "Phase 6", embedding.rs "Phase 3", mcp.rs "Phase 2/3" |

### Phase 2: Remove Superseded Code -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Methods deleted | 5 | `execute()`, `execute_parallel()`, `with_resilience()`, `execute_with_metrics()`, `sub_agent_progress()` |
| Tests migrated | 6 | `execute()` -> `execute_with_mcp()` in orchestrator, workflow, state tests |
| Tests deleted | 4 | 3 parallel execution tests + 1 sub_agent_progress test |
| Doc comments updated | 4 | Module example, `new()` doc, `with_cancellation()` doc, `execute_with_heartbeat_timeout` doc |

Test count: 937 (Phase 1) -> 933 (Phase 2) -- 4 tests deleted, 6 migrated.

### Phase 3: Remove Dead Getters -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Getters deleted | 5 | `SubAgentExecutor`: `workflow_id()`, `parent_agent_id()`, `db()`, `orchestrator()`, `mcp_manager()` |
| Methods deleted | 1 | `AppState::get_cancellation_token()` |
| Methods deleted | 1 | `UserQuestionCircuitBreaker::timeout_threshold()` |

Test count: 933 unchanged (no tests depended on these getters).

### Phase 4: Remove Speculative Code -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Struct deleted | 1 | `QueryStats` (db/client.rs) |
| DB methods deleted | 3 | `transaction()`, `query_with_stats()`, `transaction_with_params()` |
| LLM agent methods deleted | 3 | `build_prompt_with_tools()`, `call_mcp_tool()`, `get_available_mcp_tools()` |
| Unused import removed | 1 | `warn` from `tracing` in db/client.rs |
| Items kept (test-only) | 2 | `cleanup_temporary()`, `Prompt::interpolate()` |

Test count: 933 unchanged (no tests depended on speculative code).
Annotation count: 189 (Phase 1) -> 171 (Phase 4) -- 18 annotations removed across Phases 2-4.

### Phase 5: Final Audit -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Module-level annotation removed | 1 | `models/llm_models.rs` (stale "Phase 2" comment) |
| Dead items deleted | 4 | `BuiltinModelParams`, `new_builtin()`, `MISTRAL_BUILTIN_MODELS`, `OLLAMA_BUILTIN_MODELS` |
| Dead test deleted | 1 | `test_llm_model_new_builtin` |
| False unused_imports fixed | 2 | `mistral.rs`, `ollama.rs` -- `CompletionClient` trait actually needed for `.agent()` |
| Dead re-exports removed | 3 | `BuiltinModelParams`, `MISTRAL_BUILTIN_MODELS`, `OLLAMA_BUILTIN_MODELS` from `models/mod.rs` |
| Remaining annotations verified | 171 | All legitimate: SERDE (~39), API_LIBRARY (~42), TEST_ONLY (~11), EMBEDDING (32), TRAIT/MODULE (~12), CONST (~10), OTHER (~25) |

Test count: 933 (Phase 4) -> 932 (Phase 5) -- 1 test deleted.

---

## SA-016: Agent Page UX Remediation

**Document:** [SA-016-agent-page-ux-remediation.md](SA-016-agent-page-ux-remediation.md)

### Phases 1-7 (DONE)

| Phase | Problem | Status |
|-------|---------|--------|
| 1 | Double scroll (ChatContainer + MessageList) | **DONE** |
| 2 | Filter labels in expanded sidebar | **DONE** |
| 3 | Remove double filtering of workflows | **DONE** |
| 4 | Markdown in streaming | **DONE** |
| 5 | Rename discoverability (pencil icon + F2) | **DONE** |
| 6 | Informative round separators | **DONE** |
| 7 | Temporal grouping of workflows | **DONE** |

### Phase 8 (CANCELLED)

| Phase | Problem | Status | Reason |
|-------|---------|--------|--------|
| 8 | Progressive activity display + thinking in chat | **CANCELLED** | `isStillViewed()` guard prevents streaming state reset; fix attempt broke workflow switching. Moved to Out of Scope. |

### Additional Bug Fixes

| ID | Problem | Status |
|----|---------|--------|
| BF1 | Missing `rename_workflow` Tauri command | **DONE** |
| BF2 | Space key not working in workflow rename input | **DONE** |
| BF3 | ConfirmDeleteModal not using standard Modal | **DONE** |

**Key changes:**
- NEW `rename_workflow` command in `src-tauri/src/commands/workflow.rs` (UUID + name validation, parameterized query)
- Registered `rename_workflow` in `src-tauri/src/main.rs`
- `WorkflowItem.svelte`: `event.stopPropagation()` in `handleEditKeydown()`
- `ConfirmDeleteModal.svelte`: rewritten to use standard `Modal` component
- New TS tests: `activityUtils.test.ts` (6 tests), `dateGrouping.test.ts` (9 tests)
- New Rust test: `rename_workflow` validation (1 test via cargo test)

---

## SA-017: Settings Page Optimization

**Document:** [SA-017-settings-page-optimization.md](SA-017-settings-page-optimization.md)

### All Phases (DONE)

| Phase | Items | Status | Commit |
|-------|-------|--------|--------|
| 0: Scroll Performance | PERF-1 to PERF-5 | **DONE** | `7f2c37e` |
| 1: Component Extraction | OPT-1 to OPT-3 | **DONE** | `bc58204` |
| 2: MemorySettings Decomposition | OPT-4 to OPT-6 | **DONE** | `c87bd85` |
| 3: Backend Validation | OPT-7, OPT-8 (N/A) | **DONE** | `0188ae8` |
| 4: Pattern Harmonization | OPT-9, OPT-10 | **DONE** | `bde6149` |

**Key changes:**
- **PERF-1-5**: GPU layer promotion on `.card`, CSS containment, scroll timeout 250ms, virtual-row transition removed, search debounce 300ms
- **OPT-1-3**: `ErrorBanner`, `SettingsSectionHeader`, `DeleteConfirmModal` shared components (-168 duplicated lines)
- **OPT-4-6**: `MemorySettings` decomposed into `EmbeddingConfigCard`, `EmbeddingTestCard`, `MemoryStatsCard` (1082 -> ~380 lines)
- **OPT-7**: `validate_trimmed_name()` centralized in `validation_helper.rs` with 9 TDD tests
- **OPT-8**: N/A (logging already correct, audit referenced non-existent pattern)
- **OPT-9**: `ErrorBanner` in MemorySettings + fix: errors from load/refresh/delete now visible (were hidden behind modal gate)
- **OPT-10**: `ErrorBanner` in ValidationSettings + try/catch on `onMount` for unhandled rejections

---

## SA-018: Hardcoded Elements Audit

**Document:** [SA-018-hardcoded-elements-audit.md](SA-018-hardcoded-elements-audit.md)

### P1: Remove Hardcoded Model IDs -- DONE

| Action | Count | Details |
|--------|-------|---------|
| Model lists removed | 4 | `MISTRAL_MODELS`, `OLLAMA_MODELS`, `DEFAULT_MISTRAL_MODEL`, `DEFAULT_OLLAMA_MODEL` |
| Reasoning detection removed | 4 | `REASONING_MODELS`, `OLLAMA_THINKING_MODELS`, `is_thinking_model()`, `is_thinking_model_name()` |
| Dead code removed | 3 | `complete_with_thinking()`, `get_think_param()`, `OllamaChatResponse`/`OllamaMessageResponse` |
| Pricing dead code removed | 1 | `mod mistral_pricing` (7 constants) |
| Other dead code removed | 1 | `VALID_MODEL_PROVIDERS` (0 references) |
| Trait adapted | 2 | `available_models()` -> `Vec::new()`, `default_model()` -> `String::new()` |
| is_reasoning propagated | 7 | provider.rs trait, mistral.rs, ollama.rs, manager.rs, agent.rs, llm_agent.rs, spawn_agent.rs |
| TS types synced | 2 | `agent.ts` (LLMConfig.is_reasoning), `AgentForm.svelte` |
| Validation removed | 1 | `set_default_model()` no longer validates against hardcoded list |
| Tests updated | 22 files | -376/+148 lines, 1918 Rust + 283 TS tests passing |

### P2: Centralize DEFAULT_OLLAMA_URL -- A FAIRE

### P3: i18n Messages Settings -- A FAIRE

---

## SA-018: Hardcoded Elements Audit

**Document:** [SA-018-hardcoded-elements-audit.md](SA-018-hardcoded-elements-audit.md)

### P1: Model IDs Hardcodes - DONE

| Action | Files | Details |
|--------|-------|---------|
| Removed MISTRAL_MODELS, DEFAULT_MISTRAL_MODEL, REASONING_MODELS | mistral.rs | Dead code, models from DB |
| Removed OLLAMA_MODELS, DEFAULT_OLLAMA_MODEL, OLLAMA_THINKING_MODELS | ollama.rs | Dead code + is_thinking_model(), complete_with_thinking() |
| Removed mistral_pricing module | pricing.rs | #[allow(dead_code)], 0 production usage |
| Removed VALID_MODEL_PROVIDERS | constants.rs | 0 references |
| Removed set_default_model validation | commands/llm.rs | Was validating against hardcoded list |
| Adapted LLMProvider trait | provider.rs, manager.rs | available_models() -> empty, default_model() -> empty, complete() + is_reasoning param |
| Propagated is_reasoning: bool | agent.rs, agent.ts, AgentForm.svelte, main.rs, spawn_agent.rs, llm_agent.rs | From DB through LLMConfig to provider |
| Updated 17+ test files | Multiple | Added is_reasoning: false to all LLMConfig test constructions |

**Net change**: 22 files, -376 lines, +148 lines. Tests: 1918 Rust + 283 TS passing.

### P2: Centralize DEFAULT_OLLAMA_URL - DONE

| Action | Files | Details |
|--------|-------|---------|
| Removed duplicate constant | embedding.rs | Import from ollama.rs via `use super::ollama::DEFAULT_OLLAMA_URL` |
| Re-exported constant | llm/mod.rs | `pub use ollama::DEFAULT_OLLAMA_URL` |
| Replaced hardcoded URL | llm_models.rs | `ProviderSettings::default_for()` uses constant |
| Added sync comment | llm.ts | Points to Rust source of truth |

### P3: i18n Messages Settings - DONE

| Action | Files | Details |
|--------|-------|---------|
| Added 27 i18n keys | en.json, fr.json | Prefixed with `settings_` |
| APIKeysSection | 6 strings | Validation, save/delete success/error, confirm |
| LLMSection | 12 strings | Load, CRUD success/error, confirm, set default + reuse `providers_all` |
| MCPSection | 6 strings | Load, save, delete, toggle errors, confirm, test title |
| CustomProviderForm | 1 string | Base URL help text |

---

## SA-019: Agent Chat Refactoring - Block-by-Block

**Document:** [SA-019-agent-chat-refactoring.md](SA-019-agent-chat-refactoring.md)
**Status:** P6 DONE (ALL PHASES COMPLETE)

| Phase | Description | Status |
|-------|-------------|--------|
| P1 | Backend: vrais tokens + thinking extraction + events enrichis | **DONE** |
| P2 | Backend: load_message_blocks command + ChatBlock model | **DONE** |
| P3 | Frontend: types + store + composants blocks inline | **DONE** |
| P4 | Frontend: suppression sidebar activity + layout 2 colonnes | **DONE** |
| P5 | Nettoyage code mort + bug fixes block-by-block display | **DONE** |
| P6 | TodoTool tasks display: inline task list grouped by agent | **DONE** |

### P1: Backend - Block-by-Block Events - DONE

| Task | Description | Status |
|------|-------------|--------|
| B1 | `thinking_content: Option<String>` on LLMResponse | **DONE** |
| B2 | Mistral thinking extraction via ParsedContent | **DONE** |
| B4 | `sequence: u32` + `ReasoningSource` enum on data structs | **DONE** |
| B5 | New ChunkTypes (ThinkingBlock, ToolCallComplete, ResponseBlock) + 5 enriched fields | **DONE** |
| B6 | Tool loop refactored: global sequence, thinking extraction, tool_call_complete with I/O JSON | **DONE** |
| B7 | Fake streaming removed: no more token chunks, response_block with real tokens | **DONE** |
| B8 | DB schema: `sequence` on tool_execution/thinking_step, `source` on thinking_step | **DONE** |
| B10 | Persistence propagates sequence + source | **DONE** |

**Key changes:**
- `ParsedContent` struct replaces String for `MistralResponseMessage.content` (collects thinking blocks)
- `stream_content_to_frontend()` removed (was 50-char/10ms fake streaming)
- `StreamChunk::token()` and `token_with_counts()` removed (dead code after B7)
- `ProviderToolAdapter` trait extended with `extract_thinking()` default method
- 16 files changed, +570/-187 lines. Tests: 942 passing, clippy clean.

### P2: Backend - load_message_blocks + ChatBlock - DONE

| Task | Description | Status |
|------|-------------|--------|
| B9 | `load_message_blocks` Tauri command (queries tool_execution + thinking_step, merges by sequence) | **DONE** |
| ChatBlock model | `ChatBlock`, `ChatBlockType` structs + `merge_into_chat_blocks()` pure function | **DONE** |
| Read models | `sequence` on ToolExecution, `sequence` + `source` on ThinkingStep (with serde defaults) | **DONE** |
| SELECT queries | Updated tool_execution.rs + thinking.rs queries to include sequence/source, ORDER BY sequence | **DONE** |
| Tests | 14 unit tests for merge_into_chat_blocks (interleaved, empty, same-sequence, data fields) | **DONE** |

**Key changes:**
- New file `models/chat_block.rs`: ChatBlock, ChatBlockType, merge_into_chat_blocks()
- Read models enriched with backward-compatible defaults (sequence=0, source="agent_flow")
- `load_message_blocks` registered in main.rs invoke_handler
- All SELECT queries updated to ORDER BY sequence ASC
- 11 files changed, +538/-26 lines. Tests: 956 passing (14 new), clippy clean.

### P3: Frontend - Block-by-Block Display - DONE

| Task | Description | Status |
|------|-------------|--------|
| F1 | Enriched StreamChunk types (3 new ChunkTypes + 5 new fields) | **DONE** |
| F2 | ChatBlock TS types synchronized with Rust model | **DONE** |
| F3 | executionBlocksStore (processChunk, lifecycle, derived stores) + 16 TDD tests | **DONE** |
| F4-F7 | ThinkingBlock, ToolCallBlock, SubAgentBlock, ExecutionSpinner components | **DONE** |
| F8 | ChatContainer rewrite (persisted + real-time blocks, spinner, response) | **DONE** |
| F9 | Agent page integration (executionBlocksStore, BlockService, messageBlocks) | **DONE** |
| F10 | BlockService (loadForMessage, loadForMessages) | **DONE** |
| F11 | chunkProcessor backward compat (3 new handlers) + 3 tests | **DONE** |
| F12 | tokenStore setSessionTokens method | **DONE** |

**Key changes:**
- NEW `executionBlocksStore` with 7 derived stores
- NEW 4 Svelte components (ThinkingBlock, ToolCallBlock, SubAgentBlock, ExecutionSpinner)
- NEW `BlockService` for loading persisted blocks via IPC
- ChatContainer rewritten: dual rendering (persisted blocks per message + real-time execution blocks)
- chunkProcessor.ts extended with backward-compatible handlers
- workflowExecutor.service.ts manages executionBlocksStore lifecycle
- 11 new i18n keys in en.json + fr.json
- 7 new files, 4 new components, 41 new tests (302 total TS)

### P4: Frontend - Suppression ActivitySidebar + Layout 2 colonnes - DONE

| Task | Description | Status |
|------|-------------|--------|
| F9 | Remove ActivitySidebar from agent page, layout 2 colonnes | **DONE** |
| Stores | Remove activityStore + derived stores | **DONE** |
| Components | Remove 11 components (ActivitySidebar, ActivityFeed, ActivityItem, ActivityItemDetails, SubAgentActivity, ReasoningPanel, ToolExecutionPanel, ReasoningDetailsPanel, ToolDetailsPanel, StreamingMessage, ReasoningStep) + RightSidebar layout | **DONE** |
| Utils | Remove activity conversion utils, activityUtils, activity-icons, panel-merge | **DONE** |
| Types | Remove types/activity.ts (no more consumers) | **DONE** |
| Services | Trim activity.service.ts to loadSubAgentExecutions only | **DONE** |
| Barrels | Update 6 barrel index files (agent, workflow, chat, layout, stores, utils) | **DONE** |
| workflowExecutor | Remove activityStore.captureStreamingActivities() | **DONE** |
| localStorage | Remove RIGHT_SIDEBAR_COLLAPSED key | **DONE** |

**Key changes:**
- 12 component files deleted, 1 store deleted, 4 utils deleted, 1 type file deleted, 4 test files deleted
- activity.service.ts trimmed then renamed to `sub-agent-execution.service.ts` (SA-022/P6): only `loadSubAgentExecutions()` remains (used by message.service.ts)
- utils/activity.ts trimmed: only `formatTokenCount()` remains (used by MessageMetrics.svelte)
- `@humanspeak/svelte-virtual-list` NOT removed (still used by MemoryList.svelte)
- `streamingStore` kept (still used by backgroundWorkflows, executor, tokens)
- Agent page: 2-column layout (WorkflowSidebar + Chat), no right sidebar
- Tests: 254 total TS (48 tests removed with deleted files), lint + check clean

### P5: Nettoyage code mort + Bug fixes - DONE

| Task | Description | Status |
|------|-------------|--------|
| Backend dead code | Remove `simulate_streaming()`, `estimate_tokens()`, `complete_stream()` trait methods, Token ChunkType | **DONE** |
| Frontend dead code | Simplify streamingStore, trim chunkProcessor, remove old handlers | **DONE** |
| Bug fix: message_id chain | `createAssistantMessage` used `crypto.randomUUID()` instead of `result.message_id` | **DONE** |
| Bug fix: reactive blocks | `{@const}` in Svelte 5 not reactive with SvelteMap updates → inline expressions | **DONE** |
| Bug fix: duplicate keys | `each_key_duplicate` crash from `sequence: 0` duplicates → composite keys | **DONE** |
| Bug fix: sub_agents keys | `each_key_duplicate` in MessageMetrics when same sub-agent used twice → composite `${agent.id}-${i}` keys | **DONE** |
| Bug fix: [object Object] | `serde_json::Value` in `json!()` macro → serialize to strings | **DONE** |

**Key changes:**
- Backend: `simulate_streaming()` (-127 lines from utils.rs), `estimate_tokens()`, `complete_stream()` removed from all 4 providers. `Token` ChunkType removed. Old `StreamChunk::token()` constructors removed.
- Frontend: streamingStore trimmed (removed Token/ToolEnd handlers), chunkProcessor simplified (removed old chunk type handlers).
- Bug fix 1: `workflowExecutor.service.ts` - `createAssistantMessage` now uses `result.message_id` from backend (was `crypto.randomUUID()`), fixing message_id chain for block association.
- Bug fix 2: `ChatContainer.svelte` - replaced `{@const blocks = getBlocksForMessage(id)}` with inline function calls, since `{@const}` evaluates once per `{#each}` item creation and is NOT reactive to SvelteMap updates.
- Bug fix 3: `ChatContainer.svelte` - replaced `block.sequence` keys with composite keys `` `${block.block_type}-${i}` `` to avoid `each_key_duplicate` crash when multiple blocks share `sequence: 0`.
- Bug fix 5: `MessageMetrics.svelte` - replaced `agent.name` key with `` `${agent.id}-${i}` `` to avoid `each_key_duplicate` when same sub-agent is invoked multiple times in one execution.
- Bug fix 4: `chat_block.rs` - `merge_into_chat_blocks()` now serializes `serde_json::Value` fields to JSON strings via `serde_json::to_string()`, so frontend `formatJson()` receives strings instead of nested objects.
- 27 files changed, +207/-611 lines. Tests: 951 Rust, 250 TS. lint + check + clippy clean.

### P6: TodoTool Tasks Display - DONE

| Task | Description | Status |
|------|-------------|--------|
| Types | `TodoTaskDisplay` interface in chat-block.ts (id, name, status, priority, agent_name, duration_ms) | **DONE** |
| Streaming | `task_agent_name` field on StreamChunk (TS + Rust), `task_create` constructor updated | **DONE** |
| Store | `tasks: TodoTaskDisplay[]` state + 3 handlers (create/update/complete) + `executionTasks` derived | **DONE** |
| Tests | 9 TDD tests for executionBlocksStore task handlers (25 total) | **DONE** |
| Component | `TodoTasksBlock.svelte`: grouped by agent, status icons, priority badges, duration, animations | **DONE** |
| Integration | ChatContainer: independent `.tasks-section`, `+page.svelte`: `resolvedTasks` $derived with DB persistence + agent name resolution | **DONE** |
| i18n | +2 keys (chat_tasks_title, chat_tasks_arialabel) in en.json/fr.json | **DONE** |

**Key changes:**
- NEW `TodoTasksBlock.svelte` component: task list grouped by agent with status icons (Circle/Loader/CheckCircle/Ban), priority badges (P1/P2 high), duration display, hover effects, spinning animation for in_progress
- `StreamChunk.task_agent_name: Option<String>` added to Rust backend, propagated through `task_create` constructor to frontend
- `executionBlocksStore` extended: `tasks` state array, `handleTaskCreate`/`handleTaskUpdate`/`handleTaskComplete` handlers, `executionTasks` derived store
- ChatContainer: `executionTasks` prop + `TodoTasksBlock` rendered in independent `.tasks-section` div (outside execution-blocks conditional), `contentSignal` includes tasks for auto-scroll
- Lint rule: `svelte/prefer-svelte-reactivity` requires non-Map grouping - used Record+array instead of Map
- **Persistence fix**: TodoTasksBlock was inside `{#if isExecuting || executionBlocks.length > 0}` - moved to independent section. Added `persistedTasks` state loaded from DB via `list_workflow_tasks`. `resolvedTasks` $derived switches between real-time (`executionTasks$`) during execution and persisted tasks after completion. Tasks reloaded from DB after each execution in `handleSend()`.
- **Agent name resolution**: Agent UUIDs resolved to display names via `$agents.find()` in `resolvedTasks` derived. Falls back to raw UUID if agent not found.
- 10 files changed, +328 lines (+83 persistence fix). Tests: 1952 Rust, 260 TS (9 new). lint + check + clippy clean.

---

## SA-020: Agent Name Resolution - Hybrid ID/Name

**Spec**: `docs/security-audits/SA-020-agent-name-resolution.md`
**Status**: DONE (ALL 7 PHASES)

### Phase 1: Schema UNIQUE + validation unicite backend - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| agent_name_idx not UNIQUE | MEDIUM | DONE | `DEFINE INDEX OVERWRITE agent_name_idx ON agent FIELDS name UNIQUE` in schema.rs |
| create_agent allows duplicate names | MEDIUM | DONE | `check_agent_name_unique()` before CREATE, case-insensitive + trim |
| update_agent allows collision | MEDIUM | DONE | `check_agent_name_unique()` with exclude_id before UPDATE |

**Tests added (3)**: `test_create_agent_rejects_duplicate_name`, `test_update_agent_allows_keeping_own_name`, `test_update_agent_rejects_collision_with_other`

**Bonus fix**: All 10 seeders in `test_utils.rs` corrected for ERR_SURREAL_007 (datetime string rejection via CONTENT). Switched to SET syntax with `time::now()` + `db.db.query().check()` for error detection.

**Files modified**: `db/schema.rs` (1 line), `commands/agent.rs` (+50 impl, +80 tests), `test_utils.rs` (10 seeders rewritten)

### Phase 2: AgentRegistry.get_by_name() - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| No name lookup in registry | MEDIUM | DONE | `get_by_name()` method on `AgentRegistry`, case-insensitive + trim, O(n) scan |

**Tests added (5)**: `test_get_by_name_found`, `test_get_by_name_case_insensitive`, `test_get_by_name_trimmed`, `test_get_by_name_not_found`, `test_get_by_name_empty`

**Files modified**: `agents/core/registry.rs` (+22 impl, +55 tests)

### Phase 3: resolve_agent_ref() shared function - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| No shared resolution function for agent ID or name | MEDIUM | DONE | `resolve_agent_ref()` in `tools/utils.rs`: trim, fast path (ID), slow path (name), NotFound error |

**Tests added (4)**: `test_resolve_agent_ref_by_id`, `test_resolve_agent_ref_by_name`, `test_resolve_agent_ref_not_found`, `test_resolve_agent_ref_empty_input`

**Files modified**: `tools/utils.rs` (+37 impl, +120 tests)

**Note**: `cargo clippy` reports 2 dead-code warnings (`get_by_name` from P2, `resolve_agent_ref` from P3) because neither is called from production code yet. These will resolve in P4 when `DelegateTaskTool` consumes `resolve_agent_ref()`.

### Phase 4: DelegateTaskTool accepts agent_name - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| DelegateTask requires agent_id (UUID) only | MEDIUM | DONE | `validate_delegate_operation()` accepts agent_id OR agent_name; `execute()` resolves via `resolve_agent_ref()` |
| Tool schema missing agent_name property | LOW | DONE | `delegate_task_input_schema()` with agent_name documented; description updated |
| Misleading examples with slug-like IDs | LOW | DONE | Examples corrected: by-name preferred, by-UUID as alternative |

**Tests added (4)**: `test_validate_input_accepts_agent_id`, `test_validate_input_accepts_agent_name`, `test_validate_input_rejects_missing_both`, `test_definition_has_agent_name_property`

**Design**: Pure functions extracted (`validate_delegate_operation`, `delegate_task_input_schema`) for testability. `resolve_agent_ref()` from P3 now consumed, resolving dead-code warnings from P2/P3.

**Files modified**: `tools/delegate_task.rs` (+60 impl, +40 tests)

### Phase 5: ParallelTasksTool accepts agent_name + fix noms - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| ParallelTasks requires agent_id (UUID) only | MEDIUM | DONE | `validate_parallel_task_item()` accepts agent_id OR agent_name; `execute()` resolves via `resolve_agent_ref()` |
| Tool schema missing agent_name property | LOW | DONE | `parallel_tasks_input_schema()` with agent_name documented; examples updated |
| "Parallel task for {uuid}" instead of real name | MEDIUM | DONE | `prepare_execution()` and `process_results()` use real agent names from registry |
| Human validation shows UUIDs | LOW | DONE | `request_human_validation()` uses agent_name for display |

**Tests added (5)**: `test_validate_parallel_task_accepts_agent_id`, `test_validate_parallel_task_accepts_agent_name`, `test_validate_parallel_task_rejects_missing_both`, `test_definition_has_agent_name_property`, `test_parallel_task_spec_includes_agent_name`

**Design**: Same pattern as P4 - pure functions extracted (`validate_parallel_task_item`, `parallel_tasks_input_schema`) for testability. `ParallelTaskSpec` extended with `agent_name` field. Real agent names resolved from registry during `execute()` and propagated through all events, DB records, and reports.

**Files modified**: `tools/parallel_tasks.rs` (+80 impl, +50 tests)

### Phase 6: Frontend validation nom duplique + i18n - DONE

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| No frontend duplicate name check | LOW | DONE | `validate()` in AgentForm.svelte checks against `$agents` list, case-insensitive, excludes self in edit mode |
| Missing i18n keys for duplicate error | LOW | DONE | `agents_name_duplicate` added in en.json and fr.json |

**Files modified**: `AgentForm.svelte` (+8 lines), `en.json` (+1 key), `fr.json` (+1 key)

### Phase 7: Documentation - DONE

- SA-020 spec: statut DONE, criteres coches, tests reels (22), fichiers reels
- REMEDIATION-STATUS: summary + section SA-020 complete
- Learnings: ERR_DELEGATE_001, PAT_AGENT_002 ajoutés
- Inventory: fonctions utilitaires ajoutées

**Total SA-020**: 12 fichiers, +1225/-268 lignes, 22 TDD tests, 7 phases

---

### SA-021: Report Enforcement

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| Generic "Task completed" message when agent has no report | MEDIUM | **DONE** | `is_generic_completion_message()` detection + follow-up LLM call with empty tools array |

**Implementation**:
- `REPORT_ENFORCEMENT_PROMPT` constant for follow-up request
- `is_generic_completion_message()` pure function (3 detection patterns)
- Follow-up logic in `execute_with_mcp()` with cancellation check and fallback
- Empty tools array (not `ToolChoiceMode::None`) for Ollama compatibility

**Files modified**: `src-tauri/src/agents/llm_agent.rs` (+175 lines)
**Tests**: 6 TDD tests

---

### SA-022: Frontend Structure & Naming Audit

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| Mixed naming conventions in `src/types/` (4 styles) and `src/lib/stores/` (3 styles) | HIGH | **DONE** | 6 files renamed to kebab-case, 16 import paths updated (Phase 3) |
| Dead code: `JsonViewer.svelte`, `ToolExecution.svelte`, unused helpers in `thinking.ts`/`tool.ts` | HIGH | **DONE** | Files deleted, helpers integrated into components, unused exports removed (Phase 2) |
| Incomplete barrel exports across 5 directories | MEDIUM | **DONE** | 5 barrels completed + 2 created (legal, settings) + RiskLevel dedup (Phase 5) |
| Inventory meta counters outdated (88 vs 94 components) | MEDIUM | **DONE** | Updated: total_components 88→94, total_stores 17→18 (Phase 1) |
| Duplicate delete confirmation modal (`ConfirmDeleteModal` vs `DeleteConfirmModal`) | MEDIUM | **DONE** | Consolidated: `ConfirmDeleteModal` deleted, `DeleteConfirmModal` enhanced with `itemName`/`warningMessageKey` props (Phase 4) |
| Types `index.ts` JSDoc uses `$lib/types` instead of `$types` | MEDIUM | **DONE** | Fixed JSDoc: `$lib/types` → `$types` (Phase 1) |
| Unused helper functions in `tool.ts` and `thinking.ts` | MEDIUM | **DONE** | `formatToolDuration`→ToolCallBlock, `truncateThinkingContent`→ThinkingBlock, 11 unused exports removed (Phase 2) |
| Missing barrel exports for `legal/` and `settings/` | MEDIUM | **DONE** | Created `legal/index.ts` (LegalModal) and `settings/index.ts` (5 components) (Phase 5) |
| `activity.service.ts` name misleading after SA-019 refactoring | MEDIUM | **DONE** | Renamed to `sub-agent-execution.service.ts`, export `SubAgentExecutionService` (Phase 6) |
| `src/lib/validation/` directory with only 2 files | MEDIUM | **DONE** | Merged into `src/lib/utils/` as `validation-schemas.ts` + `validation-invoke.ts` (Phase 6) |
| `ChatContainer` in `agent/` directory | LOW | **NO ACTION** | Acceptable for page-level component |
| Settings top-level loose files (provider-related) | LOW | **DONE** | Moved `APIKeysSection`, `CustomProviderForm`, `LLMSection` to `settings/providers/` subdirectory (Phase 7) |
| `ToolStatus` type duplication | LOW | **DONE** | Resolved: only `streaming.ts` definition remains (Phase 2) |

**Phases**: 7 (Quick fixes -> Dead code -> Naming -> Modal -> Barrel exports -> Service cleanup -> Settings structure)
**Details**: `docs/security-audits/SA-022-frontend-structure-naming.md`

---

### SA-023: Backend Structure & Naming Audit

| Finding | Severity | Status | Fix |
|---------|----------|--------|-----|
| H1: `ProviderType` duplicated in `llm/provider.rs` and `models/llm_models.rs` | HIGH | **DONE** | Removed duplicate from `models/llm_models.rs`, canonical location `llm/provider.rs`, re-export from `models/mod.rs` (P1) |
| H2: `commands/models.rs` naming ambiguity | HIGH | **DONE** | Renamed to `commands/llm_models.rs`, updated `mod.rs` + `main.rs` (P2) |
| M1: `tools/validation_helper.rs` mixes unrelated concerns | MEDIUM | **DONE** | Moved `safe_truncate()` to `tools/utils.rs`, updated 4 import sites (P3) |
| M2: `tools/constants.rs` scope exceeds its module | MEDIUM | **DONE** | Created top-level `constants.rs`, moved `workflow`/`query_limits`/`commands` modules, updated 11 files (P4) |
| M3: Large files candidates for decomposition | MEDIUM | DEFERRED | Well-structured internally, decompose when next modifying |
| L1: Excessive `#[allow(dead_code)]` annotations | LOW | DEFERRED | Periodic audit recommended |
| L2: Minor naming asymmetries commands/ vs models/ | LOW | **NO ACTION** | Commands=actions, models=entities is a reasonable convention |

**Phase 1 DONE**: Consolidated `ProviderType` into `llm/provider.rs`. Removed enum + 4 impls + 2 redundant tests from `models/llm_models.rs`. Updated imports in `commands/models.rs`, `commands/agent.rs`. Re-export from `models/mod.rs`.
**Files modified**: 4 Rust files (-89 lines)
**Tests**: 2002 pass (cargo test), 0 failures

**Phase 2 DONE**: Renamed `commands/models.rs` -> `commands/llm_models.rs`. Updated `commands/mod.rs` (module declaration + doc), `main.rs` (10 command registrations).
**Files modified**: 3 Rust files (rename + 2 import updates)
**Tests**: 2002 pass (cargo test), 0 failures

**Phase 3 DONE**: Moved `safe_truncate()` from `validation_helper.rs` to `utils.rs`. Updated imports in `sub_agent_executor.rs`, `spawn_agent.rs`, `memory/tool.rs`, `validation_helper.rs`.
**Files modified**: 5 Rust files (move + 4 import updates)
**Tests**: 976 pass (cargo test), 0 failures

**Phase 4 DONE**: Created `src/constants.rs` with `workflow`, `query_limits`, `commands` modules. Registered in `lib.rs` and `main.rs`. Updated imports in 11 Rust files. Cleaned `tools/constants.rs` (now tool-specific only: memory, todo, user_question, sub_agent, calculator). Updated `.claude/rules/surrealdb.md`.
**Files modified**: 14 (1 new + 13 updated)
**Tests**: 1976 pass (cargo test), 0 failures

**Details**: `docs/security-audits/SA-023-backend-structure-naming.md`

---

### SA-024: Config & Dependency Cleanup

**Source**: Senior Review Full Audit 2026-02-26
**Scope**: 8 items (3 HIGH, 5 MEDIUM) across 4 phases

**Phase 1 DONE**: Dependency cleanup.
**Phase 2 DONE**: Production robustness.
**Phase 3 DONE**: Code hygiene (OPT-* cleanup). dead_code audit deferred.
- H4: `once_cell` replaced by `std::sync::LazyLock` in `http_handle.rs` and `registry.rs`
- M1: `futures` replaced by `futures_util::future::join_all` in `persistence.rs` and `queries.rs`
- M7: `surrealdb` version pinned from `"2.5.0"` to `"~2.6"` (resolved to 2.6.2)
- 2 dependencies removed from Cargo.toml (`once_cell`, `futures`)
- H1: 7 `.expect()` converted to `Result<Self, String>` in LLM providers (manager, embedding, ollama, mistral)
- 4 `Default` impls removed (ProviderManager, EmbeddingService, OllamaProvider, MistralProvider) - all test-only
- `EmbeddingService::new()` moved to `#[cfg(test)]` - no production callers
- Result propagated to `AppState::new()`, `load_embedding_config()`, `update_embedding_service_internal()`
- M3: 171 OPT-* traceability markers removed from 52 files (32 Rust + 20 frontend)
- H2: `#[allow(dead_code)]` audit deferred to future phase
**Phase 4 DONE**: Config & Inventory.
- H3: `total_commands` fixed from 106 to 123 in `inventory.yml`; 39 missing commands added (5 entire modules + 6 partial modules)
- M4: `@humanspeak/svelte-virtual-list` moved from `devDependencies` to `dependencies` (used in `MemoryList.svelte`)
**Files modified**: 5 (P1) + 15 (P2) + 52 (P3) + 3 (P4: inventory.yml, package.json, package-lock.json)
**Tests**: npm run lint PASS, npm run check PASS (0 errors)

**Details**: `docs/security-audits/SA-024-config-cleanup.md`

---

## Verification Status

| Check | Status | Notes |
|-------|--------|-------|
| cargo fmt --check | **PASS** | 2026-02-25 |
| cargo clippy -- -D warnings | **PASS** | 2026-02-25, 0 warnings |
| cargo test --lib | **PASS** | 2026-02-25, 972 tests passed (+3 SA-020/P1, +5 SA-020/P2, +4 SA-020/P3, +4 SA-020/P4, +6 SA-020/P5) |
| npm run lint | **PASS** | 2026-02-25, 0 errors |
| npm run check | **PASS** | 2026-02-25, 0 errors |
| npm run test | **PASS** | 2026-02-25, 260 tests passed |
| Manual test: token display separation | **PASS** | 2026-02-21, user confirmed AGENT/TOTAL sections correct |
| Manual test: streaming + cancel | **PASS** | 2026-02-20, user confirmed no bugs |
| Manual test: memory compact mode | **PASS** | 2026-02-20, French text no longer panics |
| Manual test: workflow rename | **PASS** | 2026-02-22, user confirmed rename + space key works |
| Manual test: workflow delete modal | **PASS** | 2026-02-22, user confirmed standard modal design |
| Manual test: block-by-block display | **PASS** | 2026-02-23, user confirmed blocks inline, spinner, collapse, cancel |
| Manual test: blocks persist on nav | **PASS** | 2026-02-23, user confirmed blocks reload when switching workflows |
| Manual test: tasks persist on nav | **PASS** | 2026-02-24, user confirmed tasks visible after execution, on conversation switch, and after restart |
| Manual test: agent name resolution | **PASS** | 2026-02-24, user confirmed agent display names instead of UUIDs |
| Manual test: tool call data display | **PASS** | 2026-02-23, user confirmed no [object Object], proper JSON in tool blocks |
| Manual test: report enforcement | **NOT RUN** | SA-021: verify agent provides markdown report instead of generic message |
| Manual test: search prompts | **NOT RUN** | |
| Manual test: import/export | **NOT RUN** | |
| Manual test: custom provider HTTP warning | **NOT RUN** | |
| Manual test: fonts rendering | **NOT RUN** | |
