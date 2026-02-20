# SA-008: Agent System Quality & Performance Audit

**Date**: 2026-02-19
**Status**: Documented
**Scope**: `src-tauri/src/agents/`, `src-tauri/src/llm/`, `src-tauri/src/commands/streaming.rs`

## Executive Summary

| Category | Findings |
|----------|----------|
| Code Duplication | 5 patterns, ~480 lines eliminable |
| Performance Hotspots | 3 issues (1 HIGH, 2 MEDIUM) |
| Architecture | Well-structured, 2 improvement opportunities |
| Error Handling | Consistent, 1 improvement opportunity |

---

## 1. Architecture Overview

### 1.1 Component Responsibilities

```
agents/
  core/
    agent.rs      - Agent trait + data types (Task, Report, ReportMetrics, etc.)
    registry.rs   - Thread-safe agent registry (HashMap<String, Arc<dyn Agent>>)
    orchestrator.rs - Thin dispatch layer (registry.get + agent.execute)
  llm_agent.rs    - Primary agent: LLM calls + tool loop + MCP integration
  simple_agent.rs - Test-only stub (sleep + fixed report)

llm/
  provider.rs     - LLMProvider trait + ProviderType enum + LLMError
  manager.rs      - Provider dispatch + retry + circuit breaker
  retry.rs        - Exponential backoff with retryable error classification
  circuit_breaker.rs - Circuit breaker pattern for provider health
  tool_adapter.rs - ProviderToolAdapter trait for JSON function calling
  adapters/
    mistral_adapter.rs - Mistral format (choices[0].message, string args)
    ollama_adapter.rs  - Ollama format (message, object args, synthetic IDs)
    openai_adapter.rs  - OpenAI standard (identical to Mistral except tool_choice)

commands/
  streaming.rs    - Tauri command: workflow orchestration, persistence, events
```

### 1.2 Execution Flow Diagram

```
User Message (frontend)
    |
    v
[streaming.rs] execute_workflow_streaming()
    |-- Validate inputs (UUID, message, agent_id)
    |-- Create cancellation token
    |-- Load workflow from DB
    |-- Load conversation history from DB
    |-- Build Task { description, context }
    |
    v
[orchestrator.rs] execute_with_mcp()
    |-- registry.get(agent_id) -> Arc<dyn Agent>
    |
    v
[llm_agent.rs] execute_with_mcp()
    |-- Validate provider type (parse from config)
    |-- Check provider is configured
    |-- Select adapter (Mistral | Ollama | OpenAI)
    |-- Create local tools via ToolFactory
    |-- Discover MCP tools + server summaries
    |-- Build system prompt + tool definitions
    |-- Initialize messages array
    |
    v
    +== TOOL LOOP (max_iterations clamped 1-200) ==+
    |                                                |
    |  1. messages.clone() <-- PERF HOTSPOT          |
    |  2. tools_json.clone()                         |
    |  3. provider_manager.complete_with_tools()     |
    |     |                                          |
    |     v                                          |
    |  [manager.rs] complete_with_tools()            |
    |     |-- check_circuit_breaker()                |
    |     |-- match provider_type:                   |
    |     |     Mistral -> with_retry(mistral.call)  |
    |     |     Ollama  -> with_retry(ollama.call)   |
    |     |     Custom  -> with_retry(custom.call)   |
    |     |-- record_circuit_success/failure()       |
    |     |                                          |
    |  4. adapter.parse_tool_calls(response)         |
    |  5. If no tool_calls -> BREAK (done)           |
    |  6. For each call:                             |
    |     |-- emit_progress(tool_start)              |
    |     |-- execute_function_call()                |
    |     |   |-- MCP? -> mcp_manager.call_tool()    |
    |     |   |-- Local? -> tool.execute()           |
    |     |-- Collect ToolExecutionData              |
    |     |-- emit_progress(tool_end)                |
    |     |-- messages.push(tool_result)             |
    |  7. messages.push(assistant_message)           |
    |                                                |
    +================================================+
    |
    v
    Return Report { content, response, metrics, system_prompt }
    |
    v
[streaming.rs] (post-execution)
    |-- Persist system prompt (first message only)
    |-- Load model for pricing
    |-- Calculate cost_usd
    |-- UPDATE workflow SET tokens, cost, model_id
    |-- Persist tool_executions (sequential loop)   <-- PERF: sequential
    |-- Persist thinking_steps (sequential loop)    <-- PERF: sequential
    |-- Emit workflow_complete event
    |-- Return WorkflowResult
```

---

## 2. Code Duplication Analysis

