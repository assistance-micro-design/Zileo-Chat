# Stratégie Tests

> Tests unitaires, intégration, E2E, CI/CD

## Objectifs

**Coverage Target** : ~70% critical paths backend
**Focus** : Chemins critiques > coverage exhaustif
**Philosophy** : Tests E2E prioritaires > Unit tests exhaustifs

---

## Test Counts Summary

| Category | Files | Tests |
|----------|-------|-------|
| **Backend Unit** | 75 | 786 |
| **Backend Integration** | 2 | 46 |
| **Frontend Unit** | 7 | 165 |
| **Frontend E2E** | 10 | 112 |
| **Total** | **94** | **1109** |

> **Last Updated**: 2025-12-10 (post OPT-TODO optimizations)

---

## Test Dependencies

### Frontend (package.json devDependencies)

| Package | Version | Purpose |
|---------|---------|---------|
| vitest | 2.0.0 | Unit testing framework |
| @playwright/test | 1.47.0 | E2E testing framework |
| jsdom | 27.2.0 | DOM environment for testing |

### Backend (Cargo.toml dev-dependencies)

| Crate | Version | Purpose |
|-------|---------|---------|
| tempfile | 3.14 | Temporary file/directory for test DBs |

---

## Critical Paths

### 1. Workflow Execution
- User input → Agent processing → LLM call → Response streaming
- Validation human-in-the-loop (approve/reject)
- Error handling et recovery

### 2. Agent Orchestration
- Création workflow multi-agents
- Communication inter-agents (reports markdown)
- State persistence et reload

### 3. Tools Execution
- MCP tool calls (success + error cases)
- Database operations (CRUD)
- Memory storage/retrieval vectorielle

---

## Tests Backend (Rust)

### Unit Tests (635 tests, 73 files)

**Localisation** : `src-tauri/src/**/*.rs`

**Pattern** :
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_add() {
        let context = create_test_context();
        let result = memory_tool.add(&context, content).await;
        assert!(result.is_ok());
    }
}
```

**Exécution** :
```bash
cargo test               # All tests
cargo test --lib         # Library tests only
cargo test -- --nocapture # With logs
```

**Module Coverage** :

| Module | Files | Tests | Key Areas |
|--------|-------|-------|-----------|
| tools | 14 | 210 | CalculatorTool (63), MemoryTool (55+), TodoTool (26) |
| models | 18 | 144 | Data structures, serialization |
| llm | 9 | 95 | Provider adapters, embedding, pricing |
| commands | 16 | 75 | Tauri command validation |
| mcp | 6 | 44 | Protocol, error handling, server management |
| agents | 4 | 34 | LLM agent, orchestrator, registry |
| security | 2 | 30 | Input validation (24), keystore (6) |
| state | 1 | 10 | Application state management |
| db | 1 | 5 | Database client |

---

### Integration Tests (46 tests, 2 files)

**Localisation** : `src-tauri/tests/`

| File | Tests | Description |
|------|-------|-------------|
| `memory_tool_integration.rs` | 20 | MemoryTool with ToolFactory integration |
| `sub_agent_tools_integration.rs` | 26 | Sub-agent tools with context and validation |

**Pattern** :
```rust
// tests/memory_tool_integration.rs
fn create_test_db_path() -> (tempfile::TempDir, String) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("integration_test_db");
    (temp_dir, db_path.to_str().unwrap().to_string())
}

#[tokio::test]
async fn test_memory_workflow_integration() {
    let (_temp, db_path) = create_test_db_path();
    // Setup isolated test DB
    // Execute tool operations
    // Assert results
    // Temp dir auto-cleanup
}
```

**Exécution** :
```bash
cargo test --test memory_tool_integration
cargo test --test sub_agent_tools_integration
cargo test --test '*'  # All integration tests
```

---

### Test Helpers

**Tempfile Pattern** (for isolated test databases):
```rust
use tempfile::tempdir;

fn create_test_db_path() -> (tempfile::TempDir, String) {
    let temp_dir = tempdir().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_db");
    (temp_dir, db_path.to_str().unwrap().to_string())
}
```

**Create Test Helpers** (common pattern across modules):
```rust
// tools/factory.rs
fn create_test_factory() -> ToolFactory { ... }

// tools/memory/tool.rs
fn create_test_tool() -> MemoryTool { ... }

// tools/context.rs
fn create_test_state() -> ToolExecutionState { ... }

// agents/core/orchestrator.rs
fn create_test_config() -> AgentConfig { ... }
```

**JSON Construction** (for tool execution tests):
```rust
use serde_json::json;

