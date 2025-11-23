# Stratégie Tests

> Tests unitaires, intégration, E2E, CI/CD

## Objectifs

**Coverage Target** : ~70% critical paths backend
**Focus** : Chemins critiques > coverage exhaustif
**Philosophy** : Tests E2E prioritaires > Unit tests exhaustifs

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

### Unit Tests

**Localisation** : `src-tauri/src/**/*.rs`

**Pattern** :
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_execution() {
        // Test logic
    }
}
```

**Exécution** :
```bash
cargo test
cargo test --lib          # Tests library
cargo test -- --nocapture # Avec logs
```

**Couverture** :
- Agent creation/execution
- Tool invocation logic
- Memory operations (sans vector search - trop coûteux)
- Error handling paths

---

### Integration Tests

**Localisation** : `src-tauri/tests/`

**Pattern** :
```rust
// tests/workflow_integration.rs
#[tokio::test]
async fn test_workflow_end_to_end() {
    // Setup DB
    // Create workflow
    // Execute agent
    // Assert results
    // Cleanup
}
```

**Exécution** :
```bash
cargo test --test workflow_integration
```

**Couverture** :
- Workflow creation → execution → completion
- Agent ↔ Database interactions
- MCP Client ↔ Server communication (mock servers)
- Multi-agent coordination

---

### Mocking

**Database** : SurrealDB in-memory
```rust
let db = Surreal::new::<Mem>(()).await?;
```

**MCP Servers** : Mock responses
```rust
struct MockMCPServer;
impl MCPServer for MockMCPServer {
    async fn call_tool(&self, name: &str) -> Result<String> {
        Ok("mocked_response".into())
    }
}
```

**LLM Providers** : Mock Rig.rs responses
```rust
struct MockProvider;
impl Provider for MockProvider {
    async fn generate(&self, prompt: &str) -> Result<String> {
        Ok("mocked_llm_response".into())
    }
}
```

---

## Tests Frontend (SvelteKit)

### Unit Tests (Vitest)

**Localisation** : `src/lib/**/*.test.ts`

**Config** : `vitest.config.ts`
```typescript
export default defineConfig({
  test: {
    environment: 'jsdom',
    globals: true,
  }
});
```

**Exécution** :
```bash
npm run test
npm run test:ui  # Interface visuelle
```

**Couverture** :
- Composants UI isolés (render, props)
- Stores Svelte (state management)
- Utils functions (token counting, formatting)

**Pattern** :
```typescript
import { render } from '@testing-library/svelte';
import { describe, it, expect } from 'vitest';
import WorkflowCard from '$lib/components/workflow/WorkflowCard.svelte';

describe('WorkflowCard', () => {
  it('renders workflow name', () => {
    const { getByText } = render(WorkflowCard, {
      workflow: { id: '1', name: 'Test', status: 'idle' }
    });
    expect(getByText('Test')).toBeInTheDocument();
  });
});
```

---

### E2E Tests (Playwright via MCP)

**Localisation** : `tests/e2e/`

**Config** : `playwright.config.ts`
```typescript
export default defineConfig({
  testDir: './tests/e2e',
  use: {
    baseURL: 'http://localhost:5173',
  },
});
```

**Exécution** :
```bash
npm run test:e2e
npm run test:e2e:ui  # Mode interactif
```

**Couverture** :
- Workflows complets (create → execute → complete)
- Validation human-in-the-loop
- Multi-workflow simultané
- Settings configuration (providers, agents, MCP)

**Pattern** :
```typescript
import { test, expect } from '@playwright/test';

