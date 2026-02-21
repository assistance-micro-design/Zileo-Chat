# SA-014: Data Persistence & Display After Restart

**Date:** 2026-02-20
**Status:** Remediated
**Scope:** Sub-agent data persistence, activity sidebar display, message enrichment after app restart
**Findings:** 0 CRITICAL, 2 HIGH, 3 MEDIUM, 5 LOW + 3 additional (P11-P13) found during verification

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Architecture Overview](#2-architecture-overview)
3. [Findings Detail](#3-findings-detail)
4. [Impact Analysis](#4-impact-analysis)
5. [Recommendations](#5-recommendations)

---

## 1. Executive Summary

After app restart, sub-agent execution data (internal tool calls, reasoning steps, tokens), tool execution details, and MCP call data no longer display correctly in the activity sidebar and message area. The root cause is a dual pipeline architecture where live streaming data and DB-loaded data follow separate paths with different types, IDs, and conversion logic.

### Severity Distribution

| Severity | Count | Category |
|----------|-------|----------|
| HIGH | 2 | Data never persisted (sub-agent tools + reasoning) |
| MEDIUM | 3 | Display degradation (sidebar, messages, tokens) |
| LOW | 5 | Minor display/schema inconsistencies |

---

## 2. Architecture Overview

### Dual Pipeline

```
LIVE (during execution):
  Backend emit "workflow_stream" -> chunkProcessor -> streaming store -> reactive UI

DB-LOADED (after restart):
  loadHistorical() -> ActivityService.loadAll() -> conversion functions -> activity store -> UI
```

### Data Flow for Sub-Agents

| Stage | What Happens | Data Lost? |
|-------|-------------|------------|
| Sub-agent executes | `Report` produced with full `tool_executions` + `reasoning_steps` | No |
| `execute_with_heartbeat_timeout` | Extracts only `content` + 3 scalar metrics | **YES** - tools + reasoning dropped |
| `update_execution_record` | Saves status, duration, tokens, result_summary (200 chars) | Partial |
| Frontend streaming | Full live data displayed via `ActiveSubAgent` type | No |
| Frontend after restart | Loads `SubAgentExecution` from DB (incomplete) | Display degraded |

---

## 3. Findings Detail

### P1 (HIGH): Sub-agent internal tool executions never persisted

**Location:** `src-tauri/src/tools/sub_agent_executor.rs:935-957`

**Root cause:** `execute_with_heartbeat_timeout` extracts only `report.content` and 3 scalar metrics (`tokens_input`, `tokens_output`, `duration_ms`) from the `Report` struct. `report.metrics.tool_executions` (a `Vec<ToolExecutionData>`) is silently dropped.

**Evidence:**
```rust
// Only these fields are extracted:
ExecutionResult {
    success: true,
    report: report.content,           // content only
    metrics: SubAgentMetrics {
        duration_ms,
        tokens_input: report.metrics.tokens_input as u64,
        tokens_output: report.metrics.tokens_output as u64,
    },
    // report.metrics.tool_executions -> DROPPED
    // report.metrics.reasoning_steps -> DROPPED
}
```

**Impact:** After restart, there is no record of which tools (including MCP) each sub-agent used, their inputs/outputs, or success/failure. This data is permanently lost.

**Key insight:** The `tool_execution` DB table already has an `agent_id` field with index. The `persist_tool_executions_batch` function already exists in `streaming.rs`. The infrastructure is ready but unused for sub-agents.

---

### P2 (HIGH): Sub-agent internal reasoning steps never persisted

**Location:** `src-tauri/src/tools/sub_agent_executor.rs:935-957`

**Root cause:** Same as P1. `report.metrics.reasoning_steps` (a `Vec<ReasoningStepData>`) is dropped in `execute_with_heartbeat_timeout`.

**Impact:** Sub-agent thinking/reasoning chains are permanently lost after the streaming session ends.

**Key insight:** Same as P1 - `thinking_step` table has `agent_id` field and `persist_reasoning_steps_batch` exists.

---

### P3 (MEDIUM): Sub-agent tokens not aggregated in workflow totals

**Location:** `src-tauri/src/commands/streaming.rs` - `update_workflow_cumulative_metrics`

**Root cause:** `update_workflow_cumulative_metrics` only receives the primary agent's token counts. Sub-agent tokens are stored individually in `sub_agent_execution.tokens_input/tokens_output` but never summed into `workflow.total_tokens_input/total_tokens_output`.

**Impact:** Workflow token totals are inaccurate for multi-agent workflows. Cost calculations underreport actual usage.

---

### P4 (MEDIUM): SubAgentActivity component only accepts live data

**Location:** `src/lib/components/workflow/SubAgentActivity.svelte:50-58`

**Root cause:** Component's `Props` interface only accepts `ActiveSubAgent[]` (the live streaming type). After restart, the streaming store is reset (empty array), so the component shows nothing.

```typescript
interface Props {
    subAgents?: ActiveSubAgent[];  // streaming-only type
    isStreaming?: boolean;
}
```

**Impact:** The standalone sub-agent panel is blank after restart. Historical sub-agent data IS loaded via `activityStore.loadHistorical` but only flows through `ActivityFeed` items, not this dedicated panel.

---

### P5 (MEDIUM): Message sub-agent chips use fragile timestamp correlation

**Location:** `src/lib/services/message.service.ts:54-90`

**Root cause:** `message.sub_agents` is explicitly transient (comment: "transient, captured from StreamingState"). After restart, `enrichMessagesWithSubAgents()` re-attaches sub-agent summaries by comparing timestamps:

```typescript
const execDate = new Date(exec.completed_at ?? exec.created_at);
const target = assistantMessages.find((m) => new Date(m.timestamp) >= execDate);
```

**Failure modes:**
- If `completed_at` is null, falls back to `created_at` (start time), attaching to wrong message
- If multiple sub-agents complete near the same timestamp, assignment is non-deterministic
- No deduplication guard against double-assignment from overlapping executions

---

### P6 (LOW): `parent_execution_id` silently dropped by SCHEMAFULL

**Location:** `src-tauri/src/db/schema.rs:335-357` vs `src-tauri/src/models/sub_agent.rs:107`

**Root cause:** ERR_SURREAL_001. The `SubAgentExecution` Rust model has `parent_execution_id: Option<String>`, and `SubAgentExecutionCreate` sends it. But the `sub_agent_execution` table schema does not define this field. Since the table is SCHEMAFULL, SurrealDB silently drops it on every write.

**Impact:** Batch sub-agent correlation (OPT-SA-11) does not actually work. The field is populated by `ParallelTasksTool` but never persists.

---

### P7 (LOW): Undefined task_description produces display garbage

**Location:** `src/lib/utils/activity.ts` - `subAgentExecutionToActivity`

**Root cause:**
```typescript
description: exec.task_description?.slice(0, 200) +
    (exec.task_description?.length > 200 ? '...' : ''),
```

If `exec.task_description` is `undefined`, this evaluates to `undefined + '' = "undefined"`.

---

### P8 (LOW): Activity IDs differ between live and historical

**Location:** `src/lib/utils/activity.ts`

**Root cause:** Live sub-agent activities get IDs like `agent-${agent.id}-${index}`, while DB-loaded ones get `agent-hist-${exec.id}-${index}`. The dedup Set in `allActivities` cannot merge them, causing potential duplicates during the transition from live to historical.

---

### P9 (LOW): `activeToolToActivity` never sets `executionId`

**Location:** `src/lib/utils/activity.ts:41-57`

**Root cause:** During live streaming, `activeToolToActivity` does not set `metadata.executionId`. The expand button in `ActivityItem.svelte` (line 75-78) requires `executionId` to show. So tool input/output details are unavailable during live execution.

**Note:** This is the inverse of the restart problem - expand works after restart (when `toolExecutionToActivity` sets `executionId`) but not during live execution.

---

### P10 (LOW): Cancelled sub-agents shown as "error" after restart

**Location:** `src/lib/utils/activity.ts` - `subAgentExecutionToActivity`

**Root cause:**
```typescript
case 'cancelled':
    type = 'sub_agent_error';
    status = 'error';
```

Cancelled sub-agents are mapped to `sub_agent_error` type and `error` status, making them appear with red error icons in the activity feed.

---

## 4. Impact Analysis

### User Experience

| Scenario | During Execution | After Restart |
|----------|-----------------|---------------|
| Sub-agent tools in sidebar | Visible (streaming) | **Missing** (P1) |
| Sub-agent reasoning | Visible (streaming) | **Missing** (P2) |
| Sub-agent chips on messages | Visible (transient) | **Fragile** (P5) |
| Workflow token totals | Primary only | Primary only (P3) |
| Tool expand/details | **No expand button** (P9) | Expand works |
| Sub-agent panel | Shows live data | **Empty** (P4) |

### Data Loss

| Data | Persisted? | Recoverable? |
|------|-----------|--------------|
| Sub-agent tool calls (incl. MCP) | No | No - permanently lost |
| Sub-agent reasoning | No | No - permanently lost |
| Sub-agent tokens | Partial (individual, not aggregated) | Yes - can recalculate |
| Parent execution correlation | No (ERR_SURREAL_001) | Yes - schema fix |

---

## 5. Recommendations

### Priority 1: Persist sub-agent internal data (P1, P2)

Extend `ExecutionResult` to carry `tool_executions` and `reasoning_steps`. Extract persist functions from `streaming.rs` to a shared module. Call them from `SubAgentExecutor` with the sub-agent's `agent_id`.

### Priority 2: Token aggregation + schema fix (P3, P6)

Aggregate sub-agent tokens into workflow totals. Add `parent_execution_id` to DB schema.

### Priority 3: Frontend display fixes (P4, P5, P7, P8, P10)

Fix activity conversion functions, harmonize IDs, improve message enrichment correlation.

### P11 (HIGH): update_execution_record uses format!() causing silent SQL failures

**Location:** `src-tauri/src/tools/sub_agent_executor.rs:599-640`

**Root cause:** `update_execution_record()` uses `format!()` to construct the UPDATE query, embedding the `result_summary` (LLM-generated text) directly into the SQL string. When the text contains characters that confuse the SurrealQL parser (quotes, backslashes, special chars), the query silently fails. `db.execute()` only checks transport-level errors, not SQL statement errors. The record retains its initial CREATE values: `status = "running"`, `duration_ms = null`, `tokens_input = null`.

**Evidence:**
- After restart, sub-agents show blue/spinning (status "running") instead of green (completed)
- Duration displays as "nullms" (`formatDuration(null)` passes through JS null coercion)
- Token counts are missing (null in DB)

**Impact:** Sub-agent completion data is silently lost. All metrics (status, duration, tokens) remain at their initial values from the CREATE record.

---

### P12 (LOW): "failed" status doesn't match SubAgentStatus enum

**Location:** `src-tauri/src/tools/sub_agent_executor.rs:603`

**Root cause:** `update_execution_record()` writes `status = "failed"` for unsuccessful executions, but `SubAgentStatus` enum (with `serde(rename_all = "snake_case")`) has variant `Error` which serializes as `"error"`. The value `"failed"` doesn't match any variant, causing deserialization failures when loading.

---

### P13 (MEDIUM): Message enrichment dedup uses name instead of ID

**Location:** `src/lib/services/message.service.ts:87`

**Root cause:** `enrichMessagesWithSubAgents()` uses `s.name === summary.name` for dedup. When two sub-agents share the same name (e.g., parallel tasks with same agent), the second is skipped. Also, the first sub-agent's chip may show mismatched tokens due to incorrect timestamp correlation.

**Evidence:** User reports "when the same sub-agent appears twice, the first shows the second's tokens."

---

## 6. Remediation Status

| ID | Severity | Status | Fix Summary |
|----|----------|--------|-------------|
| P1 | HIGH | DONE | Extended `ExecutionResult` with `tool_executions` + `reasoning_steps`; extracted `persist_tool_executions`/`persist_reasoning_steps` to `db/persistence.rs`; called from all 3 tool paths (spawn, delegate, parallel) |
| P2 | HIGH | DONE | Same as P1 - reasoning steps now persisted via `persist_sub_agent_internals()` |
| P3 | MEDIUM | DONE | Added `aggregate_sub_agent_tokens()` in `streaming.rs`. **Revised**: sub-agent tokens now stored in separate fields (`sub_agent_tokens_input/output`) instead of being added to `total_tokens_*`. Frontend TokenDisplay shows AGENT (main only) + TOTAL (main+sub-agents) sections. Context gauge uses cumulative main agent tokens. |
| P4 | MEDIUM | DOCUMENTED | `SubAgentActivity.svelte` is exported but never used. Sub-agents display via unified `ActivityFeed`/`ActivityItem` pipeline which already handles both live and historical data |
| P5 | MEDIUM | DONE | Added `cancelled` status to enrichment filter; map cancelled to `completed` for display |
| P6 | LOW | DONE | Added `DEFINE FIELD OVERWRITE parent_execution_id ON sub_agent_execution TYPE option<string>` to schema |
| P7 | LOW | DONE | Guarded `task_description` against undefined; added `agentId` to `toolExecutionToActivity` metadata |
| P8 | LOW | DOCUMENTED | ID mismatch between live/historical is by design (no DB ID during streaming); dedup works within each pipeline |
| P9 | LOW | DOCUMENTED | `executionId` intentionally absent during live streaming (no DB record yet); works after capture |
| P10 | LOW | DONE | Cancelled sub-agents now map to `sub_agent_complete`/`completed` instead of `error` |
| P11 | HIGH | DONE | Replaced `format!()` with parameterized queries in `update_execution_record()`; increased result_summary truncation to 5000 chars. `delegate_task.rs` now reuses `update_execution_record()` instead of duplicated logic |
| P12 | LOW | DONE | Changed `"failed"` to `"error"` in `update_execution_record()` to match `SubAgentStatus` enum |
| P13 | MEDIUM | DONE | Added `id` field to `SubAgentSummary`; enrichment dedup now uses execution ID instead of name |

### Files Modified

| File | Changes |
|------|---------|
| `src-tauri/src/tools/sub_agent_executor.rs` | Extended `ExecutionResult` + `persist_sub_agent_internals()` + P11/P12: parameterized `update_execution_record()` |
| `src-tauri/src/db/persistence.rs` | NEW - Shared persistence module |
| `src-tauri/src/db/mod.rs` | Added persistence module |
| `src-tauri/src/commands/streaming.rs` | Replaced local persist fns with shared module + `aggregate_sub_agent_tokens()` |
| `src-tauri/src/tools/spawn_agent.rs` | Call `persist_sub_agent_internals` |
| `src-tauri/src/tools/parallel_tasks.rs` | Call `persist_sub_agent_internals` + fix `ExecutionResult` construction |
| `src-tauri/src/tools/delegate_task.rs` | Call `persist_sub_agent_internals` + P11: reuse `executor.update_execution_record()` |
| `src-tauri/src/db/schema.rs` | Added `parent_execution_id` field + P3: `sub_agent_tokens_input/output` fields |
| `src-tauri/src/db/queries.rs` | P3: Added `sub_agent_tokens_input/output` + `current_context_tokens` to SELECT_BASE/SELECT_LIST |
| `src-tauri/src/models/workflow.rs` | P3: Added `sub_agent_tokens_input/output` fields with `#[serde(default)]` |
| `src/lib/stores/tokens.ts` | P3: Added `subAgent` state, `updateFromWorkflow` loads sub-agent tokens, derived computes `workflow_total_cost` |
| `src/lib/components/workflow/TokenDisplay.svelte` | P3: AGENT/TOTAL sections, context gauge uses cumulative, cost on total |
| `src/types/workflow.ts` | P3: Added `sub_agent_tokens_*` to Workflow, `sub_agent_input/output` + `workflow_total_cost` to TokenDisplayData |
| `src/messages/en.json` + `fr.json` | P3: Added `workflow_token_agent` i18n key |
| `src/lib/stores/__tests__/workflows.test.ts` | P3: Updated Workflow mock with new fields |
| `src/lib/utils/activity.ts` | P7 (undefined guard), P9 (doc), P10 (cancelled mapping), P7 (agentId) |
| `src/lib/services/message.service.ts` | P5 (cancelled enrichment), P10 (cancelled as completed), P13 (ID-based dedup) |
| `src/lib/services/workflowExecutor.service.ts` | P13: Add `id` to live SubAgentSummary creation |
| `src/lib/utils/__tests__/activity.test.ts` | 8 new tests (P7, P10, agentId attribution) |
| `src/types/message.ts` | P13: Added `id` field to `SubAgentSummary` interface |
| `src/lib/utils/duration.ts` | P11: Handle `null` in `formatDuration()` |

### Tests Added

| File | Test | Purpose |
|------|------|---------|
| `sub_agent_executor.rs` | `test_execution_result_preserves_tool_executions` | P1: tool_executions on ExecutionResult |
| `sub_agent_executor.rs` | `test_execution_result_preserves_reasoning_steps` | P2: reasoning_steps on ExecutionResult |
| `activity.test.ts` | `SA-014 P7: handles undefined task_description` | P7: no "undefined..." |
| `activity.test.ts` | `SA-014 P7: truncates long task_description` | P7: 200 char limit |
| `activity.test.ts` | `SA-014 P7: preserves short task_description` | P7: no data loss |
| `activity.test.ts` | `SA-014 P10: cancelled not error` | P10: cancelled mapping |
| `activity.test.ts` | `SA-014 P7: includes agentId` | P7: tool attribution |
| `activity.test.ts` | `maps error status correctly` | Regression: error still works |
| `activity.test.ts` | `preserves token metrics in metadata` | Regression: tokens |
| `activity.test.ts` | `preserves agent identity in metadata` | Regression: identity |
