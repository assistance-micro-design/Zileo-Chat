# SA-013: Tools + Types Coherence Audit (TS / Rust)

**Date**: 2026-02-19
**Status**: Documented
**Scope**: 179 TS types, 122 Rust types, 60+ type pairs, ~250 fields, tools registry architecture

## Executive Summary

| Metric | Value |
|--------|-------|
| Types compared | 60+ type pairs across 9 domains |
| Fields analyzed | ~250 |
| CRITICAL issues | 4 |
| HIGH issues | 2 |
| MEDIUM issues | 5 |
| LOW / INFO | 2 |
| Convention violations (TS) | 28 (`console.*` only) |
| Tools architecture | Clean |

**Nullability convention reference**:

| Rust | TypeScript | JSON behavior |
|------|-----------|---------------|
| `Option<T>` + `skip_serializing_if` | `field?: T` | Absent when None |
| `Option<T>` (without skip) | `field: T \| null` | Explicit `null` |
| `T` (required) | `field: T` | Always present |
| `T` + `#[serde(default)]` | `field: T` (required) | Always present in output; default on deserialization only |

---

## 1. Coherence Matrix by Domain

### 1.1 AGENT Domain - 4 issues

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| Lifecycle (enum) | - | OK | - |
| Agent | 7 | OK | - |
| AgentConfig | 8 | **2 MISMATCHES** | `max_tool_iterations`, `enable_thinking` |
| LLMConfig | 4 | OK | - |
| AgentConfigCreate | 8 | **2 MISMATCHES** | Same fields |
| AgentConfigUpdate | 7 | OK | - |
| AgentSummary | 7 | OK | - |

#### CRITICAL: `serde(default)` fields marked optional in TS

| # | Field | Rust | TS | Impact |
|---|-------|------|----|--------|
| 1 | `AgentConfig.max_tool_iterations` | `usize` + `#[serde(default = "default_max_tool_iterations")]` | `number?` | TS treats as optional what Rust always sends |
| 2 | `AgentConfig.enable_thinking` | `bool` + `#[serde(default = "default_enable_thinking")]` | `boolean?` | Same |
| 3 | `AgentConfigCreate.max_tool_iterations` | Same as above | `number?` | Same |
| 4 | `AgentConfigCreate.enable_thinking` | Same as above | `boolean?` | Same |

**Root cause**: `#[serde(default)]` provides a fallback for *deserialization* only. The field is always present in serialized output. TS should declare it as required.

**Fix**: Change TS from `field?: T` to `field: T`:
```typescript
// Before (wrong)
max_tool_iterations?: number;
enable_thinking?: boolean;

// After (correct)
max_tool_iterations: number;
enable_thinking: boolean;
```

---

### 1.2 WORKFLOW Domain - 1 issue

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| WorkflowStatus (enum) | - | OK | - |
| Workflow | 10 | **1 CONVENTION** | `model_id` |
| WorkflowToolExecution | 8 | OK | `input_params`/`output_result` use flexible `Value` vs typed TS interfaces (acceptable) |
| WorkflowResult | 6 | OK | - |
| WorkflowMetrics | 6 | OK | - |
| WorkflowFullState | 4 | OK | - |

#### MEDIUM: `Workflow.model_id` convention inconsistency

| # | Field | Rust | TS | Impact |
|---|-------|------|----|--------|
| 5 | `Workflow.model_id` | `Option<String>` + `#[serde(default)]` (no skip) | `string?` (optional) | Rust serializes as `null`, but TS declares optional (expects absent). Should be `string \| null` or add `skip_serializing_if`. |

**Fix options**:
- A) Add `#[serde(skip_serializing_if = "Option::is_none")]` to Rust (keeps TS `string?`)
- B) Change TS to `model_id: string | null` (keeps Rust as-is)

---

### 1.3 MESSAGE Domain - 3 issues

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| MessageRole (enum) | - | OK | `#[serde(rename_all = "snake_case")]` matches TS string literals |
| Message | 13 | OK | `sub_agents` is intentionally TS-only (transient, documented) |
| MessageCreate | 10 | **3 issues** | Missing field, type mismatch |
| PaginatedMessages | 5 | OK | - |

#### Findings

| # | Severity | Field | Issue |
|---|----------|-------|-------|
| 6 | **HIGH** | `MessageCreate.tokens` | Rust has required `tokens: usize`, TS `MessageCreate` doesn't include it. Backend must handle its absence. |
| 7 | **MEDIUM** | `MessageCreate.role` | Rust uses `String`, TS uses `MessageRole` enum. Rust loses type safety at deserialization. |
| 8 | INFO | `Message.sub_agents` | TS-only transient field for streaming state. Documented in source comments. Intentional. |

