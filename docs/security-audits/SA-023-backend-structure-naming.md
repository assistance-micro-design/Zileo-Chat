# SA-023 - Backend Structure & Naming Audit

**Date**: 2026-02-26
**Branch**: `security/audit-remediation-tdd`
**Scope**: `src-tauri/src/` - 95 Rust files, ~48,300 lines, 15 directories, 8 top-level modules
**Status**: PENDING

---

## Backend Structure Map

```
src-tauri/src/
  lib.rs                  # Crate root (9 modules)
  main.rs                 # Tauri entry point (commands registration + app setup)
  state.rs                # AppState (shared across all commands)
  test_utils.rs           # Test harness (#[cfg(test)])

  agents/                 # Multi-agent system
    core/                 # Agent trait + registry + orchestrator
      agent.rs            # Agent trait, Task, Report, ReportMetrics
      orchestrator.rs     # AgentOrchestrator
      registry.rs         # AgentRegistry
    llm_agent.rs          # LLM-based agent (1872 lines)
    simple_agent.rs       # Test-only agent

  commands/               # Tauri IPC handlers (20 modules)
    agent.rs, custom_provider.rs, embedding.rs, import_export.rs,
    llm.rs, llm_models.rs, mcp.rs, memory.rs, message.rs, migration.rs,
    prompt.rs, security.rs, streaming.rs, sub_agent_execution.rs,
    task.rs, thinking.rs, tool_execution.rs, user_question.rs,
    validation.rs, workflow.rs

  db/                     # SurrealDB layer
    client.rs             # DBClient (query, execute, CRUD)
    persistence.rs        # Shared persistence for tool_executions and thinking_steps
    queries.rs            # Centralized SQL query constants
    schema.rs             # Database schema definitions
    utils.rs              # sanitize_for_surrealdb, count helpers

  llm/                    # LLM provider abstraction
    adapters/             # Tool adapters per provider
      mistral_adapter.rs, ollama_adapter.rs, openai_adapter.rs, tests.rs
    circuit_breaker.rs    # LLM provider circuit breaker
    embedding.rs          # Embedding service (1103 lines)
    manager.rs            # ProviderManager (1010 lines)
    mistral.rs            # MistralProvider
    ollama.rs             # OllamaProvider
    openai_compatible.rs  # Generic OpenAI-compatible provider
    pricing.rs            # Token pricing
    provider.rs           # LLMProvider trait + ProviderType enum
    retry.rs              # Retry with exponential backoff
    tool_adapter.rs       # ProviderToolAdapter trait
    utils.rs              # estimate_tokens

  mcp/                    # Model Context Protocol
    circuit_breaker.rs    # MCP server circuit breaker
    client.rs             # MCPClient
    error.rs              # MCPError, MCPResult
    helpers.rs            # parse_deployment_method, parse_env_json
    http_handle.rs        # HTTP/SSE transport
    manager.rs            # MCPManager (1233 lines)
    protocol.rs           # JSON-RPC 2.0 types
    server_handle.rs      # Stdio transport + process management

  models/                 # Data models (20 modules)
    agent.rs, chat_block.rs, custom_provider.rs, embedding.rs,
    function_calling.rs, import_export.rs, llm_models.rs, mcp.rs,
    memory.rs, message.rs, prompt.rs, serde_utils.rs, streaming.rs,
    sub_agent.rs, task.rs, thinking_step.rs, tool_execution.rs,
    user_question.rs, validation.rs, workflow.rs

  security/               # Security layer
    keystore.rs           # AES-256-GCM key storage
    validation.rs         # Input sanitization (Validator, validate_uuid_field, etc.)

  tools/                  # Agent tools framework (13 files + 4 subdirectories)
    calculator/           # CalculatorTool
    memory/               # MemoryTool
    todo/                 # TodoTool
    user_question/        # UserQuestionTool + circuit breaker
    constants.rs          # ALL app constants (tools, commands, workflows, query limits)
    context.rs            # AgentToolContext for sub-agent tools
    delegate_task.rs      # DelegateTaskTool
    factory.rs            # ToolFactory
    parallel_tasks.rs     # ParallelTasksTool
    registry.rs           # TOOL_REGISTRY
    response.rs           # ResponseBuilder
    spawn_agent.rs        # SpawnAgentTool
    sub_agent_circuit_breaker.rs  # Sub-agent circuit breaker
    sub_agent_executor.rs # Sub-agent execution engine (1770 lines)
    utils.rs              # ensure_record_exists, resolve_agent_ref, safe_truncate
    validation_helper.rs  # Human-in-the-loop validation
```

---

## Findings

| Severity | ID | Description | Status |
|----------|----|-------------|--------|
| HIGH | H1 | `ProviderType` duplicated in two modules | DONE |
| HIGH | H2 | `commands/models.rs` naming ambiguity | DONE |
| MEDIUM | M1 | `tools/validation_helper.rs` mixes unrelated concerns | DONE |
| MEDIUM | M2 | `tools/constants.rs` scope exceeds its module | DONE |
| MEDIUM | M3 | Large files candidates for decomposition | DEFERRED |
| LOW | L1 | Excessive `#[allow(dead_code)]` annotations | DEFERRED |
| LOW | L2 | Minor naming asymmetries commands/ vs models/ | NO-ACTION |

