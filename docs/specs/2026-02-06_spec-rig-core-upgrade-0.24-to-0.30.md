# Specification - rig-core Upgrade 0.24.0 -> 0.30.0

## Metadata
- Date: 2026-02-06
- Stack: Rust 1.91.1 + Tauri 2.9 + rig-core 0.24.0 -> 0.30.0
- Complexity: **simple** (minimal surface area, well-abstracted)
- Risk: **low** (only 2 files use rig directly, no type exposure through Tauri)

## Context

**Request**: Update rig-core from 0.24.0 to the latest version (0.30.0)
**Objective**: Stay current with rig-core for bug fixes, streaming improvements, and better provider support
**Scope**: Backend only (Rust) - no frontend changes needed

**Success Criteria**:
- [ ] `cargo check` passes with rig-core 0.30.0
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes (existing tests + any new ones)
- [ ] No regression in LLM completion flow

## Current State

### rig-core Usage Inventory

| Metric | Value |
|--------|-------|
| Current version | 0.24.0 (features = "all") |
| Files using rig | **2** (mistral.rs, ollama.rs) |
| Import statements | 6 total (3 per file) |
| APIs used | `Prompt`, `CompletionClient`, `mistral::Client`, `ollama::Client` |
| Types crossing Tauri boundary | **0** (fully abstracted) |
| Custom HTTP workarounds | 4 (reasoning, thinking, tool calls x2) |

### Imports

**mistral.rs**:
```rust
use rig::completion::Prompt;           // Line 24
use rig::providers::mistral;           // Line 25
use rig::client::CompletionClient;     // Line 30
```

**ollama.rs**:
```rust
use rig::completion::Prompt;           // Line 20
use rig::providers::ollama;            // Line 21
use rig::client::CompletionClient;     // Line 25
```

### Usage Pattern

Both providers follow the same pattern:
1. Create provider-specific client (`mistral::Client::new(key)` / `ollama::Client::new()`)
2. Build an agent with `.agent(model).preamble().temperature().max_tokens().build()`
3. Execute with `.prompt(text).await`
4. For advanced features (tools, thinking, reasoning), bypass rig-core entirely with direct HTTP calls

### Architecture

```
LLMProvider trait (abstraction layer)
    |
    +-- MistralProvider
    |       |-- rig::providers::mistral::Client (basic completions only)
    |       |-- reqwest::Client (reasoning models, tool calls)
    |
    +-- OllamaProvider
            |-- rig::providers::ollama::Client (basic completions only)
            |-- reqwest::Client (thinking models, tool calls)
```

No rig types leak through the Tauri IPC boundary. The `LLMProvider` trait, `LLMResponse`, and `LLMError` are project-owned types.

## Changelog: rig-core 0.24.0 -> 0.30.0

### 0.25.0 - Provider Client Consolidation (BREAKING)
- All providers now use unified `client::Client<Ext, H>` generic architecture
- `Client::new()` for all providers **now returns `Result`** instead of a plain `Client`
- `ClientBuilder` syntax changed to `Client::builder().api_key().build()`
- Removed unused `chatbot` module
- API keys masked in debug output

### 0.26.0 - Request Routing
- All providers route requests through standardized `client::Client`
- Anthropic prompt caching support
- Fixed streaming span leak

### 0.27.0 - Crate Re-org
- Internal crate restructuring
- `reqwest-middleware` support (new feature flag, not in "all")
- Tool call signature changes

### 0.28.0 - Streaming Improvements
- `StreamingError` and `StreamingResult` types now public
- Tool name in streaming tool call deltas
- Custom HTTP headers for outgoing requests

### 0.29.0 - Agent Improvements
- Agent names in tracing spans
- Default max depth for agents
- Agentic loop early termination reason
- `reqwest` client re-export via `rig::reqwest`
- `AgentBuilder::tools` for adding static tools dynamically

### 0.30.0 - Hooks Rework (BREAKING)
- `max_depth` renamed to `max_turns` (not used by us)
- Hook control flow encoded in type signatures (not used by us)
- `on_tool_call` hook can reject tool execution (not used by us)
- Ollama streaming tool calls fix
- Concurrent tool execution fix

## Breaking Changes Affecting Our Code

### 1. Mistral Client Construction (mistral.rs)

`mistral::Client::new()` now returns `Result` instead of `Client`:

**Current** (lines 279, 297):
```rust
let client = mistral::Client::new(api_key);
```

**Required**:
```rust
let client = mistral::Client::new(api_key)
    .map_err(|e| LLMError::RequestFailed(format!("Failed to create Mistral client: {}", e)))?;
```

**Locations**:
- `mistral.rs:279` (`with_api_key()`)
- `mistral.rs:297` (`configure()`)

### 2. Ollama Client Construction (ollama.rs)

`ollama::Client::new()` now requires a `Nothing` argument (no API key) and returns `Result`.
`ollama::ClientBuilder` syntax changed.

**Current** (lines 151-155):
```rust
let client = if server_url != DEFAULT_OLLAMA_URL {
    ollama::ClientBuilder::new().base_url(server_url).build()
} else {
    ollama::Client::new()
};
```

