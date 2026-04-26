# Architecture Decisions

> Last updated: 2026-04-25 | Phase 8 complete + Security Audit Remediation + Code Quality Refactoring + Validation Audit Backend
> Validated and implemented. 1000+ tests passing across backend and frontend.

---

## 1. Architecture and Stack

### Q2: Project Structure

**Decision**: From scratch with standard Tauri structure (monorepo).

**Rationale**:
- Generic Tauri templates add overhead for a multi-agent architecture
- Full control over folder organization for agents/prompts/tools
- Frontend (SvelteKit) and backend (Rust) are tightly coupled via Tauri IPC, no need for multi-package versioning

### Q3: Monorepo

**Decision**: Single monorepo, no multi-package setup.

**Rationale**:
- Desktop app with no external reuse planned
- One repo, one build, simplified coordination

---

## 2. Database and Persistence

### Q4: SurrealDB Schema

**Decision**: Full schema with graph relations (SCHEMAFULL tables with DEFINE TABLE/FIELD).

**Rationale**:
- Multi-agent system requires complex relations (agent -> workflow -> tasks -> memory)
- SurrealDB graph capabilities exploited natively
- Relational queries essential for cross-entity lookups

**Core entities**: `agent`, `workflow`, `memory`, `message`, `validation_request`, `validation_audit`, `task`, `tool_execution`, `thinking_step`, `sub_agent_execution`, `mcp_server`, `mcp_call_log`, `llm_model`, `provider_settings`, `custom_provider`, `user_question`, `workflow_folder`, `skill`.

**Memory types**: `user_pref`, `context`, `knowledge`, `decision`. Vectorial embeddings for semantic search.

**Graph relations**: workflow <-> agent (many-to-one), workflow -> messages/tasks/validations (one-to-many), agent -> memory (one-to-many), memory <-> memory (semantic links).

See `docs/DATABASE_SCHEMA.md` for the complete schema.

### Q5: Workflow Versioning

**Decision**: Simplified audit trail, not full versioning.

**Rationale**:
- Audit trail sufficient for debugging and compliance
- Full versioning (snapshots, rollback, diff) excessive for v1
- Tracks: all state changes, validations, tool calls, MCP calls with timestamps

### Q6: Retention Policy

**Decision**: Differentiated retention by data type.

- **Workflows**: completed 90d, error 180d, running no auto-delete
- **Logs**: app 30d, audit 1y, metrics 90d (monthly aggregation after)
- **Memory**: temporary (deleted with workflow), permanent (no expiry), pruning by relevance score
- **Reports**: 30d in DB, then archived to filesystem

---

## 3. Security and Operations

### Q7: Security Level

**Decision**: Production-ready from v1.

**Rationale**:
- App handles sensitive user data and LLM API keys
- Desktop app has filesystem/system access
- Measures: encrypted secrets, input validation, sanitization before DB/LLM, process isolation for MCP servers, audit logging

Not in scope for v1: external pentesting, SOC2 certification, multi-factor auth (local desktop app).

### Q8: API Keys Storage

**Decision**: OS-native secure storage via Tauri (Keychain / Credential Manager / libsecret) with additional AES-256 encryption.

**Rationale**:
- Defense in depth: OS keystore + app-level encryption
- Keys read only at LLM call time, temporary encrypted cache during session
- No plaintext keys in memory longer than necessary
- API keys are never exported (Import/Export omits them, warns user to reconfigure)

### Q9: Logging Framework

**Decision**: `tracing` (Tokio ecosystem) with structured JSON logging and spans.

**Rationale**:
- Async-first, compatible with Tokio runtime
- Contextual fields (agent_id, workflow_id, tool_name) for multi-agent correlation
- Levels: ERROR/WARN/INFO/DEBUG/TRACE. Dev: console formatted. Prod: rotated JSON files.

### Q10: Error Handling

**Decision**: `anyhow` + `thiserror` combined.

**Rationale**:
- `anyhow` for application code (agents, tools, commands; propagation with `.context()`)
- `thiserror` for typed public errors in `llm` (LLMError, embedding error). `mcp` exposes a typed `MCPError` enum with manual Display impl
- IPC boundary: errors converted to user-friendly strings, never stack traces in UI (commands return `Result<T, String>`)
- Recovery strategy: graceful degradation, no `panic!` in production, workflow pause on critical error

---

## 4. LLM Providers

### Q12: Provider Routing

**Decision**: User choice with intelligent suggestions. No auto-routing.

**Supported providers**:
- **Mistral**: Cloud API via api.mistral.ai (small/large/medium models, reasoning via Magistral)
- **Ollama**: Local inference, privacy-first, free
- **Custom providers (OpenAI-compatible)**: Any provider exposing `/v1/chat/completions` (OpenRouter, RouterLab, etc.)

**Behavior**:
- User selects provider per agent configuration
- Last choice persisted as default
- Fallback: proposed (not silent) if provider is down, user approves or rejects
- Suggestions based on task complexity (small model for simple tasks, large for reasoning)

