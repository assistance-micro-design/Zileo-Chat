# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Zileo-Chat-3** is a desktop multi-agent application with a conversational interface, built using a modern stack:
- **Frontend**: SvelteKit 2.49.0 + Svelte 5.43.14 + Vite 5.4.0
- **Backend**: Rust 1.91.1 + Tauri 2.9.4
- **Database**: SurrealDB 2.3.10 (embedded RocksDB for desktop)
- **LLM Framework**: Rig.rs 0.24.0 (multi-provider abstraction)
- **Protocol**: MCP 2025-06-18 (official Anthropic SDK)
- **Additional**: async-trait 0.1, futures 0.3 (multi-agent async patterns)

**Current Status**: Base implementation in progress - Phase 0 structure created, core implementation ongoing.

## Essential Commands

### Development

```bash
# Start dev mode (frontend + backend with HMR)
npm run tauri dev

# Frontend only (UI development)
npm run dev

# Backend only (Rust command testing)
cd src-tauri && cargo run
```

### Quality & Validation

```bash
# Frontend validation
npm run lint              # ESLint
npm run check             # svelte-check + TypeScript strict mode
npm run test              # Vitest unit tests
npm run build             # Production build test

# Backend validation
cd src-tauri
cargo fmt --check         # Format verification
cargo clippy -- -D warnings  # Linting with warnings as errors
cargo test                # Unit tests
cargo test --lib          # Library tests only
cargo build --release     # Release build
```

### Testing

```bash
# Frontend tests
npm run test              # Vitest
npm run test:e2e          # Playwright E2E tests

# Backend tests
cd src-tauri
cargo test                # All tests
cargo test -- --nocapture # With debug output
```

## Architecture Overview

### Multi-Agent System Hierarchy

```
Agent Principal (Orchestrator)
├─ Agents Spécialisés (permanent) - DB, API, Analytics, UI
└─ Agents Temporaires (lifecycle limité)
```

**Communication Protocol**: Markdown reports (standardized, human-readable, machine-parsable)

### Project Structure

```
zileo-chat-3/
├─ src/                     # Frontend (SvelteKit)
│  ├─ routes/               # File-based routing
│  │  ├─ settings/          # Settings page
│  │  └─ agent/             # Agent interaction page
│  ├─ lib/
│  │  ├─ components/        # Reusable Svelte components
│  │  └─ stores/            # Svelte state management (uses $types)
│  └─ types/                # TypeScript interfaces (alias: $types)
│
├─ src-tauri/               # Backend (Rust)
│  ├─ src/
│  │  ├─ main.rs            # Entry point
│  │  ├─ commands/          # Tauri IPC commands
│  │  ├─ agents/            # Multi-agent system
│  │  │  ├─ core/           # Orchestrator, registry
│  │  │  ├─ specialized/    # Permanent agents
│  │  │  ├─ config/         # TOML agent configurations
│  │  │  └─ prompts/        # System prompts + templates
│  │  ├─ llm/               # Rig.rs integration, providers
│  │  ├─ mcp/               # MCP client/server
│  │  ├─ tools/             # Custom MCP tools
│  │  ├─ db/                # SurrealDB client
│  │  └─ models/            # Rust structs (sync with TS types)
│  ├─ Cargo.toml
│  └─ tauri.conf.json       # Tauri configuration
│
└─ docs/                    # Comprehensive documentation
```

### IPC Communication Pattern

Frontend → Backend communication uses Tauri's `invoke()`:

```typescript
// Frontend (TypeScript)
const result = await invoke<WorkflowResult>('execute_workflow', {
  workflowId: string,
  message: string,
  agentId: string
});
```

```rust
// Backend (Rust)
#[tauri::command]
async fn execute_workflow(
    workflow_id: String,
    message: String,
    agent_id: String
) -> Result<WorkflowResult, String> {
    // Implementation
}
```

**Critical**: All Tauri commands must be registered in `src-tauri/src/main.rs` using `tauri::generate_handler![]`.

## Type Synchronization

TypeScript and Rust types **must be kept in sync** for IPC communication.

### TypeScript Import Pattern

**IMPORTANT**: Always use the `$types` alias for imports. Never use `$lib/types`.

```typescript
// CORRECT - use $types alias
import type { Workflow, WorkflowResult } from '$types/workflow';
import type { AgentConfig, Lifecycle } from '$types/agent';
import type { LLMConfigResponse, ProviderStatus } from '$types/llm';

// INCORRECT - do not use
import type { Workflow } from '$lib/types/workflow';  // NO!
import type { Workflow } from '../types/workflow';     // NO!
```

The `$types` alias is configured in `svelte.config.js` and points to `src/types/`.

### Type Definition Examples

**TypeScript** (`src/types/`):
```typescript
export interface FeatureData {
  id: string;
  name: string;
  metadata: Record<string, unknown>;
}
```

**Rust** (`src-tauri/src/models/`):
```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeatureData {
    pub id: String,
    pub name: String,
    pub metadata: serde_json::Value,
}
```

## Multi-Agent Configuration

Agents are configured via TOML files in `src-tauri/agents/config/`:

```toml
[agent]
id = "db_agent"
name = "Database Agent"
lifecycle = "Permanent"  # or "Temporary"

[llm]
provider = "Mistral"  # Phase 1: Mistral|Ollama
model = "mistral-large"
temperature = 0.7
max_tokens = 4096

[tools]
enabled = ["SurrealDBTool", "QueryBuilderTool", "AnalyticsTool"]

[mcp_servers]
required = ["serena"]  # Optional MCP servers to use
```

## Database Schema (SurrealDB)

Key entities with graph relations:

