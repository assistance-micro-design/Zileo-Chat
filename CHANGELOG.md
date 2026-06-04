# Changelog

All notable changes to Zileo Chat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.27.0] - 2026-06-04

### Added

- **Startup splash screen and immediate window creation** (`src/lib/components/SplashScreen.svelte`, `src-tauri/src/commands/boot.rs`, `src-tauri/src/main.rs`, `src-tauri/src/state.rs`, `src/lib/tauri/app.ts`, `src/routes/+layout.svelte`, `src/messages/{en,fr}.json`) -- the application window now opens immediately instead of staying blank for several seconds while heavy services initialize. A splash screen fills the window during startup: it displays the application name, a scrolling subsystem progress indicator, the version number (bottom right), and a link to `assistance-micro-design.fr` (bottom left). It fades out once the UI-critical services (providers, embedding) finish initializing; MCP server connections continue in the background and do not block the transition. A new `boot_ready_state` Tauri command (`commands/boot.rs`) lets the frontend poll the ready flag reliably, covering the race where the `boot_ready` event fires before the listener is attached. A new `getAppVersion` wrapper in `src/lib/tauri/app.ts` reads the version at runtime so the splash stays up to date without hardcoding. The deferred-initialization order preserves the existing security property: the MCP network settings snapshot is seeded into the process-global guard before any MCP connection attempt.