### DUP-1: Report::failed() - 5 instances in llm_agent.rs [HIGH]

**Lines**: 799-819, 835-855, 1004-1024, 1040-1060, 1287-1307
**Duplicated code**: ~100 lines total (5 x ~20 lines)

Each constructs an identical Report with:
- `status: ReportStatus::Failed`
- `content: format!("# Agent Report: ...\n...\n## Error\n{}", error_msg)`
- `metrics: ReportMetrics { all zeros }`
- `system_prompt: None, tools_json: None`

**Subtle inconsistency already present**: Lines 803 and 1008 use slightly different format strings for content.

**Recommendation**: Add `Report::failed(task_id, agent_id, description, error_msg, duration_ms)` constructor.

### DUP-2: execute() is a subset of execute_with_mcp() [MEDIUM]

**Lines**: execute() at 779-959 vs execute_with_mcp() at 985-1060 (early validation)

Shared patterns:
- Provider type validation (~20 lines identical)
- Provider configuration check (~20 lines identical)
- LLM error match block (~20 lines identical, lines 916-933 vs 1269-1285)

Additionally, `execute_with_mcp()` falls back to `execute()` when no tools are available (line 1129), meaning both methods must be maintained.

**Recommendation**: Extract `validate_provider()` helper and `format_llm_error()` helper. Consider making `execute()` a thin wrapper calling `execute_with_mcp(task, None)`.

### DUP-3: OpenAI adapter = Mistral adapter copy-paste [HIGH]

**Files**: `mistral_adapter.rs` (214 lines), `openai_adapter.rs` (203 lines)
**Identical methods** (8 of 11):
- `format_tools()` - identical
- `parse_tool_calls()` - identical (both use choices[0].message.tool_calls)
- `format_tool_result()` - identical
- `extract_content()` - identical
- `has_tool_calls()` - identical
- `is_finished()` - identical
- `build_assistant_message()` - identical
- `extract_usage()` - identical

**Only differences** (3 of 11):
- `get_tool_choice(Required)`: `"any"` (Mistral) vs `"required"` (OpenAI)
- `provider_name()`: `"mistral"` vs `"openai_compatible"`
- Debug log messages use different provider names

**Duplicated production code**: ~100 lines

**Recommendation**: Create a `ChoicesBasedAdapter` struct with a `required_tool_choice: &'static str` field. Instantiate as `MistralToolAdapter` and `OpenAiToolAdapter` with different values.

### DUP-4: Provider dispatch in manager.rs - 3x match arms [MEDIUM]

**Methods**: `complete()` (lines 455-516), `complete_with_provider()` (lines 548-607), `complete_with_tools()` (lines 664-725)

Each contains:
```rust
match &provider {
    ProviderType::Mistral => { clone_inputs; with_retry(mistral.call) }
    ProviderType::Ollama  => { clone_inputs; with_retry(ollama.call) }
    ProviderType::Custom  => { clone_inputs; with_retry(custom.call) }
}
// + circuit_breaker success/failure recording
```

**Duplicated code**: ~180 lines across 3 methods

**Recommendation**: Extract a `dispatch_with_retry<F, T>()` helper that resolves the provider, wraps with retry, and records circuit breaker state. The 3 methods become thin wrappers.

### DUP-5: ToolExecution triple struct mapping in streaming.rs [LOW]

**Data flow**: `ToolExecutionData` (agent.rs) -> `WorkflowToolExecution` (streaming) -> `ToolExecutionCreate` (DB)

Three near-identical structs with the same fields, mapped field-by-field twice:
- Lines 569-584: ToolExecutionData -> WorkflowToolExecution
- Lines 588-603: WorkflowToolExecution -> ToolExecutionCreate

**Recommendation**: Consider `From<ToolExecutionData>` impls or a shared trait to reduce boilerplate.

### Duplication Summary

| ID | Location | Lines Duplicated | Severity | Effort to Fix |
|----|----------|-----------------|----------|---------------|
| DUP-1 | llm_agent.rs | ~100 | HIGH | Low (constructor) |
| DUP-2 | llm_agent.rs | ~60 | MEDIUM | Low (helpers) |
| DUP-3 | adapters/ | ~100 | HIGH | Medium (refactor) |
| DUP-4 | manager.rs | ~180 | MEDIUM | Medium (generic dispatch) |
| DUP-5 | streaming.rs | ~40 | LOW | Low (From impls) |
| **Total** | | **~480** | | |

---

## 3. Performance Hotspots

### PERF-1: messages.clone() in tool loop [HIGH]

**Location**: `llm_agent.rs:1240`
**Context**: Inside the main `loop` in `execute_with_mcp()`

