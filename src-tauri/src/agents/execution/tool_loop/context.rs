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
//! `ToolLoopContext`: grouped dependencies for the tool loop.
use crate::llm::ProviderManager;
use crate::models::AgentConfig;
use crate::tools::{context::AgentToolContext, ToolFactory};
use std::sync::Arc;

/// Context for the tool execution loop, grouping all dependencies.
pub(crate) struct ToolLoopContext<'a> {
    pub config: &'a AgentConfig,
    pub provider_manager: &'a ProviderManager,
    pub tool_factory: Option<&'a Arc<ToolFactory>>,
    pub agent_context: Option<&'a AgentToolContext>,
    /// True for an UNATTENDED (detached) run with no human at the keyboard:
    /// auto-analyze, compose-card, worker re-run. Carried explicitly
    /// — NOT derived from `agent_context.is_none()`, which would misclassify
    /// `rerun_worker` (it passes `Some(agent_context)`) as attended = fail-open.
    pub is_detached: bool,
    /// True when this detached run is a DELEGATED sub-agent (DelegateTask /
    /// ParallelTasks). Set by `LLMAgent::execute_with_mcp` from
    /// `task.is_delegated()`; the direct detached callers (rerun_worker,
    /// analyze, compose) leave it `false`. Threaded into the MCP gate so a
    /// delegated run additionally requires the entry's `allow_in_delegated_runs`
    /// flag. Spawned sub-agents leave it `false` (clone = same privilege).
    pub is_delegated: bool,
}
