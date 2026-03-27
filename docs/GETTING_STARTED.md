# Getting Started - Zileo Chat 3

> Development environment setup and first workflow

## Prerequisites

### Minimum Versions

- **Node.js**: 20.19+ or 22.12+ (required by Vite 7)
- **Rust**: 1.93.0+ (required by SurrealDB SDK)
- **npm/pnpm/yarn**: Latest stable

### Optional Tools

- **Docker**: For local MCP servers
- **Python 3.10+**: For Python-based MCP servers (uvx)

### Verification

```bash
node --version && rustc --version && cargo --version
```

---

## Installation

### 1. Clone the Project

```bash
git clone <repo_url> && cd zileo-chat-3
```

### 2. Install Frontend Dependencies

```bash
npm install
```

### 3. Install Tauri CLI

```bash
cargo install tauri-cli --version ^2.10
```

### 4. Configure SurrealDB

**Option A: Embedded (Recommended for Dev)**
- Automatic, no setup required
- RocksDB local storage in `appDataDir()`

**Option B: Local Server**

```bash
surreal start --user root --pass root memory
```

---

## Configuration

### Tauri Config

Check `src-tauri/tauri.conf.json`:
- `identifier`: Unique bundle ID
- `security.csp`: Content Security Policy

---

## Development

### Full Dev Mode (Frontend + Backend)

```bash
npm run tauri:dev
```

This starts:
- Frontend: Vite HMR on http://localhost:5173
- Backend: Rust compile + watch mode
- Window: Tauri application window

### Frontend Only

```bash
npm run dev
```

Useful for UI work without the backend.

### Backend Only

```bash
cd src-tauri && cargo run
```

For testing Rust commands in isolation.

---

## Project Structure

```
zileo-chat-3/
+- src/                     # Frontend SvelteKit
|  +- routes/               # Pages (file-based routing)
|  |  +- settings/          # Settings page (9 sections)
|  |  +- agent/             # Agent page (main chat)
|  +- lib/
|  |  +- components/        # Svelte components
|  |  +- stores/            # Svelte stores
|  |  +- services/          # Business logic layer
|  |  +- i18n/              # Internationalization
|  +- types/                # TypeScript type definitions
|  +- messages/             # Translations (en.json, fr.json)
|
+- src-tauri/               # Backend Rust
|  +- src/
|  |  +- main.rs            # Entry point
|  |  +- commands/          # Tauri commands
|  |  +- agents/            # Multi-agent system
|  |  +- llm/               # LLM integration (Mistral, Ollama, OpenAI-compatible)
|  |  +- mcp/               # MCP client/server
|  |  +- tools/             # Agent tools (Memory, Todo, Calculator, TaskBridge, FileManager, ReadSkill, SpawnAgent, DelegateTask, ParallelTasks, UserQuestion)
|  |  +- models/            # Rust structs (synced with TS types)
|  |  +- security/          # Keystore + validation
|  |  +- db/                # SurrealDB client
|  +- Cargo.toml
|  +- tauri.conf.json
|
+- docs/                    # Documentation (13 files)
```

See [TECH_STACK.md](TECH_STACK.md) for detailed component, store, command, and table counts.

---

## First Workflow

### 1. Launch the Application

```bash
npm run tauri:dev
```

### 2. Onboarding Assistant

On first launch, a guided assistant helps with initial configuration:
1. **Language**: Select English or French
2. **Theme**: Light or Dark
3. **Provider**: Configure an API key (Mistral, Ollama, or any OpenAI-compatible provider)
4. **Import**: Optionally import an existing configuration

### 3. Advanced Configuration

**Settings Page** (9 sections):

| Section | Description |
|---------|-------------|
| **Providers** | Configure Mistral, Ollama, or OpenAI-compatible custom providers (API keys + models) |
| **Agents** | Create your first agent (no default agents) |
| **MCP Servers** | Configure MCP servers (Docker/NPX/UVX) |
| **Skills** | Manage skill documents assignable to agents |
| **Memory** | Configure embeddings + manage memories |
| **Validation** | Human-in-the-loop parameters |
| **Prompts** | Prompt library |
| **Import/Export** | Backup and restore configuration (schema v1.1) |
| **Theme** | Light/Dark theme selection |