```rust
// Called each iteration - O(total_message_size)
messages.clone(),  // Vec<serde_json::Value>
tools_json.clone(), // Vec<serde_json::Value>
```

**Growth pattern**:
- Iteration 1: messages = [system, user] (~5KB system prompt + user msg)
- Iteration 2: + [assistant_with_tool_calls, tool_result] (~2KB)
- Iteration N: accumulated context grows linearly

**Cost**: O(message_size * iterations). With 30 iterations and 50KB average context, this means ~1.5MB of unnecessary allocation.

**Root cause**: `complete_with_tools()` takes `Vec<serde_json::Value>` by value because the retry closure needs to own the data.

**Recommendation**:
1. Change `complete_with_tools()` to accept `&[serde_json::Value]` for messages
2. Clone only inside the retry closure (max 3 retries vs potentially 50 loop iterations)
3. `tools_json` is immutable - pass as `&[serde_json::Value]` too

**Estimated impact**: Reduces allocations from O(iterations) to O(retries_per_iteration), typically 50x -> 1-3x.

### PERF-2: Sequential DB writes in streaming.rs [MEDIUM]

**Location**: `streaming.rs:588-635`

```rust
// Sequential: each await blocks the next
for (idx, te) in tool_executions.iter().enumerate() {
    state.db.create("tool_execution", &execution_id, execution).await  // ~5ms each
}
for rs in &report.metrics.reasoning_steps {
    state.db.create("thinking_step", &step_id, step).await  // ~5ms each
}
```

**Cost**: With 10 tool executions + 5 thinking steps = 15 sequential DB writes = ~75ms serial.

**Recommendation**: Use `futures::join_all()` or `tokio::JoinSet` to parallelize independent DB writes. Since each write has a unique ID and table, there are no ordering constraints.

**Estimated impact**: ~75ms -> ~10ms (bounded by single slowest write).

### PERF-3: Retry closure cloning in manager.rs [LOW]

**Location**: `manager.rs:669-670` (inside `complete_with_tools`)

```rust
with_retry(|| {
    let msgs = messages.clone();  // Full message history clone per retry
    let tls = tools.clone();      // Tool definitions clone per retry
    // ...
})
```

**Cost**: With max 3 retries, worst case is 3 full clones. Since retries are rare (transient errors only), the amortized cost is low.

**Mitigation**: If PERF-1 is fixed (pass references), this becomes a moot point for the outer loop. The retry-specific cloning is an inherent Rust limitation with `Fn()` closures.

### Performance Summary

| ID | Location | Severity | Type | Est. Impact |
|----|----------|----------|------|-------------|
| PERF-1 | llm_agent.rs:1240 | HIGH | Allocation in hot loop | 50x reduction possible |
| PERF-2 | streaming.rs:588-635 | MEDIUM | Sequential I/O | ~7x speedup |
| PERF-3 | manager.rs:669 | LOW | Retry clone | Rare path, low impact |

---

## 4. Implicit State Machine in execute_with_mcp

**Location**: `llm_agent.rs:1186-1423` (~237 lines)

The tool execution loop encodes 5 implicit states:

```
                    +----> [MAX_ITERATIONS] ---> break with warning
                    |
[INIT] ---> [LLM_CALL] ---> [PARSE_RESPONSE]
                ^                   |
                |              tool_calls?
                |             /          \
                |           Yes           No
                |            |             \
                |    [EXECUTE_TOOLS]    [FINISHED] --> break
                |            |
                +------------+

            [LLM_ERROR] --> return Report::failed
```

**Current encoding**: A `loop` with scattered `break` conditions and `return` statements.

**Assessment**: Not problematic at current complexity. The flow is linear and well-commented. However, if more states are added (e.g., thinking/reasoning mode, streaming tokens), an explicit enum-based state machine would be cleaner.

**Recommendation**: No immediate action. Document the states. Consider refactoring if the loop grows beyond ~300 lines or gains new states.

---

## 5. Error Handling Analysis

### 5.1 Consistency

Error handling is **consistent and well-structured**:
- All Tauri commands return `Result<T, String>` (correct pattern)
- `map_err(|e| format!(...))` used throughout (261 instances per SA-007)
- `LLMError` enum is comprehensive with `thiserror::Error` derives
- `is_retryable()` correctly classifies transient vs permanent errors

### 5.2 One Improvement: LLM Error Formatting

**Location**: `llm_agent.rs:916-933` and `llm_agent.rs:1269-1285`

The same LLMError-to-String match block appears twice:

