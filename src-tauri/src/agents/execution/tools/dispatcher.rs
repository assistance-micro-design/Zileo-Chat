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
//! `execute_function_call` orchestration: validation, dispatch, budgets, audit.
use super::definitions::FunctionCallContext;
use super::permissions::{is_mcp_tool_armed, mcp_run_budget_check};
use super::validation::manager_write_gate;
use crate::models::function_calling::{FunctionCall, FunctionCallResult};
use crate::models::RiskLevel;
use crate::tools::validation_helper::is_destructive_file_op;
use tracing::{info, warn};

/// Executes a single function call (local or MCP tool).
///
/// `mcp_result_bytes` is the cumulative sink byte size (serialized result +
/// error) of all prior MCP results in this run — success AND error — consulted by
/// the per-run budget gate (the caller accumulates it in `iteration.rs`
/// after each MCP call via [`result_sink_byte_len`] + [`mcp_result_budget_charge`]).
///
/// `manager_writes_made` is the run-scoped count of *Manager content/privilege
/// writes already executed, consulted + incremented by the *Manager write gate
/// for the per-run volume cap.
pub(crate) async fn execute_function_call(
    call: &FunctionCall,
    ctx: &FunctionCallContext<'_>,
    tools_used: &mut Vec<String>,
    mcp_calls_made: &mut Vec<String>,
    mcp_result_bytes: usize,
    manager_writes_made: &mut usize,
) -> FunctionCallResult {
    let start = std::time::Instant::now();

    // Check if MCP tool
    if let Some((server, tool)) = call.parse_mcp_name() {
        // Execute via MCP
        if let Some(mcp) = ctx.mcp_manager {
            // Snapshot the pre-call count BEFORE recording this
            // attempt, so the per-run cap (CHECK-BEFORE-REFUSE) counts only
            // prior calls and allows EXACTLY MCP_MAX_CALLS_PER_RUN.
            let calls_before = mcp_calls_made.len();
            mcp_calls_made.push(call.name.clone());

            // In a DETACHED run there is no human to answer the
            // validation modal, so the gate is the per-agent allowlist —
            // evaluated UNCONDITIONALLY (independent of ValidationMode, which
            // would otherwise let `Auto` short-circuit to fail-open). Keyed by
            // the immutable `server_id` (resolved from the tool's server name),
            // never the display name. Empty allowlist = nothing armed = refuse.
            // Evaluated FIRST so an unarmed detached call is refused and audited
            // as a security refusal even when the run is also over budget.
            if ctx.is_detached {
                let server_id = mcp.get_server_id_by_name(server).await;
                let armed = is_mcp_tool_armed(
                    ctx.mcp_tool_allowlist,
                    server_id.as_deref(),
                    tool,
                    ctx.is_delegated,
                );
                if !armed {
                    // Structured refusal audit (no secret). `delegated`
                    // distinguishes the confused-deputy refusal (armed for the
                    // agent but not flagged `allow_in_delegated_runs`) from a
                    // plain unarmed refusal.
                    warn!(
                        server = %server,
                        server_id = server_id.as_deref().unwrap_or("<unknown>"),
                        tool = %tool,
                        delegated = ctx.is_delegated,
                        "MCP tool refused: not armed for this agent in a detached run"
                    );
                    // Persist the refusal into the EXISTING audit log
                    // (Settings > Audit Log) so a no-human-in-the-loop block is
                    // visible, not only traced. Unconditional + best-effort
                    // (never blocks or fails the refusal itself).
                    if let Some(helper) = ctx.validation_helper {
                        helper
                            .record_security_refusal(
                                &call.name,
                                server_id.as_deref(),
                                "not armed for this agent in a detached run",
                                ctx.is_delegated,
                                ctx.workflow_id,
                            )
                            .await;
                    }
                    let scope = if ctx.is_delegated {
                        "unattended (detached) delegated runs"
                    } else {
                        "unattended (detached) runs"
                    };
                    return FunctionCallResult::failure(
                        &call.id,
                        &call.name,
                        format!(
                            "MCP tool '{}' is not enabled for this agent in {}",
                            call.name, scope
                        ),
                    );
                }
                // Armed: proceed without the interactive modal (no human
                // present). Record the pre-approved execution into the audit
                // log so an unattended MCP call that ran by allowlist policy is
                // traceable (decided_by = PreApproved). Best-effort + run-scoped
                // dedup per (tool, op) inside the helper; MCP is Medium risk so
                // no toast is emitted.
                if let Some(helper) = ctx.validation_helper {
                    helper
                        .record_preapproved(
                            &call.name,
                            "armed_mcp_call",
                            // Discriminant empty: the full mcp__server__tool name
                            // is already in `tool_name`, so dedup is per-tool/run.
                            "",
                            &RiskLevel::Medium,
                            ctx.workflow_id,
                        )
                        .await;
                }
            }

            // Per-run cap + cumulative byte budget. Checked AFTER the
            // detached gate (a budget refusal can never short-circuit / fail-open
            // that gate) and BEFORE the attended modal (don't prompt the human for
            // a call we will refuse anyway). Applies to BOTH run types. An
            // in-flight result is never truncated — the NEXT call is refused.
            if let Err(reason) = mcp_run_budget_check(calls_before, mcp_result_bytes) {
                warn!(tool = %call.name, reason = %reason, "MCP call refused: per-run budget/cap reached");
                return FunctionCallResult::failure(&call.id, &call.name, reason);
            }

            if !ctx.is_detached {
                if let Some(helper) = ctx.validation_helper {
                    // Attended run: existing interactive validation modal.
                    if let Err(e) = helper
                        .request_mcp_validation(
                            ctx.workflow_id,
                            server,
                            tool,
                            call.arguments.clone(),
                            // This branch only runs for an ATTENDED run
                            // (`if !ctx.is_detached`), so the modal has a human.
                            false,
                        )
                        .await
                    {
                        warn!(tool = %call.name, error = %e, "MCP validation rejected");
                        return FunctionCallResult::failure(&call.id, &call.name, e.to_string());
                    }
                }
            }

            match mcp.call_tool(server, tool, call.arguments.clone()).await {
                Ok(result) => {
                    if result.success {
                        info!(tool = %call.name, "MCP tool executed successfully");
                        FunctionCallResult::success(&call.id, &call.name, result.content)
                            .with_execution_time(start.elapsed().as_millis() as u64)
                    } else {
                        let error_msg = result.error.unwrap_or_else(|| "Unknown error".to_string());
                        warn!(tool = %call.name, error = %error_msg, "MCP tool returned error");
                        FunctionCallResult::failure(&call.id, &call.name, error_msg)
                    }
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "MCP tool call failed");
                    FunctionCallResult::failure(&call.id, &call.name, e.to_string())
                }
            }
        } else {
            FunctionCallResult::failure(&call.id, &call.name, "MCP manager not available")
        }
    } else {
        // Execute local tool
        let matching_tool = ctx.local_tools.iter().find(|t| t.id() == call.name);

        if let Some(tool) = matching_tool {
            tools_used.push(call.name.clone());

            // *Manager WRITE gate (prompt/skill/workflow). Runs BEFORE
            // the generic tool validation: a write op is fully handled here
            // (executed + audited PreApproved, validated via modal, or refused).
            // A read op / non-Manager tool returns None and falls through.
            if let Some(result) =
                manager_write_gate(call, ctx, tool, manager_writes_made, start).await
            {
                return result;
            }

            // Request validation for local tool
            // Skip validation for sub-agent tools (they have their own validation)
            let is_sub_agent_tool = call.name == "SpawnAgentTool"
                || call.name == "DelegateTaskTool"
                || call.name == "ParallelTasksTool";

            if !is_sub_agent_tool {
                // FileManagerTool: use file-specific validation if destructive + confirmation enabled
                if call.name == "FileManagerTool" && ctx.require_file_confirmation {
                    let operation = call
                        .arguments
                        .get("operation")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown");

                    if is_destructive_file_op(operation) {
                        if let Some(helper) = ctx.validation_helper {
                            let path = call
                                .arguments
                                .get("path")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");

                            if let Err(e) = helper
                                .request_file_validation(
                                    ctx.workflow_id,
                                    operation,
                                    path,
                                    call.arguments.clone(),
                                    ctx.is_detached,
                                )
                                .await
                            {
                                warn!(tool = %call.name, operation = %operation, error = %e, "File operation validation rejected");
                                return FunctionCallResult::failure(
                                    &call.id,
                                    &call.name,
                                    e.to_string(),
                                );
                            }
                        }
                    }
                } else if let Some(helper) = ctx.validation_helper {
                    // Standard tool validation for non-FileManagerTool
                    let operation = call
                        .arguments
                        .get("operation")
                        .and_then(|v| v.as_str())
                        .unwrap_or("execute");

                    if let Err(e) = helper
                        .request_tool_validation(
                            ctx.workflow_id,
                            &call.name,
                            operation,
                            call.arguments.clone(),
                            ctx.is_detached,
                        )
                        .await
                    {
                        warn!(tool = %call.name, error = %e, "Tool validation rejected");
                        return FunctionCallResult::failure(&call.id, &call.name, e.to_string());
                    }
                }
            }

            match tool.execute(call.arguments.clone()).await {
                Ok(result) => {
                    info!(tool = %call.name, "Local tool executed successfully");
                    FunctionCallResult::success(&call.id, &call.name, result)
                        .with_execution_time(start.elapsed().as_millis() as u64)
                }
                Err(e) => {
                    warn!(tool = %call.name, error = %e, "Local tool execution failed");
                    FunctionCallResult::failure(&call.id, &call.name, e.to_string())
                }
            }
        } else {
            let available_tools: Vec<String> =
                ctx.local_tools.iter().map(|t| t.id().to_string()).collect();

            FunctionCallResult::failure(
                &call.id,
                &call.name,
                format!(
                    "Unknown tool '{}'. Available tools: {}",
                    call.name,
                    available_tools.join(", ")
                ),
            )
        }
    }
}