test('create and execute workflow', async ({ page }) => {
  await page.goto('/agent');

  // Create workflow
  await page.click('button:has-text("+ New")');
  await page.fill('input[name="workflow-name"]', 'E2E Test');
  await page.click('button:has-text("Create")');

  // Send message
  await page.fill('textarea', 'Query users');
  await page.click('button:has-text("Send")');

  // Verify execution
  await expect(page.locator('.status-running')).toBeVisible();

  // Wait completion
  await expect(page.locator('.status-completed')).toBeVisible({ timeout: 30000 });
});
```

---

## Tests MCP

### MCP Client Tests

**Localisation** : `src-tauri/src/mcp/client/tests.rs`

**Couverture** :
- Connection à MCP server (mock)
- Tool discovery (list_tools)
- Tool execution avec params
- Error handling (server down, timeout)

---

### MCP Server Tests

**Localisation** : `src-tauri/src/mcp/server/tests.rs`

**Couverture** :
- Exposition tools custom (SurrealDBTool, etc.)
- Exposition resources (project config, memory)
- Request handling JSON-RPC
- Validation inputs

---

## Tests Database

### Schema Validation

**Tests** : Structure tables, indexes, relations

```rust
#[tokio::test]
async fn test_workflow_schema() {
    let db = setup_test_db().await;

    // Insert workflow
    let workflow: Workflow = db
        .create("workflow")
        .content(workflow_fixture())
        .await?;

    // Assert fields
    assert_eq!(workflow.status, WorkflowStatus::Idle);
    assert!(workflow.created_at.is_some());
}
```

---

### Vector Search

**Tests** : Embeddings generation, KNN search, similarité

```rust
#[tokio::test]
async fn test_memory_vector_search() {
    let db = setup_test_db().await;

    // Insert memories with embeddings
    let memories = vec![
        memory_with_embedding("user preferences", vec![0.1, 0.2, ...]),
        memory_with_embedding("project context", vec![0.8, 0.9, ...]),
    ];

    // Search similar
    let query_embedding = vec![0.15, 0.25, ...];
    let results = db.vector_search("memory", query_embedding, 5).await?;

    // Assert relevance
    assert!(results[0].score > 0.9);
}
```

**Note** : Tests coûteux, exécuter manuellement ou CI nightly

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

### GitLab CI

**Workflow** : `.gitlab-ci.yml`

#### Stages
1. **lint** : Linting Rust + TypeScript
2. **test** : Unit tests backend + frontend
3. **integration** : Tests intégration
4. **e2e** : Tests E2E (Playwright)
5. **security** : Audit dependencies

#### Artifacts
- Coverage reports (XML)
- Test logs
- Screenshots E2E (si failures)

---

## Coverage Reporting

### Backend (Rust)

**Tool** : `cargo-tarpaulin`

**Install** :
```bash
cargo install cargo-tarpaulin
```

**Run** :
```bash
cargo tarpaulin --out Html
```

**Output** : `tarpaulin-report.html`

---

### Frontend (TypeScript)

**Tool** : Vitest coverage (v8)

**Config** : `vitest.config.ts`
```typescript
export default defineConfig({
  test: {
    coverage: {
      provider: 'v8',
      reporter: ['text', 'html', 'lcov'],
    }
  }
});
```

**Run** :
```bash
npm run test:coverage
```

**Output** : `coverage/index.html`

---

## Testing Best Practices

### Backend
- **Isolation** : Tests indépendants, pas side effects
- **Async** : Utiliser `tokio::test` pour async functions
- **Cleanup** : Nettoyer DB après chaque test
- **Mocking** : Mock external dependencies (LLM, MCP servers externes)

### Frontend
- **Render** : Testing Library pour components Svelte
- **User Events** : Simuler interactions utilisateur
- **Accessibility** : Tester ARIA labels, keyboard navigation
- **Responsiveness** : Tester breakpoints mobile/desktop

### E2E
- **Stability** : Attendre éléments (waitFor), pas timeouts arbitraires
- **Cleanup** : Reset DB entre tests
- **Parallelization** : Tests isolés, pas dépendances ordre
- **Screenshots** : Capturer failures pour debugging

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

## Performance Tests

### Load Testing (Optionnel)

**Tool** : `criterion` (Rust benchmarks)

**Use Cases** :
- Agent orchestration avec 10+ agents
- Vector search avec 10K memories
- Workflow avec 100+ messages

**Config** : `benches/performance.rs`
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_vector_search(c: &mut Criterion) {
    c.bench_function("vector_search_10k", |b| {
        b.iter(|| {
            // Benchmark logic
        });
    });
}

criterion_group!(benches, benchmark_vector_search);
criterion_main!(benches);
```

**Run** :
```bash
cargo bench
```

---

## Références

**Vitest** : https://vitest.dev
**Playwright** : https://playwright.dev
**Cargo Test** : https://doc.rust-lang.org/book/ch11-00-testing.html
**Tarpaulin** : https://github.com/xd009642/tarpaulin
**Testing Library** : https://testing-library.com/docs/svelte-testing-library/intro
