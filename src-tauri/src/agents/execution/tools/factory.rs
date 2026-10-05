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
//! Local tool instantiation (`create_local_tools`).
use crate::models::agent::AgentKind;
use crate::models::AgentConfig;
use crate::tools::{context::AgentToolContext, Tool, ToolFactory};
use std::sync::Arc;
use tracing::debug;

/// Creates local tool instances for configured tools.
///
/// When `is_primary_agent` is true and `agent_context` is available,
/// this method will also create sub-agent tools (SpawnAgentTool,
/// DelegateTaskTool, ParallelTasksTool) in addition to basic tools.
pub(crate) async fn create_local_tools(
    config: &AgentConfig,
    tool_factory: Option<&Arc<ToolFactory>>,
    agent_context: Option<&AgentToolContext>,
    workflow_id: Option<String>,
    is_primary_agent: bool,
    context_override: Option<&AgentToolContext>,
) -> Vec<Arc<dyn Tool>> {
    let Some(factory) = tool_factory else {
        return Vec::new();
    };

    // Use override if provided, otherwise fall back to agent_context
    let effective_context = context_override.or(agent_context);

    // Extract app_handle from context if available
    let app_handle = effective_context.and_then(|ctx| ctx.app_handle.clone());

    // Auto-inject ReadSkillTool when agent has skills assigned
    let mut tool_names: Vec<String> = config.tools.clone();
    if !config.skills.is_empty() && !tool_names.iter().any(|t| t == "ReadSkillTool") {
        debug!(
            agent_id = %config.id,
            skills_count = config.skills.len(),
            "Auto-injecting ReadSkillTool for agent with skills"
        );
        tool_names.push("ReadSkillTool".to_string());
    }

    // Kanban agents are confined: they orchestrate cards but must NEVER act as
    // a delegation caller (they are already
    // excluded as a callee in delegate_task_execution). Strip the three
    // sub-agent tools defensively in case one was persisted on the config, and
    // force the basic-tools branch below so `create_tools_with_context` (which
    // auto-injects Spawn/Delegate/Parallel for a primary agent) is never taken.
    // Streaming/attribution stay intact — only the sub-agent tools are omitted.
    //
    // UserQuestionTool is stripped too: the confined card review chat only runs
    // on `/kanban`, which does not mount the UserQuestionModal, so a question
    // would hang in the void until its 5-minute timeout. The Kanban supervisor
    // must phrase everything as a direct turn, never an interactive prompt.
    let is_kanban = config.kind == Some(AgentKind::Kanban);
    if is_kanban {
        tool_names.retain(|t| {
            t != "SpawnAgentTool"
                && t != "DelegateTaskTool"
                && t != "ParallelTasksTool"
                && t != "UserQuestionTool"
        });
        // Auto-inject the per-card chat tools only when a context is present,
        // i.e. the streaming card review chat (the only Kanban streaming path).
        // The detached analyze/compose runs pass `agent_context: None`, so they
        // never receive these tools. The tools self-gate anyway (they resolve
        // the card via `review_chat_workflow_id`), so this is scoping for
        // prompt clarity, not security.
        if effective_context.is_some() {
            for card_tool in ["RerunWorkerTool", "MoveCardTool", "ScheduleCardTool"] {
                if !tool_names.iter().any(|t| t == card_tool) {
                    tool_names.push(card_tool.to_string());
                }
            }
        }
    } else {
        // The *Manager tools (prompt / skill / workflow) are reserved to
        // Kanban supervisors. Strip them
        // defensively for a non-Kanban agent so they never appear in the tool
        // set — the tools ALSO self-gate at execution (`ensure_kanban`), so this
        // is defense-in-depth, not the sole guard. Without it a user could
        // persist a Manager tool on a standard agent and at least see it in the
        // system prompt (it would still refuse on call).
        tool_names.retain(|t| {
            t != "PromptManagerTool" && t != "SkillManagerTool" && t != "WorkflowManagerTool"
        });
    }

    // If this is the primary agent and we have context, use create_tools_with_context
    // (skipped for Kanban agents so the delegation tools are never auto-added).
    let tools = match (is_primary_agent && !is_kanban, effective_context) {
        (true, Some(context)) => {
            debug!(
                agent_id = %config.id,
                "Creating tools with context for primary agent (sub-agent tools available)"
            );
            factory
                .create_tools_with_context(
                    &tool_names,
                    workflow_id,
                    config.id.clone(),
                    Some(context.clone()),
                    true,
                )
                .await
        }
        _ => {
            // Sub-agents or agents without context use basic tool creation.
            debug!(
                agent_id = %config.id,
                is_primary_agent = is_primary_agent,
                has_context = effective_context.is_some(),
                "Creating basic tools (sub-agent tools NOT available)"
            );
            factory
                .create_tools(&tool_names, workflow_id, config.id.clone(), app_handle)
                .await
        }
    };

    // A detached run NO LONGER wraps the *Manager tools read-only.
    // Write governance is now enforced centrally by `manager_write_gate` in
    // `execute_function_call`, armed by the explicit `is_detached` flag (which,
    // unlike the old `context.is_none()` wrapper trigger, also covers
    // `rerun_worker` — detached WITH a context). A detached content write
    // either passes per the user's validation settings (audited `PreApproved`)
    // or is refused there.
    tools
}
