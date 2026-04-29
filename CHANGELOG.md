# Changelog

All notable changes to Zileo Chat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

---

## [0.21.0] - 2026-04-29

### Added

- **MCP HTTP authentication (v1.2)**: First-class HTTP auth for remote MCP servers -- methods `Bearer`, `API Key` (custom header, default `X-API-Key`) and `Basic`, plus a cumulative "Extra HTTP headers" list. Secrets are persisted in the OS keychain (per-server namespace `mcp_auth_<id>`) and never written to the database, logs, or export bundles. Symmetric TS+Rust validation (length limits, `\r\n` rejection, header-name regex `^[A-Za-z0-9_-]+$`), redacted debug logging, and full i18n coverage (`mcp_auth_*`, EN+FR)
- **Database schema migration (`mcp_auth_v1`)**: Adds `auth_type`, `auth_metadata`, `extra_headers` columns on the `mcp_server` table (idempotent `DEFINE FIELD OVERWRITE`)
- **Legacy migration assistance UI**: Settings > MCP banner that lists HTTP servers still relying on `API_KEY` / `HEADER_*` env vars; the form exposes one-click actions to convert the legacy value into the new Bearer or `X-API-Key` field
- **Import/Export schema v1.2**: `EXPORT_SCHEMA_VERSION` bumped to `"1.2"` (still accepts `1.0` and `1.1`). Adds `authType`, `authMetadata`, `extraHeaderKeys` on MCP summaries, `clearAuthMetadata` / `clearExtraHeaders` checkboxes in `MCPFieldEditor`, a `Secret required` badge in `ImportPreview`, and a structured `McpSecretMissing` warning + post-import action per HTTP MCP server with active auth
- **Provider-aware reasoning options (`$lib/utils/agent-reasoning.ts`)**: New utility module (with tests) that exposes the supported `reasoning_effort` values per provider and normalizes the agent's stored value when switching providers (auto-promotes `low` / `medium` to `high` for Mistral so the user's intent survives the reduced option set)

### Changed

- **BREAKING -- HTTP MCP auth**: HTTP servers no longer interpret the legacy `API_KEY` / `HEADER_*` env vars at runtime. Existing servers must be migrated to the new auth fields via the in-app banner; the legacy env values are preserved for one-click migration but ignored by the HTTP transport
- **`create_mcp_server` / `update_mcp_server` IPC payload**: Now accepts `MCPServerConfigWithSecret` (`MCPServerConfig` + optional `authSecret`). Read commands never echo the secret back
- **`AgentForm` reasoning effort**: Now provider-aware -- shows `Off` / `High` only when the agent's provider is Mistral (with help text), full `Off` / `Low` / `Medium` / `High` for OpenAI-compatible providers
- **Agent snapshot hydration**: `hydrate_llm_from_model` is now called on agent create/update so `is_reasoning` and `context_window` are re-read from the `llm_model` row. Toggling reasoning on a model card now propagates to existing agents on the next save (user-editable `temperature` and `max_tokens` are left untouched)

### Fixed

- **Mistral thinking not displayed**: `extract_thinking` only walked the content-blocks array, so variants surfacing thinking at `message.reasoning` / `reasoning_content` / top-level `message.thinking` returned `None` on `mistral-medium-3.5` (and OpenRouter-relayed Mistral). Now delegates to `llm::utils::extract_thinking_from_message` which covers all six known shapes; diagnostic log when nothing surfaces despite `is_reasoning=true`
- **Mistral assistant-message replay rejected (`extra_forbidden`)**: `build_assistant_message` echoed the raw response, including `ThinkChunk` fields (`signature`, `closed`) which Mistral rejects on input. Now flattens content to the visible text and preserves only `role` + `tool_calls` (drops thinking blocks entirely, including the empty-content / tool_calls-only case)
- **Mistral `reasoning_effort` low/medium errored**: Mistral only accepts `high` or no field; sending `low` / `medium` errored. New `ReasoningEffort::to_mistral_str` maps `Low` / `Medium` to `high` so any explicit level means "reasoning enabled"; `off` is `None` (no field). Used by `build_mistral_tool_request` and the chat path. OpenAI-compatible providers (OpenRouter, vLLM, ...) still get the full `low/medium/high` mapping via `from_params`

### Documentation

- Synced `API_REFERENCE`, `ARCHITECTURE_DECISIONS`, `DATABASE_SCHEMA`, `FRONTEND_SPECIFICATIONS`, `GETTING_STARTED` with the v1.2 schema, MCP auth fields, `mcp_auth_v1` migration, provider-aware reasoning utility, and updated component / utils tables (utils 14 -> 16, components 102 -> 103)

---

## [0.20.1] - 2026-04-26

### Fixed