- **workflow**: Manages agent workflows (relations: agent, messages, tasks, validations)
- **agent_state**: Persistent agent state and configuration
- **memory**: Vector embeddings for agent memory (user_pref, context, knowledge types)
- **message**: Conversation messages with role (user/assistant/system)
- **validation_request**: Human-in-the-loop validation tracking
- **task**: Todo items with status tracking

## Security Considerations

**Production-ready from v1**:
- API keys stored via Tauri secure storage (OS keychain) + AES-256 encryption
- Input validation on both frontend and backend
- Tauri allowlist for IPC commands (explicit permission model)
- MCP servers run in isolated processes (Docker containers for external servers)
- CSP configured in `tauri.conf.json`

## Code Quality Standards

### Strict Prohibitions

**Never include in code**:
- ❌ `any` type in TypeScript - strict typing required
- ❌ Mock data or placeholders in production code
- ❌ `TODO` comments for core functionality
- ❌ Incomplete implementations (`throw new Error("Not implemented")`)
- ❌ Emojis in code or comments

### Required Practices

- ✅ **Type Safety**: Strict TypeScript + Rust types synchronized
- ✅ **Documentation**: JSDoc/TSDoc for TypeScript, Rustdoc for Rust
- ✅ **Error Handling**: `Result<T, E>` in Rust, proper try/catch in TypeScript
- ✅ **Testing**: Critical paths coverage (~70% backend)
- ✅ **Async Patterns**: Tokio for Rust, async/await for TypeScript

## Custom Slash Commands

This project includes specialized workflows:

- `/Plan_Zileo <description>`: Create detailed technical planning spec without implementation
- `/Build_zileo <description>`: Full implementation workflow with quality validation

Both commands follow a rigorous process including parallel discovery, architectural analysis, and comprehensive validation.

## LLM Provider Configuration

**Phase 1 Providers**:
- **Mistral**: Cloud API (mistral-large, mistral-medium)
- **Ollama**: Local models (llama3, mistral, codellama)

**Future**: Claude, GPT-4, Gemini

Provider selection is user-controlled via Settings UI. The application uses Rig.rs for multi-provider abstraction.

## MCP Server Configuration

MCP servers are user-configured (not bundled) via Settings UI:

```json
{
  "mcpServers": {
    "server_name": {
      "command": "docker|npx|uvx",
      "args": ["array", "of", "arguments"],
      "env": { "VAR": "value" }
    }
  }
}
```

Supported deployment methods:
- **Docker**: Local containers (e.g., Serena for code analysis)
- **NPX**: Node.js-based servers (e.g., Context7 for docs)
- **UVX**: Python servers with isolated environments
- **SaaS**: Remote managed services

## Workflow Execution Pattern

1. User sends message in Agent page
2. Frontend invokes Tauri command with workflow_id and agent_id
3. Backend orchestrator:
   - Loads agent configuration
   - Initializes LLM provider (Rig.rs)
   - Executes agent with available tools and MCP servers
   - Streams response back to frontend
4. Human-in-the-loop validation if required (critical operations)
5. Agent generates markdown report with metrics
6. State persisted to SurrealDB

## Key Documentation Files

Essential reading for context:

- `docs/TECH_STACK.md`: Exact versions and requirements (Node 20.19+, Rust 1.91.1+)
- `docs/ARCHITECTURE_DECISIONS.md`: All architectural decisions with justifications
- `docs/MULTI_AGENT_ARCHITECTURE.md`: Agent hierarchy, communication, factory patterns
- `docs/API_REFERENCE.md`: Tauri command signatures and types
- `docs/GETTING_STARTED.md`: Development setup and first workflow
- `docs/TESTING_STRATEGY.md`: Testing approach and coverage targets
- `docs/MCP_CONFIGURATION_GUIDE.md`: MCP server setup and configuration

## Development Workflow Best Practices

### For New Features

1. **Discovery**: Read relevant docs, explore existing patterns
2. **Types First**: Define TypeScript and Rust types (synchronized)
3. **Backend**: Implement Rust command with proper error handling
4. **Frontend**: Create Svelte component with strict props
5. **Validation**: Lint, typecheck, tests - zero errors required
6. **Documentation**: JSDoc/Rustdoc with clear descriptions

### Agent Development

1. Create TOML config in `src-tauri/agents/config/`
2. Define system prompt in `src-tauri/agents/prompts/`
3. Implement agent logic following trait pattern
4. Register in agent registry
5. Add tools and MCP server dependencies

### For Bug Fixes

1. Reproduce issue with tests
2. Root cause analysis (never skip)
3. Fix systematically
4. Verify with full validation suite
5. Update tests to prevent regression

## Version Requirements

**Minimum**:
- Node.js: 20.19+ or 22.12+ (Vite 7 requirement)
- Rust: 1.80.1+ (SurrealDB SDK requirement)
- npm/pnpm/yarn: Latest stable

**Check versions**:
```bash
node --version    # >= 20.19
rustc --version   # >= 1.91.1
cargo --version   # >= 1.91.1
```

## CI/CD

**On Push** (feature branches):
- Linting (clippy + eslint)
- Unit tests
- Build verification

**On PR** (to main):
- Integration tests
- Security audit (`cargo audit`)
- Coverage report

**On Merge** (main):
- Release builds (Linux, macOS, Windows planned)
- E2E tests
- Artifact packaging

## OS Targets

**Priority**:
1. **Linux** (Phase 1): AppImage + .deb
2. **macOS** (Phase 1.5): .dmg
3. **Windows** (Phase 2): .msi

Build outputs in `src-tauri/target/release/bundle/`
