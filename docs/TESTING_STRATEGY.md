# Testing Strategy

> Unit tests, integration, E2E, CI/CD

## Objectives

| Item | Value |
|------|-------|
| **Coverage Target** | ~70% critical paths backend |
| **Focus** | Critical paths over exhaustive coverage |
| **Philosophy** | TDD (Red-Green-Refactor) for testable logic |

For bugs, a reproduction test is written BEFORE the fix.

---

## Test Counts Summary

| Category | Files | Tests |
|----------|-------|-------|
| Backend (lib) | 80+ | 1086 |
| Backend (bin/integration) | 5+ | 48 |
| Frontend Unit | 16 | 285 |
| **Total** | **101+** | **~1419** |

> Last Updated: 2026-03-27

---

## Test Dependencies

### Frontend (devDependencies)

| Package | Version | Purpose |
|---------|---------|---------|
| vitest | ^4.0.15 | Unit testing framework |
| @playwright/test | ^1.58.0 | E2E testing framework |
| jsdom | ^27.4.0 | DOM environment for testing |

### Backend (dev-dependencies)

| Crate | Version | Purpose |
|-------|---------|---------|
| tempfile | 3.24 | Temporary file/directory for test DBs |

---

## Critical Paths

1. **Workflow Execution**: User input -> Agent processing -> LLM call -> Response streaming, human-in-the-loop validation, error handling and recovery
2. **Agent Orchestration**: Multi-agent workflows, inter-agent communication (markdown reports), state persistence and reload
3. **Tools Execution**: MCP tool calls (success + error), database CRUD, vector memory storage/retrieval

---

## Test Categories

| Type | Scope | Tools | Location |
|------|-------|-------|----------|
| Backend Unit | Individual modules | `cargo test --lib` | `src-tauri/src/**/*.rs` |
| Backend Integration | Cross-module workflows | `cargo test --test '*'` | `src-tauri/tests/` |
| Frontend Unit | Stores, utils, types | `npm run test` | `src/lib/**/__tests__/`, `src/types/__tests__/` |
| Frontend E2E | Full user scenarios | `npm run test:e2e` | `tests/`, `tests/e2e/` |

---

## Backend Tests (Rust)

### Shared Test Harness

Module `src-tauri/src/test_utils.rs` provides:
- `setup_test_state()`: Complete AppState with in-memory DB
- 10+ seed helpers: `seed_agent()`, `seed_workflow()`, `seed_message()`, etc.

No mocking library used. Tests use real implementations with temporary databases via `tempfile::tempdir()`.

### Unit Tests (80+ files)

See `src-tauri/src/` for Rust test modules.

| Module | Files (with tests) | Tests | Key Areas |
|--------|-------------------|-------|-----------|
| tools | 26 | ~274 | CalculatorTool, MemoryTool, TodoTool, FileManager, UserQuestion, ReadSkill, SubAgent |
| models | 21 | ~199 | Data structures, serialization |
| llm | 18 | ~140 | Provider adapters, embedding, pricing, tool_adapter |
| mcp | 8 | ~78 | Protocol, error handling, server management |
| commands | 16 | ~65 | Tauri command validation |
| security | 2 | ~35 | Input validation, keystore |
| agents | 5 | ~13 | LLM agent, orchestrator, registry |

### Integration Tests

| File | Tests | Description |
|------|-------|-------------|
| `memory_tool_integration.rs` | 20 | MemoryTool with ToolFactory integration |
| `sub_agent_tools_integration.rs` | 26 | Sub-agent tools with context and validation |

See `src-tauri/tests/` for integration test files.

---

## Frontend Tests (SvelteKit)

### Unit Tests (285 tests, 16 files)

See `src/lib/` for TypeScript test files.

| File | Tests | Target |
|------|-------|--------|
| `stores/__tests__/llm.test.ts` | 57 | LLM models and providers |
| `stores/__tests__/workflows.test.ts` | 36 | Workflow state management |
| `stores/__tests__/execution-blocks.test.ts` | 25 | Block-by-block display |
| `stores/__tests__/agents.test.ts` | 22 | Agent CRUD operations |
| `stores/__tests__/streaming.test.ts` | 18 | Real-time workflow execution |
| `types/__tests__/memory.test.ts` | 16 | Memory structure validation |
| `types/__tests__/embedding.test.ts` | 15 | Embedding config validation |
| `utils/__tests__/debounce.test.ts` | 10 | Debounce/throttle utilities |
| (others) | - | Error handling, URL validation, date grouping, chunk processing |