- **Multi-turn conversation continuation**: Two compounding bugs broke conversation history end-to-end. (1) `load_conversation_history` only injected `conversation_messages` when a `system` row existed in the DB, but `system` rows are persisted by the frontend `catch{}` branch as error notifications -- never as real system prompts -- so regular continuations had no memory. (2) `execute_with_tools` then re-appended `task.description` on top of an already-complete history (the frontend persists the user turn before streaming), duplicating the last user message, while reusing a stale system prompt from the first turn. Fix: trigger continuation on any non-empty history, filter `system` rows at the SQL level, extract `build_initial_messages`, regenerate the system prompt every turn against live agent config (tools, MCP servers, locale, current date), and replay persisted history as-is without re-appending the description
- **Tokio runtime-in-runtime panic on app exit (#110)**: `RunEvent::ExitRequested` ran `tauri::async_runtime::block_on` from inside the `#[tokio::main]` runtime, panicking on shutdown. Replaced with `api.prevent_exit()` + `tauri::async_runtime::spawn` + `AtomicBool` re-entry guard so MCP shutdown completes asynchronously before `app_handle.exit(0)`

### Changed

- **Typed `ToolDescriptionBuilder`**: Replaces the previous string-concatenation pattern across all local tools with a typed builder (`Tool::id()` + structured sections), making tool descriptions consistent and reducing boilerplate
- **MCP server summaries**: First-sentence extraction normalized so MCP tools surface a single-line `summary` field in the system prompt (no description duplication)
- **Dropped ~1.4k LOC of dead code (#109)**: Removed unused functions, structs, and enum variants flagged during the senior review; wired MCP shutdown on app exit (the change that introduced the tokio panic later fixed in #110)

### Documentation

- Synced `AGENT_TOOLS_DOCUMENTATION`, `API_REFERENCE`, `MULTI_AGENT_ARCHITECTURE`, `WORKFLOW_ORCHESTRATION`, `README`, and `TECH_STACK` with the new conversation flow (`build_initial_messages`, system rows filtering), expanded TodoTool/MemoryTool operation lists, and current dependency versions (vite 7.3.2, dompurify 3.4.1)

---

## [0.20.0] - 2026-04-25

### Added

- **Validation timeout & timeout behavior**: New `validation.timeoutSeconds` and `validation.timeoutBehavior` settings (`auto-approve` / `auto-reject`) on validation rules — backend enforces server-side, frontend exposes the configuration in Settings > Validation
- **Validation audit log (backend)**: `validation_audit` SurrealDB table + write helper + 4 Tauri commands (`list_validation_audit`, `get_validation_audit_stats`, `purge_validation_audit`, `export_validation_audit_csv`)
- **Settings > Audit Log page**: New `/settings/audit-log` route with list view, filters (date range, status, tool name, agent), stats panel, CSV export, and purge action — backed by `audit-log.ts` store
- **Cancellation propagation**: `oncancel` now propagates from UI down through the agent loop into LLM HTTP calls (cancellable retries)
- **Domain error enums via thiserror**: `AgentError` and `CommandError` enums replace ad-hoc `String` errors in agent and command layers, structured `Display` impls preserved for UI
- **Shared tool chat request body + POST helper**: Factored common LLM POST request building (auth headers, body shape) across providers (`llm/tool_format.rs`)
- **Centralized timeout constants**: `validation::DEFAULT_TIMEOUT_SECONDS`, LLM HTTP timeout constants (`llm::http::*`) — single source of truth across modules

### Changed

- **`tool_loop.rs` modular split**: Extracted into `reasoning/`, `completion/`, `iteration/` submodules (matches the existing modular architecture refactor pattern from v0.19.0)
- **`streaming/execution.rs` modular split**: Split into 4 focused modules for clarity and testability
- **Pipeline robustness (phase 2.2-2.6)**: Concurrency hardening, sequencing guarantees, security checks (sanitize_for_surrealdb on audit writes), explicit flush on shutdown
- **Production `.unwrap()` removal**: Last 6 production-path `.unwrap()` replaced with documented `.expect("invariant: ...")` carrying the upheld invariant
- **`ValidationAuditEntry` snake_case → camelCase remapping**: DB rows now correctly remap on read (was leaking snake_case fields into the frontend)
- **Documentation sync**: docs harmonized with `validation_audit` backend (FRONTEND/BACKEND_SPECIFICATIONS, TECH_STACK), stale references cleaned

### Fixed

- **Approval modal stuck on backend timeout**: Modal now closes when the backend times out a pending validation (was leaving the user blocked)
- **Audit log row mapping**: snake_case DB columns no longer leak into `ValidationAuditEntry` (HIGH from senior review)
- **Senior review fixes (HIGH/MEDIUM/LOW)**: Multiple fixes across audit log frontend (filter handling, store derivations, accessibility, loading states)

### Security

- **dompurify** bumped to `^3.4.1` (was `^3.3.1`) — fixes 8 advisories including mutation-XSS via re-contextualization, prototype pollution, ADD_TAGS/ADD_ATTR bypasses, SAFE_FOR_TEMPLATES bypass (used at runtime in `MarkdownRenderer.svelte` and `url.ts`)
- **vite** bumped to `^7.3.2` (was `^7.3.1`) — fixes 3 path-traversal / dev-server file-read advisories (build/dev only, not shipped in the Tauri binary)
- **rollup** transitive bumped to `4.60.2` — fixes arbitrary file write via path traversal (build only)
- **picomatch** transitive bumped to `4.0.4` — fixes ReDoS + method injection in POSIX character classes (build only)
- **postcss** transitive bumped to `8.5.10` — fixes XSS via unescaped `</style>` (build only)
- Remaining advisories (ajv, brace-expansion, cookie, flatted, minimatch, yaml) are transitive deps of ESLint / @sveltejs/kit (adapter-static, no SSR) / postcss-load-config — no runtime surface on Tauri desktop, will be picked up by future major upgrades

### Documentation

- **Specifications updated**: `docs/BACKEND_SPECIFICATIONS.md` and `docs/FRONTEND_SPECIFICATIONS.md` synchronized with `validation_audit` table, audit log commands, audit log store + page, validation timeout settings
- **Stale references harmonized**: Cleaned references to renamed/split modules across the docs tree

---

## [0.19.1] - 2026-04-24

### Added

- **`$lib/utils/uuid.ts`**: Canonical 8-4-4-4-12 hex UUID validator (`isUuid()`) shared across stores and components
- **`$lib/utils/constants.ts`**: `ITERATIONS_LIMITS` frontend constants synchronized with backend clamping (max iterations / max parallel)
- **`$lib/utils/settings-refresh.ts`**: `onSettingsRefresh()` / `attachSettingsRefreshListener()` / `SETTINGS_REFRESH_EVENT` helpers to subscribe to the global settings refresh event after import/export
- **`$lib/actions/focusTrap.ts`**: Svelte 5 `{@attach}` action for WCAG 2.1 modal keyboard focus trap with Tab cycling and focus restoration on teardown

### Changed

- **Toast vs ErrorBanner unification**: Single toast layer for transient errors, ErrorBanner reserved for persistent UI errors
- **SvelteKit redirects**: Migrated route guards to `+page.ts` redirects (instead of imperative `goto()` in components)
- **Modal accessibility**: `Modal.svelte` now uses `focusTrap` action (replaces ad-hoc keydown handlers)
- **Chat block ordering**: Chronological merge by `created_at` across primary/sub-agent blocks (round 3, MEDIUM-4)
- **Block keys**: Stable `block.sequence` keys for `{#each}` lists (round 3, MEDIUM-1 + LOW-5)
- **Agent name resolution**: New `resolveAgentName()` helper backed by `isUuid()` (round 3, MEDIUM-3)
- **Duration formatting**: Single `formatDuration()` utility now handles ms / s / m,s formats (round 3, LOW-3)
- **Iteration limits**: Both frontend and backend reference the shared `ITERATIONS_LIMITS` constant (round 3, LOW-6)
- **Modal accent**: `UserQuestionModal` aligned with `accent-color` design token (round 3, LOW-1)
- **Dependencies aligned**: `@tauri-apps/plugin-dialog` JS package upgraded to `^2.7.0` to match the Rust crate

### Fixed

- **HIGH `--color-warning-light` illegibility**: Replaced low-contrast token usages with WCAG-compliant variants (round 2)
- **Design tokens sweep**: Removed remaining hardcoded colors in favor of CSS custom properties (round 2)
- **Agent restore guard**: Prevents restoring a deleted/missing agent into the active selection (round 2)
- **Legacy `executionResponse` bubble**: Dropped duplicate response rendering path (round 3, MEDIUM-2)
- **`task_id` guard**: Skip block emission when no `task_id` is associated (round 3, MEDIUM-5)
- **Chat `--color-danger` scope**: Scoped to chat surface to avoid bleeding into other components (round 3, LOW-2)
- **Thinking content slicing**: New `truncateThinkingContent()` for safe slicing (round 3, LOW-4)
- **Orphan root `+page.svelte`**: Removed unused root page component

### Documentation

- **`docs/FRONTEND_SPECIFICATIONS.md`**: Added new `utils/uuid`, `utils/constants`, `utils/settings-refresh` modules and the `actions/focusTrap` section
- **`docs/TECH_STACK.md`**: Refreshed dependency versions (Svelte 5.55, SvelteKit 2.55, rig-core 0.34, tokio 1.51, plugins)

---

## [0.19.0] - 2026-03-31

### Changed

- **Massive code quality refactoring**: 332 files, +32k/-45k lines across 38 commits
  - Split monolithic Rust modules into modular architecture: `llm_agent.rs`, `file_manager/tool.rs`, `sub_agent_executor.rs`, `memory/tool.rs`, `commands/agent.rs`, LLM providers, MCP, Tools, Commands scopes
  - Dead code removal: models (-702 lines), security/state (-316 lines), test-only methods moved to `#[cfg(test)]`
  - Senior review fixes across all layers: components, stores, services, utils, routes, types, CSS
- **ToolDefinition summary/description split**: `summary` for system prompt (1-line), `description` for API tools parameter (structured). Reduces system prompt token usage (-191 lines)
- **Parallel startup**: `tokio::join!` for MCP + providers + embedding init, `join_all` for MCP server connections
- **ChatInput**: `oncancel` prop + integrated stop button (removed ChatContainer wrapper)
- **FloatingMenu**: Direct `$theme` store access, removed `$state`+`subscribe` pattern

### Added

- **Import/Export v1.1**: Skills + custom providers + agent fields + cross-dependency validation + i18n warnings
- **Task Bridge**: TodoTool primary/sub-agent scoping + `task_ids` in DelegateTask/ParallelTasks
- **Sub-agent message correlation**: `parent_message_id`, `load_message_blocks` backend integration
- **StreamChunk enrichment**: `tool_type`/`server_name` for MCP tool identification in blocks
- **Custom provider thinking extraction**: 6 formats (reasoning, reasoning_content, reasoning_details[], message.thinking, `<think>` tags, content blocks array)

### Fixed

- **Ollama provider**: Removed rig dependency, direct HTTP, real token counts, `tool_call_id` correlation
- **Sub-agent model config**: Resolution from DB on provider/model override
- **Thinking step sequence**: Fixed duplicate emission in tool loop
- **SubAgentBlock dedup**: Via `_sub_agent_id` in execution-blocks.ts

---

## [0.18.0] - 2026-03-22

### Added

- **Mistral reasoning support**: `reasoning_effort` parameter now sent to Mistral API for both chat and tool-call paths (previously silently dropped)
- **Dual-format thinking blocks**: Mistral deserializer handles both array format (Magistral) and string format (mistral-small with reasoning_effort)
- **Thinking display in simple path**: `execute()` (no-tools path) now emits `StreamChunk::thinking_block` so reasoning content is visible in UI
- **3 new Mistral tests**: reasoning_effort serialization (2 tests) + string-format thinking deserialization

### Changed

- **Unified `complete_with_tools()` signatures**: All 3 providers (Mistral, Ollama, OpenAI-compatible) now accept `&ToolCompletionParams` instead of individual positional parameters
- **`ToolCompletionParams`**: Added `reasoning_effort` field for providers that support thinking + tool calling simultaneously
- **`MistralToolChatRequest`** / **`ToolChatRequest`**: Added `reasoning_effort` field to HTTP request body
- **Manager `complete_with_tools()`**: Simplified from ~75 lines of destructuring to ~45 lines of uniform `prov.complete_with_tools(&p)` calls
- **`context_window`**: Now traced in debug logs for all providers (was Ollama-only)

### Fixed

- **Mistral `extract_content()` with reasoning format**: Content returned as array of blocks (thinking + text) was not parsed, causing "Task completed" fallback instead of actual response
- **Mistral `reasoning_effort` in tool-call loop**: `ToolCompletionParams` was missing the field, so Mistral never received it during tool iterations

---

## [0.17.0] - 2026-03-22

### Added

- **Sidebar Improvements**: Complete overhaul of the workflow sidebar (6 phases)
  - Phase 1: Sidebar collapsed state persistence to localStorage, status filters (All/Idle/Running/Completed/Error)
  - Phase 2: Right-click context menu on workflow items (rename, delete, pin, move to folder)
  - Phase 3: Multi-selection mode with Shift+Click range selection and batch delete (skips running workflows)
  - Phase 4: Workflow folders with color-coded labels, move-to-folder support, pin/unpin workflows
  - Phase 5: Pinned workflows section, wired folder/pin handlers to backend, query deduplication
  - Phase 6: Drag & drop workflows into folders with multi-select drag support and drop zones
- **`withToastError` utility**: Higher-order function to wrap async handlers with toast error notifications, replacing 7 repetitive try/catch blocks
- **`async.test.ts`**: 5 unit tests for the new `withToastError` utility

### Changed

- **Sidebar header layout**: Title + create button on first line, secondary actions (help, folder, selection) on second line
- **Sidebar collapse toggle**: Moved from invisible edge-positioned button to visible footer button with accent color
- **Batch delete optimization**: Replaced N+1 status queries with single `IN` query
- **Workflow query fields**: Deduplicated with shared `FIELDS` constant in `queries.rs`

### Fixed

- **Context menu move-to-folder**: Now lists folders individually instead of as a group
- **Pinned field backfill**: Existing workflows with `NONE` pinned value are backfilled to `false` at startup
- **Vite ENOSPC**: Excluded `src-tauri/target` from file watcher to prevent ENOSPC errors on Linux

---

## [0.16.0] - 2026-03-21

### Added

- **Multi-breakpoint Prompt Cache Optimization**: Intelligent cache breakpoint placement for LLM requests
  - Multi-breakpoint strategy with system prompt, conversation history, and tool results
  - Cache hit rate display in TokenDisplay UI component
  - Per-iteration cost tracking with cache read/write pricing
- **MCP HTTP Request Throttling**: 500ms minimum delay between HTTP requests to MCP servers to prevent rate limiting

### Fixed

- **Context Bar**: Shows actual context window size instead of cumulative sum across iterations
- **Code Cleanup**: Removed parasitic SA-xxx audit reference comments from codebase

---

## [0.15.1] - 2026-03-05

### Fixed

- **ProviderType case mismatch**: Models created for Ollama/Mistral were stored with capitalized provider name ("Mistral"/"Ollama") due to using `Display` trait instead of `Serialize` for DB storage. This caused:
  - Provider filter in Settings→Models not showing user-created models
  - Agent form (Settings→Agents) not listing models when selecting Mistral/Ollama
  - TokenDisplay showing `0/128000` instead of actual model context window and pricing
  - `fetchModelByApiName` silent failures affecting temperature, reasoning, and token data
- **Cache pricing fields missing**: `get_model` and `get_model_by_api_name` queries were missing `cache_read_price_per_mtok` and `cache_write_price_per_mtok` fields, causing cache cost calculations to always return 0

## [0.15.0] - 2026-03-04

### Added

- **Reasoning Effort** (#65): Granular thinking control for LLM agents
  - New `ReasoningEffort` enum (low/medium/high) replacing boolean `enable_thinking`
  - DB migration: `enable_thinking` -> `reasoning_effort` with ASSERT validation
  - `LLMProvider` trait updated with `reasoning_effort: Option<ReasoningEffort>` on all 3 providers
  - `thinking_tokens` field added to Message, StreamChunk, and metrics
  - `extract_thinking_from_message()` utility for response parsing
  - Agent form: reasoning effort dropdown (conditional on `is_reasoning` model flag)
  - MessageMetrics: BrainCircuit icon with thinking token count
  - Design decision: reasoning_effort intentionally not passed during tool-loop iterations

### Changed

- **Dead Code Cleanup**: Removed `#[allow(dead_code)]` annotations from production code
  - Removed 2 unused methods (`with_retry_config`, `has_custom_provider`) from `ProviderManager`
  - Moved 5 test-only methods to `#[cfg(test)]` impl block
  - Removed incorrect `#[allow(dead_code)]` on `http_client` field/accessor (actually used)
- **Agent Deserialization**: Replaced ~70 lines of manual `unwrap_or` deserialization with `serde_json::from_value()` leveraging serde defaults on `AgentConfig`, `LLMConfig`, and `Lifecycle`
- **Dead Code Removal**: Removed unused command module (`llm.rs`), unused TS type files (`fileManager.ts`, `function-calling.ts`, `security.ts`, `task.ts`), and dead `execute_workflow`/`test_llm_completion`/`ProviderManager::complete()` methods

### Fixed

- **Pipeline Cleanup**: Net reduction of ~860 lines of dead/redundant code

---

## [0.14.0] - 2026-03-03

### Added

- **Prompt Caching Metrics**: Full prompt caching support with cost tracking
  - `cache_control` injection on system messages for Anthropic-compatible providers (`apply_prompt_cache_control`)
  - `TokenUsage` struct replacing tuple returns from `extract_usage()` across all LLM adapters (OpenAI, Mistral, Ollama)
  - `IterationMetrics` struct for per-API-call metrics (tokens, cost, duration, cache hits)
  - 3-tier input pricing: regular, cache-read, cache-write with `calculate_cost_with_cache()` in `pricing.rs`
  - Cache pricing fields on model schema (`cache_read_price_per_mtok`, `cache_write_price_per_mtok`)
  - Cache token display in `TokenDisplay` and `MetricsBar` components
  - Model form fields for cache pricing configuration
  - 13 pricing tests covering all cache scenarios (free reads, 50% reads, 1.25x writes, overflow clamping)
- **FileManagerTool** (#63): Sandboxed filesystem operations for LLM agents
  - 10 operations: list, read, write, replace, create, delete, move, rename, search_glob, search_content
  - Per-agent folder sandboxing with 6-layer path validation
  - Trash-based safe deletion with timestamped backups (30-day retention, 100MB cap)
  - Integration with ValidationHelper for destructive ops (High risk for delete, Medium for write/replace)
- **Tool Skills System** (#62): Full-stack skill document system
  - CRUD backend (5 commands), ReadSkillTool (hidden, auto-injected)
  - Frontend Settings > Skills UI with category filters, enable/disable toggle
  - Agent form skills selection, prompt `{{skill:name}}` syntax
  - i18n translations (FR/EN)

### Changed

- **Cumulative Token Tracking**: Fixed token accumulation from last-call-only to proper cumulative addition (`+=`)
- **Token Store**: Replaced `updateStreamingTokens()` and `setInputTokens()` with unified `setSessionTokens()` API
- **Import/Export**: Added cache pricing fields to model export/import
- **Validation Schema**: Added cache pricing fields to model validation

### Fixed

- **seed_builtin_models**: Added missing `cache_write_price_per_mtok` field that was silently defaulting to 0
- **Modal Positioning** (#59): Removed CSS `contain: content` that broke `position: fixed` modals in settings
- **confirm() Migration** (#59): Replaced 8 `window.confirm()` calls with `DeleteConfirmModal` across 5 settings files
- **Backend Code Quality** (#61): Extracted duplicate `Regex::new()` to `static LazyLock`, replaced `expect()` with `?` in `AppState::new()`
- **Frontend Cleanup** (#60): Standardized error handling, removed SA-xxx references from component headers, untracked internal docs

### Maintenance

- **CI** (#57): Removed redundant Validate run on push to main (was duplicating ~23min CI run after every merge)
- **Dependencies** (#58): Batch dependency updates March 2026 (6 Dependabot PRs)

---

## [0.13.0] - 2026-03-01

### Added

- **Block-by-block Agent Chat (SA-019)**: Complete rewrite of agent message display
  - Real-time token streaming with thinking extraction and new ChunkTypes
  - `ChatBlock` model with `load_message_blocks` command for structured display
  - Frontend execution blocks store with inline block-by-block rendering
  - Removed ActivitySidebar (22 files deleted, -5585 lines), replaced with 2-column layout
  - TodoTool tasks display with persistence and agent name resolution
  - Auto-scroll with smart detection (short-circuit, timing)
- **Hybrid Agent ID/Name Resolution (SA-020)**: Agents addressable by name or UUID
  - UNIQUE index on agent name with backend uniqueness validation
  - `AgentRegistry.get_by_name()` with case-insensitive + trim lookup
  - `resolve_agent_ref()` shared function (ID fast path, name slow path)
  - `DelegateTaskTool` and `ParallelTasksTool` accept `agent_name` as alternative to `agent_id`
  - Real agent names in events and reports
  - Frontend duplicate name validation with i18n
- **Report Enforcement (SA-021)**: Detects generic completion messages and triggers follow-up LLM call for proper markdown report
- **Workflow UX Improvements (SA-016)**: Temporal grouping, round separators, markdown streaming, workflow rename (F2), filter labels
- **Settings Decomposition (SA-017)**: Shared UI components, centralized name validation with TDD, error handling with ErrorBanner
- **Internationalization (SA-018)**: Removed hardcoded model IDs, centralized `DEFAULT_OLLAMA_URL`, internationalized settings messages

### Changed

- **Code Organization (SA-022)**: Barrel exports, provider components moved to `settings/providers/`, filenames normalized to kebab-case, dead code removal, JSDoc import paths fixed
- **Consolidation (SA-023)**: `ProviderType` in single canonical location, app-wide constants in `constants.rs`, `safe_truncate()` in `utils.rs`, `commands/models.rs` renamed to `commands/llm_models.rs`
- **Dependency Cleanup (SA-024)**: Replaced `once_cell` and `futures` with std alternatives, pinned `surrealdb`, moved `svelte-virtual-list` to deps, converted `.expect()` to `Result` in LLM providers

### Fixed

- **Scroll Performance (SA-017)**: WebKit2GTK scroll fixes for settings pages
- **each_key_duplicate**: Composite keys `${type}-${i}` in ChatContainer blocks and MessageMetrics sub-agents
- **`{@const}` non-reactive**: Inline function calls instead of `{@const}` with SvelteMap
- **serde_json::Value in json!()**: Serialize to string first
- **message_id chain**: Correct propagation through block-by-block display

### Security

- **SurrealQL Injection Prevention (SA-001)**: Parameterized queries with `.bind()` / `execute_with_params()`
- **Type Safety (SA-013)**: Aligned enums and types between Rust and TypeScript (ChunkType, AgentConfigCreate, ProviderSettings, MessageCreate)
- **Defense-in-depth**: `validate_uuid_field()` (47 sites), `serialize_for_query()` (25 sites), `sanitize_for_surrealdb()` on external data
- **Dead Code Removal (SA-015)**: 5-phase cleanup of annotations, superseded code, dead getters, speculative methods
- **MCP HTTP Validation (SA-002)**: `base_url` validation warning for MCP servers
- **Console Violations (SA-013)**: Removed all `console.*` from frontend
- **Cancellation Token Propagation**: Through agent chain with UTF-8 safe truncation
- **Migration Guard (SA-005)**: Prevents embedding destruction during migrations
- **Function Decomposition (SA-007)**: Long functions decomposed (workflow executor, import/export)
- **Sub-agent Token Tracking (SA-014)**: Separate tracking and data persistence

### Removed

- ActivitySidebar component and related 22 files (-5585 lines)
- 171 OPT-* traceability markers from codebase
- `once_cell` and `futures` crate dependencies (replaced by std)
- Unused `Default` impls in LLM providers

---

## [0.12.0] - 2026-02-12

### Added

- **OpenAI-compatible Custom Providers**: Full support for user-created providers (RouterLab, OpenRouter, Together AI, etc.)
  - `OpenAiCompatibleProvider`: HTTP-based provider with SSE streaming and tool calling (OpenAI function call format)
  - `OpenAiToolAdapter`: Converts MCP tools to OpenAI function call schema
  - `ProviderType::Custom(String)`: Extensible provider enum replacing hardcoded validation
  - `custom_provider` DB table with CRUD commands (`list_providers`, `create_custom_provider`, `update_custom_provider`, `delete_custom_provider`)
  - `CustomProviderForm` component: modal form with auto-generated URL-safe provider ID
  - Dynamic provider selection in `AgentForm`, `ModelForm`, `ProviderCard`, `LLMSection`
  - `loadAllLLMData()`: unified data loader for providers + models + settings
  - SecureKeyStore integration for custom provider API keys
  - Provider auto-registration at startup from DB
  - 10 new i18n keys (fr + en) for custom provider UI

### Changed

- `ProviderType` TypeScript type: `'mistral' | 'ollama'` -> `BuiltinProvider | string` (extensible)
- Agent validation uses `ProviderType::from_str()` instead of hardcoded provider list
- `LLMSection` dynamically loads provider list instead of hardcoding Mistral/Ollama
- `ProviderCard` supports custom provider actions (delete, configure)

### Documentation

- `API_REFERENCE.md`: Custom Providers CRUD section (4 commands)
- `DATABASE_SCHEMA.md`: `custom_provider` table, count 19->20, SurrealDB 2.5.0
- `FRONTEND_SPECIFICATIONS.md`: CustomProviderForm component, updated types/stores/counts

---

## [0.11.0] - 2026-02-08

### Added

- **Chat Bubble Redesign (Phases 1-4)**: Redesigned message display with structured content separation
  - Backend `response` field on `Report` and `WorkflowResult` for clean LLM output extraction
  - `MarkdownRenderer` component: safe markdown rendering using `marked` + `DOMPurify` with link interception
  - `MessageMetrics` component: model, tokens, duration, cost display below assistant messages
  - Sub-agent chips on assistant messages (name, status, duration, tokens)
  - Copy button with 2-second visual feedback on assistant messages
  - Backward compatible: old messages (full report) still render gracefully via `MarkdownRenderer`
- **Sub-agent chips persistence**: Sub-agent execution data now survives page reload
  - `enrichMessagesWithSubAgents()` correlates `sub_agent_execution` DB records to messages by timestamp
  - `MessageService.loadWithSubAgents()` loads messages and executions in parallel
- **Dependencies**: `marked` ^17.0.1, `dompurify` ^3.3.1, `@types/dompurify` ^3.0.5

### Changed

- `MessageBubble` uses `MarkdownRenderer` for assistant messages instead of `pre-wrap` plain text
- `workflowExecutor.service.ts` extracts `result.response` for assistant message content
- `WorkflowResult` TypeScript type includes `response: string` field

### Documentation

- Synced all docs with codebase: version corrections, Memory Tool v2 operations, DB schema updates
- Removed 4 completed spec documents (background workflow, rig-core upgrade, activity sidebar v2, memory tool v2)
- Updated CLAUDE.md, TECH_STACK.md, README.md, AGENT_TOOLS_DOCUMENTATION.md, DATABASE_SCHEMA.md, REMAINING_TASKS.md

---

## [0.10.0] - 2026-02-07

### Added

- **Activity Sidebar v2**: Enhanced activity feed with rich details and interaction
  - Badge counts on filter tabs (tool, reasoning, message, error)
  - Expandable tool details with lazy-loaded input/output via `get_tool_execution` command
  - Expandable reasoning step details with full text display
  - Message grouping by conversation rounds (user message + agent responses)
  - Token count display on tool and reasoning activities
  - Absolute timestamps on hover (tooltip)
  - Activity export to JSON with full content (not truncated)
  - New `JsonViewer` component for recursive JSON display with collapse/expand
  - New `ToolDetailsPanel` and `ReasoningDetailsPanel` components
  - 14 unit tests for activity utility functions
  - i18n translations (en/fr) for export dialog and toast
- **Memory Tool v2**: Intelligent memory management with auto-scoping and semantic search
  - Auto-scoping: `user_pref`/`knowledge` memories are general, `context`/`decision` are workflow-scoped
  - Importance scoring (1-10) and TTL (time-to-live) for automatic expiry
  - `describe` operation for agents to discover memory stats before searching
  - Composite scoring: cosine_similarity*0.7 + importance*0.15 + recency*0.15
  - Compact list mode with truncated content for token efficiency
  - Shared helper functions between tool and commands (`search_memories_core`, `describe_memories_core`)
  - Stateless tool design with immutable `default_workflow_id`

### Fixed

- **Reasoning steps lost on workflow switch**: Agent intermediate reasoning steps were only emitted to frontend via `emit_progress()` but never persisted to DB. Added `ReasoningStepData` collection during execution, passed through `ReportMetrics`, and persisted by `streaming.rs`
- **Tool input/output empty in historical view**: SurrealDB SCHEMAFULL `TYPE object` silently dropped dynamic keys from tool I/O JSON (ERR_SURREAL_001). Changed schema to `TYPE string` with custom serde for JSON string serialization/deserialization with backward compatibility
- **Export content truncated**: Activity export now uses `metadata.content` (full text) instead of `description` (truncated to 200 chars)

---

## [0.9.4] - 2026-02-06

### Added

- **Background Workflow Execution**: Run workflows in background with concurrent multi-workflow support
  - Central dispatch store (`backgroundWorkflowsStore`) with Tauri event listeners
  - Concurrent workflow limits: 3 in auto mode, 1 in manual/selective mode
  - Toast notification system for background workflow events
  - Visual indicators in sidebar: running pulse dot, question badge, section headers
  - UserQuestion support for background workflows with persistent toast
  - i18n translations (en/fr) for all toast and sidebar strings

### Changed

- **rig-core**: Upgraded from 0.24.0 to 0.30.0
  - Client constructors now return `Result` (Mistral, Ollama)
  - Ollama client uses `Nothing` type for API key parameter
  - No changes to completion/prompt API
- **Sub-Agent Limit**: Increased `MAX_SUB_AGENTS` from 3 to 15 concurrent operations per workflow
- **Dependencies (Rust)**:
  - `rig-core` 0.24.0 -> 0.30.0
  - `uuid` 1.18.1 -> 1.20.0
  - `tokio-util` 0.7.17 -> 0.7.18
  - `thiserror` 2.0.17 -> 2.0.18
  - `tauri-build` 2.5.2 -> 2.5.3
  - `tauri-plugin-dialog` 2.4.2 -> 2.6.0
- **Dependencies (NPM)**:
  - `eslint-plugin-svelte` 2.46.1 -> 3.14.0 (major)
  - `globals` 16.5.0 -> 17.2.0 (major)
  - `svelte` 5.48.0 -> 5.49.1
  - `@typescript-eslint/parser` 8.53.1 -> 8.54.0
  - `@tauri-apps/plugin-dialog` 2.4.2 -> 2.6.0

### Fixed

- **ESLint**: Resolved 52 eslint-plugin-svelte 3.x lint errors
  - Added keys to all `{#each}` blocks (`svelte/require-each-key`)
  - Replaced `$state`+`$effect` with `$derived` for synced props (`svelte/prefer-writable-derived`)
  - Disabled `svelte/no-navigation-without-resolve` for Tauri desktop app
  - Configured TypeScript parser for `.svelte.ts` files in ESLint config

---

## [0.9.3] - 2026-01-30

### Fixed

- **SurrealDB Panic**: Prevent database panic on null characters in MCP responses
  - Created `sanitize_for_surrealdb()` utility to remove `\0` from JSON strings
  - Applied to MCP call logging, user questions, and embedding imports
- **Token Display**: Sync token counter with streaming in real-time
  - Cross-store synchronization between `streamingStore` and `tokenStore`
- **Agent Config**: Load agent configuration when creating workflow
- **Import/Export**: Add missing `enable_thinking` field for agents
- **Security**: Add native keyring features for API key persistence

### Changed

- **Tool Descriptions**: Improved sub-agent tool descriptions for LLM clarity
  - Added "DO NOT USE WHEN" sections for usage guidance
  - Added ⚠️ CONTEXT ISOLATION warnings
  - Improved examples with structured prompts (TASK/CONTEXT/FOCUS/REPORT)
  - Applied to SpawnAgentTool, DelegateTaskTool, ParallelTasksTool

---

## [0.9.2] - 2026-01-25

### Added

- **Human-in-the-Loop Validation**: Complete validation system for workflow operations
  - Three validation modes: Auto, Manual, Selective
  - Granular control per operation type (Tools, Sub-agents, MCP)
  - Risk threshold overrides (auto-approve-low, always-confirm-high)
  - Dynamic UI showing available tools and MCP servers with status badges
- **New Command**: `list_available_tools` for retrieving tools/MCP info
- **New Type**: `AvailableToolInfo` for tool metadata

### Changed

- **ToolFactory**: Now stores `app_handle` for sub-agent validation support
- **LLMAgent**: Integrated ValidationHelper before tool/MCP execution
- **ValidationSettings UI**: Enhanced with mode-specific displays and visual feedback

### Documentation

- **WORKFLOW_ORCHESTRATION.md**: Added comprehensive "Human-in-the-Loop Validation" section
- **FRONTEND_SPECIFICATIONS.md**: Updated validation settings description
- **API_REFERENCE.md**: Documented new validation commands

---

## [0.9.1] - 2026-01-23

### Added

- **Legal Notices**: GDPR-compliant privacy policy and legal notices accessible from Help menu
- **GitHub Actions**: CI/CD workflows for validation and release

### Changed

- **Dependencies (Rust)**:
  - `keyring` 2.3.3 → 3.6.3 (with API migration: `delete_password` → `delete_credential`)
  - `reqwest` 0.12.24 → 0.12.28
  - `tauri-plugin-opener` 2.5.2 → 2.5.3
  - `thiserror` 1.0.69 → 2.0.17
  - `tracing-subscriber` 0.3.20 → 0.3.22
- **Dependencies (NPM)**:
  - `typescript-eslint` 8.48.1 → 8.53.1
  - `@playwright/test` 1.57.0 → 1.58.0
  - `@tauri-apps/cli` 2.9.5 → 2.9.6
  - `@sveltejs/vite-plugin-svelte` 6.2.1 → 6.2.4
- **GitHub Actions**:
  - `actions/checkout` v4 → v6
  - `actions/setup-node` v4 → v6
  - `actions/download-artifact` v4 → v7
  - `softprops/action-gh-release` v1 → v2

### Fixed

- **CI/CD**: Added frontend dist placeholder for Tauri compile-time validation
- **CI/CD**: Added clang/llvm for RocksDB compilation in CI
- **CI/CD**: Added rustup targets for macOS universal binary builds
- **Security**: Updated keyring API for v3.x compatibility (`delete_credential`)
- **Error Handling**: Replaced `unwrap()` with proper pattern matching in production code (`models.rs`)
- **Clippy Warnings**: Fixed 13 clippy warnings in test code

### Documentation

- **ROADMAP_TO_1.0.md**: Updated with detailed analysis of `unwrap()`/`expect()` occurrences
- **DEPLOYMENT_GUIDE.md**: Added GitHub Actions configuration

---

## [0.9.0-beta] - 2025-12-14

### Added

- **Multi-Agent System**: Full CRUD operations for agents via Settings UI
- **Tool System**: 7 integrated tools (Memory, Todo, Calculator, UserQuestion, InternalReport, SubAgent, WebSearch)
- **MCP Integration**: Support for Docker, NPX, and UVX MCP servers
- **Sub-Agent System**: Agent delegation with parent-child relationships
- **i18n Support**: English and French translations
- **Settings Navigation**: Route-based settings with deep linking
- **Circuit Breaker**: Resilience pattern for UserQuestionTool
- **Virtual Scrolling**: Performance optimization for large lists

### Changed

- **Icon Library**: Migrated from `lucide-svelte` to `@lucide/svelte` (OPT-FA-12)
- **Workflow Executor**: Extracted as dedicated service (OPT-FA-8)
- **PageState Interface**: Aggregated for cleaner component architecture (OPT-FA-9)
- **Tool Descriptions**: Optimized for token efficiency (OPT-TD-1 to OPT-TD-8)

### Performance

- **Scroll Optimization**: WebKit2GTK scroll performance improvements (OPT-SCROLL)
- **Messages Area**: Virtual scroll and derived store consolidation (OPT-MSG-1 to OPT-MSG-6)
- **Activity Feed**: Memoized filtering and lazy-loaded modals (OPT-FA-7 to OPT-FA-13)
- **Workflow Engine**: Reduced N+1 queries, optimized streaming (OPT-WF-1 to OPT-WF-9)
- **TodoTool**: Parameterized queries, reduced N+1 patterns (OPT-TODO-1 to OPT-TODO-12)
- **MemoryTool**: Query consolidation and input validation (OPT-MEM-1 to OPT-MEM-8)
- **UserQuestionTool**: Strategic optimizations with circuit breaker (OPT-UQ-1 to OPT-UQ-12)

### Fixed

- **LLM Provider**: Removed erroneous `#[allow(dead_code)]` attributes
- **Virtual Scroll**: Fixed overflow issues in ActivityFeed and MemoryList
- **MCP Resilience**: Added timeouts, retry logic, and sub-agent heartbeat fixes
- **Integration Tests**: Updated for new ToolFactory API

### Security

- **SQL Injection Prevention**: Parameterized queries across all tools
- **API Key Storage**: Tauri secure storage with AES-256 encryption
- **CSP Policy**: Strict Content Security Policy (`default-src 'self'`)

### Documentation

- Comprehensive documentation in `docs/` directory
- API Reference with all Tauri command signatures
- MCP Configuration Guide
- Multi-Agent Architecture documentation
- Tool development patterns and examples

---

## [Unreleased]

### Planned for 1.0.0

- Integration tests with ephemeral SurrealDB
- E2E tests with Playwright
- macOS and Windows distribution packages

---

## Project History

### Phase 0 - Project Setup
- Initial Tauri + SvelteKit + Rust configuration
- SurrealDB embedded integration
- TypeScript/Rust type synchronization

### Phase 1-2 - Database Foundation
- SurrealDB schema design (SCHEMAFULL tables)
- Agent, Workflow, Memory persistence
- Query patterns and utilities

### Phase 3 - Multi-Agent Infrastructure
- Agent lifecycle management
- Tool registry and factory patterns
- MCP client/server architecture

### Phase 4 - Command Layer
- Tauri IPC commands
- Frontend-backend communication
- Error handling patterns

### Phase 5 - Frontend Implementation
- SvelteKit routing and stores
- Component library (atomic design)
- Theme system and i18n

### Phase 6-9 - Optimization Sprints
- Performance profiling and fixes
- Security hardening
- Documentation sync

---

[Unreleased]: https://github.com/assistance-micro-design/Zileo-Chat/compare/v0.21.0...HEAD
[0.21.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.21.0
[0.20.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.20.1
[0.20.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.20.0
[0.19.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.19.1
[0.19.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.19.0
[0.18.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.18.0
[0.17.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.17.0
[0.16.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.16.0
[0.15.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.15.1
[0.15.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.15.0
[0.14.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.14.0
[0.13.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.13.0
[0.12.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.12.0
[0.11.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.11.0
[0.10.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.10.0
[0.9.4]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.4
[0.9.3]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.3
[0.9.2]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.2
[0.9.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.1
[0.9.0-beta]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.0-beta