- **Per-agent MCP tool allowlist governing unattended runs** (`src-tauri/src/agents/execution/tools.rs`, `src-tauri/src/db/schema.rs`, `models/agent.rs`, `src/lib/components/settings/validation/{AgentAuthorizations,AgentMcpAllowlist}.svelte`, `src/routes/settings/validation/+page.svelte`) -- an MCP tool auto-called inside a detached run (Kanban analyze / compose, supervisor `RerunWorker`) now executes only if it is explicitly armed in the agent's `mcp_tool_allowlist` (keyed by `server_id` + tool name). The gate is fail-closed and unconditional: it sits above the validation mode, so `ValidationMode::Auto` no longer short-circuits it. A sub-agent of a detached parent inherits the detached signal (Spawn intersects the parent allowlist, Delegate/Parallel resolve the permanent agent's), and a per-entry `allow_in_delegated_runs` flag (default `false` = strict) closes the cross-agent confused-deputy in delegated runs. The allowlist is edited from a dedicated **"Agent authorizations"** section on `settings/validation` (moved out of `AgentForm` so editing an agent's name can never silently disarm it): per-server collapsible tool lists with an "auto-approve all" toggle, a per-agent `require_file_confirmation` toggle, scoped to the agent's running assigned servers, with stopped/de-selected servers preserved read-only (and an explicit "remove" to revoke them). The agent dropdown marks agents that have at least one genuinely armed MCP tool.
- **Opt-in private-network access for HTTP MCP servers** (`src-tauri/src/mcp/{ssrf,network_settings}.rs`, `commands/settings_mcp_network.rs`, `src/routes/settings/mcp/+page.svelte`) -- a new **Settings > MCP** network section exposes an off-by-default toggle that allows MCP HTTP servers on private / LAN ranges (RFC1918, CGNAT `100.64/10`, ULA `fc00::/7`). The SSRF resolver stays strict by default; the opt-in only unblocks `IpClass::Private` -- cloud metadata (`169.254.x`), link-local, reserved and multicast stay blocked unconditionally, and the redirect policy is unchanged. Persisted as a `settings:mcp_network` JSON blob (STT-style), seeded into a process-global fail-secure snapshot at boot before MCP auto-connect.
- **Asynchronous Kanban card generation with a "tasks to validate" zone** (`src-tauri/src/commands/compose_card.rs`, `models/kanban_card.rs`, `src/lib/stores/kanban-compose.ts`, `src/lib/components/kanban/KanbanCardCreator{,Auto}.svelte`) -- the auto-compose flow now runs detached (`start_compose_card`) and materialises generated cards as `status = 'proposed'` in a dedicated validation zone, where they are approved (`approve_proposed_card` → `ready`) or rejected before the scheduler ever runs them. The "New task" creator is a single root-mounted component driven by `cardCreatorStore`, reachable from the global `FloatingMenu` on any route (quick create + `goto('/kanban')`).
- **Configurable Kanban compose timeout** (`src-tauri/src/commands/settings_kanban.rs`, `src/routes/settings/kanban/+page.svelte`, `constants.rs`) -- a new **Settings > Kanban** section replaces the hardcoded 300 s compose wall-clock cap with a bounded, hot-reloaded setting (`compose_timeout_secs`, default 600, clamp 60-1800), persisted as a `settings:kanban` blob. The previous fixed cap killed slow reasoning-model composes (e.g. `deepseek-v4-pro` at `xhigh`) even after the card had already been captured.
- **Governed self-improvement writes for the `*Manager` tools** (`src-tauri/src/agents/execution/tools.rs`, `models/validation.rs`, `src/lib/components/workflow/ValidationModal.svelte`) -- prompt / skill / workflow `*Manager` write operations in detached runs now transit the existing **Settings > Validation** flow (mode + `always_confirm_high`) instead of being hard-refused. With `Auto` + `always_confirm_high` off the write executes and is recorded as a `pre_approved` audit entry with an opportunistic toast; with `always_confirm_high` on it is reviewed (attended modal, or refused in a detached run with no boot-DoS). A new `ValidationType::ManagerWrite` and `DecidedBy::PreApproved` carry the decision through the audit chain, bounded by a per-run write cap.
- **First-launch onboarding flow** (`src/lib/stores/onboarding.ts`, `src/lib/components/onboarding/steps/`, `src/types/onboarding.ts`) -- an 8-step guided setup (language, theme, welcome, features, API key, import, getting started, complete). The API-key step auto-saves on blur; the import step runs a real simple import and reports partial results, warnings and per-item errors; the getting-started step deep-links into the relevant Settings sections and is skipped after a successful import. Interactive and animated, honouring `prefers-reduced-motion` (`src/lib/utils/motion.ts`), fully bilingual.
- **Import/Export schema v1.3 with typed import warnings** (`src-tauri/src/commands/import_export/`, `models/import_export.rs`, `src/types/import-export.ts`, `src/lib/components/settings/import-export/`) -- the round-trip now carries `agent.kind`, `auto_analyze_reports`, the model `supports_vision` / `supports_forced_tool_choice` flags, `skill.kind`, and the custom-provider strict toggles. Model conflicts are detected by `(provider, api_name)` with a charset-safe, length-bounded unique rename. The MCP tool allowlist is reset fail-closed on import with a translated warning. The catch-all `MissingDependency` warning is split into per-entity variants (`MissingModel` / `MissingMcpServer` / `MissingSkill` / `MissingProvider`) and the API-key case renamed `ApiKeyRequired`, so each maps 1:1 to a localized key (no more parsing English backend text). Local / LAN HTTP MCP servers -- which export fine but are refused on re-import by the SSRF defense -- are flagged up front in the export preview.
- **Confined per-card review chat with the Kanban supervisor** (`src-tauri/src/commands/kanban_card_chat.rs`, `tools/{rerun_worker,move_card,schedule_card}.rs`, `src/lib/components/kanban/KanbanCardReportViewer.svelte`) -- when a card sits in `review`, the report viewer now opens an in-place chat with the card's Kanban agent, backed by a dedicated hidden workflow (`workflow.hidden_from_list`, filtered out of the `/agent` sidebar via `SELECT_LIST`). The conversation is seeded with a structured summary (worker report + last verdict + any suggested prompt edit), persists across modal close/navigation (linked via `kanban_card.review_chat_workflow_id`), and resumes idempotently. The supervisor is confined to the card: `create_local_tools` strips Spawn/Delegate/Parallel **and `UserQuestionTool`** for any `kind = Kanban` agent and never takes the sub-agent-tool branch (strict Kanban separation extended to the caller side; `UserQuestionTool` is excluded because `/kanban` does not mount `UserQuestionModal` — a question would silently time out). All `WorkflowManagerTool` operations filter out `hidden_from_list` workflows so the supervisor cannot read its own chat workflow via the tool. Deleting or auto-purging a card cascades the linked review-chat workflow and all its rows (messages, tool executions, thinking steps); the worker workflow is left untouched. It gains three auto-injected, self-gating tools usable only inside a card chat: **RerunWorkerTool** (re-runs the worker workflow detached with an extra instruction and refreshes its report), **MoveCardTool** (validate → Done, or send back to Todo — re-queued as `ready` for a fresh scheduler run; `doing` is never a manual target) and **ScheduleCardTool** (attaches a weekly recurrence, turning the card into a template). `is_transition_allowed` permits re-queue to `Todo` (status reset to `ready`) from both `Review` and `Done`; `Review→Done` for validation; `Review→Doing` is not a valid transition. `create_workflow_core` was extracted to create the hidden chat workflow. The supervisor's tool calls and reasoning now stream live into the modal (see Changed below).
- **Edit existing custom providers** (`src/lib/components/settings/providers/CustomProviderForm.svelte`, `LLMSection.svelte`, `src/lib/components/llm/ProviderCard.svelte`) -- custom OpenAI-compatible providers can now be edited in place from Settings, including the `supports_cache_control` and `supports_reasoning_param` strict-mode toggles. Previously a misconfigured toggle required deleting the provider (and its models) and recreating it. The form gains an edit mode that seeds its fields from the provider, keeps the name immutable, and treats the API key as optional (blank keeps the stored key). The backend `update_custom_provider` command already supported this; only the UI was missing.
- **Per-model `supports_forced_tool_choice` capability** (`src-tauri/src/db/schema.rs`, `models/llm_models.rs`, `commands/llm_models/crud.rs`, `src/lib/components/llm/ModelForm.svelte`, `src/types/llm.ts`) -- a new boolean toggle on each LLM model (default `true`). Some upstreams reject a forced `tool_choice` (`required` / by-name): `deepseek-v4` proxied through RouterLab returns HTTP 400 on any forced tool call, even without thinking, while sibling models on the same gateway (GLM, Kimi, Claude) accept it. When the flag is `false`, the tool loop downgrades an opening-turn `Required` to `Auto` so the call succeeds; the empty-capture-slot risk this re-introduces for the Kanban analyze/compose flows is already covered by the boot-time catch-up re-analysis. `DEFAULT true` does not backfill existing rows, so SELECTs read it via `(supports_forced_tool_choice ?? true)`; the live lookup is provider-scoped to avoid trusting another provider's row for the same `api_name`.

### Changed

- **The Kanban board is now the default surface** (`src/routes/+page.ts`, `src/routes/+layout.svelte`, `src/lib/components/layout/FloatingMenu.svelte`, `src/styles/global.css`) -- entering `/` redirects (307) to `/kanban` (the task board) instead of `/agent`; `/agent` stays reachable from the nav bar. The nav buttons reuse the existing palette tokens (Kanban reads the project blue, Agent the secondary orange accent) -- no new colors.
- **The human-in-the-loop validation modal is mounted globally** (`src/lib/components/workflow/GlobalValidationModal.svelte`, `src/routes/+layout.svelte`, `src/lib/components/ui/Modal.svelte`, `workflow/ValidationModal.svelte`) -- previously the `ValidationModal` was mounted only on `/agent`, so an attended Kanban workflow (e.g. a card-supervisor chat) requesting a tool validation emitted the event into the void and the backend poll timed out silently. A single root-mounted `GlobalValidationModal` now owns the store lifecycle and renders on any route. The security gate cannot be dismissed (`closeOnBackdrop` / `closeOnEscape` / `showCloseButton` all `false`, no Cancel button); it closes only on an explicit Approve / Reject or a backend resolution, and an IPC error keeps it open for retry instead of failing open.
- **Built-in provider keystore keys are canonicalized and the runtime is reconfigured after save** (`src-tauri/src/commands/security.rs`) -- built-in provider keys are stored under a canonical name (`mistral` → `Mistral`) so the save matches every read site on case-sensitive keychains, and the running provider is reconfigured right after `save_api_key`, so a key entered in onboarding or Settings now works without an app restart.
- **Speech-to-text now defaults to `voxtral-mini-latest`** (`src-tauri/src/models/stt.rs`, `src/types/stt.ts`) -- the dictation model id ships with a transcription-capable default instead of being empty on first launch. The previously shipped `voxtral-small-latest` is an audio-understanding chat model served only by `/v1/chat/completions`; the transcription endpoint rejects it, so the default now uses the transcription-specialized mini alias.
- **Nested modals render above their parent** (`src/lib/components/ui/Modal.svelte`, `src/lib/components/settings/versions/VersionsHistoryModal.svelte`) -- a dedicated `--z-index-modal-nested` tier and a Modal `elevated` prop let the delete-confirmation opened from `VersionsHistoryModal` render above it; the add/edit prompt and skill modals now open full-screen with a taller content textarea.
- **`WorkflowManagerTool.read_workflow` now surfaces the workflow's folder** (`src-tauri/src/tools/workflow_manager.rs`) -- the response gains `folder_id` plus a resolved `folder_name` (both `null` when uncategorized). Previously the only way to tell which folder a workflow lived in was to cross-reference `list_workflows` (which carries `folder_id`) against `list_workflow_folders`; reading a single workflow gave no folder info at all. Folder-name resolution is best-effort (a dangling id degrades to a null name, not an error).
- **Per-card review chat: available on done cards, full block replay, live streaming, background continuation** (`src/lib/components/kanban/KanbanCardReportViewer.svelte`, `src/routes/kanban/+page.svelte`) -- the chat is no longer confined to the `review` column: it now also opens on `done` cards, so the supervisor can keep working a card after it has been validated or moved (previously, the chat section vanished mid-conversation the moment the supervisor moved the card out of `review`). The modal now reuses the same `ChatContainer` + `WorkflowExecutorService.execute` + `backgroundWorkflowsStore` / `executionBlocksStore` pipeline as the `/agent` page instead of a hand-rolled message list: reopening a card replays every persisted tool-call and reasoning block (via `load_workflow_blocks` / `BlockService.loadForMessages`), the supervisor's turn streams live into the modal, and the turn keeps running detached on close/navigation -- reopening mid-run restores the in-flight timeline (`restoreFromChunks`) and a completion effect reloads the authoritative conversation. The `/kanban` page registers the global stream forward-callbacks (so live chunks reach the shared execution store even when `/agent` was never visited), and closing the modal stops viewing the workflow without cancelling it. The seed-message persistence moved to the shared executor, so a turn is no longer lost on a full reload.
- **`ListAgentsTool` now exposes `has_file_manager`** (`src-tauri/src/tools/list_agents.rs`) -- the compose flow can now tell whether a target agent has a `FileManagerTool` without a separate lookup, so the supervisor can select an agent that matches the card's `target_folder_id` constraint.
- **Dependency maintenance** (`package.json`, `src-tauri/Cargo.toml`, `.github/workflows/`) -- ESLint 9 → 10 (flat config already in use, so a no-op migration), `@sveltejs/kit` → 2.63, `typescript-eslint` → 8.60, `uuid` → 1.23, and `rand` 0.9 → 0.10 (single `rand::random` jitter call in `llm/retry.rs`). CI pinned to `actions/checkout` v6.0.3 and `actions/setup-node` v6.4.0. Full lint/check/test (frontend) and fmt/clippy/test (backend) green.

### Fixed

- **Database indexes were rebuilt on every application launch, causing a multi-second blank window** (`src-tauri/src/db/client.rs`, `src-tauri/src/db/schema.rs`) -- `initialize_schema` re-executed the full schema SQL (`SCHEMA_SQL`) unconditionally at every boot. Because the HNSW vector index on `memory_chunk` was defined with `DEFINE INDEX OVERWRITE`, SurrealDB rebuilt the whole graph from the stored embeddings on every startup (roughly 10 s on a populated store), and the B-tree indexes added another 4 s. Since the window was not created until this init completed, the application appeared unresponsive for the duration. Two changes eliminate the cost: (1) the HNSW index definition changes from `DEFINE INDEX OVERWRITE` to `DEFINE INDEX IF NOT EXISTS`, so the graph is built once and kept across restarts (to change the index definition in a future release, ship a guarded `REMOVE INDEX IF EXISTS ...; DEFINE INDEX ...` migration); (2) `initialize_schema` computes a stable text fingerprint of `SCHEMA_SQL`, compares it against a hash stored in a `schema_meta:current` record, and skips re-applying the schema when the fingerprint is unchanged -- the schema is re-applied only when the SQL text actually changes. If any statement fails during apply, the fingerprint is not cached so the next boot retries. The result: startup on a populated database goes from several seconds of blank window to near-instant. No API cost: embeddings stay in storage, only the in-memory index structure is affected.

- **A late approve/reject could be clobbered by the validation timeout (fail-open)** (`src-tauri/src/commands/validation.rs`) -- `apply_timeout_behavior` did an unconditional `UPDATE`, so a user decision landing in the final window was overwritten by the timeout behavior (and a late user command could overwrite a fired timeout). On a human-in-the-loop security gate under `timeout_behavior = Approve`, a user's Reject could be flipped to approved. A new `resolve_validation_if_pending` helper makes resolution first-writer-wins via an atomic `UPDATE ... WHERE status = 'pending'` decision point: whoever sees `pending` first writes, the loser matches 0 rows and early-returns without overwriting, emitting a conflicting status, or double-auditing.
- **Existing agents vanished from the list after the `mcp_tool_allowlist` column shipped** (`src-tauri/src/models/agent.rs`, `db/schema.rs`) -- agents created before the column carried it as `NONE`; under SCHEMAFULL the boot SELECT returned an explicit `null`, which `#[serde(default)]` does not intercept, so `AgentConfig` failed to deserialize and the agent was dropped at load -- the whole agent list disappeared from the UI. Fixed with a null-tolerant `deserialize_vec_default` on the field plus an idempotent DB backfill (`UPDATE agent SET mcp_tool_allowlist = [] WHERE mcp_tool_allowlist IS NONE`). The `LLMModel.supports_vision` / `supports_forced_tool_choice` columns gained the same null-tolerant safety net.
- **Validated Kanban cards stalled as `doing` zombies under concurrent runs** (`src-tauri/src/commands/scheduler.rs`, `src/lib/stores/{kanban-runtime,kanban}.ts`, `src/routes/kanban/+page.svelte`) -- the board read a stale concurrency cap (validation settings were not loaded at app root, so `canStart()` saw `null` → cap 1 instead of the real Auto cap 3), and Kanban worker runs competed with `/agent` for interactive slots. The backend promotion budget is now the single source (`get_max_concurrent_workflows`), Kanban runs bypass the interactive gate and register as `backendInitiated` (the `/agent` path is unchanged), and boot recovery (`recover_stuck_doing_cards_core`) reconciles cards stuck in `doing`: one with a persisted assistant message is finalized to `review` (the boot auto-analyze catch-up handles it -- never re-runs a paid LLM call), one with no message is reset to `ready` / `todo`. A per-card reentrancy guard (`runCardWorkflow`) closes a TOCTOU where mount reconciliation and a `card_ready` event could both launch a workflow for one card.
- **DeepSeek reasoning models broke on the second tool-loop turn (HTTP 400)** (`src-tauri/src/llm/sse.rs`) -- the streaming collector accumulated `delta.reasoning_content` (DeepSeek's field) but re-emitted the reassembled assistant message under `message.reasoning`. When that message was echoed back on the next tool-loop turn, DeepSeek (including `deepseek-v4` proxied through RouterLab) rejected the renamed field with `HTTP 400: invalid upstream request`, so any DeepSeek agent using tools failed as soon as it produced a reasoning trace plus a tool call. The collector now remembers the field name the reasoning arrived under (`reasoning` for vLLM, `reasoning_content` for DeepSeek) and echoes it back unchanged. Verified empirically against `deepseek-v4-pro` / `deepseek-v4-flash` on RouterLab: the round-trip succeeds with `reasoning_content` and 400s with `reasoning`.
- **`xhigh` reasoning effort rejected by the database** (`src-tauri/src/db/schema.rs`) -- the `agent.reasoning_effort` schema assertion only allowed `low` / `medium` / `high`, so selecting the "Think Max" tier failed to persist. `xhigh` is now an accepted value.
- **Orphaned `doing` cards deadlocked the Kanban queue** (`src-tauri/src/commands/scheduler.rs`) -- a card whose worker workflow was lost (app restart, crash) stayed in `doing` with no `workflow_id`, consuming a concurrency slot and preventing new cards from being promoted. `reclaim_orphaned_doing_cards_core` now runs at each scheduler tick: cards in `doing` with no `workflow_id` past a grace window are reset to `ready` / `todo` so the slot is reclaimed. `/kanban` also reconciles on mount.
- **Double-promotion race for a single `ready` card** (`src-tauri/src/commands/scheduler.rs`) -- two concurrent scheduler ticks could promote the same card and launch two workflows for it. `try_claim_pending_card_core` now uses a `WHERE status = 'ready'` flip so only one promoter can win; the second sees an empty RETURN and skips the card.
- **Worker questions were only answerable on `/agent`** (`src/routes/+layout.svelte`) -- `UserQuestionModal` was mounted exclusively in `/agent/+page.svelte`. A card run launched from `/kanban` that raised a `UserQuestion` event could not be answered without navigating away; the question timed out after 5 minutes. `UserQuestionModal` is now mounted at the root layout and the toast "go to workflow" action opens it on any route.
- **`needs_improvement` loop had no exit path** (`src/lib/components/kanban/KanbanCardReportViewer.svelte`) -- the "Improve prompt" modal applied the suggested edit but left the card in `review` with no way to trigger a fresh run. "Save and re-run" now re-queues the card to `todo` (`ready`) so the scheduler picks it up.
- **`failed` / `reject` verdict cards had no retry affordance** (`src/lib/components/kanban/KanbanCardReportViewer.svelte`) -- cards in `review` with a failed execution or a `reject` verdict could not be retried from the UI. A retry button now re-queues the card to `todo` (`ready`).
- **Card-chat analyze seed could pick a stale verdict** (`src-tauri/src/commands/kanban_card_chat.rs`) -- `load_latest_analyze_seed` ordered the `analyze` interactions by `created_at DESC` but never selected `created_at`, so SurrealDB silently dropped the `ORDER BY` and fell back to insertion order. The chat could therefore be seeded with a non-latest verdict. `created_at` is now part of the projection so the most recent analyze interaction is actually chosen.

### Security

- **SSRF protection for the HTTP MCP client** (`src-tauri/src/mcp/{ssrf,http_handle,redact}.rs`) -- a new `classify_ip` / `SsrfResolver` layer (a `reqwest::dns::Resolve` that refuses the whole resolved set if any address is forbidden) blocks MCP HTTP requests to loopback, link-local, private, reserved, multicast and cloud-metadata (`169.254.169.254`) targets, decapsulating mapped / 6to4 / Teredo / NAT64 addresses so they cannot be smuggled past the classifier. The shared HTTP client is rebuilt with `.no_proxy()`, the custom resolver, and a 3-hop redirect limit; redirects are screened (cross-host, non-global literal IPs and `https → http` downgrades refused) and `http + auth` to a non-loopback host is blocked. MCP URLs are redacted (`redact_url_userinfo`) so userinfo never leaks into logs.
- **Docker spawn guard for stdio MCP servers** (`src-tauri/src/mcp/docker_guard.rs`) -- a deny-by-default `validate_docker_spawn_args` is wired at the `build_command` choke-point (covering boot / create / update / import and legacy paths). It refuses container-escape flags (`--privileged`, `--device`, `--cap-add`, `--network=host` and other host namespaces, `--gpus`, `--env-file`, `--volumes-from`, `--cidfile` / `--pidfile`, `--label-file`, `--sysctl`, `--cgroup-parent`, `--group-add`) and validates mount sources (system-prefix blocklist, lexical normalization + `canonicalize`). `docker exec` is routed through a reduced surface that also refuses `--privileged` / `--env-file`.
- **MCP configurations are re-validated on import** (`src-tauri/src/commands/import_export/`, `mcp/validation.rs`) -- imported MCP servers are mapped to a `Config` and run through the full validator plus an SSRF screen (loopback / private / metadata refused at import time, strictest policy), so a malicious export cannot smuggle an internal-network server onto the user's machine. The Docker-spawn guard is applied to imported stdio servers; rejected entries are skipped and reported per-batch.
- **Per-run MCP call cap and result-byte budget** (`src-tauri/src/agents/execution/tools.rs`, `constants.rs`) -- a run that hammers MCP or accumulates huge results is now bounded: a per-run call cap (`MCP_MAX_CALLS_PER_RUN = 1000`) and a cumulative result-byte budget (`MCP_MAX_RESULT_BYTES_PER_RUN = 50 MiB`) are checked before each call (an in-flight result is never truncated), and a per-result cap (`MCP_MAX_SINGLE_RESULT_BYTES = 10 MiB`) replaces any oversized single result with a short error so a compromised server cannot inject one giant payload into the run context before the next call is refused. Both the cap and the cumulative charge measure the real sink size (serialized `result` + `error`), so a giant _error_ payload is capped identically to a giant success. The check runs after the detached allowlist gate and before the attended modal, so the budget never fails the security gate open. MCP-only (local tools are user-trusted).
- **MCP security refusals are logged to the audit journal** (`src-tauri/src/commands/validation_audit.rs`, `models/validation.rs`, `src/routes/settings/audit-log/`) -- a detached-run MCP allowlist refusal was previously silent (a `tracing` warn only). It is now persisted as a `Blocked` / `Policy` / `High` entry, visible and filterable in **Settings > Audit Log**. The write is unconditional (a security refusal must always be traceable) but best-effort (never fails the gate, which stays fail-closed), carries no secret, and is deduplicated per run (`≤ 1 row per distinct blocked tool`) to close an externally-pilotable flood vector.
- **Detached analyze/compose runs were writable for Skill/Prompt managers** (`src-tauri/src/agents/execution/tools.rs`) -- the Kanban analyze and compose tool loops received the full `SkillManagerTool` and `PromptManagerTool` (read + write), so a misbehaving model could have edited prompts or skills during analysis. The detached write paths were first hardened with a blanket read-only guard; this was then superseded by the unified `manager_write_gate` (see "Governed self-improvement writes for the `*Manager` tools" under Added), which routes every `*Manager` write through the Validation settings instead of silently dropping it. The analyze system prompt no longer invites skill edits, and the untrusted worker report is spotlighted with a clear delimiter.
- **Empty `days_of_week` was a per-tick card-spawn DoS** (`src-tauri/src/commands/kanban_schedule.rs`) -- a schedule row with an empty `days_of_week` array would compute `next_run_at = now()` on every tick and clone the template card indefinitely. Empty arrays are now rejected at schedule create/update time.
- **`move_workflow_to_folder` did not honour the `hidden_from_list` guard** (`src-tauri/src/tools/workflow_manager.rs`) -- a Kanban agent could have moved a hidden chat-workflow into a folder, making it visible in `list_workflows`. The operation now pre-checks `hidden_from_list` and returns `PermissionDenied` if the target workflow is hidden.

### Removed

- **`workflow_slots` command module removed** (`src-tauri/src/commands/workflow_slots.rs`) -- dead code: `get_workflow_slots_available` was a Tauri command that only the now-inlined scheduler logic consulted. Slot gating is handled entirely inside `scheduler.rs` (`select_cards_to_promote_core`).
- **Backend dead-code removed** (`branch refactor/quality-multi`, 15 files, +88/-328) -- all `#[allow(dead_code)]` annotations eliminated from the production codebase (0 remaining). Removed symbols: `ProviderConfig.{active_provider,…}` and the matching `set_active_provider` / `get_active_provider` / `get_configured_providers` accessors in `llm/manager.rs` (never read outside tests); the test-only `EmbeddingConfig` struct and its `Default` impl + `ollama_nomic`/`ollama_mxbai` constructors from `llm/embedding/mod.rs` (superseded by `EmbeddingConfigSettings` in `models/embedding.rs`); `FunctionCall::new()` and `ToolChoiceMode::as_str()` from `models/function_calling.rs` (test-only ctors); the dead re-exports `MAX_PARALLEL_TASKS_PER_BATCH` / `MAX_SUB_AGENTS` from `tools/constants.rs` (constants themselves intact in `models/sub_agent.rs`) and `UserQuestionCircuitBreaker` from `tools/user_question/mod.rs`; `MCPContent` from the `mcp` re-export. The two module-level `#[allow(dead_code)]` on `mcp/circuit_breaker` and `mcp/client` were replaced by targeted `#[cfg(test)]` gates on the test-only accessor methods (`state`, `failure_count`, `reset`, `is_connected`). Hardening: 6 `.expect(…)` calls in `tools/memory/tool.rs` converted to `?`; `commands/embedding/operations.rs` rewritten with `let-else` for cleaner early returns.

## [0.26.0] - 2026-05-25

Kanban auto-analyze stabilization (`fix/kanban-analyze-stabilization`). Fixes the root cause of a Kanban card finishing its worker workflow but never receiving a verdict, leaving it stuck in the `review` column. The agent tool loop hard-coded `tool_choice = Auto` on every iteration, so the analyze and compose flows -- which depend on the model emitting one mandatory submit call to capture their result -- could silently fail whenever the model replied in prose instead. The loop now forces a tool call on the opening turn only (reverting to `Auto` afterwards so it can still terminate once the result is submitted). A boot-time pass re-analyzes cards orphaned by an app closed mid-workflow or an earlier silent failure, the verdict is now produced in the UI language, and the analyze lifecycle survives navigation away from the board.

### Added

- **Opening-turn forced tool call** (`src-tauri/src/agents/execution/tool_loop.rs`, `iteration.rs`) -- `execute_with_tools` takes an `opening_tool_choice` applied to the first iteration only. The analyze and compose flows pass `Required` so the model must engage `SubmitAnalysis` / `SubmitComposedCard` instead of finishing with an empty capture slot; every later turn reverts to `Auto` so the loop can terminate. The standard workflow path keeps `Auto` end-to-end.
- **Boot-time analyze catch-up** (`src-tauri/src/commands/kanban_analyzer.rs`, `main.rs`) -- at startup a detached, best-effort task re-runs the analyzer for every card stuck in `review` (finished into `done`, has a workflow, no analysis yet), covering cards orphaned by an app closed mid-workflow or a pre-fix silent failure. Self-gated by each agent's `auto_analyze_reports`.
- **UI-language verdicts** (`src-tauri/src/db/schema.rs`, `commands/streaming/execution.rs`) -- a new `workflow.locale` field is stamped at execution time and read back by the detached analyzer, so the verdict is produced in the user's language without a frontend round-trip. The compose command gains a `locale` parameter. Legacy workflows fall back to the default language.
- **`grant_skill_to_agent` operation on `SkillManager`** (`src-tauri/src/tools/skill_manager/`) -- attaches an existing skill to another agent's allowlist, guarded by existence checks and the same-kind separation (a kanban skill only grants to a kanban agent, and vice versa). Idempotent.
- **Manual "Re-analyze" button** (`src/lib/components/kanban/KanbanCardReportViewer.svelte`) -- re-runs the analysis on a review card on demand, with inline loading and error states; overlapping runs are blocked.

### Changed

- **Kanban analyze lifecycle is now root-mounted** (`src/lib/stores/kanban-events.ts`, `src/routes/+layout.svelte`, `kanban/+page.svelte`) -- the analyze and board-refresh event listeners moved from the `/kanban` page to a store initialized at the app root, so a verdict that arrives while the user is on another page still refreshes the board and pre-opens the improve-prompt modal. The workflow-launch listener stays page-local.
- **`ReadSkill` and `SkillManager` tool descriptions disambiguated** -- both previously said "read skill documents", leading the Kanban supervisor to pick `ReadSkill` (scoped to its own allowlist) for a skill it does not own and hit a permission gate. `ReadSkill` now states it is read-only and limited to the agent's own skills; `SkillManager` states it reaches any skill and should be used to inspect or improve one the agent does not own. No behavior change.
- **Analyzer no longer truncates the worker report** -- the full report is fed to the analyzer verbatim (previously capped at 12k characters), since a partial report could hide the very issue the verdict must catch.

### Fixed

- **`xhigh` ("très élevé") reasoning effort lost on agent reopen** (`src/lib/utils/agent-reasoning.ts`) -- saving an agent with the "Think Max" tier then reopening its form showed "élevé" (high) instead. The form's normalization effect ran on first render, before the LLM model list finished loading, so the still-unknown model was wrongly treated as not supporting `xhigh` and the stored value was downgraded to `high`. The normalizer now downgrades `xhigh` only when the model is known and unsupported; while the list is loading (unknown model) the persisted value is preserved.
- **SSE read timeout during reasoning thinking phase** (`src-tauri/src/constants.rs`, `llm/sse.rs`, `llm/manager.rs`) -- DeepSeek V4 pro/flash (and other reasoning models) emit their entire thinking trace before any answer token, so the streaming body can stay silent longer than the 30s per-read timeout. The read timed out mid-thinking and surfaced as reqwest's opaque "SSE stream read failed: error decoding response body". The per-read timeout is raised to 120s (it bounds only the gap between two SSE frames, never the total request, so fast models are unaffected), and the error message now distinguishes a read timeout from a connection drop via `reqwest::Error::is_timeout()` for an actionable hint.
- **Onboarding language step** shows "EN" instead of "GB" for English.
- **Navigation label** renamed from "Board" / "Tableau" to "Task board" / "Tableau de tâches".

---

Provider hardening, vision gating defense-in-depth, and `xhigh` reasoning tier (`feature/provider-hardening-vision-gating`). Three independent axes shipped together: (1) every outbound HTTP request from the app now carries a `User-Agent: ZileoChat/<version>` header so upstream providers can correlate traffic to a specific release, (2) a fourth `ReasoningEffort::XHigh` variant ("Think Max") is exposed in the Agent settings selector for the model families that accept it (DeepSeek, GPT-5.x, Grok, Claude Opus -- case-insensitive substring match against `api_name`), and (3) image attachments are now blocked on four independent layers when the active model is not flagged `supports_vision: true`, so a model swap or a misconfigured row can no longer ferry images upstream and trigger a 400 from the provider.

### Added

- **`ReasoningEffort::XHigh` variant** (`src-tauri/src/models/agent.rs`) -- 4th tier serialized as `"xhigh"`, forwarded verbatim to OpenAI-compatible providers, collapsed to `"high"` for Mistral (which has no equivalent), and mapped to a 16384-token budget in `effort_to_max_tokens()` (providers clamp to their own ceiling). The UI Select exposes it only when the model's `api_name` matches the `XHIGH_MODEL_PATTERNS` allowlist (`deepseek`, `gpt-5.`, `grok`, `claude-opus`), kept deliberately broad so future point releases inherit the tier without re-editing.
- **Canonical HTTP `User-Agent`** (`src-tauri/src/llm/http.rs`) -- new `HTTP_USER_AGENT` constant (`ZileoChat/<CARGO_PKG_VERSION>`, resolved at compile time) and `default_http_client_builder()` factory used by every reqwest client in the codebase (LLM manager, Mistral / Ollama / OpenAI-compatible providers, embedding service, Voxtral STT). Every new HTTP client must start from this builder.
- **`validate_attachments` IPC command** (`src-tauri/src/commands/message.rs`) -- pre-send validator invoked from `ChatInput.svelte` that rejects image attachments when the active workflow's agent resolves to a non-vision model. Resolves the agent then model lookup through the new `resolve_workflow_supports_vision` / `resolve_agent_supports_vision` helpers and fails closed on any DB error.
- **`FileManagerTool::new(folders, supports_vision)` signature** (`src-tauri/src/tools/file_manager/`) -- the `read_image` operation is now omitted from both the `definition()` and the JSON schema when `supports_vision = false`, and the `execute()` path additionally refuses the operation with a clear error if the model somehow forges the call. Factory wired through `factory_creation.rs` so every code path that builds a FileManager honours the flag.
- **`xhigh` i18n keys** (`messages/en.json` + `fr.json`) -- `agents_reasoning_xhigh` label ("Think Max" / "Reflexion Max") plus surrounding helper strings.

### Changed

- **`ChatInput.svelte` defense-in-depth on image attachments** -- paste / picker / drop handlers all hard-block when the agent's model is non-vision; a `$effect` watches the agent/model selection and auto-strips any already-attached images on switch, surfacing a sticky toast with the count so the user is never silently dropped. Image picker button is hidden entirely when the model lacks vision.
- **`AgentForm.svelte` reasoning selector** is now model-aware: `getReasoningOptions(provider, t, modelApiName)` only emits the `xhigh` option for matching families, and `normalizeReasoningEffortForProvider` auto-downgrades stored `xhigh` to `high` when the user switches the agent to a non-matching model so the form state stays consistent with the visible options.
- **All reqwest client construction** (`llm/manager.rs`, `llm/mistral.rs`, `llm/ollama.rs`, `llm/openai_compatible.rs`, `llm/embedding/service.rs`) now starts from `default_http_client_builder()` instead of `reqwest::Client::builder()` directly.
- **`/scripts/` directory** is now gitignored so local diagnostic helpers (one-off API probes, repro scripts) stay out of the repository.

### Fixed

- **Vision-capability lookup on SCHEMAFULL agent rows** -- `resolve_agent_supports_vision` now does `SELECT llm FROM agent` then walks the nested object client-side (`r["llm"]["model"]`) instead of relying on a nested-AS projection (`SELECT llm.model AS model`) which returned an empty or differently-wrapped result on SCHEMAFULL tables, silently making every agent look non-vision and fail-closing the attachment validation even on correctly-configured rows.
- **`api_name` lookup is now provider-scoped** -- two custom providers can legitimately expose the same `api_name` with divergent `supports_vision` flags; the previous `WHERE api_name = $x` returned the first match by insertion order, occasionally trusting the wrong row. The `WHERE` clause now additionally constrains `string::lowercase(provider) = string::lowercase($p)`, so an agent on a non-vision custom-provider model no longer sees `read_image` exposed (which previously caused upstream 400s when the model bounced the image part).

### Notes

- **Vision gating is defense-in-depth, not a single chokepoint**: UI hard-block (paste/picker/drop), UI auto-strip on model switch with a sticky toast, IPC `validate_attachments` rejecting on the wire, and the `FileManagerTool` itself omitting `read_image` from its schema and refusing the operation at `execute()` time. Every layer fails closed on any database error so a transient SurrealDB hiccup blocks the send rather than allowing it through.
- **Why broad substrings for the `xhigh` tier rather than an exact allowlist**: gateways add point releases on their own cadence (`gpt-5.1`, `claude-opus-4.7`, `deepseek-v4-pro`). An exact-match list would silently hide the tier from users every time a vendor renames; a substring match means false positives surface a clean upstream error ("model does not support xhigh") and false negatives never occur. `gpt-5.` deliberately requires the trailing dot so the base `gpt-5` (which only exposes low/medium/high) is excluded.
- **`User-Agent` is set at the client level, not per-request**: every reqwest client built via `default_http_client_builder()` carries the header by default, which means new HTTP-touching code automatically inherits the convention as long as it starts from the builder. The version is resolved at compile time via `env!("CARGO_PKG_VERSION")` so release tags propagate without any runtime overhead.

---

Kanban supervisor page and agent kind (`feature/kanban-page-and-agent`). A new `/kanban` page hosts a 4-column board (`todo / doing / review / done`) where the user composes work items, hands them off to standard agents for execution, and reviews the resulting reports. Two execution modes: **Auto** (a dedicated "Kanban" agent kind composes the card from a free-text description, picks the target agent, fills the prompt variables, then submits via the new `SubmitComposedCardTool`) and **Manual** (the user picks agent + prompt + variables themselves). A tokio scheduler ticks every 60s to (1) push cards in the `ready` state to `doing` via the `WorkflowExecutorService`, (2) honour recurrence schedules (`days_of_week / hour / minute / skip_if_pending`), and (3) auto-purge cards stuck in `done` for more than 3 days unless they are the template of an enabled recurrence (the user's blueprint). Prompts and skills now snapshot a version row on every update and expose a `VersionsHistoryModal` in their settings forms for restore and diff. Strict separation: Kanban-kind agents only see the supervisor toolkit (`PromptManagerTool`, `SkillManagerTool`, `WorkflowManagerTool`, `ListAgentsTool`, plus the private compose/analyze submit tools); standard agents never see those, and Kanban agents cannot be delegated to.

### Added

- **`/kanban` page** (`src/routes/kanban/+page.svelte` + 12 components under `src/lib/components/kanban/`) — 4-column board (`todo / doing / review / done`), per-card report viewer with inline interaction history, recurrence schedule modal, "Improve prompt" feedback loop modal, and a creator panel split into `KanbanCardCreatorAuto` (free-text description fed to a Kanban agent) and `KanbanCardCreatorManual` (explicit agent + prompt + variables form).
- **Kanban scheduler** (`src-tauri/src/commands/scheduler.rs`) — tokio loop, 60s tick, three responsibilities: transition `ready` cards to `doing` through `WorkflowExecutorService`, run recurrences (`kanban_schedule.days_of_week / hour / minute / next_run_at / skip_if_pending / enabled`), and auto-purge stale `done` cards (>3 days, never if the card is the template of an enabled schedule). Emits `kanban:cards_purged` to refresh the board live. A `workflow_complete` listener transitions the linked card to `review` and writes the analyzer verdict when `auto_analyze_reports` is on.
- **Agent kind `Kanban`** (`AgentConfig.kind: Option<AgentKind>`) — strict separation enforced at the toolkit boundary: Kanban agents receive `PromptManagerTool`, `SkillManagerTool`, `WorkflowManagerTool`, `ListAgentsTool` and the two private submit tools (`SubmitComposedCardTool`, `SubmitAnalysisTool`); they have no access to the standard skill / tool catalogue and cannot be delegated to. UI Settings filter the agent/skill/tool pickers according to the kind.
- **Four new supervisor tools** (`src-tauri/src/tools/`):
  - `ListAgentsTool` (private, auto-injected on Kanban agents) — discovery of standard agents: returns name, system prompt summary, `folders`, `has_file_manager`, available skills.
  - `SubmitComposedCardTool` (private, auto-injected during compose) — finalize the composed card. Validates the prompt-variable contract (set diff between the prompt's declared variables and the agent-provided keys) before persisting.
  - `SubmitAnalysisTool` (private, auto-injected during analyze) — finalize the verdict (`approve | reject | needs_improvement`) and an optional `suggested_prompt_edit` consumed by the "Improve prompt" UI.
  - `PromptManagerTool` (Kanban-only) — `list / get / create / update` on prompt templates. No delete. Every `update` auto-snapshots a `prompt_version` row.
- **`SkillManagerTool` refactor + versioning** (`src-tauri/src/tools/skill_manager/` folder: `mod.rs`, `crud.rs`, `grant.rs`, `validators.rs`, `versions.rs`, `tests.rs`) — `list / get / create / update / grant / revoke / list_versions / restore_version`. Reserved to Kanban agents. Every `update` auto-snapshots a `skill_version` row; restore writes a new snapshot before applying.
- **`WorkflowManagerTool`** (Kanban-only) — `list_workflows / rename_workflow / folders_crud / read_workflow / list_workflow_errors / list_workflow_sub_agents`. Gives the Kanban analyzer access to historical workflow data so it can ground its verdict on real tool errors and sub-agent reports.
- **Prompt / skill version history** — new tables `prompt_version` and `skill_version` (full snapshot of `name / description / category / content` plus `edited_by`, `edit_summary`, `edited_at`, `version` integer). Tauri commands `list_prompt_versions / get_prompt_version / restore_prompt_version / delete_prompt_version` and the four matching `*_skill_version` commands. `delete_*_version` enforces a "last-one" safeguard (the final version is never deletable so the audit trail cannot be wiped). UI: `VersionsHistoryModal` opens from `PromptForm` and `SkillForm` with a versions count badge and z-index fix.
- **Persistence schema** — 5 new SurrealDB tables: `kanban_card`, `kanban_schedule`, `kanban_card_interaction`, `prompt_version`, `skill_version`. Two new optional booleans on `agent`: `require_file_confirmation: bool` (default `true`, already in use for FileManager destructive ops) and `auto_analyze_reports: bool` (default `false`, triggers the analyzer on `workflow_complete`). Both deserialize null-tolerant via shared deserializers so legacy rows stay readable.
- **Cross-page streaming** — `backgroundWorkflowsStore.init()` moved up to the root `+layout.svelte`. Workflows launched from `/kanban` now keep streaming when the user navigates to `/agent`, and vice versa. Previously, leaving the launching page tore down the Tauri event listener and the chunks were lost.
- **i18n** (`messages/en.json` + `fr.json`, 156 new keys each) — full Kanban surface translated. User-facing "Kanban" is rendered as **Tableau** (FR) / **Board** (EN), and the agent kind reads **Superviseur** / **Supervisor**. The technical term "Kanban" is kept in code, schema, types, and developer docs.
- **Shared `edit_summary` validator** (`src-tauri/src/security/validation.rs`) — single source of truth for the optional summary field on prompt/skill updates: trim, 256-char cap, control-character rejection.

### Changed

- **`KanbanCardCreator` split** into two sibling sub-panes (`KanbanCardCreatorAuto.svelte` + `KanbanCardCreatorManual.svelte`) behind a tab switch — clearer mode boundary, no shared mutable state between the two flows.
- **`AgentForm` (Settings)** gains a kind picker (`standard | kanban`), an `auto_analyze_reports` toggle (visible on Kanban only), and conditional tool/skill pickers per kind.
- **Default Kanban-agent toolkit at creation time** (`src-tauri/src/tools/factory_creation.rs`) — when an agent is created with `kind: Kanban`, the supervisor tools are pre-selected; standard tools are filtered out client-side and rejected server-side.
- **Card execution path** routed through `WorkflowExecutorService` and persists `kanban_card.workflow_id` so the board can deep-link to the running workflow on `/agent`.

### Fixed

- **`?workflow=` deep-link on `/agent`** is honoured on first load (the agent page used to ignore the query param if the workflow was streaming from a different page; now the listener attaches before reading the URL).
- **Card `workflow_id` persistence** — was lost on the scheduler-driven execution path, breaking the "Open workflow" button on the card report viewer. Now persisted in the same transaction that flips the card from `ready` to `doing`.
- **SurrealDB `ORDER BY column_order`** on the board query — column order survives reload, and bool fields on `agent` deserialize as `false` when the row is null instead of failing the whole load.
- **Native `window.confirm()` removed** — destructive Kanban operations (delete card, delete schedule, delete version) now use the in-app `DeleteConfirmModal` for consistent styling and tab focus management.
- **Cards stuck in `doing`** can now be force-deleted from the report viewer (e.g. when a workflow crashed without emitting `workflow_complete`). The cascade still removes the linked `kanban_card_interaction` rows; `workflow` rows are preserved.
- **`svelte-ignore` directives removed** — root causes fixed instead (`untrack()` for `state_referenced_locally`, real keyboard handlers + `role="presentation"` on click-backdrops). No silenced warnings remain in the Kanban surface.
- **Version history modal z-index** — was rendered below the surrounding settings sheet on some viewport ratios; now stacked at the topmost layer with a count badge surfacing the number of stored versions on the trigger button.

### Notes

- **Strict separation rationale**: Kanban agents are an orchestration role, not a delegate target. Allowing a standard agent to call a Kanban agent (or a Kanban agent to call another Kanban agent) would create supervision loops and circular toolkits. The boundary is enforced both server-side (factory + tool registry) and client-side (Settings pickers, delegate listings).
- **Variable contract on `SubmitComposedCardTool`**: the auto-compose flow validates that the keys the Kanban agent provides exactly match the variable names declared on the chosen prompt template (set diff, no implicit defaults). Mismatches reject the submission and feed back into the next iteration so the model corrects itself.
- **Version snapshots are storage-cheap on prompts/skills** (markdown text only — no embeddings, no large blobs). The decision to snapshot eagerly on every `update` (rather than on user demand) trades a few KB per edit for a deterministic audit trail and one-click restore.
- **Cross-page streaming via root layout**: previously the `backgroundWorkflowsStore.init()` lived in `/agent/+page.svelte`, which meant navigating away from `/agent` torn down the Tauri event listener and the in-flight workflow lost its chunks. Hoisting `init()` to `+layout.svelte` makes the store outlive any single page, with a single global listener for the lifetime of the app session.
- **Auto-purge safety**: the 3-day window applies only to `done` cards that are NOT the template of an enabled `kanban_schedule`. Recurrence templates are blueprints and must survive purge to fire again on their next scheduled tick. Linked `kanban_card_interaction` rows cascade; `workflow` rows are deliberately preserved so the completed run remains consultable from `/agent` independently of the card.

---

## [0.25.0] - 2026-05-20

Push-to-talk voice dictation via Mistral Voxtral (`feature/dictee-vocale-mistral-stt`). Global microphone button in the top navigation (next to the "Zileo Chat" title) plus a `Ctrl+Shift+Space` hotkey — both record while held and transcribe on release. The resulting text is inserted at the cursor of the originally-focused field (textarea or text-like input), so dictation works in the chat composer, in agent forms, in settings, anywhere a text field has focus. Configured under **Settings > Voice Dictation**: enable toggle, Voxtral model id (validated against an allowlist), free-text context-bias hints (project nouns / jargon that get forwarded into the prompt), and an optional language override (auto / explicit ISO 639-1). Tests verts : 1463 Rust lib (+10 STT) + clippy `--all-targets` clean + svelte-check 4262 / 0 errors + 522 Vitest (+70 STT) + ESLint / Prettier clean. Zero schema migration. One new Linux-only Cargo dependency (`webkit2gtk = "2.0"`, target-conditional).

### Added

- **`MicButton` component** (`src/lib/components/ui/MicButton.svelte`) embedded in `FloatingMenu` — 36 px round button sized to match the neighbouring icon buttons. Pointer-down and `Ctrl+Shift+Space` keydown both transition the singleton STT store from `idle` to `recording`; pointer-up and keyup transition to `transcribing`, then back to `idle` after Voxtral returns. Tooltip surfaces the hotkey.
- **Push-to-talk state machine** (`src/lib/stores/sttStore.svelte.ts`) — singleton Svelte 5 `$state` store with phases `idle → recording → transcribing → idle`. Idempotent against concurrent triggers (pointer-up arriving while a hotkey is held doesn't cancel; double-press doesn't spawn a second `MediaRecorder`). Owns the `MediaRecorderSession` plus a snapshot of the editable field captured _before_ recording starts.
- **Audio capture wrapper** (`src/lib/utils/audio-capture.ts`) — `pickSupportedMime` walks a priority codec probe chain (`webm/opus → webm → ogg/opus → mp4/mp4a → mp4`) and returns the first MIME the browser claims; `startRecording` / `stopRecording` / `cancelRecording` manage the `getUserMedia` lifecycle with explicit microphone constraints (16 kHz mono, echo cancellation, noise suppression); typed `AudioCaptureError` with seven kinds (`permission-denied / no-codec / no-device / too-short / too-large / empty / recorder-failed`) lets callers branch without string parsing; `blobToBase64` chunks the encoding (32 KB) to avoid `Maximum call stack size exceeded` on large blobs, with a Node `Buffer` fallback for jsdom tests.
- **DOM insertion helpers** (`src/lib/utils/dom-insert.ts`) — `captureActiveField(doc?)` snapshots the focused `<input>` (text-like only — `password` is explicitly excluded) or `<textarea>` plus its selection range, returns `null` for any non-editable element; `insertTextIntoField(target, start, end, text)` writes via `setRangeText` then dispatches a synthetic `InputEvent('input', {bubbles:true, inputType:'insertText'})` so Svelte 5's `bind:value` actually picks the change up — without the synthetic event, the bound state stays stale and the next re-render overwrites the inserted text. Selection range is clamped against `target.value.length` in case the user truncated the field while transcription was in flight.
- **Backend STT module** (`src-tauri/src/llm/stt/`, with `mod.rs` + `mistral_batch.rs`) — provider-agnostic `transcribe_audio_core(db, audio, settings)` selects the configured backend (currently Mistral Voxtral batch endpoint via `multipart/form-data` upload of the audio blob + model id + optional context bias prompt + optional language hint). Validates `SUPPORTED_AUDIO_MIMES` (mirror of the frontend allowlist), `MAX_AUDIO_BYTES = 25 MiB`, `MIN_AUDIO_BYTES = 512`. Designed to receive sibling providers (Ollama Whisper, OpenAI Whisper) without touching the call site.
- **`transcribe_audio` Tauri command** (`src-tauri/src/commands/stt.rs`) — receives `{mime_type, data_base64}` from the frontend, validates the base64 payload size against `MAX_AUDIO_BASE64_LEN` (binary cap × 4/3 with documented math proof in the source), loads the live `STTSettings`, delegates to the provider-agnostic core, returns the transcript string.
- **STT settings persistence** (`src-tauri/src/commands/settings_stt.rs` + `src-tauri/src/models/stt.rs`) — `get_stt_settings` / `update_stt_settings` / `reset_stt_settings` persist `STTSettings` as a JSON blob on the `settings` table (key `settings:stt`, no dedicated SurrealDB schema). `apply_update` mutates a working copy and is unit-tested in isolation: enable toggle, model id allowlist, context-bias trim + drop-empties + 10 × 200-char cap + control-char rejection, ISO 639-1 language allowlist, and a tri-state language override (`Option<Option<String>>` via the shared `deserialize_explicit_option` helper — `absent` keeps the current value, `null` clears to auto, `Some(code)` sets explicit).
- **Settings page** (`src/routes/settings/speech-to-text/+page.svelte`) bound to the new `sttSettings` store — enable toggle, model id field (with inline allowlist validation via `stt-validation.ts`, mirroring the backend), context-bias chips, language dropdown, `Ctrl+Shift+Space` hint, save / reset buttons.
- **Linux WebKitGTK permission hook** (`src-tauri/src/main.rs`, `cfg(target_os = "linux")` block) — wires `connect_permission_request` on the main webview, downcasts the request to `UserMediaPermissionRequest`, allows only when `is_for_audio_device()`, denies every other permission class (video, geolocation, notifications, …). Without this hook, `wry`'s WebKitGTK backend silently denies `getUserMedia` on Fedora / Ubuntu builds.
- **macOS microphone usage description** (`src-tauri/Info.plist`) — `NSMicrophoneUsageDescription` is auto-merged into the bundled `Info.plist` so macOS surfaces a permission prompt the first time the user records.
- **i18n strings** (`messages/en.json` + `fr.json`, 56 new keys each): settings labels, tooltips, push-to-talk hint, error toasts.
- **+70 Vitest cases** covering the new utilities and the state machine end-to-end:
  - `stt-validation.test.ts` (11): model id boundary + casing.
  - `dom-insert.test.ts` (17): `captureActiveField` whitelist (password excluded) and `insertTextIntoField` selection-clamp + `InputEvent` dispatch.
  - `audio-capture.test.ts` (23): `pickSupportedMime` order, `AudioCaptureError` kinds, `blobToBase64` (Node Blob polyfill for jsdom), `startRecording` / `stopRecording` / `cancelRecording` with a `FakeMediaRecorder`.
  - `sttStore.test.ts` (19): attach/detach, `startRecording` error mapping, `stopAndTranscribe` happy path plus every failure branch, `cancel` idempotency.

### Changed

- **`Cargo.toml`** adds `webkit2gtk = "2.0"` under `[target.'cfg(target_os = "linux")'.dependencies]`. macOS and Windows builds are unaffected.
- **`src/routes/+layout.svelte`** no longer mounts a global mic FAB (the initial commit shipped a bottom-right FAB that collided with the toast stack). The mic now lives inside `FloatingMenu`, which is already present on every page.

### Notes

- **Push-to-talk only — no streaming transcription.** The Mistral platform exposes both a batch endpoint and a realtime websocket-based endpoint; the v1 here is batch-only. Realtime is on the roadmap once we know what UX (per-word streaming insert vs. final-on-release) users prefer.
- **Capture the field _before_ `getUserMedia`.** On WebKitGTK Linux, the permission request UI briefly steals focus from the textarea — even when the permission is auto-allowed by the new permission hook. Reading `document.activeElement` _after_ `startRecording()` would return `<body>` and the transcript would land in nowhere. The store snapshots the editable element + its selection range before opening the microphone, and re-uses that snapshot at insertion time.
- **Defense in depth on the prompt-injection vector.** The context-bias entries are forwarded verbatim into the Voxtral prompt, so a malicious string (newline + role injection) could attempt to coerce the model. The backend validator rejects any entry containing `c.is_control()` (NUL/LF/CR) and caps each entry at 200 characters; the language field is constrained to a closed ISO 639-1 allowlist. The same control-character rule applies to the existing message attachment `name` field (consistent posture across two adjacent surfaces).
- **No schema migration.** `STTSettings` is persisted as a JSON blob on the `settings` table under the key `settings:stt`. Fresh installs get the defaults via `Default::default()`; legacy installs without the key get the same defaults on first read.
- **Hotkey choice (`Ctrl+Shift+Space`).** Picked because it does not collide with any existing OS shortcut on Linux / macOS / Windows, and because `Space` alone would conflict with the textarea's natural typing. The hotkey is registered in `MicButton`'s `onMount` so it's only active while the button is on screen (which is always — `FloatingMenu` is mounted on every page).

---

Multimodal vision support cross-provider (`feature/multimodal-vision-support`). End-to-end image attachments for user messages: paste from clipboard, file picker (`@tauri-apps/plugin-dialog`), drag&drop on the `ChatInput` frame, thumbnails preview with per-image remove, click-to-open in `MessageBubble`, persistence across reloads via SurrealDB. Backend persists raw base64 + MIME on `message.attachments`, re-emits multipart `content[]` arrays for replay, normalizes per-provider at body-build time (Mistral / OpenAI / Custom / Ollama). New `FileManagerTool.read_image` operation lets agents load an image from authorized folders and analyze it on the next iteration. New manual `supports_vision` flag on `LLMModel` (checkbox in `ModelForm`) drives a soft warning in `ChatInput` when the user attaches an image while the selected model is not flagged multimodal. Tests verts: 1453 Rust lib + clippy `--all-targets` clean + svelte-check 4260 / 0 errors + 452 Vitest + ESLint / Prettier clean. Zero new npm or cargo dependency.

### Added

- **`MessageAttachment` cross-language type** (`models/message.rs` + `src/types/message.ts`). Carries `kind: "image"`, `mime_type`, `data_base64` (raw, no `data:` prefix), optional `name` + `size_bytes`. Mirrored 1:1 from Rust to TypeScript via the existing `$types` alias. Persisted on the `message` table under a new `attachments option<array<object>>` schema field with all five sub-fields explicitly declared (SCHEMAFULL would otherwise drop dynamic keys).
- **`LLMModel.supports_vision: bool`** with the eight sync points cloned from the existing `is_reasoning` pattern: SurrealDB schema field with `DEFAULT false`, struct field with `#[serde(default)]`, `CreateModelRequest` field, `UpdateModelRequest` field, `LLM_MODEL_SELECT_COLUMNS` with `(supports_vision ?? false) AS supports_vision` for legacy rows, `insert_model_record`, `update_model` SET clause, and the matching TypeScript field on the three TS interfaces.
- **Image attachments in `ChatInput`** — three input methods: `onpaste` filters `clipboardData.items` for image kinds and routes them through the attachment pipeline (preventing the textarea's default text-fallback); a new `Paperclip` button opens the Tauri dialog (`png/jpg/jpeg/webp/gif` filter, multi-select); `ondragover` + `ondrop` on the `chat-input-frame` accept dropped files. Each input feeds `addAttachmentFromFile` which validates the size (4 MB cap), MIME whitelist, and count (8 max), then runs `processImageFile` (canvas resize to max 1568 px on the longest side, GIF → PNG, base64 encode). Thumbnails render in a `.attachments-preview` row above the textarea with a remove button per image.
- **`FileManagerTool.read_image` operation** (`tools/file_manager/operations.rs::op_read_image`). Validates the path against authorized folders, extension whitelist (`png/jpg/jpeg/webp/gif`), and size cap (8 MB). Returns `{path, mime_type, size_bytes, name, _image_attachment: {...}}`. The `_image_attachment` sentinel is consumed in `agents/execution/iteration.rs` right after `format_tool_result`: a synthetic `role: "user"` multipart message is queued, carrying a `[text: "[Image loaded from …]", image_url: {…}]` array so the next LLM turn actually sees the picture (every adapter stringifies `role: "tool"`, so the image cannot live there).
- **`read_image_for_attachment` Tauri command** (`commands/file_manager.rs`). Reads the file selected through the Tauri dialog, validates extension + size (4 MB cap), and returns `{data_base64, mime_type, size_bytes, name}` to the frontend. Used by `ChatInput`'s picker — the dialog already provides OS-level consent, so this path intentionally does not cross-check against the agent's `authorized_folders`.
- **i18n strings** (`messages/en.json` + `fr.json`): `chat_attach_image`, `chat_attachment`, `chat_remove_attachment`, `chat_image_too_large`, `chat_image_unsupported_format`, `chat_max_attachments_reached`, `chat_warning_model_no_vision`, `models_supports_vision`, `models_supports_vision_help`.

### Changed

- **`ChatMessage.content` (openai_compatible) / `MistralMessage.content` (mistral) refactored from `String` to `serde_json::Value`** so the same struct serializes a plain-text message and a multipart `[{type:"text"}, {type:"image_url"}, ...]` array. Touches the two `complete()` non-tool paths only; the tool path (`ToolChatRequest`) already used `Vec<serde_json::Value>`.
- **Per-provider image-url shape** centralized in the new `llm::image_format` module. `build_image_content_part_openai` emits the default OpenAI shape (`{type: "image_url", image_url: {url}}`) used by `load_conversation_history` and `tool_loop::build_initial_messages`. Mistral's tool-request builder calls `normalize_messages_for_mistral` (re-shapes any `image_url` object to a bare string per Mistral's non-standard schema). Ollama's `complete_with_tools` calls `normalize_messages_for_ollama_native_api` (flattens content arrays into `content: "<text>"` + sibling `images: [base64]`, stripping the `data:` prefix — the format `/api/chat` actually accepts). All normalizers are idempotent.
- **`save_message`** accepts a new optional `attachments` parameter, validates it through `save_message_core` (PAT_RUST_015 testability extraction): user role only, count ≤ 8, MIME in the whitelist, payload size cap mirroring the frontend cap. **`execute_workflow_streaming`** accepts the same optional parameter and `build_task` surfaces it under `task.context["pending_attachments"]` for `build_initial_messages` to consume.
- **`load_workflow_messages_core`, `load_workflow_messages_paginated`, `load_conversation_history`** widen their SELECT to include `attachments`. The replay path in `load_conversation_history` emits multipart `content[]` arrays for any historical row carrying attachments, so multi-turn conversations preserve images across reloads.
- **`MessageBubble.svelte`** renders thumbnails above the message body when `message.attachments` is present (200 px max, click opens the original in a new tab via `data:` URL).
- **`ModelForm.svelte`** exposes a `supports_vision` checkbox right under the existing `is_reasoning` checkbox. Defaults to `false` for new models, mirrors the DB value on edit. **No auto-detection** — the user explicitly toggles it.

### Notes

- **Out of scope for v1** (deliberate, flagged in the spec): auto-detection of vision capability (Ollama `/api/show.capabilities`, OpenRouter `architecture.input_modalities`); whitelist regex of known multimodal model names; file-system storage of attachments (currently base64-in-DB — acceptable at 4 MB × 8 = 32 MB cap per message, and Mistral's upstream is 10 MB / file); a dedicated `ImageContent` type in `mcp/protocol.rs` (no MCP tool in Zileo returns an image today); export/import support for attachments. The `#[serde(default)]` on the new field keeps old export bundles backward-compatible.
- **Soft warning, no hard block** when the user attaches an image with `supports_vision === false`: the spec called for a banner, not a gate. Some users want to test how a non-vision model responds; the message goes through and the user sees the model's reply (typically "I cannot see the image"). Backend never rejects on the flag.
- **Pre-existing `MessageBubble` test fixtures** required adding `supports_vision: false` to three TS literal-typed `LLMModel` mocks (`AgentForm.helpers.test.ts`, `llm.test.ts`, `tokens.test.ts`) after the TS interface became strict on the new field.
- **`ChatMessage.content` widened to `serde_json::Value`**: a deliberate type-system relaxation. The trade-off was either (a) keep a typed `String` and route image payloads through a parallel struct, or (b) accept `serde_json::Value` and rely on each call site to construct the right shape. Option (b) keeps a single `messages: Vec<ChatMessage>` field through the entire `complete()` path without forking the data model; the only call sites are `openai_compatible::complete()` and `mistral::custom_complete()`, both confined to system/user prompt building.

---

Persisted-field struct sync drift — workflow cache totals + sub-agent cache/thinking round-trip + full-state SELECT delegation (`fix/persisted-field-struct-sync-drift`). Three latent struct-sync drifts of fields written to SurrealDB via raw `UPDATE`: (1) `workflow.total_cached_tokens` / `total_cache_write_tokens` were written by the streaming pricing updater and defined on the schema, but the centralized SELECT constant in `db::queries::workflow::FIELDS` omitted them — every reload silently zeroed the cumulative cache columns in `TokenDisplay`; (2) `sub_agent_execution.{cached_tokens, cache_write_tokens, thinking_tokens}` were written on completion and persisted, but the Rust struct lacked the three fields, two replay SELECTs ignored them, the `merge_into_chat_blocks` projection never surfaced them, the live wire `SubAgentStreamMetrics` chunk omitted them, and the entire frontend chain (`SubAgentBlockData`, `SubAgentExecution`, `SubAgentStreamMetrics`, `SubAgentSummary`, `ActiveSubAgent`) was unaware they existed — cost of storage + cost of write for zero observable signal; (3) `load_workflow_full_state`'s three hand-rolled child-entity SELECTs diverged from the canonical loaders (`load_workflow_messages`, `load_workflow_tool_executions`, `load_workflow_thinking_steps`), omitting `thinking_tokens` / `cached_tokens` / `cache_write_tokens` / `model_id_used` from messages, `sequence` from tool executions, `sequence` + `source` from thinking steps — no observable bug today since the only caller is a freshly-created workflow, but a trap if it ever serves a real recovery. Tests verts: 1434 Rust lib (+10) + 24 integration + 2 doctests + clippy `--all-targets` clean + svelte-check 4259 / 0 errors + 452 Vitest (+3) + ESLint/Prettier clean. Zero schema migration, zero new dependency, zero breaking IPC change.

### Fixed

- **`workflow.total_cached_tokens` / `total_cache_write_tokens` survive reload** (`src-tauri/src/db/queries.rs`). Both columns are now coalesced (`(total_cached_tokens ?? 0) AS total_cached_tokens`) in the centralized `FIELDS` constant feeding `SELECT_BASE` / `SELECT_LIST` / `RETURN_FIELDS`. Six commands (`load_workflows`, `update_workflow_name`, `move_workflow_to_folder`, `move_workflows_to_folder`, `toggle_workflow_pinned`, `load_workflow_full_state`) now return the cache totals instead of `None`. The `Workflow` struct in `src-tauri/src/models/workflow.rs` was switched from `Option<u64>` to plain `u64` with `#[serde(default)]` for symmetry with the other counters and to simplify the TypeScript downstream (`Workflow.total_cached_tokens: number`). Locked by a new regression test in `db/queries.rs` asserting the constant contains both column names.
- **`SubAgentExecution.cached_tokens` / `cache_write_tokens` / `thinking_tokens` are now round-tripped through the full chain** (struct → SELECT × 2 → projection → wire → live emit → TS types × 5 → UI). `SubAgentBlock.svelte` displays a new conditional cache row (`cache: X +write: Y` `thinking: Z`) visible both live (from the `sub_agent_complete` chunk) and on replay (from `merge_into_chat_blocks`). `MessageMetrics.svelte` displays `cache:X` and `think:Y` mini-chips per sub-agent. The chain is locked by 6 backend tests (struct Some/None/missing, wire shape, projection Some/None) and 3 Vitest tests (live chunk → block, legacy provider → undefined, chunk processor → ActiveSubAgent.metrics).
- **`load_workflow_full_state` now delegates to the canonical SELECTs**. Extracted `load_workflow_messages_core`, `load_workflow_tool_executions_core`, `load_workflow_thinking_steps_core` (`pub(crate)` helpers taking `&DBClient`) and a `load_workflow_full_state_core(db: &Arc<DBClient>, workflow_id: &str)` wrapping the parallel `tokio::try_join!` block. The Tauri command becomes a 1-line delegator. Removes the divergent inline queries that omitted `thinking_tokens` / `cached_tokens` / `cache_write_tokens` / `model_id_used` from messages, `sequence` from tool executions, `sequence` + `source` from thinking steps. The orphan `query_and_deserialize` helper (sole caller was the previous full_state) is removed. Locked by 3 integration tests in `commands/workflow_tests.rs` seeding real DB rows with the previously-dropped columns and asserting they survive the round-trip.

### Notes

- **Persisted-field struct sync recipe**: any field written to SurrealDB via raw `UPDATE` (i.e. not via serde `.create().content()`) must mirror through (a) the `DEFINE FIELD` on the schema, (b) the struct field in `models/<table>.rs`, (c) every SELECT replay of that table including the canonical loader AND `load_workflow_blocks` AND `load_workflow_full_state`, (d) the `merge_into_*` JSON projection when applicable, (e) the live `*StreamMetrics` wire shape when also emitted live, (f) the live emit site (`emit_complete_event` etc.), (g) the consumer TypeScript type, (h) the UI component if surfaced. Drift is silent — no compiler error, no runtime warning, just a quietly-zeroed field on reload. Audit recipe after any new raw UPDATE: grep `<field>` recursively across `src-tauri/src/` and `src/` and verify each link in the chain.
- **Backward compatibility**: extension is additive on serde (`#[serde(default, skip_serializing_if = "Option::is_none")]` for the 3 new sub-agent fields, `#[serde(default)]` for the 2 workflow fields that switched type). Legacy DB rows missing the columns deserialize as `0` (workflow) or `None` (sub-agent) — both safe defaults. No IPC contract change observable from the frontend.
- **Decoupling rationale for `load_workflow_full_state_core`**: extracted not just for test ergonomics but to guarantee the delegation to the three canonical loaders survives any future refactor. Adding a column to messages now requires editing exactly one SELECT.

---

Cost display gaps — live per-iteration cost + per-sub-agent cost + ModelForm Ollama UX + tokenStore reset (`fix/cost-display-gaps`). Three independent display gaps that all converged on the same surface: (1) the per-message cost was missing live during a workflow run and only appeared at completion, (2) the per-sub-agent cost was never propagated to the UI even though the backend computed it, (3) the `TokenDisplay` metrics bar kept stale values after a workflow deletion. A fourth, unrelated UX gap was bundled: the model form exposed cache-price fields for Ollama providers even though the Ollama adapter unconditionally forces `cached_tokens` / `cache_write_tokens` to `None` — decorative inputs that misled users. Tests verts: 1424 Rust lib (+7) + clippy `--all-targets` clean + svelte-check 4259 / 0 errors + 449 Vitest (+3) + ESLint/Prettier clean.

### Added

- **Live per-iteration cost on the streaming `iteration_progress` chunk**. The backend now loads each agent's pricing row once at the start of `execute_with_tools` (new `PricingCache` in `agents/execution/tool_loop.rs`, replacing N per-iteration DB queries) and emits the iteration's incremental cost via `tokens.iter_input` / `iter_output` / `iter_cached` / `iter_cache_write`. The frontend `chunkProcessor.handleIterationProgress` sums those deltas into `partialCostUsd` while a workflow runs and the final `setSessionCost(metrics.cost_usd)` overwrites the running sum at completion (backend-as-source-of-truth invariant). Helper `compute_iteration_local_cost(tokens, pricing) -> f64` extracted as a pure function (FN_RUST_026) with 3 unit tests covering no-pricing, basic case, and cache-savings paths.
- **Per-sub-agent cost on the SubAgentBlock UI**. `SubAgentStreamMetrics.cost_usd: Option<f64>` (serde `skip_serializing_if`) added on the wire shape; `emit_complete_event` in `tools/sub_agent_executor/records.rs` propagates `result.metrics.cost_usd` computed by `compute_sub_agent_cost` against the sub-agent's OWN pricing row (not the parent's). Mirrored on the TypeScript side via `SubAgentSummary.cost_usd?`, `ActiveSubAgent.metrics.cost_usd?`, `SubAgentBlockData.cost_usd?`, and `SubAgentStreamMetrics.cost_usd?`. Rendered in `MessageMetrics.svelte` as a per-sub-agent cost chip (4 decimals below $0.01, 2 decimals above) and as a new column in the `TokenDisplay` sub-agent row.
- **`tokenStore.reset()` in `handleDeleteWorkflow` + `handleBatchDelete`** (`src/routes/agent/+page.svelte`). Without this, the metrics bar kept showing the deleted workflow's cost / token counts until the next iteration of any remaining workflow updated them. Subtle UX bug that was always present — the new live cost just made it visible.

### Fixed

- **`SubAgentExecution.cost_usd` was persisted by the DB but stripped from the replay path** — latent bug since PR #147. Sub-agent rows were `UPDATE`d with `cost_usd = $cost_usd` via raw SQL on the write path, but the `SubAgentExecution` Rust struct in `src-tauri/src/models/sub_agent.rs` had no `cost_usd` field, so the replay `SELECT` in `commands/message.rs::load_workflow_blocks` never retrieved it and `merge_into_chat_blocks` projected `None`. The live path read from a different in-memory source so the bug only surfaced on reload of a completed workflow with sub-agents — the cost chip silently went to 0. Fix: `SubAgentExecution.cost_usd: Option<f64>` + SELECT widened + projection `"cost_usd": sa.cost_usd` in `merge_into_chat_blocks`. Locked by 4 integration tests in `tests/sub_agent_tools_integration.rs` covering live emit + replay round-trip.
- **Cache-price fields hidden on the `ModelForm` when provider is Ollama**. Wrapped behind `{#if formData.provider !== 'ollama'}` in `src/lib/components/llm/ModelForm.svelte`. The Ollama adapter (`src-tauri/src/llm/ollama_adapter.rs:228-230`) unconditionally forces `cached_tokens: None` + `cache_write_tokens: None` on the wire response, so any value entered for an Ollama model was purely decorative. The form preserves the values in `formData` rather than resetting to 0 on provider switch, so toggling back to a cache-aware provider does not lose the user's input.

### Notes

- **`iteration_progress` cost MUST be a DELTA, never cumulative** — the frontend chunk processor sums `partialCostUsd += chunk.cost_usd` on every chunk. Emitting cumulative values triple-counts in a triangular fashion (iter1=0.01, iter2=0.04 instead of 0.03, iter3=0.10 instead of 0.06). One-line difference in `iteration.rs` between the two behaviours; locked by a Vitest test on the chunk-processor side.
- **Pattern frozen — `PAT_PERSISTED_FIELD_RUST_STRUCT_SYNC`**: every field written to a SurrealDB row via raw `UPDATE` (i.e. not through serde `.create().content()`) must be mirrored in the corresponding Rust struct AND in any `merge_into_*` projection on the replay path. Drift is silent — no compiler error, no runtime warning, just a quietly-zeroed field on reload. Audit recipe: after adding a new raw `UPDATE … SET <field> = …`, grep `<field>` recursively across `src-tauri/src/` and confirm the chain `struct field → SELECT → projection → serde rename` is uninterrupted.
- **`PricingCache` lives for the duration of one tool loop** (`execute_with_tools`), not for the process. Two concurrent workflow calls each pay one `load_pricing_row` DB hit at startup; subsequent iterations within the same call reuse the cached `Option<PricingRow>`.

---

Chat input — textarea auto-resize + post-send reset + hint clipping + scroll-button overlap (`fix/chatinput-textarea-resize-post-send`, merged as PR #160 — CHANGELOG backfill). Four cascading bugs on `ChatInput.svelte`: (1) the textarea grew during typing only up to ~2 lines and then froze; (2) sending a message left the textarea sized at the previous content height instead of collapsing back to the single-line default; (3) the keyboard hint was clipped on multi-line content and pushed the action buttons out of vertical alignment; (4) the floating scroll-to-bottom button overlapped the input frame as soon as the textarea grew past 80px. 100% frontend (Svelte 5 + CSS), zero Rust / IPC / DB / test changes. Tests verts: 1417 Rust lib + clippy `--all-targets` clean + svelte-check 4259 / 0 errors + 446 Vitest + ESLint/Prettier clean.

### Fixed

- **Textarea auto-resize during typing**. The `.chat-input` CSS rule was `flex: 1` (shorthand for `flex: 1 1 0%`), which per the Flexbox spec ignores any `height` value — including the inline `style.height` that the `adjustHeight()` function writes on every `oninput`. The textarea was being sized by intrinsic content (the browser fallback when `flex-basis: 0%` and no available height), giving the illusion of partial auto-resize up to ~2 lines before the fallback bottomed out. Fix: replace `flex: 1` with `width: 100%` on the textarea; the parent `.textarea-wrapper` keeps `flex: 1` so the horizontal sizing in the row-flex container is preserved.
- **Post-send reset race**. Assigning `value = ''` in Svelte 5 is async (reactive batching), so calling `adjustHeight()` immediately afterwards read the still-old `scrollHeight` from the DOM. `await tick()` mitigated but did not fully eliminate the race on slow Tauri WebView. Robust fix: write `textareaRef.value = ''` directly (synchronous DOM mutation) before calling `adjustHeight()`. The Svelte reactive assignment `value = ''` is still kept for downstream consumers (derived signals, the conditional `{#if value.trim()}` for the hint, parent bindings).
- **Keyboard hint clipped on multi-line content**. The `.keyboard-hint` was positioned `position: absolute; bottom: 4px` inside the `.textarea-wrapper`, which clipped it on multi-line content AND pushed the action buttons out of vertical alignment. Restructured: new `.chat-input-frame` wrapper hosts the background + top border; `.chat-input-container` (flex row, `align-items: flex-end`) hosts the buttons + textarea-wrapper; `.keyboard-hint` is rendered `display: block` below the container on its own line (text-align: right). The hint is no longer a flex item, so it no longer fights the textarea for vertical space.
- **Scroll-to-bottom floating button overlap**. Previously fixed at `bottom: 80px` against `.chat-container`, which collided with the input frame as soon as the textarea grew past 80px. Moved the button INSIDE `.input-area` (now `position: relative`) with `bottom: calc(100% + var(--spacing-sm))` — the button now sits exactly above the input regardless of its current height.

### Changed

- **`onpaste` safety net** in `ChatInput.svelte`: a `requestAnimationFrame(adjustHeight)` is now attached to the `onpaste` handler in addition to `oninput`. The HTML5 spec guarantees an `input` event fires after a paste, so the `oninput` handler is theoretically sufficient — but on paste payloads larger than ~10kB into a slow Tauri WebView, the layout can be stale by one frame. The double-resize is idempotent (`adjustHeight()` is a pure DOM mutation with no side effects).

### Notes

- **Pattern frozen — `PAT_CSS_FLEX_BASIS_AUTO_FOR_JS_HEIGHT`**: for any flex-item whose height is JS-controlled (auto-resize textarea, animation height, dynamic chart), use `flex-basis: auto` (or omit the `flex` shorthand entirely, or `flex: 0 1 auto`). Otherwise `style.height` is silently ignored. Particularly perfidious because the browser's content-fallback gives the illusion of partial functionality.
- **Pattern frozen — `PAT_RESET_BINDVALUE_DOM_SYNC`**: when resetting a controlled input's value AND immediately reading the DOM (e.g. for auto-resize), write to `inputRef.value` directly first, then mutate the Svelte signal afterwards. The signal still propagates to downstream consumers but the DOM is already in sync before the read.
- **Backfill rationale**: this entry was omitted from the original PR #160 (merged on main as `2297009`). Adding it here keeps the `[Unreleased]` section complete for the next release; the actual code on main is unchanged by this entry.

---

Consolidated dependency cleanup (`chore/deps-cleanup-consolide`). Eight Dependabot pull requests were rolled into a single PR alongside one dead-library removal and one stale-doc cleanup. The drop of `rig-core` (declared but never imported since the early v0.20.x days) eliminates ~50 unused transitive crates from the build. Three majors land together — TypeScript 6.0, marked 18, `@lucide/svelte` 1.16 — each verified against Context7 documentation snapshots before the bump (no compiler-option drift in TypeScript, `parse()` stayed synchronous in marked, deprecated icon aliases still re-exported in lucide 1.16 but renamed for v2 future-proofing). Tests verts after every commit: 1417 Rust lib + clippy `--all-targets` clean + svelte-check 4259 / 0 errors + 446 Vitest + ESLint/Prettier clean. Zero applicative code changed except the 41 icon renames in 13 Svelte files (mechanical).

### Removed

- **`rig-core`** Rust dependency (`Cargo.toml`, declared as `0.34.0` but with zero `use rig` / `rig::` references anywhere in `src-tauri/src/`). The actual LLM abstraction lives in `src-tauri/src/llm/` (direct HTTP for Mistral, Ollama, and OpenAI-compatible providers). `Cargo.lock` shrinks by 426 lines (transitive OpenAI/Anthropic crates pulled by `rig-core` features).
- **Zod documentation references** in `CONTRIBUTING.md` and `docs/TECH_STACK.md` (the `Zod 4 (from Zod 3)` migration section). Zod was removed from `package.json` several releases ago — the doc lingered and only mentioning it now risked confusing new contributors into thinking the project still uses Zod. Historical entries in `CHANGELOG.md` are preserved.
- **`rig-core` documentation references** in `README.md` (Tech Stack row, Acknowledgments line), `docs/README.md` (Tech Stack line, ASCII architecture diagram node, External Resources link), `docs/TECH_STACK.md` (LLM & Multi-Agent bullet), and `THIRD_PARTY_LICENSES.md` (table row). The licenses table row was already 2 minor versions behind `Cargo.toml` (`0.32.0` vs `0.34.0`), so the removal also eliminates a pre-existing drift.

### Changed

- **`typescript`** 5.9.3 → 6.0.3 (major). Verified against `/microsoft/typescript/v6.0.2` (Context7): TS 6.0 deprecates four compiler options (`outFile`, `module=AMD`, `target=ES5`, `moduleResolution=classic`), none of which are set in `tsconfig.json`. `verbatimModuleSyntax` is not active either. Zero functional impact for this project.
- **`marked`** 17.0.6 → 18.0.3 (major). Single call site (`MarkdownRenderer.svelte:46`). Verified against `/markedjs/marked` (Context7) that `async: false` has been the default since v4.1.0 and is unchanged in v18 — `marked.parse(content) as string` remains valid (the `as string` assertion would have flagged a Promise return as an error during `npm run check`).
- **`@lucide/svelte`** 0.563.1 → 1.16.0 (major). 41 occurrences renamed across 13 Svelte files following lucide v1's canonical-naming reversal: `AlertTriangle` → `TriangleAlert`, `CheckCircle` → `CircleCheckBig`, `CheckCircle2` → `CircleCheck`, `AlertCircle` → `CircleAlert`, `XCircle` → `CircleX`, `StopCircle` → `CircleStop`, `HelpCircle` → `CircleHelp`, `TestTube2` → `TestTubeDiagonal`. The deprecated names are still re-exported in v1.16 (verified in `node_modules/@lucide/svelte/dist/aliases/`), so `npm run check` did not surface the imports as errors — the renames are pre-emptive for v2.x removal of the aliases.
- **`svelte`** 5.55.5 → 5.55.7 (patch).
- **`@playwright/test`** 1.59.1 → 1.60.0 (minor).
- **`rand`** (Rust crate) 0.8 → 0.9 (minor). Single workspace-level call site (`llm/retry.rs:45`, `rand::random::<f64>() * 0.1`). API unchanged in 0.9 for this pattern (verified against `/rust-random/rand` via Context7). The transitive `rand 0.8` still ships through `surrealdb-core` dependencies (`linfa-linalg`, `ndarray-stats`, `phf_generator`) — expected cohabitation, no warnings, `cargo audit` clean.
- **Versioned documentation** (`README.md`, `docs/README.md`, `docs/TECH_STACK.md`, `THIRD_PARTY_LICENSES.md`) re-synced with the new dependency state (versions, drop of rig-core mentions, reworded LLM provider phrasing).

### Notes

- **Six Dependabot PRs are superseded** and will be closed in favour of this consolidated PR: #118 (marked 18), #149 (svelte 5.55.7), #150 (typescript 6.0), #152 (open — varies), #153 (lucide 1.16), #154 (playwright 1.60).
- **PR #151** (bump `rig-core` to a newer 0.34.x) becomes moot — `rig-core` no longer exists in `Cargo.toml`.
- **PR #85** (SurrealDB 2.6 → 3.0) stays open and out of scope. The breaking changes are catalogued in `docs/reviews/sync-rules/snapshots/surrealdb-2026-05-18.md` (local, gitignored).
- **Application version unchanged** (0.24.0). This is the 8th section accumulated under `[Unreleased]`. The next tag will collapse all eight under a single `[X.Y.Z]` heading.
- **Single zero-code-changed commit per bump** keeps `git bisect run` trivial if a regression slips past CI: C1 (rig-core drop), C2 (svelte/playwright/rand), C3 (typescript), C4 (marked), C5 (lucide), C6 (doc sync rig-core), C7 (Zod doc + this CHANGELOG entry).

---

Settings → Agents save flow + Prettier cleanup (`fix/agents-save-bugs-and-format`). Two bugs surfaced during an audit of "why does the agent list sometimes not reappear after save?", plus the residual Prettier dirt left over from PR #148. Bug 1: the `settings:refresh` DOM event bus was triggering a redundant reload on the very page that just dispatched, racing with the CRUD store's own refresh and hiding the freshly-updated list on slow machines (5 settings pages affected: agents, providers, mcp, validation, plus the AgentForm self-listener that re-ran `loadAgentFormResources` mid-unmount). Bug 2: `reasoning_effort` could not be cleared ("Off" dropdown) on an existing reasoning-capable agent without switching to a non-reasoning model first — the frontend omitted `undefined` from JSON.stringify, the backend read the absent field as "keep existing", and the saved value never moved off `High`. Tests verts (post-implementation): 1417 Rust lib (+4 TDD documenting the tri-state contract) + clippy `--all-targets` clean + svelte-check OK + 441 Vitest (+8 across `AgentForm.helpers` and `settings-refresh`) + ESLint/Prettier clean.

### Fixed

- **Settings → Agents list visibility race after save**. The `settings:refresh` event bus now carries an optional `source` tag and listeners can pass `ignoreSource` to skip the echo of their own page's dispatch. Affects 5 pages: `AgentForm` dispatches `source: 'agents'` (and its own internal `loadAgentFormResources` listener skips it), `LLMSection` dispatches `source: 'providers'`, `MCPSection` dispatches `source: 'mcp'`, `ValidationSettings` dispatches `source: 'validation'`, and the import-export wrapper dispatches `source: 'import'`. The host `/settings/{agents,providers,mcp,validation}/+page.svelte` listeners each declare a matching `ignoreSource`, while cross-page consumers (the workflow `/agent` sidebar, the `/settings/{prompts,skills,memory,audit-log}` pages with no dispatcher of their own) leave the option unset and continue to receive every event. Backward-compatible: dispatchers that pass no detail still flow through every listener, including those with an `ignoreSource` (the filter only triggers on an exact match).
- **`reasoning_effort` cannot be cleared on an existing reasoning-capable agent**. `AgentForm.handleSubmit` now splits the payload by mode: create still sends `reasoning_effort: reasoningEffort` (undefined omitted by JSON, read as outer `None` by the backend — fine for a brand-new row), but update sends `reasoning_effort: reasoningEffort ?? null` so the backend deserialises to `Some(None)` and clears the existing value. The shared shape lives in two pure helpers (`buildAgentCreatePayload` / `buildAgentUpdatePayload` in `AgentForm.helpers.ts`) with 4 unit tests asserting the wire shape (undefined → omitted, null → preserved, lifecycle never sent on update).

### Changed

- **`AgentConfigUpdate.reasoning_effort` deserialisation tightened to honour the tri-state PATCH contract**. The default serde behaviour for `Option<Option<T>>` collapses JSON `null` and "field absent" into the same outer `None`, which silently breaks the "send `null` to clear" pattern that the frontend fix relies on. A new `deserialize_explicit_option` helper in `src-tauri/src/models/agent.rs` is wired via `#[serde(default, deserialize_with = …)]` on the `reasoning_effort` field: present + `null` deserialises to `Some(None)` (clear), present + value to `Some(Some(_))` (set), and absent stays outer `None` (keep existing, via the `default` attribute). No DB migration — the on-disk schema for `agent.reasoning_effort` is unchanged (`option<string>` ASSERT in `low,medium,high`). 4 TDD tests lock both the serde contract and `merge_agent_config`'s behaviour for the two new branches (`Some(None)` clear with a still-reasoning model; outer `None` preserves existing).
- **Cosmetic Prettier reformat** on `ChatInput.svelte`, `ThinkingBlock.svelte`, `ToolCallBlock.svelte`, `TokenDisplay.svelte` (residual dirt from PR #148). 4 files, +8 / −21 LOC, pure layout (`disabled={disabled}` → `{disabled}`, span attributes inlined, import lines compacted under the 100-column limit). `npm run format:check` is once again clean across the entire `src/` tree.

### Notes

- **Spec extension over the original plan**: the spec presumed the backend `Option<Option<T>>` contract already deserialised `null` → `Some(None)` out-of-the-box. A direct serde test proved the opposite — serde collapses `null` and "absent" into outer `None` by default. The `deserialize_explicit_option` helper closes that gap and is the only backend code change in this PR (~10 LOC). Without it, the frontend `reasoning_effort: null` payload would still be read as "keep existing" and the bug would persist despite the cleaner client code.
- **No IPC contract change**: the `update_agent` Tauri command signature is unchanged; only the deserialiser for one field tightens, and only the frontend payload shape on update is adjusted. Existing callers that omit `reasoning_effort` keep the same semantics.

---

DelegateTaskTool — per-agent FileManager perimeter visibility (`feature/delegate-folders-visibility`). The `list_agents` operation now exposes each permanent agent's `folders` (authorized paths) and a derived `has_file_manager` boolean, so the primary LLM can route file-bound tasks to an agent whose perimeter covers the target path — or skip an agent whose perimeter does not — instead of delegating blindly and burning a turn on the eventual `PathOutsideAuthorized` error. Backend-only change, +30 LOC + 3 TDD tests, zero TypeScript / schema / frontend impact. Tests verts: 1413 Rust lib (+3) + clippy `--all-targets` clean + svelte-check 4162 / 0 errors + 436 Vitest + ESLint/Prettier clean.

### Added

- **`folders: string[]` and `has_file_manager: boolean` on every entry returned by `DelegateTaskTool::list_agents`**. The primary LLM that orchestrates a delegation now sees, per permanent agent, the absolute paths the agent is authorized to read/write via FileManagerTool plus a boolean flag. `folders` is force-cleared to `[]` when the agent does not have `FileManagerTool` in its `tools` (even if `config.folders` still carries residual values from an older configuration) — preserves the principle of not advertising a capability the agent cannot exercise. Mirrors the symmetric pattern in `FileManagerTool::build_definition` which already injects the agent's own folders into its tool description; here we propagate the same information to the _caller_ via the dynamic payload rather than the static description, to preserve `PAT_TOOL_DEF_CACHE` (the `LazyLock<ToolDefinition>` stays byte-identical across calls so prompt-cache hit rate is unaffected).
- **`build_agent_listing_entry` pure helper** (`src-tauri/src/tools/delegate_task_execution.rs`). Projects `(id, &AgentConfig, capabilities)` into the JSON entry, extracted from the body of `list_agents()` to enable direct unit testing without instantiating the full `DelegateTaskTool` (which would require an `AgentRegistry`, `AgentOrchestrator`, `DBClient`, etc.). Follows `PAT_RUST_015` (extract pure logic for testability without test-only constructors).
- **3 TDD tests** in `delegate_task_tests.rs` lock the three contractual cases: (1) FileManagerTool present + folders set → folders projected verbatim, (2) FileManagerTool absent + folders set in config → folders forced to `[]` and `has_file_manager: false`, (3) FileManagerTool present + folders unconfigured → folders empty and `has_file_manager: true` (LLM sees the tool flag but knows the agent has no usable perimeter).

### Changed

- **`DelegateTaskTool` description grows a third `.note(...)`** documenting the new contract (still inside the same `LazyLock<ToolDefinition>` — no migration to per-instance `OnceLock`). The static description tells the LLM that `list_agents` now carries the per-agent folder perimeter and how to read the two new fields.
- **`DelegateTaskTool` `output_schema` extended** to declare `agents[].folders` (string array) and `agents[].has_file_manager` (boolean). Informational — the LLM does not read the schema at execution time, but the documentation surface stays in sync.

### Notes

- **No frontend changes**: the `list_agents` payload of `DelegateTaskTool` is internal to the backend and consumed only by the LLM via `Tool::execute()` dispatch. Distinct from the Tauri command `commands::agent::list_agents` (returns `AgentSummary[]`), which is unchanged and still consumed by `agents.ts` / `ExportPanel.svelte`.
- **No canonicalisation of paths in `list_agents`**: the listed `folders` are the raw values persisted on `AgentConfig` (what the user typed in the agent form). Canonicalisation stays isolated to `ToolFactory::resolve_agent_folders` at the moment the sub-agent's `FileManagerTool` is instantiated — avoids unnecessary disk I/O on every listing and keeps the LLM's view aligned with the user-facing configuration.

---

Memory Settings — statistics + export/import round-trip + update_memory chunk sync post PR #147 (`fix/memory-settings-bugs`). Five Tauri commands (`get_memory_stats`, `get_memory_token_stats`, `export_memories`, `import_memories`, `update_memory`) were still operating against the legacy `memory.embedding` field / pre-multi-chunk assumptions that PR #147 removed in favour of `memory_chunk`. Stats returned `with_embeddings = 0` regardless of state, export omitted `importance` + `expires_at`, import dropped `workflow_id` + `importance` + `expires_at` + `created_at` AND never produced any `memory_chunk` rows, and `update_memory` left the chunk index pointing at the old text (silent semantic-search drift) while echoing back the wrong `importance` / `expires_at` on the IPC response. 18 TDD tests lock the contracts (4 helpers + 2 stats + 2 export + 6 import + 1 helpers cast + 3 update_memory). No TypeScript / schema / frontend changes — the IPC contract was already correct, only the SQL had drifted.

### Fixed

- **`get_memory_stats.with_embeddings` and `get_memory_token_stats.categories[].with_embeddings`** now reflect the number of parent memories that have at least one `memory_chunk` row (post-PR #147 schema), via a `DISTINCT memory_id` subquery on `memory_chunk`. Previously both fields queried the dropped `memory.embedding` field and returned 0 for every install — Settings → Mémoire displayed `0/N Avec incorporations` regardless of actual state.
- **`export_memories` (JSON + CSV) now includes `importance` and `expires_at`**. The two SELECT clauses widen to also pull these columns, and the CSV header becomes `id,type,content,workflow_id,metadata,importance,expires_at,created_at`. Round-trip preservation is now lossless for the four optional/scoring columns.
- **`import_memories` preserves `workflow_id`, `importance`, `expires_at`, `created_at` AND creates `memory_chunk` rows for every imported memory**. The command now delegates to `add_memory_core` (the same helper the live `MemoryTool` uses), so parent + N chunks (+ embeddings if the service is configured) are produced atomically. Imports are once again visible to semantic search.
- **`import_memories` no longer silently coerces unknown / missing `type` to `knowledge`** — invalid items are counted in `failed` with an explicit error in `ImportResult.errors` (symmetric with the existing `content` validation). The previous `unwrap_or("knowledge")` fallback masked broken exports as successful imports.
- **`update_memory` keeps the `memory_chunk` index in sync with the new `content`**. When `content` changes, the existing chunks (still indexed against the OLD text and embeddings) are dropped and re-created from the new content via the new `replace_memory_chunks` helper. Without this fix the parent row carried the new content while semantic search kept matching the old one — silent drift across every UI edit since PR #147. Metadata-only updates skip the re-chunkification round-trip (the chunk ids are preserved verbatim).
- **`update_memory` SELECT widened to include `importance` and `expires_at`**. The previous query omitted both, which silently coerced `importance` to the serde `default = 0.5` and `expires_at` to `None` in the IPC response — the DB row was correct but the frontend rendered the wrong values until the next `list_memories`.

### Removed

- **Decorative `force: bool` parameter on `reindex_memory_chunks`**. The parameter was received and logged but never consumed (carried over from the pre-PR #147 sync API). Frontend `MemorySettings.svelte` always passed `false`; the call site now invokes the command with no arguments. Pure dead-code removal — no behavioural change.

### Added

- **`count_parents_with_chunks` + `count_parents_with_chunks_by_type` helpers** (`src-tauri/src/commands/embedding/helpers.rs`). Factor out the `SELECT count() FROM (SELECT memory_id FROM memory_chunk GROUP BY memory_id) GROUP …` subquery. The per-type variant uses the record-link traversal `memory_id.type` in SELECT context (safe — ERR_SURREAL_013 only affects WHERE / DELETE).
- **`set_created_at` helper** (same file). Overrides `created_at` on an existing memory row with the `<datetime>` cast pattern (ERR_SURREAL_007) — mirror of `set_expires_at_if_present` in `tools/memory/helpers.rs`. Used by `import_memories` to preserve the original creation date on round-trip.
- **`replace_memory_chunks` helper** (`src-tauri/src/tools/memory/helpers.rs`). Drops every `memory_chunk` row tied to a given parent (direct equality on the `record<memory>` link — not a traversal, so ERR_SURREAL_013 doesn't apply) and re-creates the chunks from the new content, optionally embedding each one. Called by `update_memory` whenever `content` changes.

---

Custom provider strict-mode toggles (`feature/custom-strict-toggles`). +Option<bool> × 2 fields persisted on `custom_provider`, runtime-wired through `OpenAiCompatibleProvider`, surfaced in `CustomProviderForm` as two checkboxes. Unlocks Fireworks / Groq / Together / Cerebras integration without a new provider type. Defaults (`None`) preserve OpenRouter behaviour bit-for-bit — no migration of existing rows. Tests verts: 1390 Rust lib (+9) + clippy `--all-targets` clean + svelte-check 4162 / 0 errors + 436 Vitest + ESLint/Prettier clean.

### Added

- **`supportsCacheControl` and `supportsReasoningParam` toggles on every custom provider**. Two `Option<bool>` columns on the `custom_provider` table (`DEFINE FIELD OVERWRITE … TYPE option<bool>`, no `DEFAULT` — existing rows stay `NONE`). When `Some(false)`, the OpenAI-compat wire path skips Anthropic-style `cache_control` content parts (`apply_prompt_cache_control` shortcut), clears the OpenRouter-style top-level `reasoning: { effort, max_tokens }` object, **and strips `reasoning` / `reasoning_content` / `reasoning_details` / `provider_specific_fields` from echoed assistant messages on multi-turn tool loops** (`OpenAiToolAdapter::build_assistant_message` re-injects the previous turn's message verbatim — strict providers reject those fields on iteration 2+ with `HTTP 400: Extra inputs are not permitted, field: 'messages[i].reasoning'`). Empirically validated against `accounts/fireworks/models/deepseek-v4-pro`: Fireworks returns HTTP 400 on cache_control + top-level reasoning, and HTTP 400 on echoed `messages[i].reasoning` after a first thinking turn. `None` / `Some(true)` keep the current OpenRouter Anthropic + RouterLab behaviour (ERR_LLM_012 + ERR_LLM_016 régression locked by 7 dedicated unit tests; `reasoning_details` is preserved when the flag is None so signed Anthropic thinking blocks still survive).
- **`build_openai_compat_tool_request` pure helper** (`src-tauri/src/llm/openai_compatible.rs`). Mirror of `build_mistral_tool_request` (`ERR_LLM_014`), parameterised for runtime-configured custom providers. Honoured by `OpenAiCompatibleProvider::complete_with_tools` after reading the two `Arc<RwLock<Option<bool>>>` flags from `self`. 5 TDD tests assert skip behaviour AND non-regression on the `None` default.
- **Form UI** — two `<input type="checkbox">` rows on `CustomProviderForm.svelte`, each with an inline help paragraph. Default `true` on creation = preserve OpenRouter behaviour. Uncheck both for Fireworks / Groq / Together / Cerebras.

### Changed

- **`llm_form_cache_read_price_help` enriched** in EN + FR to list the provider-specific synonyms (`Cached input` on Fireworks and OpenAI, `Cache read` on Anthropic, `Cache hit` on DeepSeek). Helps users locate the value on each provider's pricing page when entering a new model manually. Label and other helper texts unchanged.
- **`create_custom_provider` / `update_custom_provider` Tauri command signatures** gain two trailing `Option<bool>` parameters (`#[allow(clippy::too_many_arguments)]`). Frontend store actions and `ProviderInfo` (serde `rename_all = "camelCase"`) propagate the new fields to TypeScript transparently. `list_providers` SELECT widened to include the two new columns; the boot path in `state.rs` reads them and calls `OpenAiCompatibleProvider::set_strict_compat` so live providers reflect the persisted state from the first request.

### Notes

- **No backfill needed**: `DEFINE FIELD OVERWRITE … option<bool>` without `DEFAULT` leaves existing rows at `NONE`, which the wire path treats as the OpenRouter-preserving default (`unwrap_or(true)` semantics). Toggling the checkbox writes `Some(true)` or `Some(false)`; the inverse is `None` and only reachable on legacy rows, never after a UI write.
- **Pattern frozen — `PAT_LLM_005`** (mirror of `build_mistral_tool_request`): for any OpenAI-compat custom provider, extract a pure `build_…_tool_request(params, …flags)` helper above `complete_with_tools`, gate the Anthropic / OpenRouter extensions behind explicit `Some(false)` checks, and unit-test the helper directly. Avoids HTTP mocking and keeps the wire-shape decisions reviewable in isolation.

## [0.24.0] - 2026-05-12

Agent page UX overhaul (`feature/ui-ux-agent-page`). 8 sequential commits, +1334 / −325 LOC across 26 files. Three goals: (1) attribute every streamed block to the agent that emitted it (primary vs sub-agent), (2) make the visual hierarchy of an agent run readable at a glance (collapsed sub-agents, discreet header, three-level token display), (3) keep the chat input usable during execution. Tests verts: 1381 Rust lib + clippy `--all-targets` clean + svelte-check OK + 431 Vitest (+9 TDD streaming/chat_block + 5 attribution + 5 chat-container-helpers) + ESLint/Prettier clean.

### Added

- **Per-block agent attribution end-to-end** (Phase 0): `StreamChunk` gains optional `agent_id` + `agent_name` (`Option<String>` + `skip_serializing_if`) on the four constructors that emit live execution state (`thinking_block`, `reasoning`, `tool_start`, `tool_call_complete`). The `emit_reasoning` helper signature is extended and propagated to its three call-sites (`iteration.rs`, `tool_loop.rs` x3, `completion.rs::enforce_report`). `execute_simple` now reads `is_sub_agent` off `task.context` the same way `execute_with_tools` already did. **Replay parity**: `merge_into_chat_blocks` takes a new `agent_name_lookup: &HashMap<String, String>` — `load_workflow_blocks_core` bulk-queries the `agent` table for the distinct ids seen across primary + sub-agent tools/thinking, builds the lookup, and projects it into the merge step. A miss leaves `agent_name` absent and the frontend falls back to the truncated `agent_id` (`PAT_STREAM_002`). The orchestrator bridge's spinner `tool_start` now carries the primary agent id+name.
- **Sub-agent visual differentiation** (Phases 1-2): `StreamChunk.ts`, `ToolCallBlockData`, `ThinkingBlockData` gain optional `agent_id` + `agent_name`. `execution-blocks.ts` handlers (`handleToolCallComplete` / `handleThinkingBlock` / `handleReasoning`) propagate the fields into `block.data`. `ToolCallBlock` + `ThinkingBlock` take three optional props (`agentId`, `agentName`, `primaryAgentId`) and derive `isSubAgent = agent_id != null && agent_id !== primaryAgentId`. When `isSubAgent`: 16px left margin, dashed `border-left var(--color-info)`, agent-name chip (fallback to the first 8 chars of the id), and a sub-agent variant on `aria-label`. Legacy / replay rows with a falsy `primaryAgentId` stay safely "primary".
- **`countInternalBlocks` helper + new `chat-container-helpers.ts` module** (Phase 3, `FN_UI_017`): companion TS module sister to `ChatContainer.svelte`, extracted so its semantics can be unit-tested without mounting Svelte. Counts `tool_call` / `thinking` blocks attributed to a given sub-agent that **precede the sub-agent's summary block** in the timeline (stop-at-summary semantic: once we pass the summary, we are in another sub-agent or back to the primary's next turn). `SubAgentBlock` displays a plural-aware "{count} internal action(s)" badge when collapsed AND count > 0. `ChatContainer`'s `renderBlock(block, allBlocks)` snippet wires the helper at both call-sites (persisted blocks per-message + real-time `executionBlocks`). Five Vitest cases cover the stop-at-summary contract.
- **AgentHeader iterations popover** (Phase 4): the iterations input moves out of the header strip into a popover dialog gated by a `SlidersHorizontal` button. The popover is a real `role="dialog"` + `aria-modal="true"` + `tabindex="-1"` surface using the shared `focusTrap` attachment. Escape, outside-click, and the explicit Close button all dismiss. Layout re-centers under 550px.
- **`showPendingHint` derived hint on `ChatInput`** (Phase 5, Option A): a discreet italic "Message en attente" hint sits under the textarea while a turn is running and the user keeps typing — wired via `aria-describedby` + `aria-live="polite"` + `role="status"` (one announcement per state change). The keyboard hint is hidden during `loading`.

### Changed

- **`AgentHeader` is now discreet, read-only and single-line** (Phases 4 + 7): solid background (no gradient), 44px minimum height (was 56px), bot icon and separator removed. The header now displays the agent name and the active model only — misleading controls that the user could not actually change at runtime have been removed; configuration lives in agent settings. Iterations was moved into the popover dialog in Phase 4.
- **`SubAgentBlock` is collapsed by default** (Phase 3): the block opens with `expanded = false` and surfaces its internal action count next to the header so the timeline stays scannable. Expanding the block restores the previous detail view unchanged.
- **`ChatInput` textarea stays editable during execution** (Phase 5): `disabled` no longer ORs `loading` — only the no-agent case disables the textarea. Send / Stop swap on the right (primary action), prompt-button moves left (secondary). Pre-typing for the next turn is now first-class.
- **`TokenDisplay` gains a three-level information hierarchy** (Phase 6): **L1** always-visible row with three metrics — context gauge, cost (with a leading `~` when the cost is partial), speed. Icons trimmed from 5 to 3 (`Gauge` / `CircleDollarSign` / `Activity`). **L2** hover tooltip on the cost cell via native `title` (last-turn tokens in/out + cached + hit rate) — no popover machinery added. **L3** `ChevronDown` toggle reveals the existing detail panel (`aria-expanded` + `aria-controls` + `role="region"`), shown only when expansion adds value. The "(estimate)" label is replaced by an explicit "Last turn" / "Dernier tour" row. Responsive at <700px and <480px; respects `prefers-reduced-motion`.

### Notes

- **Pattern frozen — `PAT_STREAM_002` (replay-side enrichment via bulk lookup `HashMap`)**: live emit sets the new fields directly off the agent context; replay queries the source table once for the distinct ids, builds a `HashMap`, and projects it during the merge step. The frontend treats both paths identically, falling back on a truncated id when the lookup misses. **Do not denormalize `agent_name` onto the block table** — the lookup is cheap, and the rename-rewriting-history tradeoff is rarely what we want.
- New reusable function `FN_UI_017 countInternalBlocks` (`src/lib/components/chat-container-helpers.ts`).

---

Memory multi-chunk + tags filter + streaming reindex + cascade/purge/exit hardening (`feature/memory-multi-chunk-tags-filter`). 8 commits, ~+2700 / −1200 LOC across 40 files. Refonte of the memory schema into one parent row + N indexed chunks under HNSW 1024D, tags filter (`CONTAINSANY`), cancellable streaming reindex, plus two latent runtime hazards surfaced and fixed during the audit (orphan chunks on workflow delete, RocksDB heap abort on shutdown). Embedding configuration UI rewritten around a status badge + Operations card (Test / Reindex / Purge). Tests verts: 1371 Rust lib + clippy `--all-targets` clean + svelte-check 4050 OK + 426 Vitest + ESLint/Prettier clean.

### Added

- **`memory_chunk` table + per-chunk HNSW index**: each parent `memory` row now owns N chunks linked by `memory_id: record<memory>`, each carrying its own embedding and parent indexes. HNSW + parent indexes are defined per chunk. UTF-8-safe recursive chunker (`FN_RUST_019 split_recursive`): paragraph → line → sentence → hard cut on code-point boundary. 12 TDD tests cover ASCII / French / emoji / boundary cases.
- **Tags filter on memory search**: `CONTAINSANY` clause built by `FN_RUST_018 build_tags_filter_clause`. 5 TDD tests lock the contract.
- **Streaming reindex with cancellation**: `reindex_memory_chunks(force)` spawns a tokio task, emits `reindex-progress` per parent, is cancellable via `cancel_reindex_job`. Job status is auto-purged on consultation + 10-minute sweep. Frontend: `ChunkSearchResult` / `ReindexJobStatus` types, `MemoryList` dedupes by parent, `MemorySettings` reindex card with progress bar + `LocalStorage` persistence + listener filtered by `jobId` + retroactive toast on remount.
- **Operations card** in `MemorySettings`: Test + Reindex + Purge grid (auto-fit `minmax(280px, 1fr)`). Test/Reindex hidden (not disabled) when embedding is not configured — the empty Configuration card is the single entry point. Status badge "Configured / Not configured" + Settings icon on the Configuration card.
- **`purge_expired_memories` (FN_RUST_020)** in `cleanup.rs`: SELECT expired ids → DELETE chunks `WHERE memory_id IN ($expired)` → DELETE parents. Wired in `AppState::new` as a best-effort boot purge + exposed as a Tauri command for the on-demand Purge card.
- **DB primitive `delete_memory_chunks_by_workflow_id`**: sequential chunks-first cascade step before the parallel block in `cascade::delete_workflow_related`. 2 TDD tests lock the contract (orphan chunks must not survive a workflow delete).

### Changed

- **`EmbeddingConfigSettings` no longer carries decorative fields** (`dimension` / `max_tokens` / `chunk_size` / `chunk_overlap` / `strategy`): none were honored at runtime — HNSW dimension is fixed at 1024D and chunking is owned by the new recursive chunker. The "Chunking Settings" section of the Configuration card is removed, along with the `dimension-info` block and the `nomic-embed-text` (768D) preset (incompatible with the 1024D index).
- **Ollama base URL is now read from `embedding_config.endpoint`** via `FN_RUST_017 load_ollama_base_url(db)` (no more hardcode of `http://localhost:11434`).
- **`get_embedding_config` returns `Option<EmbeddingConfigSettings>` instead of silently materializing defaults**; the UI distinguishes "Configured" from "Not configured" instead of showing fake values.
- **`MemoryList` deduplicates by parent**: search results are scored per chunk but the UI shows one row per parent (best chunk wins).
- **Schema cleanup inline**: `REMOVE FIELD/INDEX IF EXISTS memory.embedding` next to the new `DEFINE FIELD` lines in `schema.rs` (`PAT_DB_006`: replay-on-boot, no `migration_log` needed for a decorative field).
- **Documentation sync**: `docs/DATABASE_SCHEMA.md`, `docs/API_REFERENCE.md`, `docs/AGENT_TOOLS_DOCUMENTATION.md` rewritten to reflect the multi-chunk architecture (parent / chunk tables, per-chunk HNSW, tags filter shape, reindex / purge / cancel commands).

### Fixed

- **Cascade-delete leaked `memory_chunk` rows when a workflow was removed** (`ERR_SURREAL_013`): `cascade::delete_workflow_related` ignored `memory_chunk`, leaving orphan rows with broken `memory_id` record links — HNSW entries stayed live (silent memory leak) and any future search on those vectors hit `NONE` parents. Fix: chunks are deleted sequentially **before** the parallel cleanup block (the record-link traversal needs the parent still alive). DELETE traversal-in-WHERE silently matches zero rows in SurrealDB 2.6, so the new code uses `IN (SELECT VALUE id FROM ...)` subqueries (`PAT_DB_007`).
- **`context` memories were never garbage-collected**: they carried a 7-day TTL via `expires_at` but nothing actually deleted them. `purge_expired_memories` now runs at boot (best-effort) and on demand (Purge card).
- **RocksDB heap abort on app shutdown** (`ERR_TAURI_005`, superset of `ERR_TAURI_003`): the app aborted systematically with `free(): corrupted unsorted chunks` after MCP shutdown — RocksDB FFI `Drop` ran during tokio teardown. The SurrealDB Rust SDK 2.6 exposes **no** `close()` / `shutdown()` / `flush()` API (verified against upstream source + docs; only the Java + JavaScript SDKs offer `db.close()`). Pragmatic fix: after MCP terminate, the shutdown path now calls `std::process::exit(0)` directly, skipping the entire `Drop` chain. RocksDB's WAL preserves integrity across the hard exit. Bonus: removes the `exit(0) → ExitRequested` recursion (the `AtomicBool` guard is kept as defense-in-depth).

### Removed

- **`run_memory_chunk_v1` helper + 3 tests + fixtures** (−227 LOC): test-only function gated `#[cfg(test)]` exercised only by its own tests — violates the "production constructors are the single source of truth" rule. The production path (`reindex_memory_chunks` in `commands/embedding/operations.rs`) is unchanged.

#### New learning entries (cross-referenced in MEMORY.md)

- `ERR_SURREAL_013`: DELETE with record-link traversal in `WHERE` silently matches 0 rows in SurrealDB 2.6 — use `IN (SELECT VALUE id FROM ...)` subqueries.
- `ERR_SURREAL_014`: `query_json` does not surface the rows returned by `DELETE ... RETURN BEFORE` — pre-`SELECT` is required for counts.
- `ERR_TAURI_005`: `app_handle.exit(0)` triggers the `Drop` chain, which corrupts the heap under tokio teardown (RocksDB FFI). Use `std::process::exit(0)` after async essentials are flushed. SurrealDB SDK 2.6 has no `close()`.
- `PAT_DB_006`: `REMOVE FIELD IF EXISTS <field> ON TABLE <table>` next to the `DEFINE FIELD` lines (replay-on-boot, no `migration_log` for decorative drops). Symmetric complement to `PAT_DB_003`.
- `PAT_DB_007`: cascade-delete with record-link traversal — chunks-first, parent-second; use `IN (SELECT VALUE id FROM ...)` subqueries.
- `PAT_RUST_014`: shutdown hardening — `std::process::exit(0)` skips Drop when the runtime cannot safely tear down FFI handles (RocksDB-via-SurrealDB SDK 2.6).
- `FN_RUST_017 load_ollama_base_url(db)`, `FN_RUST_018 build_tags_filter_clause`, `FN_RUST_019 split_recursive`, `FN_RUST_020 purge_expired_memories`.

---

Maintenance: dependency and Tauri configuration cleanup (`chore/maintenance-0.23.2-pr`, PR #143 by external contributor [@ScioNos](https://github.com/ScioNos)).

### Changed

- Bump safe frontend tooling/runtime dependencies while staying on the current major lines: SvelteKit 2, Svelte 5, Vite 7, ESLint 9, Vitest 4 and Tauri 2.
- Declare the supported Node.js runtime explicitly with `engines.node >=20.19.0`.
- Narrow Tauri dialog permissions from `dialog:default` to explicit `dialog:allow-open` and `dialog:allow-save`.

### Removed

- Remove unused or redundant direct dependencies: `zod`, `@typescript-eslint/eslint-plugin`, `@typescript-eslint/parser` and `@types/dompurify`.
- Remove obsolete configuration/artifacts: legacy `.eslintrc.cjs` and orphaned `src-tauri/package-lock.json`.

---

Backend dead-code cleanup (`refactor/backend-deadcode-cleanup`). 13 commits, ~−1300 LOC of unreachable code removed plus one latent runtime bug surfaced during the audit. `#[allow(dead_code)]` annotations dropped from 27 to 2 (last two are documented module-level lib/bin-split allows on `mcp::circuit_breaker` and `mcp::client`, where test-only accessors observe production state but are not reachable from the binary target). Production constructors are now the single source of truth: test-only `_with_*` ctors removed, tests rewritten to exercise the production path. Tests verts: 1388 Rust lib + clippy --all-targets clean.

### Removed (backend)

- **`LLMProvider` trait** (`llm/provider.rs`): `ProviderManager` already dispatched via `enum ProviderType` + match; `is_configured()` / `complete()` are now inherent methods on `MistralProvider` / `OllamaProvider`. `OpenAiCompatibleProvider` never implemented the trait in the first place.
- **`tools::sub_agent_circuit_breaker` module + tests** (~480 LOC): never wired — every `AgentToolContext.circuit_breaker` init site passed `None`. Call-time MCP `CircuitBreaker` untouched.
- **MCP background health-check architecture** in `mcp::manager` (~140 LOC): `start_health_checks` / `stop_health_checks`, `check_*_health`, `get_circuit_breaker` / `reset_circuit_breaker`, `health_check_shutdown` broadcast field, `DEFAULT_HEALTH_CHECK_INTERVAL`. The private on-demand `refresh_tools_internal` used during `initialize` is kept.
- **MCP client methods cascade**: `MCPClient::{auto_reconnect, call_tool_raw, call_tool_text, refresh_tools (public), is_process_alive, server_info}`, `MCPServerHandle` / `MCPHttpHandle::{refresh_tools (public), is_connected, set_error_status, config, server_info}`, `helpers::extract_text_content`, `manager/db::get_server_config`, `manager/tools::list_all_tools`.
- **Server-only `JsonRpcError` constructors** (`mcp/protocol.rs`): `parse_error`, `invalid_request`, `method_not_found`, `invalid_params`, `internal_error`, `is_error`. Zileo is a JSON-RPC client; production uses `response.error.is_some()`.
- **Test-only `_with_*` constructors**: `OllamaProvider::{with_url, clear, get_server_url}`, `RetryConfig::new`, `CircuitBreaker::with_defaults` (llm + user_question), `SubAgentExecutionCreate::{new, with_parent_message}`, `AgentToolContext::{from_app_state_with_cancellation, _with_resilience, _with_handle}`, **`LLMAgent::with_factory`**. Tests rewritten with struct literal + `..Default::default()` so they go through the production constructors.
- **Orphan structs / fields**: `UserQuestionResponse` (Rust-side), `ValidationSettingsConfig` + its `Default`, dead `MistralUsage`-related embedding response fields (serde silently ignores unknown fields), `ToolMetadata.name` + 9 init sites, `DEFAULT_TIMEOUT_SECS` + duplicate `DEFAULT_TIMEOUT_THRESHOLD` / `DEFAULT_COOLDOWN_SECS` (single source of truth in `tools::constants::user_question::CIRCUIT_*`).

### Fixed (backend)

- **`SpawnAgentTool` sub-agent context propagation** (ERR_AGENT_008, latent since v0.19.0): `SpawnAgentTool` built sub-agents via `LLMAgent::with_factory(...)`, which left `agent_context: None`. When a sub-agent fired `UserQuestion` mid-task, `UserQuestionTool::emit_question_event` silently early-returned (no `app_handle`); the DB row was created but the frontend modal never opened, so the question timed out after 5 minutes. Surface effect: any sub-agent asking a question would silently hang. Fix: `SpawnAgentTool` now builds sub-agents via `LLMAgent::with_context(...)` carrying `app_handle.clone()` + the workflow's `cancellation_token`. Sub-agent tool filtering (`is_sub_agent: true`) is unchanged.

---

Cleanup of the chat zone and agent page (`refactor/cleanup-zone-chat`). 26 atomic cleanup commits + PR #140 (large frontend components helpers extraction) merged into the branch. The cleanup itself trims ~−1118 LOC (`+1002 / −2120` post-#140); branch-vs-main delta is `+151 LOC` because the helpers extraction adds companion files. Zero functional change. Tests verts: 1388 Rust lib + 435 Vitest + svelte-check 4050 files / 0 errors / 0 warnings.

### Removed

- **`streamingStore`** (`src/lib/stores/streaming.ts`): entirely redundant with `backgroundWorkflowsStore`. Chunk processing now flows directly through `executionBlocksStore` + `tokenStore` via the callbacks registered by the agent page. Net −800 LOC across the chat zone.
- **`MessageList.svelte`**: inlined into `ChatContainer.svelte` (always called with a single-message array).
- **Barrel `src/lib/components/chat/index.ts`** (no consumers).
- **`MessageService.load` / `clear`** + Tauri command `clear_workflow_messages` (dead code path).
- **`load_message_blocks` Tauri command + `BlockService.loadForMessage`**: orphan single-message loader, replaced by `load_workflow_blocks` batch (1 round-trip, was N×3 SurrealDB queries).
- **Dead `ChunkableState` / `WorkflowStreamState` fields**: reduced to the 6/6 fields actually consumed.
- **Dead `ActiveSubAgent` fields**: `statusMessage`, `progress`, and the never-emitted `'starting'` variant of `ActiveSubAgentStatus`.
- **Dead prop `MessageBubble.isUser?`** (no caller passed it).
- **`ThinkingBlockData.duration_ms`** field (no consumer).
- **Derived stores `executionResponse` / `executionError` / `executionCancelled`** + `restoreFromBlocks` (no consumers after the streaming refactor).

### Changed

- **`Message.tokens`** is now optional on the TypeScript side (`Option<u64>` mapping). Full removal across frontend writes + Rust `legacy_tokens` + DB column is tracked separately (requires migration).
- **`onCompleteForViewed` callback** signature simplified from `(complete: WorkflowComplete) => void` to `() => void` (payload no longer consumed since the streamingStore removal).
- **`load_workflow_blocks`** new batch command: 1 IPC round-trip per workflow instead of N × `load_message_blocks` calls.
- **`MessageBubble`** copy timer is now cleared on unmount and before each click (fixes leak / race on rapid clicks).

### Fixed

- **Tool `error_message` propagation**: now streams live via `StreamChunk` instead of waiting for the next reload (Rust + TS).

### Tooling

- **Prettier + `prettier-plugin-svelte`** wired in (`chore/prettier-plugin-svelte`). New devDependencies (`prettier@3.8.3`, `prettier-plugin-svelte@3.5.1`), config files (`.prettierrc.json`, `.prettierignore`), and npm scripts (`format`, `format:check`). First project-wide run reformatted 193 files (tabs, single quotes, no trailing comma, 100-char width) — 100% cosmetic, lint + svelte-check + 435 Vitest stay green. `.git-blame-ignore-revs` registers the reformatting commit so `git blame` keeps crediting the original logic authors. CI integration into `validate.yml` is intentionally deferred to a follow-up PR.
- **`src/routes/agent/+page.svelte`** mosaic indent (4 functions broken by the `refactor/cleanup-zone-chat` rebase over PR #140) is now fixed via Prettier instead of a manual sed pass.
- **`src/lib/tauri/*.ts`** zero-indent legacy from PR #130 is normalized — this subsumes the formatting follow-up that was tracked in `docs/specs/eslint-tauri-import-restriction.md`.

---

## [0.23.1] - 2026-05-08

Audit hardening release. Backend defense-in-depth on every SurrealQL interpolation site, OOM caps on the SSE / MCP read paths, TOCTOU defense on `file_manager` recursive search, retry-storm guards on transient and 4xx LLM responses, plus the previously-unreleased `reasoning_effort` live-reload fix from PR #134. Frontend strictness ratchets up: `noUncheckedIndexedAccess`, `noImplicitOverride`, `noFallthroughCasesInSwitch`, ESLint `no-console: error` + `no-explicit-any: error`. CI is hardened against tag-rewrite supply-chain attacks (SHA-pinned actions, scoped `contents:write`, `cargo audit` / `npm audit` advisory jobs).

### Added

- **`reasoning_effort` live-reload from `AgentForm`** (PR #134, was unreleased): dropping or changing the effort in Settings now dispatches a `settings:refresh` event so the running agent picks up the new value without a restart. The IPC payload also emits `reasoning.max_tokens` alongside `reasoning.effort` to satisfy the RouterLab gateway that expects both.
- **Strict UUID v4 validation in `Validator::validate_uuid`** (`security/validation.rs`): rejects UUID v1 (timestamp), v3 (MD5), v5 (SHA1), nil, oversized strings, and any payload containing backticks / null bytes / newlines. The codebase only generates v4 (`Uuid::new_v4` / `crypto.randomUUID`), so anything else is a crafted ID. 10 new unit tests cover the rejection matrix.
- **`LLMError::ResponseTooLarge` + `LLMError::ClientError { status, message }`** (`llm/provider.rs`, `llm/http.rs`, `llm/retry.rs`): non-retryable error classes for OOM-scale SSE payloads and 4xx provider responses (except 429). The retry whitelist (`is_retryable`) only matches `ConnectionError` / `RequestFailed`, so 401 / 400 / 403 / 404 no longer trigger 3-attempt exponential-backoff storms on a deterministic failure.
- **`Workflow.sub_agent_cost_usd` field on the Rust struct** (`models/workflow.rs`, `db/queries.rs`): the schema and the streaming layer already wrote it; serde silently dropped the JSON key on read because the struct was missing the field, so list / get queries returned `undefined` to the frontend and the workflow card never showed the sub-agent cost split.
- **Global `+error.svelte` boundary** (`src/routes/+error.svelte`): SvelteKit was rendering the default white error page when a `+page.ts::load` threw or a route module failed to import. The new boundary renders the app theme + i18n with Reload / Back-to-home actions; 4 new keys in `en.json` / `fr.json`.
- **`generateUuid()` helper** (`src/lib/utils/uuid.ts`): single wrap point over `crypto.randomUUID` for `toast.ts` and `workflowExecutor.service.ts` (×2). Replaces the `Math.random().toString(36).slice(2,9)` collision-prone IDs on `Input` / `Textarea` / `Select` / `PasswordInput` `generatedId`.
- **`focusTrap` on three remaining custom dialogs** (`OnboardingModal`, `NewWorkflowModal`, `UserQuestionModal`): close WCAG 2.1.2 keyboard-trap requirement. Reuses `$lib/actions/focusTrap` rather than duplicating the trap logic. `UserQuestionModal` keeps Escape blocked (volontairement) so the user must answer or skip explicitly; comment documents why.
- **`folders.test.ts`** (6 tests) and **`workflows.test.ts` "CRUD error handling"** (7 tests): cover `try / catch` on every mutating CRUD action + the clear-on-retry path.

### Changed

- **TypeScript strict flags ratcheted up** (`tsconfig.json`): `noUncheckedIndexedAccess` (was generating 143 errors at activation; all 16 production files + ~10 test files fixed — components, stores, actions, routes), `noImplicitOverride`, `noFallthroughCasesInSwitch`. Indexed access now returns `T | undefined` and the codebase guards every `array[i]` / regex `match[i]` site instead of trusting deterministic positions.
- **ESLint `no-console: error` + `@typescript-eslint/no-explicit-any: error`** (`eslint.config.js`): both moved from `warn` to `error`. Four remaining `console.warn` fallbacks in `routes/agent/+page.svelte`, `tokens.ts`, `message.service.ts`, `workflowExecutor.service.ts` were silent on errors that the rest of the flow already handled — they're removed instead of suppressed.
- **`Vitest` config consolidated** (`vitest.config.ts` is now the single source of truth): `vite.config.ts` no longer declares a `test` block. All four aliases (`$lib`, `$app`, `$types`, `$messages`) use `fileURLToPath` rather than absolute string paths so the resolver works regardless of the cwd.
- **`vite` build target bumped to `chrome105 / safari15 / es2022`** (was `chrome100 / safari13 / es2021`): Tauri 2's WebView is modern on every supported platform, so the older targets just emitted polyfills for `top-level await`, `.at()`, `error.cause`, `Object.hasOwn` for nothing.
- **`svelte-check` `--threshold warning` flag dropped** (`package.json`): the flag treated warnings as errors and masked the real failure signal in CI output. svelte-check is error-only by default.
- **MCP child-process stderr drained continuously** is preserved from 0.22.2; this release adds a **`MAX_MCP_LINE_BYTES = 4 MiB` cap on a single MCP response line** (`mcp/server_handle.rs`) so a misbehaving JSON-RPC server streaming unbounded data without `\n` cannot OOM the host.
- **Retry backoff is jittered** (`llm/retry.rs::with_retry` and `with_retry_cancellable`): a 0..10% additive jitter is applied to the base delay just before sleep so multiple clients waking up after a transient outage do not synchronize into a thundering-herd hit. `delay_for_attempt` stays pure (testable). `rand 0.8` added as a direct dependency.
- **Frontend Tauri keystore IPC sends `provider.id` (lowercase) verbatim** (`APIKeysSection.svelte`, `StepApiKey.svelte`): drop the `charAt(0).toUpperCase() + slice(1)` capitalization at the save / delete sites. The keystore happens to be case-insensitive today so the bug was latent, but case-sensitive providers (or a future keystore swap) would have broken save vs. read silently. Backend `Validator::validate_provider` already accepted both cases — the drift was purely frontend.
- **CSP whitelist tightened** (`tauri.conf.json`): explicit `connect-src 'self' ipc: http://ipc.localhost`. Every LLM / MCP request goes through the Rust backend (verified: 0 `fetch(` calls in `src/`), so whitelisting external endpoints would be cargo cult — `'self'` is the strict correct value, plus the Tauri 2 IPC allowances.
- **GitHub Actions pinned by full commit SHA** (`actions/checkout`, `actions/setup-node`, `swatinem/rust-cache`, `tauri-apps/tauri-action`, `dtolnay/rust-toolchain`) in `validate.yml`, `release.yml`, and `setup-rust-backend` composite. Each pin carries a trailing comment naming the resolved tag. Prevents tag-rewrite supply-chain attacks per SLSA Level 3 / GitHub hardening guidance.
- **`release.yml` permissions scoped**: workflow-level `contents: write` → `contents: read`. The `contents: write` permission is now declared only on the `build` job that actually uploads release artifacts, narrowing the trust boundary if any pinned action is ever recompiled maliciously.
- **`save_export_to_file` uses `tokio::fs::write`** (`commands/import_export/export.rs`): the command runs on the tokio runtime; the previous blocking `std::fs::write` parked a worker thread for the duration of disk I/O and could stall other concurrent commands under load.
- **Models serde alignment** (`models/user_question.rs`, `embedding.rs`, `llm_models.rs`, `sub_agent.rs`, `src/types/sub-agent.ts`): `UserQuestion` / `UserQuestionResponse` get `rename_all = "camelCase"` to match the TS counterparts; `EmbeddingTestResult.error` and 6 `SubAgentExecution.Option<*>` fields get `skip_serializing_if`; `ConnectionTestResult` gets `Deserialize`; the missing `parent_message_id?: string` is added on the TS `SubAgentExecution` (already present on Rust since PR #119).

### Fixed

- **Six SurrealQL interpolation sites no longer accept crafted IDs** (`commands/prompt.rs` ×3, `tools/utils.rs::delete_with_check`, `tools/memory/helpers_search.rs::describe_memories_core`, `mcp/manager/db.rs::update_server_config`, `commands/embedding/operations.rs`, `streaming/pricing.rs::update_workflow_cumulative_metrics`): every site now calls `validate_uuid_field` (or binds via `$wf_id`) before the `format!()`. `describe_memories_core` was rebuilt around a single `validate` + `bind` instead of single-quote escaping. The hot metrics path bails silently with a `warn!` rather than panic to stay infallible. Defense in depth: today's frontend always sends well-formed UUIDs, but a buggy or hostile caller no longer reaches the query layer.
- **SSE buffer / payload OOM caps** (`llm/sse.rs`): `SseParser::feed` rejects when the internal buffer (un-terminated event data accumulating across TCP frames) would exceed `MAX_SSE_BUFFER_BYTES = 16 MiB`; `collect_sse_to_json` rejects a single SSE `data:` payload before `serde_json::from_str` when its length exceeds `MAX_SSE_PAYLOAD_BYTES = 4 MiB`. Both surface `LLMError::ResponseTooLarge`, which is non-retryable, so a misbehaving upstream causes a clean error rather than amplified retries.
- **`file_manager` search escapes the sandbox via mid-walk symlink swap (TOCTOU)**: `search_glob` and `search_content` only validated the root once. A malicious agent that swapped a directory entry for a symlink between the initial validation and the actual read could list filenames or read file contents from `/etc`, `/home/<other-user>/`, etc. The recursive walk now re-canonicalizes every entry returned by `read_dir` and verifies it still resolves inside the agent's `authorized_folders` — escaping entries are skipped with a `warn!`. Two regression tests on Unix exercise the symlink-out attack on both search modes.
- **`file_manager` trash sandbox escape** (`commands/file_manager.rs::list_trash` + `restore_from_trash_cmd`): both routes now go through `validate_folder_for_authorization` and a `starts_with` check on the canonical `.zileo-trash/` path so the restore destination cannot escape the per-folder trash sandbox.
- **Cancellation did not propagate between tool executions** (`agents/execution/iteration.rs` + `tool_loop.rs`): three new gates check `cancellation_token.is_cancelled()` (a) before the function-call execution loop, (b) at the top of each iteration of that loop, (c) at the top of the outer `tool_loop` iteration. A user who cancelled mid-iteration no longer keeps running the remaining tools. Each gate produces `IterationOutcome::Failed("cancelled")` (resp. `Report::failed_with_metrics(..., "cancelled")`) preserving the metrics gathered up to the cancel point.
- **`background-workflows` store could register duplicate Tauri listeners** (`src/lib/stores/background-workflows.ts`): concurrent `init()` calls from multiple components mounting simultaneously could pass the `isInitialized` check before any of them flipped it, registering duplicate listeners. The in-flight promise is now memoized so concurrent callers share the same async init; it is reset in `destroy()` and on init failure so retry remains possible.
- **`folders` and `workflows` stores swallowed errors silently in 11 mutating actions** (`createFolder`, `renameFolder`, `updateColor`, `deleteFolder`, `reorderFolders`, `renameWorkflow`, `deleteWorkflow`, `deleteBatch`, `moveToFolder`, `moveBatchToFolder`, `togglePinned`): the call site got the rejection but the store's own `error` field stayed `null` and `loading` could stay `true` forever. Each action now follows the audit-log pattern: clear error → try → re-throw.
- **`save_export_to_file` count-based logic returned 0 on transient DB errors instead of failing**: six clear/count call sites in workflow-children deletion (`sub_agent_execution`, `thinking_step`, `tool_execution`, `message`) and memory stats used `unwrap_or_default()` on the `count()` result. A transient DB error masked the real table state and made the user-visible deletion counter wrong. Errors are now surfaced via `map_err + ?`.
- **`KeyStore::default()` silently downgraded to keyring-only when master-key bootstrap failed** (`security/keystore.rs`): the AES layer was lost without any signal. The default constructor now fails loudly instead of falling back; application startup uses explicit secure keystore initialization and aborts if secure storage cannot be initialized.
- **Mistral connection probe logged the full HTTP error body**: the body can echo account-level metadata or quota details. The log line now drops the body; the user-facing error message already carries it for diagnosis.
- **Prompt-cache breakpoint `BP2` correctness** (carried over from 0.23.0 where the underlying assistant-message preservation landed): the new tool-loop cancellation gates close a residual window where a `Continue` outcome could spin one more iteration after cancel.

### Security

- Defense-in-depth audit closure: 17 HIGH findings + ~22 MEDIUM findings from the 2026-05-08 8-agent audit (full 70k-LOC sweep across `commands`, `agents`, `llm`, `mcp`, `db`, `tools`, `security`, `models`, frontend, types, routes, i18n, configs, CI). Plan in `docs/specs/audit-hardening-2026-05-08.md`.
- `cargo audit` and `npm audit` (`production` only, `--audit-level=high`) added as advisory jobs in `validate.yml`. Both are `continue-on-error: true` for now — visible in the PR check list, not gating on merge — so the advisory baseline can be cleaned up before they become blockers.
- Sub-agent recursion-amplification guard (`agents/execution/tool_loop.rs`): defense-in-depth assertion that any task with `is_sub_agent` / `is_delegation` / `is_parallel_task` ALSO has `is_primary_agent: false`. Downgrade with `warn!` in production, `debug_assert` in debug builds, in case a future caller forgets.

---

## [0.23.0] - 2026-05-06

### Added

- **SSE streaming shim for LLM responses**: New internal `llm::sse` module decodes upstream Server-Sent Events from streamed LLM completions. Lays the groundwork for live token deltas without blocking on full responses
- **LLM snapshot hydration on startup**: At app startup, every agent's persisted `LLMConfig` snapshot now refreshes its `is_reasoning` and `context_window` fields from the current `llm_models` row via `commands/agent::hydrate_llm_from_model`. Without this, an agent saved before its model was flagged `is_reasoning=true` kept a stale snapshot -- `effective_reasoning_effort` returned `None`, the `reasoning_effort` parameter never reached the provider, and the Reflexion UI block never appeared. Failure to hydrate logs a warning and keeps the persisted snapshot, so a transient DB error does not break startup

### Changed

- **HTTP `read_timeout` replaces total `timeout` in the shared LLM client** (`llm/manager.rs`): The previous `Client::builder().timeout(300s)` cut the wire mid-thinking on long reasoning sessions even when the server kept emitting SSE chunks. The shared client now uses `read_timeout(DEFAULT_READ_TIMEOUT_SECS)` (per-read, resets on each successful read), uniform with the per-provider test clients in `mistral.rs` / `openai_compatible.rs`, so streaming sits idle through long thinking phases as long as the server keeps emitting chunks
- **Frontend Tauri adapter layer (`src/lib/tauri/`)**: 6 modules (`core`, `events`, `window`, `dialog`, `opener`, `environment`) centralize all `@tauri-apps/*` access. 62 frontend files migrated from direct `@tauri-apps/*` imports to `$lib/tauri`. Browser-runtime fallbacks make Vitest / preview environments work without touching the native runtime. Vitest mocks now target `$lib/tauri`. Includes onboarding `localStorage` guards and a new i18n parity test for `en.json` / `fr.json` placeholders (PR #130)

### Fixed

- **`ParallelTasksTool` per-batch cap was using the cumulative workflow cap**: `validate_input` and `validate_tasks` both compared `tasks.len() > MAX_SUB_AGENTS` (15). `MAX_SUB_AGENTS` is the cumulative cap counting every spawn / delegate / parallel operation across the whole workflow, not the size of a single batch. The tool's JSON schema (`maxItems: 3`) and description (`max 3 per batch`) already pointed at 3, but server-side validation accepted up to 15 per call. New constant `MAX_PARALLEL_TASKS_PER_BATCH = 3` and a pure `validate_batch_size(len)` helper now drive both validation sites; guard tests keep the description text and schema wired to the constant. `MAX_SUB_AGENTS` still enforced by `SubAgentExecutor::check_limit` for the cumulative cap (PR #131)
- **Context window gauge stuck at hardcoded `/128000`**: `tokens.ts` initial `contextMax` was 128000 plus a `?? 128000` fallback in `updateFromModel`, even though `LLMModel.context_window: number` is a required field. Frontend dead code field `PageState.currentContextWindow` (written 4 places, never read) compounded the confusion -- the actual value flows through `tokenStore.updateFromModel()`. Initial `contextMax` is now 0 (TokenDisplay already guards against division by zero, gauge stays at 0% until the model loads), `updateFromModel` reads `model.context_window` directly, and `currentContextWindow` is removed (PR #129)
- **Cancellation did not propagate to sub-agents**: Cancelling a primary workflow left active sub-agents running in the background. The cancellation token is now propagated through spawn / delegate / parallel paths so child agents stop with the parent (PR #129)
- **Orchestrator context gauge showed cumulative tokens, not the last call**: The "context" gauge on the agent page summed every iteration's tokens instead of showing the last orchestrator call. The display now reflects the last LLM call only, matching what the next call will actually send (PR #129)
- **Frontend race on workflow switch overwrote metrics from the previous workflow**: Several `await`s on workflow load (`MessageService`, runtime preferences, theme persistence) had no guard against the user switching to a different workflow mid-await. Race guards now check the currently-viewed workflow id before applying state (PR #127)
- **`localStorage` / `document` / `navigator` / `matchMedia` accesses unsafe in non-browser contexts**: Several stores (`theme`, `locale`, onboarding) accessed these globals without guards, which broke in the Vitest / preview environments and in any code path that ran before the renderer was ready. All accesses now go through guarded helpers; failures degrade gracefully with logs / toasts instead of throwing silently (PR #127)
- **Streaming cancellation cycle: backend kept handles after frontend disconnects**: The streaming execution path could leak an `MCPServerHandle` or a cancellation token if the frontend stopped listening between iterations. The backend now cleans up under the same lock that decided to abort, and the frontend toggles `isStreaming` only after the cancel ack (PR #127)
- **Prompt-cache breakpoint dropped `tool_calls` and `reasoning_details` from assistant messages**: The BP2 marker in `cache_control` rebuilt the assistant message keeping only `role` + `content`, silently stripping `tool_calls` and `reasoning_details`. On iteration 2 of the tool loop, OpenRouter forwarded `tool_result` messages whose `tool_call_id` no longer matched any `tool_use`, and Anthropic rejected the request with HTTP 400 ("Provider returned error"). The deterministic 400 was retried 3x at exponential backoff, amplifying cost on what should have been an instant fail. The marker now mirrors the existing tool-role preservation via a match on `role`; `reasoning_details` is preserved as required by OpenRouter docs for Anthropic thinking continuity (signed blocks). Mistral native and Ollama bypass this code path and were unaffected. Two regression tests added (PR #128)

---

## [0.22.2] - 2026-05-03

### Fixed

- **MCP `stop_server` race losing the client on `disconnect()` failure**: The previous order removed the client from `clients` and cleaned the lookup tables BEFORE calling `disconnect()`. If `disconnect()` errored, the `MCPServerHandle` was already gone from the registry -- the child process was leaked and the user could not restart the server cleanly. The lock is now held atomically: `disconnect()` runs while the client is still in `clients`, the registry cleanup only happens once the disconnect has succeeded, and a `disconnect()` failure surfaces to the caller without dropping the handle (PR #125)
- **MCP `restart_server` swallowing real stop errors**: `let _ = self.stop_server(id).await` discarded every failure, including legitimate disconnect errors that should have blocked respawn. The match now treats only `MCPError::ServerNotFound` as a no-op (server already stopped) and propagates every other error so a broken disconnect doesn't lead to a duplicate process / state (PR #125)
- **Workflow streaming: cancellation tokens leaked on early errors**: `execute_workflow_streaming` allocated a `cancellation_token` then bailed out via `?` on `load_workflow` / `build_task` failures, leaving the token in `state.streaming_cancellations`. Both error paths now call `state.clear_cancellation(&workflow_id)` before returning. `build_task` errors also emit `WORKFLOW_COMPLETE` to the frontend so the user sees the failure instead of a silent stall (PR #125)
- **`load_conversation_history` swallowing DB and deserialization errors**: The previous code chained `.unwrap_or_default()` on the DB response and `.filter_map(|v| ... .ok())` on row deserialization, so a real failure produced an empty history and the workflow ran without any context. Both stages now propagate `Result<_, String>` with structured `tracing::error!` logs; `build_task` and `execute_workflow_streaming` propagate the error to the frontend instead of silently masking it (PR #125)
- **Race after `getLastAssistantMetrics` in `selectWorkflow`**: The `await MessageService.getLastAssistantMetrics(workflowId)` call introduced by v0.22.0 had no `isStillViewed()` guard, so a fast workflow switch could overwrite the newly-selected workflow's session metrics with the previous workflow's last-message metrics. A `if (backgroundWorkflowsStore.getViewedWorkflowId() !== workflowId) return;` check is now wired right after the await (PR #125)
- **Assistant bubble missing cost / cache / thinking metrics until reload**: `createAssistantMessage` only forwarded `tokens`, `tokens_input`, `tokens_output`, `model`, `provider`, `duration_ms`. The other fields exposed by `WorkflowMetrics` (`cost_usd`, `thinking_tokens`, `cached_tokens`, `cache_write_tokens`, `model_id_used`) defaulted to `undefined` on the local `Message` until the next workflow reload pulled them from the persisted row. The local message now mirrors the persisted assistant message field-for-field (PR #125)

### Changed

- **MCP child process stderr is drained continuously**: `MCPServerHandle::spawn` now starts a named (`mcp-stderr-{name}`) background thread that reads the child's stderr line by line and forwards non-empty lines to the `tracing` log. Without this, a chatty MCP server eventually filled the OS pipe buffer and the child blocked on `write(stderr)`. The thread terminates naturally when the child exits / EOF is reached, and a failure to spawn the drain thread is logged via `warn!` rather than panicking (PR #125)

---

## [0.22.1] - 2026-05-03

### Fixed

- **Multi-platform release CI broken on v0.22.0**: `release.yml` (Linux / macOS aarch64 + x86_64 / Windows) failed at the `Build Tauri app` step on all four platforms with `Found version mismatched Tauri packages. tauri (v2.11.0) : @tauri-apps/api (v2.10.1)`. The v0.22.0 version-bump regenerated `Cargo.lock` and silently moved the Rust `tauri` crate from 2.10.x to 2.11.0 (caret range in `Cargo.toml`), while `package.json` still pinned `@tauri-apps/api` at `^2.9.0` (resolved 2.10.1). `tauri-action` rejects the mismatch and offers no escape hatch, so the multi-platform release never produced macOS / Windows assets -- v0.22.0 shipped Linux-only

### Changed

- **`@tauri-apps/api` bumped to `^2.11.0`** to match the Rust crate. `npm install` re-resolves the lockfile to 2.11.x. Local `tauri build` no longer needs `--ignore-version-mismatches`

---

## [0.22.0] - 2026-05-02

### Added

- **Live workflow metrics during streaming**: New `ChunkType::IterationProgress` is emitted from the tool loop after every LLM call (cumulative tokens + per-iteration cost). The metrics bar now updates ENTREE/SORTIE, contexte and t/s on each iteration instead of staying frozen at 0 until completion. `TokenDisplay` shows a `~` prefix and pulse animation while a partial cost is still progressing
- **Per-iteration provider cost**: `StreamChunk.cost_usd` is resolved by `persistence_step` before emitting `response_block`, so the chunk carries the per-iteration cost. Frontend accumulates it via `tokenStore.setPartialSessionCost` + a `sessionCostInProgress` flag; `BackgroundExecution` carries `partialCostUsd` so a switch back to a still-running workflow restores the in-progress cost
- **Sub-agent self-cost**: Sub-agents persist their own cost (computed with their own pricing, not the parent's) into `sub_agent_execution.cost_usd`; `aggregate_sub_agent_metrics` sums it into `workflow.sub_agent_cost_usd`. `compute_sub_agent_cost` covered by unit tests
- **`PricingStatus` enum**: Surfaces "free" vs "pricing missing" instead of a binary present/absent. Frontend renders a "pricing inconnu" badge for missing pricing rather than the misleading "Free"
- **`formatCost` utility (`$lib/utils/currency.ts`)**: Single source of truth for cost formatting (USD, em-dash placeholder when null) with full Vitest coverage
- **`resolveOrchestratorLabel` helper**: Resolves the agent's display name for the orchestrator spinner with a graceful fallback to `agent_id` when the name is missing or blank
- **Bounded chunk-history buffer for background workflows**: `WorkflowStreamState.chunkHistory` (FIFO, `MAX_CHUNK_HISTORY = 1000`) records every incoming streaming chunk. Pairs with the new `executionBlocksStore.restoreFromChunks(workflowId, chunks)` to rebuild the timeline when the user reattaches to a still-running workflow
- **Migration `token_cost_accuracy_v1`**: Backfills `sub_agent_cost_usd`, `total_cached_tokens` and `total_cache_write_tokens` on legacy `workflow` rows. Auto-runs at boot, idempotent via `migration_log`

### Changed

- **LLM provider response shape**: `LLMResponse` now exposes `cached_tokens`, `cache_write_tokens` and `provider_cost_usd` across Mistral, OpenAI-compatible and Ollama adapters
- **Mistral standard path tokens**: Reads from rig-core's `GetTokenUsage` instead of word-count estimates; Magistral content-block array fully handled
- **`pricing` module**: Extracted to `llm/pricing.rs` with `compute_sub_agent_cost` + `resolve_cost`; the streaming `pricing` step now drives both the resolved cost and `pricing_status`
- **`execute_simple` wiring**: Cache + provider_cost_usd flow into `ReportMetrics` so the pricing layer sees the same data the tool-loop already had
- **Embedding stats**: Use real `prompt_tokens` from the embedding response instead of estimates
- **`BackgroundExecution` carries token state**: `tokensSent`, `cachedTokens`, `cacheWriteTokens`, `partialCostUsd` so reattaching to a running workflow restores the full token panel (not only the output count)
- **`selectWorkflow`**: Restores the full session display from the last assistant message (`model_id_used` + tokens + cost) so a workflow that hasn't run today no longer shows blank zeros
- **Sub-agent `parent_message_id` set at CREATE time**: `SubAgentExecutionCreate` now persists `parent_message_id` per sub-agent (via `with_parent_message`), threading it through `AgentToolContext.current_message_id` and the spawn / delegate / parallel tools. Replaces the previous bulk `UPDATE WHERE parent_message_id IS NONE` patch in `persistence_step.rs` which incorrectly attached every orphan sub-agent to the same primary message. Spawning agents put a fresh UUID in their sub-agent's `task.context["message_id"]` so chains (B→C, defensive) attribute correctly
- **`migration_log` queries use parameter binding**: `check_migration_applied` and `record_migration_applied` switch from `format!()` interpolation to `query_json_with_params` / `execute_with_params` with `$name` binding. Defence-in-depth aligning with the SA-001 / ERR_SEC_001 cleanup; locked in by a new test that round-trips a migration name containing an apostrophe

### Fixed

- **`ERR_SURREAL_005` in `get_workflow_last_assistant_message_metrics`**: The query used `ORDER BY timestamp` without including `timestamp` in the `SELECT` idiom, which SurrealDB rejects with "Missing order idiom in statement selection". Logic extracted to `last_assistant_message_metrics_core` for testability with 4 new integration tests against a real DB
- **Speed (t/s) regression**: `setSessionTokens` now computes `tokens_output / elapsed` when streaming is active. The previous helper had been removed in a refactor, leaving the displayed speed permanently at `null`
- **Orchestrator spinner shows raw UUID at workflow start**: `tool_start` was emitted with `agent_id` as the tool name, so the spinner displayed the UUID until the first agent label was resolved. The orchestrator bridge now resolves the agent's display name via the registry once, just before the race, and feeds it through `resolve_orchestrator_label` (M4 audit 2026-05-02)
- **`submit_user_response` / `skip_question` lost workflow_id**: Both commands emitted `user_question_complete` with `String::new()`, so the background-workflows dispatcher silently dropped the chunk via `executions.get("")` -- leaving `hasPendingQuestion` stuck at `true` until `workflow_complete`. Both commands now require and validate the UUID; the emitted chunk carries it (H1 audit 2026-05-02)
- **Sub-agent execution timeline blank on reattach**: Switching back to a workflow already running in the background reset `executionBlocksStore` on every selection, leaving the execution area blank until the next chunk arrived. `selectWorkflow` now calls `restoreFromChunks` instead of `start()` when reattaching, replaying the buffered `chunkHistory` through the existing chunk handlers (H3 audit 2026-05-02)

### Removed

- **Dead code (Lot C)**: ~603 LOC across 17 files
  - Backend: `ChunkType::ToolEnd` and `SubAgentProgress` variants (no production emission site -- `tool_call_complete` already carries the closure for tools); `tokens_delta` / `tokens_total` fields on `StreamChunk` (never set, never read); `aggregate_sub_agent_tokens` backwards-compat alias (callers migrated to `aggregate_sub_agent_metrics`); duplicate `test_stream_chunk_creation` / `test_workflow_complete_creation` in `commands/streaming/execution.rs`
  - Frontend: legacy `MetricsBar.svelte` (never mounted) and `navigation/` folder (`NavItem` + barrel, never imported); `inputPrice` / `outputPrice` / `cacheReadPrice` / `cacheWritePrice` fields, `setPricingStatus` method + `pricingStatus` state, `streamingTokens` and `cumulativeTokens` derived stores from `tokens.ts`; `SubAgentSpawnResult`, `DelegateResult`, `ParallelTaskResult`, `ParallelBatchResult`, `SubAgentEventType`, `SubAgentStreamEvent`, `SubAgentOperationType`, `ValidationResponseEvent` from `sub-agent.ts`; `STREAM_EVENTS` constant; `handleToolEnd` / `handleSubAgentProgress` chunk handlers
- **41 stale "Phase N" sequencing comments**: Carried over from staged refactors and no longer meaningful once merged. Stripped from Rust production code, frontend stores, components, types, tests and schema SQL while preserving the semantic content that followed each marker

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
- 171 OPT-\* traceability markers from codebase
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
  - Composite scoring: cosine_similarity*0.7 + importance*0.15 + recency\*0.15
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

[Unreleased]: https://github.com/assistance-micro-design/Zileo-Chat/compare/v0.27.0...HEAD
[0.27.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.27.0
[0.26.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.26.0
[0.25.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.25.0
[0.24.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.24.0
[0.23.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.23.1
[0.23.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.23.0
[0.22.2]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.22.2
[0.22.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.22.1
[0.22.0]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.22.0
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