let input = json!({
    "operation": "add",
    "memory_type": "knowledge",
    "content": "test content"
});
```

**Note**: No mocking library (mockall) used. Tests use real implementations with temporary databases.

---

## Tests Frontend (SvelteKit)

### Unit Tests (165 tests, 7 files)

**Localisation** : `src/lib/**/__tests__/*.test.ts`, `src/types/__tests__/*.test.ts`

**Test Files** :

| File | Tests | Target |
|------|-------|--------|
| `lib/utils/__tests__/debounce.test.ts` | 10 | Debounce/throttle utilities |
| `lib/stores/__tests__/agents.test.ts` | 26 | Agent CRUD operations |
| `lib/stores/__tests__/streaming.test.ts` | 17 | Real-time workflow execution |
| `lib/stores/__tests__/llm.test.ts` | 57 | LLM models and providers |
| `lib/stores/__tests__/workflows.test.ts` | 24 | Workflow state management |
| `types/__tests__/embedding.test.ts` | 15 | Embedding config validation |
| `types/__tests__/memory.test.ts` | 16 | Memory structure validation |

**Config** : `vitest.config.ts`
```typescript
export default defineConfig({
  plugins: [svelte({ hot: !process.env.VITEST })],
  test: {
    environment: 'jsdom',
    globals: true,
    alias: {
      $lib: '/src/lib',
      $app: '/src/app',
    }
  }
});
```

**Exécution** :
```bash
npm run test          # Run unit tests
npm run test:watch    # Watch mode
npm run test:coverage # Generate coverage report
```

**Pattern** (Store-based testing, no @testing-library/svelte):
```typescript
import { describe, it, expect, beforeEach, vi } from 'vitest';
import { get } from 'svelte/store';
import { llmStore, models, isLoading } from '$lib/stores/llm';

// Mock Tauri
vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn()
}));

describe('llmStore', () => {
  beforeEach(() => {
    llmStore.reset();
  });

  it('loads models from backend', async () => {
    vi.mocked(invoke).mockResolvedValue([{ id: 'mistral', name: 'Mistral' }]);
    await llmStore.loadModels();
    expect(get(models)).toHaveLength(1);
    expect(get(isLoading)).toBe(false);
  });
});
```

---

### E2E Tests (112 tests, 10 files)

**Localisation** : `tests/` and `tests/e2e/`

**Test Files** :

| File | Tests | Description |
|------|-------|-------------|
| `navigation.spec.ts` | 4 | Basic page routing |
| `agent-page.spec.ts` | 4 | Agent page UI structure |
| `settings-page.spec.ts` | 7 | Settings LLM provider config |
| `e2e/workflow-crud.spec.ts` | 10 | Workflow CRUD operations |
| `e2e/chat-interaction.spec.ts` | 10 | Chat UI and messages |
| `e2e/settings-config.spec.ts` | 12 | Provider and theme settings |
| `e2e/theme-toggle.spec.ts` | 15 | Light/dark mode switching |
| `e2e/accessibility.spec.ts` | 17 | WCAG 2.1 AA compliance |
| `e2e/workflow-persistence.spec.ts` | 13 | State persistence across reloads |
| `e2e/sub-agent-scenarios.spec.ts` | 20 | Sub-agent UI and validation |

**Config** : `playwright.config.ts`
```typescript
export default defineConfig({
  testDir: './tests',
  fullyParallel: true,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',
  use: {
    baseURL: 'http://localhost:5173',
    trace: 'on-first-retry',
  },
  webServer: {
    command: 'npm run dev',
    url: 'http://localhost:5173',
    timeout: 120000,
  },
  projects: [
    { name: 'chromium', use: { ...devices['Desktop Chrome'] } },
  ],
});
```

**Exécution** :
```bash
npm run test:e2e       # Run all E2E tests
npm run test:e2e:ui    # Playwright UI mode
npm run test:e2e:debug # Debug with headed browser
```

**Pattern** :
```typescript
import { test, expect } from '@playwright/test';

test('theme toggle persists across reload', async ({ page }) => {
  await page.goto('/settings');
  await page.waitForLoadState('networkidle');

  // Toggle to dark mode
  await page.click('[data-testid="theme-toggle"]');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');

  // Verify persistence
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
});
```

---

## Tests MCP (44 tests, 6 files)

**Localisation** : `src-tauri/src/mcp/`

| File | Tests | Coverage |
|------|-------|----------|
| `protocol.rs` | 16 | JSON-RPC protocol parsing |
| `error.rs` | 9 | Error types and handling |
| `server_handle.rs` | 7 | Server lifecycle management |
| `http_handle.rs` | 5 | HTTP transport layer |
| `client.rs` | 4 | MCP client operations |
| `manager.rs` | 3 | Multi-server coordination |

**Couverture** :
- Protocol message parsing (request/response/notification)
- Error code handling and error chain
- Server connection and disconnection
- Tool discovery (list_tools)
- Tool execution with parameters
- HTTP and stdio transport

---

## Tests Database

### Database Client (5 tests)

**Localisation** : `src-tauri/src/db/client.rs`

**Couverture** :
- Connection initialization
- Query execution
- Transaction handling
- Error propagation

### Embedding Tests (25 tests)

**Localisation** : `src-tauri/src/llm/embedding.rs`

**Couverture** :
- Embedding model configuration
- Vector generation
- Dimension validation
- Provider-specific adapters

```rust
#[tokio::test]
async fn test_embedding_dimensions() {
    let config = EmbeddingConfig {
        provider: "ollama".to_string(),
        model: "nomic-embed-text".to_string(),
        dimensions: 768,
    };
    assert_eq!(config.dimensions, 768);
}
```

### Model Data Tests (144 tests, 18 files)

**Localisation** : `src-tauri/src/models/*.rs`

| File | Tests | Coverage |
|------|-------|----------|
| `streaming.rs` | 14 | StreamChunk, WorkflowComplete |
| `task.rs` | 13 | Task CRUD, status transitions |
| `prompt.rs` | 12 | Prompt templates, variables |
| `mcp.rs` | 11 | MCP server config, tool schema |
| `llm_models.rs` | 9 | LLMModel, provider settings |
| `memory.rs` | 9 | Memory types, search params |
| `agent.rs` | 9 | AgentConfig, lifecycle |
| `sub_agent.rs` | 9 | Sub-agent execution tracking |
| (others) | 58 | Serialization, validation |

**Pattern** :
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_status_serialization() {
        let status = WorkflowStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");
    }
}
```

---

## CI/CD Pipeline

### GitHub Actions

**Workflow** : `.github/workflows/test.yml`

#### On Push (branches feature)
```yaml
name: Tests
on: [push]

jobs:
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm run lint
      - run: cargo clippy

  test-backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test

  test-frontend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm install
      - run: npm run test
```

#### On PR (vers main)
```yaml
  test-integration:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --test '*'

  test-e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: npm install
      - run: npx playwright install
      - run: npm run tauri build
      - run: npm run test:e2e

  security-audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo audit
      - run: npm audit

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo tarpaulin --out Xml
      - uses: codecov/codecov-action@v3
```

---

## Testing Best Practices

### Backend
- **Isolation** : Tests indépendants via `tempfile::tempdir()` pour DB isolées
- **Async** : Utiliser `#[tokio::test]` pour async functions
- **Cleanup** : Auto-cleanup via TempDir Drop trait
- **No Mocks** : Real implementations with isolated test databases
- **Helpers** : `create_test_*()` functions for common setup

### Frontend
- **Store Testing** : Test Svelte stores with `get()` from `svelte/store`
- **Tauri Mocking** : Mock `@tauri-apps/api/core` with `vi.fn()`
- **State Reset** : `beforeEach()` hooks reset store state
- **Type Validation** : Tests verify TypeScript type compatibility

### E2E
- **Network Idle** : Use `waitForLoadState('networkidle')` for stability
- **Parallel** : `fullyParallel: true` for fast execution
- **Retries** : 2 retries in CI, 0 locally
- **Accessibility** : 17 WCAG 2.1 AA compliance tests
- **Trace** : Capture traces on first retry for debugging

---

## Edge Cases à Tester

### Workflow
- Workflow running → User ferme app → Reload state
- Multiple workflows simultanés (>5)
- Workflow très long (>10min)
- Workflow avec erreur réseau LLM

### Validation
- User ignore validation → Timeout auto-reject
- Multiple validations pending simultanées
- Validation rejected → Workflow continue (skip operation)

### Memory
- Vector search sans résultats (threshold non atteint)
- Memory overflow (>10K entries)
- Embeddings provider change (dimensions différentes)

### MCP
- MCP server crash pendant workflow
- MCP server slow (>5s response)
- MCP server retourne erreur (invalid params)

---

## References

**Vitest** : https://vitest.dev
**Playwright** : https://playwright.dev
**Cargo Test** : https://doc.rust-lang.org/book/ch11-00-testing.html
**Tarpaulin** : https://github.com/xd009642/tarpaulin
**Tempfile** : https://docs.rs/tempfile/latest/tempfile/
**Tokio Test** : https://docs.rs/tokio/latest/tokio/attr.test.html
