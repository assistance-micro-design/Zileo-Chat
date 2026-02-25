# SA-021: Agent Report Enforcement

**Date**: 2026-02-25
**Status**: DONE
**Scope**: `src-tauri/src/agents/llm_agent.rs`

## Problem

When an agent or sub-agent finishes a workflow's tool execution loop without providing a meaningful text response (empty or null content from the LLM), the system falls back to a generic message:

```
Task completed after N iteration(s). Tool executions completed successfully.
```

This message provides no useful information about what the agent accomplished, which tools it called, or what results were obtained.

## Root Cause

In `llm_agent.rs`, the tool execution loop (`execute_with_mcp`) breaks when the LLM returns no tool calls. If the LLM's final response has empty/null content, a generic fallback message is used as `final_response_content`. This happens because some LLMs focus entirely on tool calls and "forget" to provide a text summary afterwards.

## Solution

Added a **report enforcement mechanism** that detects generic completion messages and makes one additional LLM call (without tools) asking for a proper markdown report.

### Components

1. **`is_generic_completion_message(content: &str) -> bool`** - Pure function detecting 3 patterns:
   - `"Task completed after N iteration(s). Tool executions completed successfully."`
   - `"Max tool iterations (N) reached, stopping execution"`
   - Empty/whitespace-only content

2. **`REPORT_ENFORCEMENT_PROMPT`** - Constant prompt asking the LLM to provide a markdown summary of accomplished work, in the same language as the original task.

3. **Follow-up logic in `execute_with_mcp()`** - After the tool loop, if a generic message is detected AND tools were actually used (`iteration > 1`):
   - Checks cancellation token (skips if workflow cancelled)
   - Emits a reasoning step for frontend visibility
   - Adds the enforcement prompt as a user message
   - Makes one LLM call with **empty tools array** (forces text-only response)
   - Replaces `final_response_content` if the follow-up produces meaningful content
   - Falls back to generic message if the follow-up fails

### Design Decisions

| Decision | Rationale |
|----------|-----------|
| Empty tools array (not `ToolChoiceMode::None`) | Ollama ignores `tool_choice` parameter; sending no tools is universally reliable |
| `iteration > 1` guard | Prevents follow-up when no tools were used (nothing to report) |
| Single follow-up attempt | Avoids infinite loops; if one retry fails, the generic message is acceptable |
| No frontend changes | The `Report` struct is unchanged; the mechanism is transparent |

## Files Modified

| File | Changes |
|------|---------|
| `src-tauri/src/agents/llm_agent.rs` | +175 lines: constant, pure function, follow-up logic, 6 tests |

## Tests

| Test | Purpose |
|------|---------|
| `test_is_generic_completion_message_standard_pattern` | Detects "Task completed after N iteration(s)" |
| `test_is_generic_completion_message_max_iterations_pattern` | Detects "Max tool iterations (N) reached" |
| `test_is_generic_completion_message_empty` | Detects empty/whitespace content |
| `test_is_generic_completion_message_real_reports` | Does NOT flag real markdown reports |
| `test_is_generic_completion_message_with_whitespace` | Handles leading/trailing whitespace |
| `test_report_enforcement_prompt_is_valid` | Prompt is non-empty and contains "markdown" + "report" |

## Validation

- `cargo clippy -- -D warnings`: PASS (0 warnings)
- `cargo test`: PASS (all 979+ tests)
- `npm run lint`: PASS
- `npm run check`: PASS
- Manual test: pending (user to verify with real agent workflow)