### Q12b: Provider Resilience

**Decision**: Multi-layer protection with rate limiting + retry + circuit breaker.

**Rationale**:
- Rate limiting: 1 req/s minimum delay (Mistral Free Tier compliance)
- Retry: 3 max with exponential backoff (1s, 2s, 4s), capped at 30s. Non-retryable: auth errors, circuit open
- Circuit breaker: 3 consecutive failures -> 60s cooldown, half-open for test request
- HTTP connection pooling: centralized reqwest::Client, 5 idle connections/host, 300s timeout

---

## 5. Tools and Agents

### Q5b: Built-in Tools

**Decision**: Extensible tool system with built-in tools and MCP integration.

**Built-in tools (9)**: Calculator, Memory, Todo, UserQuestion, FileManager, ReadSkill (hidden, auto-injected), SpawnAgent, DelegateTask, ParallelTasks. TaskBridge is a scoping module, not a standalone tool.

**MCP tools**: Discovered dynamically from user-configured MCP servers. Cached with 1h TTL.

### Q5c: ToolDefinition Summary/Description Split

**Decision**: Every ToolDefinition has two fields: `summary` (1-line) and `description` (structured, full guidance).

**Rationale**:
- `summary` used in system prompt for token efficiency (lightweight context)
- `description` used in API `tools` parameter (full structured guidance with USE/DON'T USE bullets, operations, examples)
- MCP tools auto-extract `summary` from first sentence of their description
- Skills listed by name only in system prompt (instructions loaded via ReadSkill)
- Provider/model/MCP delegation info injected only if agent has delegation tools

### Q5d: Task Bridge

**Decision**: TaskBridge for TodoTool primary/sub-agent scoping with task_ids in DelegateTask/ParallelTasks.

**Rationale**:
- Sub-agents need scoped access to parent tasks without global state
- task_ids parameter in delegation tools enables explicit task assignment
- Clean separation between orchestrator and worker agent contexts

---

## 6. MCP Configuration

### Q11: MCP Servers

**Decision**: No pre-integrated MCP servers. User configures via Settings > MCP.

**Rationale**:
- Maximum flexibility: user chooses servers based on needs (privacy, cost, performance)
- Supports docker, npx, uvx, and custom binary commands
- Templates suggested for popular servers (serena, context7, playwright)

**Deployment approaches** (documented, not imposed):
- Docker local (privacy, offline)
- NPX/UVX (simple install)
- SaaS distant (no maintenance, scaling)

### Q17: MCP Deployment Guidance

**Decision**: Document multiple approaches, let user choose.

**Rationale**:
- Application executes any valid command configuration
- Documentation provides trade-offs per approach (privacy vs performance vs cost)
- Recommended: Docker for sensitive code, SaaS for public data, NPX/UVX for development

### Q18: Hot-Reload Registry

**Decision**: Static configuration at startup (v1). Restart required for changes.

**Rationale**:
- Hot-reload adds significant complexity
- Adding MCP servers is infrequent
- Future v2 consideration if user demand is high

### Q19: Error Recovery Strategy

**Decision**: Graceful degradation with user notification.

- **Level 1**: Automatic retry (3x exponential backoff) for transient errors
- **Level 2**: Fallback (skip unavailable tool, continue workflow, notify user)
- **Level 3**: User decision (pause workflow, offer Retry/Skip/Abort)
- Never silent failure: always logged, user informed if workflow impacted

### Q25: MCP Resilience and Monitoring

**Decision**: Circuit breaker + health checks + latency metrics.

- Circuit breaker: 3 failures -> 60s cooldown, per-server state
- Health checks: background task every 5min using refresh_tools() as probe
- Latency metrics: P50/P95/P99 from mcp_call_log table (last 1h window)
- Tool caching: 1h TTL, invalidated on errors
- ID lookup table: O(1) server operations via id_to_name HashMap

---

## 7. Features and Testing

### Q13: Testing Coverage

**Decision**: Critical paths prioritized with minimum coverage targets.

**Critical paths tested**:
- Workflow execution (input -> agent -> LLM -> streaming response)
- Agent orchestration (multi-agent workflows, inter-agent reports)
- Tools execution (MCP calls, DB operations, memory vectorial retrieval)
- Validation human-in-the-loop (approve/reject flow)

**Targets**: ~70% backend critical modules. Frontend: E2E for main workflows. 1000+ tests across Rust and TypeScript.

### Q14: CI/CD Pipeline

**Decision**: GitHub Actions with parallel jobs.

- **On PR**: linting (clippy --all-targets, eslint), unit tests, build check, security audit
- **On merge to main**: full test suite, multi-platform build (Linux, macOS, Windows)
- **On tag push (v*)**: multi-platform draft release with artifacts

---

## 8. Deployment

### Q15: Target OS

**Decision**: Linux first, then macOS, Windows Phase 2.

- Linux: primary development platform, AppImage + .deb
- macOS: easy Tauri cross-compile, .dmg
- Windows: .msi (future, lower priority)

### Q16: Auto-Updates

**Decision**: Manual updates v1, Tauri built-in updater planned for v1.5.

**Rationale**:
- Young application with frequent breaking changes
- v1: GitHub Releases (manual download)
- v1.5: Tauri updater with user notification and approval (never silent updates)

---

## 9. Frontend State Management

### Q20: Store Patterns

**Decision**: CRUD factory as canonical pattern for persisted entities, with documented alternatives.

| Pattern | Use Case | Examples |
|---------|----------|----------|
| **CRUD Factory** (canonical) | Entities with DB persistence | agents.ts, prompts.ts |
| **Pure Functions** | API calls without local state | llm.ts, mcp.ts |
| **Event-Driven** | Tauri real-time events | streaming.ts, validation.ts |
| **Custom Factory** | localStorage persistence | theme.ts, locale.ts |

**Rules**:
- New persisted entity -> use createCRUDStore factory
- Pure API calls -> pure functions (component manages its own state)
- Tauri events -> event-driven with mandatory init/cleanup/reset lifecycle
- Never duplicate: one pattern per store (hybrid pattern is deprecated)

### Q21: Svelte 5 Runes Migration

**Decision**: COMPLETED. All components and stores use Svelte 5 runes.

**Result**:
- Full migration to $state, $derived, $effect runes
- Snippets ({#snippet} + {@render}) replace slots
- {@attach} replaces use:action
- $props() replaces export let
- createContext replaces setContext/getContext

### Q22: Utility Factories

**Decision**: Factories for repetitive patterns (modal controllers, async handlers).

**Rationale**:
- Modal controller factory reduces ~30 lines per modal instance
- Async handler factory eliminates repetitive try/catch/finally boilerplate
- Both use Svelte 5 runes (.svelte.ts files) for reactive state outside components

---

## 10. Settings Page Architecture

### Q26: Settings Navigation

**Decision**: Route-based navigation instead of scroll-based.

**Rationale**:
- Scroll-based IntersectionObserver caused 30-60 callbacks/sec
- 798-line monolithic page was unmaintainable
- Route-based enables code splitting, browser history, shareable URLs

**Routes**: `/settings/providers`, `/settings/agents`, `/settings/mcp`, `/settings/memory`, `/settings/validation`, `/settings/audit-log`, `/settings/prompts`, `/settings/skills`, `/settings/import-export`, `/settings/theme`.

---

## 11. Import/Export

### Q27: Import/Export Schema v1.1

**Decision**: Versioned schema (v1.0 backward compatible, v1.1 current) with 6 entity types.

**Rationale**:
- Schema v1.1 adds skills and custom providers to the original 4 entity types
- Import order enforces dependency resolution: custom_providers -> models -> mcp_servers -> skills -> agents -> prompts
- Cross-entity references by NAME (not UUID) so orphan refs are safe (user creates missing entity later)
- API keys never exported (OS keyring) with structured ImportWarning for user guidance
- `SUPPORTED_SCHEMA_VERSIONS = ["1.0", "1.1"]` for backward compatibility
- postImportActions in ImportResult for actionable post-import checklist

---

## 12. Database Query Safety

### Q24: Parameterized Queries

**Decision**: Enforce parameterized queries and LIMIT on all list operations.

**Rationale**:
- SQL injection prevention (security audit remediation)
- Memory protection from unbounded queries
- All WHERE clauses with user input use bind parameters via `query_with_params()` / `execute_with_params()`
- Default limits: 1000 for list operations, 100 for models, 500 for logs/messages
- Transaction support with automatic rollback on failure

---

## Summary of Decisions

| Area | Key Decision |
|------|-------------|
| Architecture | Monorepo, from scratch, Tauri + SvelteKit + Rust |
| Database | SurrealDB SCHEMAFULL, graph relations, audit trail, differentiated retention |
| Security | Production-ready v1, OS keystore + AES-256, tracing, anyhow + thiserror |
| Providers | Mistral + Ollama + Custom (OpenAI-compatible), user choice, multi-layer resilience |
| Tools | Built-in (9 tools) + MCP, ToolDefinition summary/description split, TaskBridge scoping |
| MCP | User-configured, no pre-integrated servers, circuit breaker + health checks |
| Testing | Critical paths, 1000+ tests, GitHub Actions CI |
| Deployment | Linux first, manual updates v1, auto-updates v1.5 |
| Frontend | CRUD factory stores, Svelte 5 runes (completed), route-based settings |
| Import/Export | Schema v1.1, 6 entity types, cross-ref by name, no API key export |

**Technical documentation**: `docs/DATABASE_SCHEMA.md`, `docs/API_REFERENCE.md`, `docs/TECH_STACK.md`.

**Deferred post-v1**: specta + tauri-specta (Tauri 2.x incompatible), rate limiting sensitive ops, prompt injection guard, thiserror migration, live query API, Superforms integration.