```rust
let error_message = match &e {
    LLMError::ConnectionError(msg) => format!("Connection error: {}...", msg),
    LLMError::ModelNotFound(msg) => format!("Model not found: {}", msg),
    LLMError::MissingApiKey(provider) => format!("API key missing for {}...", provider),
    LLMError::RequestFailed(msg) => format!("Request failed: {}", msg),
    _ => e.to_string(),
};
```

**Recommendation**: Implement `Display` on `LLMError` to produce user-friendly messages directly, or add `LLMError::user_message()` method.

---

## 6. Architecture Assessment

### What's Well-Designed

1. **Agent trait**: Clean, minimal interface with backward-compatible default `execute_with_mcp`
2. **Provider abstraction**: ProviderToolAdapter trait handles format differences elegantly
3. **Retry + Circuit breaker**: Solid resilience pattern, correctly layered in manager.rs
4. **Tool validation**: Human-in-the-loop via ValidationHelper, sub-agent tools correctly exempted
5. **Cancellation**: `CancellationToken` with `tokio::select!` for immediate abort
6. **Metrics collection**: Comprehensive (tokens, tools, reasoning steps, duration)

### What Could Improve

1. **Adapter deduplication** (DUP-3): The ProviderToolAdapter pattern is excellent, but 2 of 3 implementations are identical
2. **Provider dispatch** (DUP-4): The manager repeats the same pattern 3 times. A trait-object approach for providers would eliminate this
3. **Report construction** (DUP-1): Most mechanical fix with highest readability improvement

---

## 7. Recommendations (Priority Order)

### Priority 1: Quick Wins (< 1 hour each)

| # | Action | Impact | Files |
|---|--------|--------|-------|
| R1 | Add `Report::failed()` constructor | -100 lines, consistent errors | agent.rs, llm_agent.rs |
| R2 | Add `LLMError::user_message()` | -40 lines, consistent messages | provider.rs, llm_agent.rs |
| R3 | Add `From<ToolExecutionData>` impls | -40 lines, cleaner mapping | streaming.rs, models |

### Priority 2: Medium Refactors (2-4 hours each)

| # | Action | Impact | Files |
|---|--------|--------|-------|
| R4 | Unify Mistral/OpenAI adapters | -100 lines, single source of truth | adapters/*.rs |
| R5 | Extract `dispatch_with_retry()` in manager | -120 lines, easier to add providers | manager.rs |
| R6 | Parallelize DB writes in streaming.rs | ~7x speedup on persistence | streaming.rs |

### Priority 3: Architectural (4+ hours)

| # | Action | Impact | Files |
|---|--------|--------|-------|
| R7 | Pass messages by reference through complete_with_tools | 50x allocation reduction in tool loop | manager.rs, llm_agent.rs, providers |
| R8 | Merge execute() into execute_with_mcp() | -60 lines, single code path | llm_agent.rs |

---

## 8. Line Counts

| File | Total Lines | Production Code | Tests | Test % |
|------|-------------|-----------------|-------|--------|
| llm_agent.rs | 1713 | 1535 | 178 | 10.4% |
| streaming.rs | 769 | 704 | 65 | 8.5% |
| manager.rs | 1050 | 820 | 230 | 21.9% |
| orchestrator.rs | 475 | 165 | 310 | 65.3% |
| registry.rs | 334 | 140 | 194 | 58.1% |
| retry.rs | 347 | 180 | 167 | 48.1% |
| tool_adapter.rs | 301 | 233 | 68 | 22.6% |
| mistral_adapter.rs | 428 | 214 | 214 | 50.0% |
| ollama_adapter.rs | 421 | 214 | 207 | 49.2% |
| openai_adapter.rs | 290 | 203 | 87 | 30.0% |
| provider.rs | 293 | 202 | 91 | 31.1% |
| agent.rs | 163 | 163 | 0 | 0% |
| simple_agent.rs | 296 | 121 | 175 | 59.1% |
| **Total** | **6880** | **4894** | **1986** | **28.9%** |

---

## Appendix: Detailed Clone Analysis in Tool Loop

For a typical workflow with 20 tool iterations:

```
Iteration  Messages Size  Clone Cost (approx)
1          ~5KB           5KB
2          ~7KB           7KB
3          ~9KB           9KB
...
20         ~43KB          43KB
                          --------
Total clone allocations:  ~480KB (sum of arithmetic series)
```

With `tools_json` (~2KB per tool, 5 tools = ~10KB):
- 20 iterations x 10KB = 200KB additional

**Grand total unnecessary allocations per workflow**: ~680KB
**With R7 fix**: ~30KB (only retry clones, max 3 per iteration)