---

### 1.4 MEMORY Domain - 3 issues

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| MemoryType (enum) | - | OK | `#[serde(rename_all = "snake_case")]` matches |
| Memory | 8 | OK | `memory_type` renamed via `#[serde(rename = "type")]` |
| MemorySearchResult | 2 | OK | - |
| MemoryDescribeResult | 7 | OK | - |
| CreateMemoryParams (TS) / MemoryCreate (Rust) | 6 | **3 issues** | API surface mismatch |
| MemoryWithEmbedding | - | Rust-only | Intentional (internal RAG) |

#### Findings

| # | Severity | Field | Issue |
|---|----------|-------|-------|
| 9 | **MEDIUM** | `CreateMemoryParams.metadata` | TS optional (`metadata?: MemoryMetadata`), Rust required (`metadata: Value`). Backend receives `undefined` if TS omits it. |
| 10 | LOW | `CreateMemoryParams` | Missing `importance`, `expires_at` fields. Backend-only creation fields, not exposed to frontend API. |
| 11 | LOW | `MemoryCreate.memory_type` | Rust uses `String` not `MemoryType` enum. Intentional for SurrealDB compatibility. |

---

### 1.5 MCP Domain - OK

| Type Pair | Fields | Status |
|-----------|--------|--------|
| MCPDeploymentMethod (enum) | - | OK |
| MCPServerStatus (enum) | - | OK |
| MCPServerConfig | 7 | OK |
| MCPServer (extends config) | 5+ | OK (uses `#[serde(flatten)]`) |
| MCPTool | 3 | OK |
| MCPResource | 4 | OK |
| MCPTestResult | 5 | OK |
| MCPToolCallRequest | 3 | OK |
| MCPToolCallResult | 4 | OK |

No issues. Well-synchronized domain.

---

### 1.6 LLM Domain - 1 issue

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| ProviderType (enum) | - | OK | Custom serialize for extensible provider names |
| LLMModel | 14 | OK | - |
| CreateModelRequest | 9 | OK | - |
| UpdateModelRequest | 8 | OK | - |
| ProviderSettings | 6 | **1 MISMATCH** | `base_url` |
| ConnectionTestResult | 5 | OK | No `skip_serializing_if` = null serialized = `T \| null` correct |

#### HIGH: `ProviderSettings.base_url` nullability mismatch

| # | Field | Rust | TS | Impact |
|---|-------|------|----|--------|
| 12 | `ProviderSettings.base_url` | `Option<String>` + `skip_serializing_if` = absent when None | `string \| null` | TS expects explicit `null`, Rust omits field. TS receives `undefined` instead of `null`. |

Note: `default_model_id` has `#[serde(default)]` but NO `skip_serializing_if` - it serializes as `null` correctly. `ConnectionTestResult` fields also have no skip - all `T | null` declarations are correct.

**Fix**: Remove `skip_serializing_if` from `base_url` in Rust, or change TS to `base_url?: string`.

---

### 1.7 CUSTOM PROVIDER Domain - OK

| Type Pair | Fields | Status |
|-----------|--------|--------|
| ProviderInfo | 8 | OK (`#[serde(rename_all = "camelCase")]` handles naming) |

`ProviderInfo.base_url`: `Option<String>` WITHOUT skip = serializes as `null` = TS `string | null` correct.

---

### 1.8 VALIDATION Domain - 1 critical issue

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| ValidationMode (enum) | - | OK | |
| ValidationType (enum) | - | OK | |
| **RiskLevel (enum)** | - | **CRITICAL** | TS has `'critical'`, Rust doesn't |
| ValidationStatus (enum) | - | OK | |
| ValidationRequest | 7 | OK | `details` as `Value` is intentional |
| ValidationSettings | 7 | OK | Uses `#[serde(rename_all = "camelCase")]` |
| SelectiveValidationConfig | 5 | OK | camelCase convention |
| RiskThresholdConfig | 2 | OK | camelCase convention |

#### CRITICAL: RiskLevel enum mismatch

| # | Field | Rust | TS | Impact |
|---|-------|------|----|--------|
| 13 | `RiskLevel` | `Low, Medium, High` (3 variants) | `'low' \| 'medium' \| 'high' \| 'critical'` (4 variants) | If TS sends `'critical'`, Rust deserialization panics with `unknown variant "critical"` |