All API keys are configured via the UI and stored securely (Tauri secure storage + OS keyring).

### 4. Create a Workflow

On the **Agent** page:
1. Click **+ New** in the workflow sidebar
2. Select the agent you created
3. Name the workflow
4. Send a message in the chat area

### 5. Observe Execution

**Real-time indicators**:
- Workflow status (Running/Complete/Error)
- Token usage (input/output/cost, updated incrementally)
- Tool calls (MemoryTool, TodoTool, TaskBridge, CalculatorTool, etc.)
- MCP server interactions (if configured)

### 6. Validation (Human-in-the-Loop)

If validation mode is enabled:
1. A modal appears: "Validation Required"
2. Operation details are shown (query, parameters)
3. Approve or Reject
4. Workflow continues after validation

---

## MCP Server Configuration (Optional)

Configure MCP servers via **Settings > MCP Servers**.

Each server needs a command, arguments, and optional environment variables. Use the **Test** button to verify connectivity (online/offline status).

Supported transport types: stdio (Docker, NPX, UVX) and SSE.

---

## Debugging

### Frontend (SvelteKit)
- **DevTools**: F12 in the Tauri window
- **Svelte Inspector**: Click components in dev mode

### Backend (Rust)
- **Logs**: `tracing` output in terminal
- **Breakpoints**: VS Code with `rust-analyzer`

### Database (SurrealDB)
- **CLI**: `surreal sql` (interactive mode)
- **Logs**: `SURREAL_LOG=trace`

---

## Tests

### Frontend

```bash
npm run test          # Vitest unit tests
npm run test:e2e      # Playwright E2E
```

### Backend

```bash
cd src-tauri && cargo test
```

---

## Production Build

```bash
npm run tauri:build
```

**Outputs** (platform-dependent):
- Linux: `src-tauri/target/release/bundle/appimage/`
- macOS: `src-tauri/target/release/bundle/dmg/`
- Windows: `src-tauri/target/release/bundle/msi/`

For release builds and CI/CD, see [DEPLOYMENT_GUIDE.md](DEPLOYMENT_GUIDE.md).

---

## Troubleshooting

### Node.js Version Error
**Symptom**: `Error: Vite requires Node.js 20.19+ or 22.12+`
**Fix**: Install a compatible Node.js version.

### Rust Compilation Error
**Symptom**: `error: package requires Rust 1.80.1+ (or higher)`
**Fix**: `rustup update stable`

### SurrealDB Connection Failed
1. Check server is running: `surreal version`
2. Embedded mode: verify permissions on `appDataDir()`
3. Server mode: verify URL and credentials

### Tauri Build Failed
1. Clear cache: `cargo clean`
2. Rebuild: `npm run tauri:build`
3. Check logs in `src-tauri/target/release/build/`

### MCP Server Offline
1. Test the command manually (e.g. `docker run -i --rm <image>`)
2. Verify Docker/NPX/UVX is installed
3. Check logs in Settings > MCP Servers

---

## Next Steps

1. **Multi-Agent Architecture**: [MULTI_AGENT_ARCHITECTURE.md](MULTI_AGENT_ARCHITECTURE.md)
2. **Agent Tools**: [AGENT_TOOLS_DOCUMENTATION.md](AGENT_TOOLS_DOCUMENTATION.md)
3. **API Reference**: [API_REFERENCE.md](API_REFERENCE.md)
4. **Workflow Orchestration**: [WORKFLOW_ORCHESTRATION.md](WORKFLOW_ORCHESTRATION.md)

---

## Resources

- **Tauri**: https://v2.tauri.app/start
- **SvelteKit**: https://kit.svelte.dev/docs
- **SurrealDB**: https://surrealdb.com/docs
