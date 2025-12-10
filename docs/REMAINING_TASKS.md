# Remaining Tasks - Post-v1 Optimizations

> **Status**: DEFERRED - To be implemented after v1 release
> **Last Updated**: 2025-12-10
> **Total Estimated Effort**: ~53-57 hours
> **Validation**: Code-verified, all v1.0 phases complete + OPT-MEM (11/11) + OPT-TODO (11/11) + OPT-UQ (12/12) + OPT-TD (8/10)

## Overview

This document consolidates all optimization items deferred to post-v1 release. These items are either:
- Blocked by external dependencies (tauri-specta v2 incompatibility)
- Low priority for initial release
- Require significant refactoring effort

## v1.0 Completed (Reference)

All optimization phases are **COMPLETE**:

| Phase | Name | Key Deliverables |
|-------|------|------------------|
| 0 | Security Critical | SurrealDB 2.4.0, CSP, API key validation, MCP env security |
| 1 | Frontend Stability | Store cleanup methods, memory leak fixes, error utility |
| 2 | DB/Backend Quick Wins | Release profile, constants, response builder |
| 3 | Types & Polish | Type sync, nullability convention, AVAILABLE_TOOLS constant |
| 4 | MCP Quick Wins | Tool caching, HTTP pooling, latency metrics |
| 5 | Strategic Backend/DB | Parameterized queries, transactions, query limits |
| 6 | Strategic MCP | Circuit breaker, ID lookup table, health checks |
| 7 | Strategic Frontend | Settings decomposition, lazy loading, cache TTL |
| 8 | LLM Optimizations | Rate limiter, retry, circuit breaker, HTTP pooling, utils |
| SA | Sub-Agent Optimizations | All 11 OPT-SA items (heartbeat, retry, circuit breaker, etc.) |
| MEM | MemoryTool Optimizations | All 11 OPT-MEM items (parameterized queries, helpers.rs, MemoryInput, indexes) |
| TODO | TodoTool Optimizations | All 11 OPT-TODO items (parameterized queries, N+1 reduction, integration tests) |
| UQ | UserQuestionTool Optimizations | All 12 OPT-UQ items (timeout 5min, circuit breaker, validation, tests, refactoring) |
| TD | Tool Description Optimizations | 8/10 OPT-TD items (enriched descriptions, dynamic constants, sub-agent template, CLAUDE.md) |

**Total Tests**: 844+ passing (backend unit)
**Code Quality**: 0 errors across all validations (clippy, eslint, svelte-check)

---

## Deferred Items by Category

### 1. Type System Optimizations

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| TYPE-OPT-5 | specta + tauri-specta | Automatic TypeScript type generation from Rust | 8-10h | **BLOCKED**: tauri-specta v2.0-rc.21 incompatible with Tauri 2.9.x |

**Prerequisites**: tauri-specta stable release with Tauri 2.9+ support

**Implementation Notes**:
- Monitor https://github.com/oscartbeaumont/tauri-specta for stable v2 release
- When available, implement in phases:
  1. Add specta derive macros to Rust models
  2. Configure tauri-specta for command bindings
  3. Generate TypeScript types automatically
  4. Remove manual type definitions

---

### 2. Security Optimizations

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| SEC-OPT-7 | Rate Limiting (Sensitive Ops) | Max requests on sensitive operations (API keys, MCP calls) | 4h | Performance impact needs production data |
| SEC-OPT-8 | Prompt Injection Guard | LLM input patterns detection and filtering | 6h | Requires LLM-specific tuning per provider |

**Prerequisites**: None (standalone features)

**Implementation Notes**:
- SEC-OPT-7: Use token bucket algorithm, configurable per operation type
- SEC-OPT-8: Pattern-based detection, configurable per agent, whitelist approach

---

### 3. Database Optimizations

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| DB-OPT-12 | thiserror Migration | Replace anyhow with thiserror for typed errors | 6h | Large refactoring, low impact on functionality |
| DB-OPT-13 | Query Caching | SurrealDB SDK query result caching | - | SDK limitation - not currently supported |
| DB-OPT-14 | Live Query API | Real-time state updates via SurrealDB live queries | 4h | Requires architecture changes for reactivity |
| OPT-TODO-8 | TodoTool helpers.rs | Create tools/todo/helpers.rs for shared logic | 2h | Waiting for MemoryTool pattern validation in production |

**Prerequisites**: None

**Implementation Notes**:
- DB-OPT-12: Migrate module by module (db/, llm/, mcp/, tools/)
- DB-OPT-14: Requires Svelte stores integration for reactivity, websocket connection
- DB-OPT-13: Monitor SurrealDB SDK releases for caching support
- OPT-TODO-8: Follow `tools/memory/helpers.rs` pattern (OPT-MEM-6) once validated

---

### 4. Frontend Optimizations

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| FE-OPT-12 | Superforms | Form library integration for complex forms | 16h | Large refactoring, current forms work adequately |
| FE-OPT-13 | use:enhance | SvelteKit server-side form handling | - | Depends on FE-OPT-12 |

**Prerequisites**: None

**Implementation Notes**:
- Start with Settings page forms (most complex)
- Consider alternatives: formsnap, sveltekit-superforms
- Evaluate validation library integration (Zod already in place)

---

### 5. LLM Optimizations (Deferred)

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| OPT-LLM-7 | HTTP Error Handling | Consolidated HTTP error parsing function | 2h | Nice-to-have, current per-provider handling works |