**Fix**: Add `Critical` variant to Rust `RiskLevel` enum.

---

### 1.9 STREAMING Domain - 2 critical issues

| Type Pair | Fields | Status | Issues |
|-----------|--------|--------|--------|
| **ChunkType (enum)** | - | **CRITICAL** | TS has 14 variants, Rust has 12 |
| StreamChunk | 18 | **1 orphan field** | `user_question` |
| SubAgentStreamMetrics | 3 | OK | |
| WorkflowComplete | 3 | OK | |

#### CRITICAL: ChunkType and StreamChunk mismatches

| # | Field | Rust | TS | Impact |
|---|-------|------|----|--------|
| 14 | `ChunkType` | 12 variants | 14 variants: adds `user_question_start`, `user_question_complete` | TS defines chunk types that Rust never emits, OR user questions are handled via a different event mechanism |
| 15 | `StreamChunk.user_question` | Absent | `UserQuestionStreamPayload?` | TS field with no Rust counterpart |

**Investigation needed**: Verify whether user questions are emitted via the `StreamChunk` mechanism or via a separate Tauri event (`validation_required`). If separate, remove the orphan TS variants and field. If intended for future use, add to Rust.

---

### 1.10 TASK Domain - 1 medium issue

| Type Pair | Fields | Status |
|-----------|--------|--------|
| TaskStatus (enum) | - | OK |
| Task | 12 | OK |
| **TaskPriority** | - | **MEDIUM**: TS `1\|2\|3\|4\|5`, Rust `u8` (no constraint) |

---

### 1.11 PROMPT Domain - OK

All types perfectly synchronized. No issues.

---

## 2. Tools Registry Architecture

### Architecture Overview

```
AGENT EXECUTION LAYER
  (agents/core/orchestrator.rs, agents/llm_agent.rs)
       |
       | Creates tools via factory
       v
TOOL FACTORY LAYER (tools/factory.rs)
  - Instantiates tools by name
  - Enforces primary-agent-only for sub-agent tools
  - Auto-adds sub-agent tools to primary agents
       |
       +-- BASIC TOOLS (no context)     SUB-AGENT TOOLS (with context)
       |   - MemoryTool                  - SpawnAgentTool
       |   - TodoTool                    - DelegateTaskTool
       |   - CalculatorTool              - ParallelTasksTool
       |   - UserQuestionTool
       |
       v
TOOL EXECUTION CORE (mod.rs Tool trait)
  - Input validation (fail fast)
  - Operation dispatch (match on operation field)
  - Error handling (ToolError enum, 8 variants)
       |
       v
PERSISTENCE & VALIDATION
  - tool_execution table (SCHEMAFULL)
  - JSON string serialization for input_params/output_result
  - ValidationHelper (human-in-the-loop)
```

### Key Design Decisions

| Aspect | Implementation | Status |
|--------|---------------|--------|
| Tool interface | `Tool` trait: `execute(Value) -> ToolResult<Value>` | Clean |
| Registration | Static registry in `ToolRegistry` (hardcoded) | OK for current scale |
| Factory | `ToolFactory` with shared deps (`Arc<DBClient>`, embedding service) | Clean |
| Sub-agent enforcement | Factory rejects sub-agent tools when `is_primary_agent == false` | Correct |
| Serialization (DB) | `input_params`/`output_result` as JSON **strings** | ERR_SURREAL_001 mitigated |
| Backward compat | Deserializer handles both JSON string and legacy object formats | PAT_SERDE_001 applied |
| Validation | Pre-execution `validate_input()` + optional human-in-the-loop | Complete |
| Resilience | Circuit breaker + cancellation token for sub-agents | OPT-SA-7/8 |

### ERR_SURREAL_001 Mitigation Verification

The tools system correctly stores dynamic JSON as **strings** in SCHEMAFULL tables:

```sql
-- Schema
DEFINE FIELD input_params ON tool_execution TYPE string;
DEFINE FIELD output_result ON tool_execution TYPE option<string>;
```

```rust
// Rust model
#[serde(serialize_with = "serialize_as_json_string")]
#[serde(deserialize_with = "deserialize_json_string")]
pub input_params: serde_json::Value,
```

No ERR_SURREAL_001 risk. Dynamic keys are preserved.

### Tool Constants Reference

