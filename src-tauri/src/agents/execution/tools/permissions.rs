// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//! Detached MCP allowlist decisions and per-run budgets.
use crate::constants::mcp::{
    MCP_MAX_CALLS_PER_RUN, MCP_MAX_RESULT_BYTES_PER_RUN, MCP_MAX_SINGLE_RESULT_BYTES,
};

/// Pure decision: is `(server_id, tool)` armed in the agent's detached
/// allowlist? Keyed by the immutable `server_id` (never the display name).
/// `server_id == None` (the server name could not be resolved to an id) is
/// fail-closed → `false`. An empty allowlist arms nothing.
///
/// Delegated runs (`is_delegated`): when this detached run is a DELEGATED sub-agent
/// (DelegateTask / ParallelTasks), the matching entry must ALSO set
/// `allow_in_delegated_runs`. A DIRECT detached run (rerun-primary / analyze /
/// compose) or a SPAWNED sub-agent (`is_delegated == false`) ignores the flag,
/// preserving the non-delegated behavior. This closes the UNION confused-deputy where
/// a detached worker delegates to a standard agent whose own allowlist arms
/// tools the worker should not be able to trigger.
pub(crate) fn is_mcp_tool_armed(
    allowlist: &[crate::models::agent::McpToolAllowlistEntry],
    server_id: Option<&str>,
    tool: &str,
    is_delegated: bool,
) -> bool {
    server_id.is_some_and(|sid| {
        allowlist.iter().any(|e| {
            e.server_id == sid
                && e.tools.iter().any(|t| t == tool)
                && (!is_delegated || e.allow_in_delegated_runs)
        })
    })
}

/// Pure decision: may another MCP call proceed in this run?
///
/// `calls_so_far` is the number of MCP calls already attempted in the run
/// (EXCLUDING the one about to be made), and `bytes_so_far` is the cumulative
/// sink byte size (serialized result + error) of all prior MCP results — success
/// AND error. Returns `Err` with a human-readable refusal once either the call cap
/// ([`MCP_MAX_CALLS_PER_RUN`]) or the cumulative byte budget
/// ([`MCP_MAX_RESULT_BYTES_PER_RUN`]) is reached.
///
/// CHECK-BEFORE-REFUSE: consulted *before* a call is dispatched, so exactly
/// `MCP_MAX_CALLS_PER_RUN` calls are allowed and an in-flight result is never
/// truncated (truncating would corrupt the JSON and hide information).
///
/// # Errors
/// Returns a refusal string when the per-run call cap or byte budget is reached.
pub(crate) fn mcp_run_budget_check(calls_so_far: usize, bytes_so_far: usize) -> Result<(), String> {
    if calls_so_far >= MCP_MAX_CALLS_PER_RUN {
        return Err(format!(
            "Per-run MCP call limit reached ({MCP_MAX_CALLS_PER_RUN} calls): this agent run has \
             made too many MCP tool calls. No further MCP calls are permitted in this run."
        ));
    }
    if bytes_so_far >= MCP_MAX_RESULT_BYTES_PER_RUN {
        return Err(format!(
            "Per-run MCP result budget reached ({MCP_MAX_RESULT_BYTES_PER_RUN} bytes): this agent \
             run has accumulated too much MCP output. No further MCP calls are permitted in this run."
        ));
    }
    Ok(())
}

/// Total bytes a tool result actually pushes to the run's sinks —
/// the serialized `result.result` PLUS the `result.error` string.
///
/// BOTH fields reach the LLM tool message (`result_to_string` emits
/// `{"error": ...}` for a failure), the persisted row (`output_result` +
/// `error_message`), and the live stream chunk. Measuring only the serialized
/// result misses a giant ERROR payload: `FunctionCallResult::failure` sets
/// `result` to `Null` (serializes to `"null"`, ~4 bytes) and routes the
/// server's (possibly giant) message into `error`. Summing both is the size the
/// per-result cap and the cumulative budget must use. `serialized_result` is the
/// caller's already-computed `to_string(result.result)` (no re-serialization).
pub(crate) fn result_sink_byte_len(serialized_result: &str, error: Option<&str>) -> usize {
    serialized_result.len() + error.map_or(0, str::len)
}

/// Pure decision: must a just-returned MCP result be replaced for
/// exceeding the per-result size cap?
///
/// Returns `Some(refusal_message)` when an MCP result's sink byte size
/// (`result_byte_len` — serialized result + error, via [`result_sink_byte_len`],
/// measured post image-strip) exceeds [`MCP_MAX_SINGLE_RESULT_BYTES`] — the
/// caller then swaps the giant payload for that error before it reaches the LLM
/// tool message, the persisted `tool_execution` row, or the live stream chunk.
/// Returns `None` (pass through unchanged) for a local-tool result (user-trusted,
/// out of scope) or a result within the cap.
///
/// SUCCESS-AGNOSTIC by design: under this threat model a compromised MCP
/// server controls its JSON-RPC response size on BOTH paths, so a giant ERROR
/// payload (`success == false`, carried in `result.error`) is just as dangerous
/// as a giant success payload and is capped identically. Replacing a giant error
/// with a small capped error is acceptable (the call already failed).
///
/// Complements the cumulative [`mcp_run_budget_check`]: this is a POST-call cap
/// on the CURRENT result; the cumulative budget is the PRE-call gate for the
/// NEXT call. The two coexist — neither truncates a payload.
pub(crate) fn mcp_oversized_result_refusal(
    is_mcp_tool: bool,
    result_byte_len: usize,
) -> Option<String> {
    if is_mcp_tool && result_byte_len > MCP_MAX_SINGLE_RESULT_BYTES {
        Some(format!(
            "MCP result exceeded the per-result size limit of {MCP_MAX_SINGLE_RESULT_BYTES} bytes; \
             refused to protect the run context."
        ))
    } else {
        None
    }
}

/// Pure accounting: bytes a just-returned tool result contributes to the
/// cumulative per-run MCP byte budget.
///
/// Counts EVERY MCP result — success AND error — because a compromised server
/// controls error size too; gating on success would let a flood of medium-sized
/// error payloads (each under the per-result cap) bypass the cumulative budget.
/// Local-tool results contribute nothing (out of scope). Call with the POST
/// per-result-cap sink byte length (`result_byte_len` from [`result_sink_byte_len`])
/// so an oversized result charges only its (small) replacement, never the giant
/// original — and so a giant error (carried in `result.error`) is charged in full.
pub(crate) fn mcp_result_budget_charge(is_mcp_tool: bool, result_byte_len: usize) -> usize {
    if is_mcp_tool {
        result_byte_len
    } else {
        0
    }
}
