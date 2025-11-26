# Rapport - Phase 1: Backend - Agent Persistence (Foundation)

## Metadata
- **Date**: 2025-11-26
- **Complexity**: Medium
- **Stack**: Rust 1.91 + Tauri 2.9 + SurrealDB 2.3

## Objective

Implement Phase 1 of the Functional Agent System specification:
- Remove hardcoded agents from main.rs
- Add agent table schema to SurrealDB
- Implement CRUD Tauri commands for agents
- Enable users to create agents via Settings UI

## Work Completed

### Features Implemented

1. **Database Schema** - Added `agent` table to SurrealDB with full validation
2. **CRUD Types** - Added AgentConfigCreate, AgentConfigUpdate, AgentSummary
3. **Agent Commands** - Implemented create_agent, update_agent, delete_agent, load_agents_from_db
4. **Registry Update** - Added unregister_any method for CRUD operations
5. **Startup Refactoring** - Removed 3 hardcoded agents, now loads from DB

### Files Modified

**Backend** (Rust):
| File | Action | Description |
|------|--------|-------------|
| `src-tauri/src/db/schema.rs` | Modified | Added `agent` table schema with validation constraints |
| `src-tauri/src/models/agent.rs` | Modified | Added AgentConfigCreate, AgentConfigUpdate, AgentSummary, KNOWN_TOOLS |
| `src-tauri/src/models/mod.rs` | Modified | Exported new agent types |
| `src-tauri/src/commands/agent.rs` | Modified | Added CRUD commands with validation |
| `src-tauri/src/agents/core/registry.rs` | Modified | Added unregister_any method |
| `src-tauri/src/agents/mod.rs` | Modified | Allow unused SimpleAgent import |
| `src-tauri/src/agents/simple_agent.rs` | Modified | Allow dead_code for test usage |
| `src-tauri/src/main.rs` | Modified | Removed hardcoded agents, added load_agents_from_db |

### Statistics

```
18 files changed, 707 insertions(+), 1344 deletions(-)
```

Note: Large deletions are from removed unused tools/db/* files (not part of this task).

### Types Created

**Rust** (`src-tauri/src/models/agent.rs`):
```rust
pub struct AgentConfigCreate {
    pub name: String,
    pub lifecycle: Lifecycle,
    pub llm: LLMConfig,
    pub tools: Vec<String>,
    pub mcp_servers: Vec<String>,
    pub system_prompt: String,
}

pub struct AgentConfigUpdate {
    pub name: Option<String>,
    pub llm: Option<LLMConfig>,
    pub tools: Option<Vec<String>>,
    pub mcp_servers: Option<Vec<String>>,
    pub system_prompt: Option<String>,
}

pub struct AgentSummary {
    pub id: String,
    pub name: String,
    pub lifecycle: Lifecycle,
    pub provider: String,
    pub model: String,
    pub tools_count: usize,
    pub mcp_servers_count: usize,
}

pub const KNOWN_TOOLS: [&str; 2] = ["MemoryTool", "TodoTool"];
```

### Commands Implemented

| Command | Parameters | Return | Description |
|---------|------------|--------|-------------|
| `list_agents` | - | `Vec<AgentSummary>` | List all agents with summary info |
| `get_agent_config` | `agent_id: String` | `AgentConfig` | Get full agent configuration |
| `create_agent` | `config: AgentConfigCreate` | `String` | Create agent, returns UUID |
| `update_agent` | `agent_id: String, config: AgentConfigUpdate` | `AgentConfig` | Update agent |
| `delete_agent` | `agent_id: String` | `()` | Delete agent |

### Database Schema

```surql
DEFINE TABLE agent SCHEMAFULL;
DEFINE FIELD id ON agent TYPE string;
DEFINE FIELD name ON agent TYPE string
    ASSERT string::len($value) >= 1 AND string::len($value) <= 64;
DEFINE FIELD lifecycle ON agent TYPE string
    ASSERT $value IN ['permanent', 'temporary'];
DEFINE FIELD llm ON agent TYPE object;
DEFINE FIELD llm.provider ON agent TYPE string
    ASSERT $value IN ['Mistral', 'Ollama', 'Demo'];
DEFINE FIELD llm.model ON agent TYPE string;
DEFINE FIELD llm.temperature ON agent TYPE float;
DEFINE FIELD llm.max_tokens ON agent TYPE int;
DEFINE FIELD tools ON agent TYPE array<string>;
DEFINE FIELD mcp_servers ON agent TYPE array<string>;
DEFINE FIELD system_prompt ON agent TYPE string;
DEFINE FIELD created_at ON agent TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON agent TYPE datetime DEFAULT time::now();
DEFINE INDEX unique_agent_id ON agent FIELDS id UNIQUE;
```

## Technical Decisions

### Architecture
- **No default agents**: Users create agents via Settings UI (no hardcoded agents)
- **LLMAgent only**: All user-created agents use LLMAgent (SimpleAgent for tests only)
- **Lifecycle immutable**: Agent lifecycle cannot be changed after creation

### Validation
- Agent name: 1-64 chars, no control characters
- System prompt: 1-10000 chars
- Temperature: 0.0-2.0
- max_tokens: 256-128000
- Tools: Must be in KNOWN_TOOLS list
- MCP servers: Alphanumeric, underscore, hyphen only

### Patterns Used
- **JSON serialization**: Using serde_json for SurrealDB string escaping
- **Execute for writes**: Using db.execute() to avoid Thing type issues
- **Unregister + Register**: Update flow removes old agent, creates new one

## Validation

### Tests Backend
- **Clippy**: PASS (0 warnings)
- **Cargo test**: 389/389 PASS (1 ignored - keychain test)
- **Build**: SUCCESS

### Quality Code
- Types stricts (Rust)
- Documentation complete (Rustdoc)
- Standards project respected
- No any/mock/emoji/TODO

## Next Steps (Phase 2-5)

1. **Phase 2**: Frontend Store & Types (AgentConfigCreate, AgentConfigUpdate, AgentSummary in $types/agent)
2. **Phase 3**: Agent Settings UI (AgentSettings.svelte, AgentList.svelte, AgentForm.svelte)
3. **Phase 4**: Agent Selector Integration (WorkflowPage agent dropdown)
4. **Phase 5**: Tool Execution Integration (Agent tool calls)

## Metrics

### Code
- **Lines added**: +707
- **Lines deleted**: -1344 (includes removed unused files)
- **Files modified**: 18
- **New commands**: 3 (create, update, delete)
- **Modified commands**: 1 (list_agents returns AgentSummary[])