---

## HIGH Severity

### H1. `ProviderType` duplicated in two modules

**Files**:
- `src-tauri/src/llm/provider.rs` (lines 27-34): `ProviderType` enum
- `src-tauri/src/models/llm_models.rs` (lines 33-41): identical `ProviderType` enum

**Problem**: Two identical `ProviderType` enums with identical Serialize/Deserialize. Used independently:
- `crate::llm::ProviderType` in `commands/llm.rs`, `agents/llm_agent.rs`, all LLM internal code
- `crate::models::llm_models::ProviderType` in `commands/agent.rs`

**Risk**: If one enum is modified without updating the other, type drift causes subtle bugs (new variant added to one but not the other).

**Remediation**: Remove duplicate from `models/llm_models.rs`. Re-export from `llm::ProviderType` OR consolidate into `models/` and have `llm/` import from there. The `models/` location is more natural as shared data contract.

**Checklist**:
- [x] Determine canonical location: `llm/provider.rs` (natural semantic location, avoids circular dependency)
- [x] Remove duplicate from `models/llm_models.rs` (enum + Serialize/Deserialize/Display/FromStr impls)
- [x] Add `use crate::llm::ProviderType` to `models/llm_models.rs`
- [x] Re-export from `models/mod.rs` via `pub use crate::llm::ProviderType`
- [x] Update `commands/models.rs` import: `crate::llm::ProviderType`
- [x] Update `commands/agent.rs` import: `crate::llm::ProviderType`
- [x] Remove redundant Display/FromStr tests from `models/llm_models.rs`
- [x] `cargo clippy -- -D warnings` PASS
- [x] `cargo test` PASS (2002 tests, 0 failures)

**Note**: Canonical location chosen as `llm/provider.rs` instead of `models/` because:
1. `ProviderType` is semantically an LLM concept (part of `LLMProvider` trait)
2. `models/llm_models.rs` already imports from `llm/` (`DEFAULT_OLLAMA_URL`)
3. Avoids backwards dependency (`llm/` -> `models/`)
4. Zero changes needed in the `llm/` module (largest consumer)

---

### H2. `commands/models.rs` naming ambiguity

**File**: `src-tauri/src/commands/models.rs`

**Problem**: `commands/models.rs` contains LLM Model CRUD commands (`list_models`, `create_model`, etc.), but "models" collides with the top-level `models/` directory. Reading `commands::models::list_models` suggests data models, not LLM models.

**Registered in main.rs**: 10 commands (`list_models`, `get_model`, etc.)

**Remediation**: Rename to `commands/llm_models.rs` to match `models/llm_models.rs` counterpart and eliminate ambiguity.

**Checklist**:
- [x] Rename `commands/models.rs` -> `commands/llm_models.rs`
- [x] Update `commands/mod.rs` module declaration + doc comment
- [x] Update `main.rs` imports (10 commands)
- [x] `cargo fmt --check` PASS
- [x] `cargo clippy -- -D warnings` PASS
- [x] `cargo test` PASS (2002 tests, 0 failures)

---

## MEDIUM Severity

### M1. `tools/validation_helper.rs` mixes unrelated concerns

**File**: `src-tauri/src/tools/validation_helper.rs` (999 lines)

**Problem**: Contains two unrelated functionalities:
1. **Human-in-the-loop validation** (`request_sub_agent_validation`, `request_tool_validation`, etc.) - the core purpose
2. **`safe_truncate()`** (line 60) - generic UTF-8 string truncation utility

`safe_truncate()` is referenced across the codebase as a key safety function. Placing it in "validation_helper" (which means human-in-the-loop) creates confusion.

**Remediation**: Extract `safe_truncate()` into `tools/utils.rs` (already exists, contains other utilities like `ensure_record_exists`).

**Checklist**:
- [x] Move `safe_truncate()` + its tests to `tools/utils.rs`
- [x] Update all imports referencing `validation_helper::safe_truncate` (4 files: sub_agent_executor.rs, spawn_agent.rs, memory/tool.rs, validation_helper.rs)
- [x] `cargo fmt --check` PASS
- [x] `cargo clippy -- -D warnings` PASS
- [x] `cargo test` PASS (976 tests, 0 failures)

---

### M2. `tools/constants.rs` scope exceeds its module

**File**: `src-tauri/src/tools/constants.rs` (258 lines)

**Problem**: Despite being in `tools/`, this file contains:
- `tools::constants::memory` - Tool constants (correct)
- `tools::constants::todo` - Tool constants (correct)
- `tools::constants::user_question` - Tool constants (correct)
- `tools::constants::calculator` - Tool constants (correct)
- `tools::constants::sub_agent` - Tool constants (correct)
- **`tools::constants::workflow`** - Workflow execution timeouts (not tool-specific)
- **`tools::constants::query_limits`** - Database query limits (not tool-specific)
- **`tools::constants::commands`** - Tauri command validation limits (not tool-specific)