**Implementation Notes**:
- Extract common error parsing from `mistral.rs:356-380` and `ollama.rs:340-350`
- Create generic `parse_http_error()` in `llm/utils.rs`
- Consistent error messages across providers

---

### 6. Tool Description Optimizations (Deferred)

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| OPT-TD-9 | Document timeouts/limits | Add LIMITS section to all tool descriptions | 1h | Nice-to-have, main constraints already documented |
| OPT-TD-10 | Section ERRORS with codes | Document error codes in tool descriptions | 2h | Nice-to-have, error handling works |
| OPT-TD-11 | Centralize descriptions | Create tools/descriptions.rs module | - | Major refactor, marginal benefit |
| OPT-TD-12 | Auto-generate docs | Generate docs from Rust structs | - | High complexity, risk of bugs |
| OPT-TD-13 | i18n descriptions | Internationalize tool descriptions | - | LLMs perform well in English, low ROI |

**Prerequisites**: None

**Implementation Notes**:
- OPT-TD-9: Add LIMITS section with all timeouts, max values, rate limits
- OPT-TD-10: Document error codes (INVALID_INPUT, NOT_FOUND, TIMEOUT, etc.) per tool
- OPT-TD-11 to 13: Deferred indefinitely (marginal benefits)

---

### 7. UserQuestionTool Optimizations (Deferred)

| ID | Item | Description | Effort | Reason for Deferral |
|----|------|-------------|--------|---------------------|
| OPT-UQ-13 | oneshot::channel (zero polling) | Replace polling with channel-based wait | 8-10h | Major refactor, current polling works well |
| OPT-UQ-14 | Svelte 5 runes ($state) | Migrate store to $state runes | 4-6h | Store works well, defer to global refactoring |
| OPT-UQ-15 | Native async traits | Remove async-trait crate | 2h | Micro-optimization, affects all tools |
| OPT-UQ-16 | Auto-cleanup questions > 7 days | Background job to clean old questions | 3h | DB size not critical yet |

**Prerequisites**: None

**Implementation Notes**:
- OPT-UQ-13: Use `tokio::sync::oneshot` to eliminate DB polling entirely
- OPT-UQ-14: Migrate when global frontend refactoring happens
- OPT-UQ-15: Apply when Rust MSRV allows native async in traits (1.75+)
- OPT-UQ-16: Implement when question table grows significantly

---

## Priority Order

When implementing post-v1, follow this order:

| Priority | ID | Item | Effort | Value |
|----------|----|----|--------|-------|
| 1 | TYPE-OPT-5 | specta + tauri-specta | 8-10h | Highest - eliminates type sync bugs |
| 2 | SEC-OPT-7 | Rate limiting (sensitive ops) | 4h | Security - production hardening |
| 3 | DB-OPT-14 | Live Query API | 4h | UX - real-time updates |
| 4 | SEC-OPT-8 | Prompt injection guard | 6h | Security - LLM hardening |
| 5 | DB-OPT-12 | thiserror migration | 6h | Code quality |
| 6 | OPT-LLM-7 | HTTP error consolidation | 2h | Maintenance |
| 7 | FE-OPT-12/13 | Superforms | 16h | Nice-to-have |

---

## Items NOT Implemented (By Design)

The following items from original specs were intentionally not implemented:

| Item | Reason |
|------|--------|
| Notification Component | Uses inline message patterns instead - simpler, no additional dependency |
| Type-safe Invoke Helpers | Direct `invoke<T>()` with TypeScript generics is sufficient |
| CRUD Factory Pattern | Pure function pattern used instead - equally effective, more explicit |
| `interaction_tools()` method | Registry uses `basic_tools()` and `sub_agent_tools()` categories - sufficient |
| rig-core 0.26.0 upgrade | Breaking changes, requires extensive testing in staging |
| Real streaming (non-simulated) | Requires rig-core investigation, current simulation adequate |
| Context window manager | Post-v1, needed only for long conversations |
| Automatic fallback | Complex, requires circuit breaker maturity data |

---

## Monitoring Blockers

### tauri-specta Status

**Current**: v2.0-rc.21 (incompatible with Tauri 2.9.x)
**Repository**: https://github.com/oscartbeaumont/tauri-specta
**Check**: Run `cargo add tauri-specta` periodically to test compatibility

When stable release is available:
1. Add to Cargo.toml: `tauri-specta = "2"`
2. Add derive macros to models
3. Configure bindings export
4. Test TypeScript generation

---

## Document History

| Date | Change |
|------|--------|
| 2025-12-09 | Created from PHASE_POST_V1.md consolidation |
| 2025-12-09 | Updated test count to 760+ after Sub-Agent optimizations |
| 2025-12-09 | Confirmed all OPT-SA-1 to OPT-SA-11 complete |
| 2025-12-10 | Confirmed all OPT-TODO-1 to OPT-TODO-12 complete (except OPT-TODO-8 deferred) |
| 2025-12-10 | Updated test count to 786+ after TodoTool optimizations |
| 2025-12-10 | Confirmed all OPT-UQ-1 to OPT-UQ-12 complete (OPT-UQ-13 to OPT-UQ-16 deferred) |
| 2025-12-10 | Updated test count to 844+ after UserQuestionTool optimizations |
| 2025-12-10 | Added Section 6: UserQuestionTool Optimizations (Deferred) |
| 2025-12-10 | Confirmed OPT-TD-1 to OPT-TD-8 complete (OPT-TD-9 to OPT-TD-13 deferred) |
| 2025-12-10 | Added Section 6: Tool Description Optimizations (Deferred), renumbered sections |