**Pattern**: Store-based testing (no @testing-library/svelte). Tauri IPC mocked via `vi.mock('@tauri-apps/api/core')`.

### E2E Tests (112 tests, 10 files)

| File | Tests | Description |
|------|-------|-------------|
| `e2e/sub-agent-scenarios.spec.ts` | 20 | Sub-agent UI and validation |
| `e2e/accessibility.spec.ts` | 17 | WCAG 2.1 AA compliance |
| `e2e/theme-toggle.spec.ts` | 15 | Light/dark mode switching |
| `e2e/workflow-persistence.spec.ts` | 13 | State persistence across reloads |
| `e2e/settings-config.spec.ts` | 12 | Provider and theme settings |
| `e2e/chat-interaction.spec.ts` | 10 | Chat UI and messages |
| `e2e/workflow-crud.spec.ts` | 10 | Workflow CRUD operations |
| `settings-page.spec.ts` | 7 | Settings LLM provider config |
| `navigation.spec.ts` | 4 | Basic page routing |
| `agent-page.spec.ts` | 4 | Agent page UI structure |

---

## MCP Tests (44 tests, 6 files)

| File | Tests | Coverage |
|------|-------|----------|
| `protocol.rs` | 16 | JSON-RPC protocol parsing |
| `error.rs` | 9 | Error types and handling |
| `server_handle.rs` | 7 | Server lifecycle management |
| `http_handle.rs` | 5 | HTTP transport layer |
| `client.rs` | 4 | MCP client operations |
| `manager.rs` | 3 | Multi-server coordination |

---

## CI/CD Pipeline

### GitHub Actions (validate.yml)

Triggered on PR to main. Two parallel jobs:

1. **Frontend**: `npm install` -> `npm run lint` -> `npm run check` -> `npm run test`
2. **Backend**: `cargo clippy --all-targets -- -D warnings` -> `cargo test`

See `.github/workflows/validate.yml` for full configuration.

---

## Best Practices

### Backend

- **Isolation**: Independent tests via `tempfile::tempdir()` for isolated DB
- **Async**: `#[tokio::test]` for async functions
- **Cleanup**: Auto-cleanup via TempDir Drop trait
- **No Mocks**: Real implementations with isolated test databases
- **Helpers**: `create_test_*()` functions for common setup

### Frontend

- **Store Testing**: Test Svelte stores with `get()` from `svelte/store`
- **Tauri Mocking**: Mock `@tauri-apps/api/core` with `vi.fn()`
- **State Reset**: `beforeEach()` hooks reset store state
- **Type Validation**: Tests verify TypeScript type compatibility

### E2E

- **Network Idle**: Use `waitForLoadState('networkidle')` for stability
- **Parallel**: `fullyParallel: true` for fast execution
- **Retries**: 2 retries in CI, 0 locally
- **Accessibility**: 17 WCAG 2.1 AA compliance tests
- **Trace**: Capture traces on first retry for debugging

---

## Edge Cases to Test

| Area | Cases |
|------|-------|
| **Workflow** | Running -> app close -> reload state; Multiple simultaneous (>5); Very long (>10min); LLM network error |
| **Validation** | Ignore -> timeout auto-reject; Multiple pending simultaneous; Rejected -> workflow continues (skip) |
| **Memory** | Vector search with no results; Overflow (>10K entries); Embeddings provider change (dimensions) |
| **MCP** | Server crash during workflow; Slow server (>5s); Server returns error (invalid params) |

---

## References

- **Vitest**: https://vitest.dev
- **Playwright**: https://playwright.dev
- **Cargo Test**: https://doc.rust-lang.org/book/ch11-00-testing.html
- **Tempfile**: https://docs.rs/tempfile/latest/tempfile/