**Required** (needs verification at compile time):
```rust
use rig::client::Nothing;

let client = if server_url != DEFAULT_OLLAMA_URL {
    ollama::Client::builder()
        .api_key(Nothing)
        .base_url(server_url)
        .build()
        .map_err(|e| LLMError::ConnectionError(format!("Failed to create Ollama client: {}", e)))?
} else {
    ollama::Client::new(Nothing)
        .map_err(|e| LLMError::ConnectionError(format!("Failed to create Ollama client: {}", e)))?
};
```

**Locations**:
- `ollama.rs:151-155` (`configure()`)

### 3. Non-Breaking Changes (No Action Needed)

The following APIs used by our code are **unchanged** in 0.30.0:
- `rig::completion::Prompt` trait
- `rig::client::CompletionClient` trait
- `.agent(model).preamble().temperature().max_tokens().build()` chain
- `.prompt(text).await` method

## Implementation Plan

### Phase 1: Dependency Update + Compile Fix

**Objective**: Update rig-core version and fix compilation errors

**Tasks**:

1. **Cargo.toml** (line 54):
   - Change `rig-core = { version = "0.24.0", features = ["all"] }` to `rig-core = { version = "0.30.0", features = ["all"] }`

2. **src-tauri/src/llm/mistral.rs**:
   - Line 279 (`with_api_key`): Add `.map_err()?` to `mistral::Client::new(api_key)`
   - Line 297 (`configure`): Add `.map_err()?` to `mistral::Client::new(api_key)`

3. **src-tauri/src/llm/ollama.rs**:
   - Add `use rig::client::Nothing;` import
   - Lines 151-155 (`configure`): Rewrite client construction with `Nothing` and error handling

4. **Compile check**: `cd src-tauri && cargo check`
   - The exact API for Ollama in 0.30.0 may differ slightly from what the changelog suggests
   - If `Nothing` or `builder()` pattern doesn't work exactly as documented, adapt based on compiler errors

**Validation**:
- [ ] `cargo check` passes
- [ ] No new warnings from `cargo clippy -- -D warnings`

### Phase 2: Test Validation

**Objective**: Ensure all existing tests pass

**Tasks**:

1. Run existing tests: `cd src-tauri && cargo test`
2. Verify Mistral tests pass (test_mistral_provider_configure, test_mistral_provider_new, etc.)
3. Verify Ollama tests pass (test_ollama_provider_configure, test_ollama_provider_custom_url, etc.)

**Validation**:
- [ ] `cargo test` passes
- [ ] No test regressions

### Phase 3: Frontend Validation (Sanity Check)

**Objective**: Confirm frontend still works (no Tauri IPC changes)

**Tasks**:
1. `npm run lint && npm run check`

**Validation**:
- [ ] Frontend checks pass (should be trivially true since no TS changes)

## Estimation

| Phase | Duration | Notes |
|-------|----------|-------|
| Phase 1 | ~10 min | Dependency update + 3 lines changed |
| Phase 2 | ~2 min | Run cargo test |
| Phase 3 | ~1 min | Sanity check |
| **Total** | **~15 min** | |

**Factors**:
- Very small surface area (6 imports, 2 files)
- Well-abstracted (no type leakage)
- Changes are mechanical (add error handling to constructors)
- Risk of unexpected API changes at compile time: low (research was thorough)

## Risk Analysis

| Risk | Probability | Impact | Mitigation | Plan B |
|------|-------------|--------|------------|--------|
| Ollama builder API different than documented | Medium | Low | Let compiler guide; adapt to actual API | Check rig-core source on GitHub |
| New transitive dependency conflicts | Low | Medium | `cargo update` + resolve | Pin specific versions |
| `features = ["all"]` changed scope | Low | Low | Already confirmed: still `derive + pdf + rayon` | Specify features explicitly |
| Agent builder chain changed | Very Low | Medium | Research confirms unchanged | Check `.agent()` source |

## New Features Available (Not Used Yet)

These are now available but do NOT need to be implemented in this upgrade:

| Feature | Version | Potential Use |
|---------|---------|---------------|
| Native streaming (StreamingResult) | 0.28.0 | Replace `simulate_streaming()` |
| Agent names in tracing | 0.29.0 | Better observability |
| `rig::reqwest` re-export | 0.29.0 | Could simplify imports |
| Ollama streaming tool calls | 0.30.0 | Replace custom HTTP for tool calls |
| Concurrent tool execution | 0.30.0 | Faster multi-tool workflows |

These should be tracked as future optimization opportunities, not part of this upgrade.

## Files Modified

| File | Change Type | Lines Changed |
|------|-------------|---------------|
| `src-tauri/Cargo.toml` | Version bump | 1 |
| `src-tauri/src/llm/mistral.rs` | Error handling on client creation | ~4 |
| `src-tauri/src/llm/ollama.rs` | Import + client construction rewrite | ~10 |
| **Total** | | ~15 lines |

## References

- [rig-core on crates.io](https://crates.io/crates/rig-core)
- [rig-core CHANGELOG](https://github.com/0xPlaygrounds/rig/blob/main/rig/rig-core/CHANGELOG.md)
- [PR #1050 - Consolidate provider clients](https://github.com/0xPlaygrounds/rig/pull/1050) (key breaking change)
- Current code: `src-tauri/src/llm/mistral.rs`, `src-tauri/src/llm/ollama.rs`