| Tool | Key Constants |
|------|--------------|
| Memory | MAX_CONTENT: 50k chars, similarity threshold: 0.7, weights: cosine 70% / importance 15% / recency 15% |
| Todo | MAX_NAME: 128, MAX_DESC: 1k, priority: 1-5 |
| Sub-Agent | MAX_SUB_AGENTS: 3, inactivity timeout: 300s, circuit breaker threshold: 3, cooldown: 60s |
| Calculator | 22 unary ops, 11 binary ops, constants (pi, e, tau, sqrt2) |

---

## 3. Convention Violations (TypeScript)

| Category | Count | Severity | Status |
|----------|-------|----------|--------|
| `field?: T \| null` (forbidden) | **0** | - | CLEAN |
| `any` type | **0** | - | CLEAN |
| `as any` casts | **0** | - | CLEAN |
| `// @ts-ignore` | **0** | - | CLEAN |
| Wrong imports (`$lib/types/`) | **0** | - | CLEAN |
| `console.*` in production | **28** | LOW | See below |

### `console.*` Breakdown

| Location | Count | Examples |
|----------|-------|---------|
| Services (error handlers) | 11 | `message.service.ts`, `activity.service.ts`, `localStorage.service.ts` |
| Routes (page-level) | 8 | `agent/+page.svelte`, `settings/agents/+page.svelte` |
| Components (UI handlers) | 9 | `AgentForm.svelte`, `PromptSettings.svelte`, `UserQuestionModal.svelte` |

All are `console.error` or `console.warn` in catch blocks. No `console.log` debug statements found.

---

## 4. Naming Convention Split

Two serde naming conventions coexist in the codebase:

| Convention | Domains | TS field names | Rust attribute |
|------------|---------|----------------|----------------|
| **snake_case** (default) | agent, workflow, message, memory, mcp, llm, streaming, task, prompt | `snake_case` | None (serde default) |
| **camelCase** | custom_provider, validation, import_export, user_question | `camelCase` | `#[serde(rename_all = "camelCase")]` |

Both sides correctly follow their respective conventions. This split is consistent but undocumented.

---

## 5. Orphan Types

### Rust-only (intentional internal types)

| Type | File | Purpose |
|------|------|---------|
| `WorkflowCreate` | workflow.rs | DB creation struct |
| `MemoryCreate`, `MemoryCreateWithEmbedding`, `MemoryWithEmbedding` | memory.rs | Backend creation / RAG internals |
| `MCPCallLog`, `MCPCallLogCreate`, `MCPServerCreate` | mcp.rs | DB audit / creation |
| `CustomProvider` | custom_provider.rs | DB entity (ProviderInfo exposed to frontend) |
| `TaskCreate`, `TaskUpdate` | task.rs | DB creation / update |
| `ValidationRequestCreate` | validation.rs | DB creation |
| `PartialSelectiveConfig`, `PartialRiskThresholds`, `PartialAuditConfig` | validation.rs | Partial update helpers |
| `CompletionStatus`, `SubAgentOperationType` | streaming.rs | Internal event types |
| `ValidationRequiredEvent`, `ValidationResponseEvent` | streaming.rs | Internal event types |
| `BuiltinModelParams` | llm_models.rs | Model seed data |

### TypeScript-only (frontend utilities)