The last three groups are app-wide constants accessed as `crate::tools::constants::query_limits::DEFAULT_LIST_LIMIT`, falsely suggesting tool relation.

**Remediation**: Create top-level `src/constants.rs` module. Move `workflow`, `query_limits`, and `commands` constants there. Keep only tool-specific constants in `tools/constants.rs`.

**Checklist**:
- [x] Create `src-tauri/src/constants.rs`
- [x] Move `workflow`, `query_limits`, `commands` sub-modules to new file
- [x] Register module in `lib.rs` and `main.rs`
- [x] Update all `crate::tools::constants::{workflow,query_limits,commands}` -> `crate::constants::{...}` (11 files)
- [x] Keep tool-specific constants in `tools/constants.rs` (memory, todo, user_question, sub_agent, calculator)
- [x] Update `tools/constants.rs` doc comment to reference `crate::constants`
- [x] Update `.claude/rules/surrealdb.md` import reference
- [x] `cargo fmt --check` PASS
- [x] `cargo clippy -- -D warnings` PASS
- [x] `cargo test` PASS (1976 tests, 0 failures)

---

### M3. Large files candidates for decomposition (DEFERRED)

| File | Lines | Description |
|------|-------|-------------|
| `agents/llm_agent.rs` | 1,872 | Full LLM agent with tool loop, report enforcement, streaming |
| `tools/sub_agent_executor.rs` | 1,770 | Sub-agent execution engine with retry, monitoring, persistence |
| `commands/mcp.rs` | 1,312 | 10 MCP-related Tauri commands |
| `mcp/manager.rs` | 1,233 | MCPManager with server lifecycle, tool routing |
| `commands/llm_models.rs` | 1,224 | LLM model CRUD + provider settings |
| `models/streaming.rs` | 1,157 | Streaming event types + builder |
| `commands/import_export.rs` | 1,155 | Import/export commands |
| `tools/parallel_tasks.rs` | 1,123 | ParallelTasksTool with batching |
| `llm/embedding.rs` | 1,103 | EmbeddingService with multiple providers |
| `llm/manager.rs` | 1,010 | ProviderManager with all provider logic |
| `tools/validation_helper.rs` | 999 | Validation + safe_truncate |

Priority candidates for future decomposition:
- **`agents/llm_agent.rs`**: Extract tool execution loop, report enforcement, streaming event emission
- **`tools/sub_agent_executor.rs`**: Extract retry logic, activity monitoring, persistence

**Status**: DEFERRED - Files are well-structured internally. Decompose when next modifying these files significantly.

---

## LOW Severity

### L1. Excessive `#[allow(dead_code)]` annotations (DEFERRED)

**Counts**: 165 `#[allow(dead_code)]` across 44 files, 48 `#[allow(unused_imports)]` across 8 files.

Most are in `mod.rs` for re-exports used by frontend via IPC or reserved for serialization symmetry. `llm/embedding.rs` alone has 31 annotations.

**Status**: DEFERRED - Periodic audit recommended during future cleanup passes.

### L2. Minor naming asymmetries commands/ vs models/ (NO-ACTION)

| Commands module | Models module | Note |
|-----------------|---------------|------|
| `commands/thinking.rs` | `models/thinking_step.rs` | action vs entity |
| `commands/sub_agent_execution.rs` | `models/sub_agent.rs` | action vs entity |
| `commands/llm.rs` | `models/llm_models.rs` | domain vs entity |

Commands are named after action domains, models after data entities. This is a reasonable convention. **No action needed.**

---

## Validated as Acceptable

### OK1. `validation` in 4 different files
Module namespacing (`models::validation`, `security::validation`, `commands::validation`, `tools::validation_helper`) makes each unambiguous.

### OK2. `Agent` trait vs `Agent` struct
`agents::core::agent::Agent` (trait) vs `models::agent::Agent` (struct) serve fundamentally different purposes. `AgentConfig` is what is actually used throughout the codebase.

### OK3. 4 separate circuit breaker implementations
Each has domain-specific configuration (thresholds, cooldowns, state). Unifying would create over-generalized abstraction. Each is under 200 lines.

### OK4. `serde_utils.rs` in `models/`
Provides serde deserializers specifically for SurrealDB model quirks. Correctly placed in `models/`.

---

## Implementation Plan

| Phase | Finding | Action | Effort |
|-------|---------|--------|--------|
| P1 | H1 | Consolidate `ProviderType` into single location | Low |
| P2 | H2 | Rename `commands/models.rs` -> `commands/llm_models.rs` | Low |
| P3 | M1 | Extract `safe_truncate()` to `tools/utils.rs` | Low |
| P4 | M2 | Create top-level `constants.rs`, move app-wide constants | Medium |

Total estimated phases: 4 (actionable), 2 deferred (M3, L1)