| Type | File | Purpose |
|------|------|---------|
| `SubAgentSummary` | message.ts | Transient streaming state (documented) |
| `PromptStoreState`, `PROMPT_CATEGORY_LABELS` | prompt.ts | Frontend state / UI constants |
| `PromptPreviewParams`, `PromptPreviewResult` | prompt.ts | Preview utility types |
| `CreateTaskParams`, `UpdateTaskParams`, `CompleteTaskParams` | task.ts | IPC parameter types |
| `TaskListResult`, `ListTasksByStatusParams` | task.ts | Query utilities |
| `LLMResponse`, `ProviderStatus`, `LLMState` | llm.ts | Frontend state types |
| `UserQuestionStreamPayload` | streaming.ts | Possibly orphan (see issue #14-15) |

---

## 6. Consolidated Issues

| # | Severity | Domain | Issue | Fix |
|---|----------|--------|-------|-----|
| 1-4 | **CRITICAL** | Agent | `max_tool_iterations` & `enable_thinking` optional in TS but always present in Rust output (`serde(default)` != optional) | Make required in TS |
| 13 | **CRITICAL** | Validation | `RiskLevel` missing `Critical` variant in Rust | Add variant to Rust enum |
| 14-15 | **CRITICAL** | Streaming | `ChunkType` has 2 extra TS variants + orphan `user_question` field | Investigate and align |
| 6 | **HIGH** | Message | `MessageCreate` missing `tokens` field in TS | Add `tokens: number` |
| 12 | **HIGH** | LLM | `ProviderSettings.base_url` has `skip_serializing_if` but TS expects `null` | Remove skip or change TS to `base_url?: string` |
| 5 | **MEDIUM** | Workflow | `Workflow.model_id` convention inconsistency (no skip but TS optional) | Add skip or change TS to `string \| null` |
| 7 | **MEDIUM** | Message | `MessageCreate.role` is `String` in Rust, not enum | Use `MessageRole` enum |
| 9 | **MEDIUM** | Memory | `CreateMemoryParams.metadata` optional in TS, required in Rust | Align nullability |
| 16 | **MEDIUM** | Task | `TaskPriority` unbounded `u8` vs TS literal `1\|2\|3\|4\|5` | Add Rust validation |
| 17 | **MEDIUM** | Streaming | `task_status`/`task_priority` untyped in Rust (`String`/`u8`) | Use proper enums |
| - | **LOW** | Global | 28 `console.*` in production code | Replace with logging utility |
| - | **LOW** | Global | Dual naming convention (snake_case / camelCase) undocumented | Document in CLAUDE.md |

---

## 7. Methodology

- 5 parallel research agents covering: Agent+Workflow, Message+Memory, MCP+Provider+LLM+Validation+Streaming+Task+Prompt, Tools architecture, Convention violations
- Manual verification of critical findings (ProviderSettings/ConnectionTestResult skip attributes)
- Bias check via thinking-mcp to prevent premature "clean" declarations
- Sub-agent errors corrected: 4 false positives in LLM/CustomProvider domain (fields without `skip_serializing_if` were incorrectly flagged as mismatches)

---

## Code Verification (2026-02-19)

**Methodology**: 4 exploration agents read the actual code. thinking-mcp bias checks applied to distinguish real security risks from type inconveniences.

### Severity Adjustments

| Finding | Original | Adjusted | Justification |
|---------|----------|----------|---------------|
| #1-4 | CRITICAL | **ADJUSTED HIGH** | `serde(default)` fields (`max_tool_iterations`, `enable_thinking`) are always present in Rust serialized output. TS declaring them as optional (`?`) is a type bug, not a security vulnerability. The value IS sent, TS just doesn't require it in the interface. No crash, no data loss, no exploitation path. HIGH because it's a real type mismatch that could cause bugs if TS code checks `if (config.max_tool_iterations)` on an optional field that's actually always present. |
| #13 | CRITICAL | **CONFIRMED CRITICAL** | `RiskLevel` missing `Critical` variant in Rust. If TS sends `'critical'`, Rust deserialization fails with `unknown variant` error. This causes an app crash (deserialization panic). Verified: TS type includes `'critical'`, Rust enum only has `Low, Medium, High`. |
| #14-15 | CRITICAL | **ADJUSTED MEDIUM** | `ChunkType` has 2 extra TS variants (`user_question_start`, `user_question_complete`) + orphan `user_question` field. Investigation shows user questions are handled via a separate Tauri event mechanism (`validation_required`), not via StreamChunk. These are orphan TS definitions that never match incoming data. No crash (unmatched variants are simply never received), no data loss. MEDIUM because orphan types indicate incomplete cleanup and could confuse future developers. |
| #6 | HIGH | **CONFIRMED HIGH** | `MessageCreate.tokens` missing in TS but required in Rust. Real type mismatch. |
| #12 | HIGH | **CONFIRMED HIGH** | `ProviderSettings.base_url` has `skip_serializing_if` but TS expects `null`. Real nullability mismatch. |
| #5 | MEDIUM | **CONFIRMED MEDIUM** | Convention inconsistency. |
| #7 | MEDIUM | **CONFIRMED MEDIUM** | `role` as String not enum in Rust. |
| #9 | MEDIUM | **CONFIRMED MEDIUM** | `metadata` optional in TS, required in Rust. |
| #16 | MEDIUM | **CONFIRMED MEDIUM** | `TaskPriority` unbounded. |
| #17 | MEDIUM | **CONFIRMED MEDIUM** | Untyped fields in streaming. |

### Summary After Verification

| Severity | Original Count | Adjusted Count | Change |
|----------|---------------|----------------|--------|
| CRITICAL | 4 | 1 | -3 (#1-4->HIGH, #14-15->MEDIUM) |
| HIGH | 2 | 4 | +2 (#1-4 as group moved here) |
| MEDIUM | 5 | 7 | +2 (#14-15 moved here) |
| LOW | 2 | 2 | No change |
