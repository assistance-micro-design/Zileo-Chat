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

//! ToolFactory creation methods.
//!
//! Contains `create_tool`, `create_tools`, `create_tool_with_context`,
//! `create_tools_with_context`, and DB resolver helpers.

use super::factory::ToolFactory;
use crate::tools::context::AgentToolContext;
use crate::tools::delegate_task::DelegateTaskTool;
use crate::tools::parallel_tasks::ParallelTasksTool;
use crate::tools::spawn_agent::SpawnAgentTool;
use crate::tools::{
    CalculatorTool, FileManagerTool, MemoryTool, ReadSkillTool, TodoTool, Tool, UserQuestionTool,
};
use std::path::PathBuf;
use std::sync::Arc;
use tracing::{debug, info, warn};

impl ToolFactory {
    /// Resolves the skill names assigned to an agent from the database.
    ///
    /// Returns an empty Vec if the agent is not found or has no skills.
    async fn resolve_agent_skills(&self, agent_id: &str) -> Vec<String> {
        let query = "SELECT skills FROM agent WHERE meta::id(id) = $agent_id";
        let results: Result<Vec<serde_json::Value>, _> = self
            .db
            .query_json_with_params(
                query,
                vec![("agent_id".to_string(), serde_json::json!(agent_id))],
            )
            .await;

        match results {
            Ok(rows) => {
                if let Some(row) = rows.into_iter().next() {
                    if let Some(skills) = row["skills"].as_array() {
                        return skills
                            .iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect();
                    }
                }
                Vec::new()
            }
            Err(e) => {
                warn!(agent_id = %agent_id, error = %e, "Failed to resolve agent skills, defaulting to empty");
                Vec::new()
            }
        }
    }

    /// Resolves the folder paths assigned to an agent from the database.
    ///
    /// Returns canonicalized PathBuf for each valid, existing folder.
    /// Invalid or missing folders are logged as warnings and skipped.
    async fn resolve_agent_folders(&self, agent_id: &str) -> Vec<PathBuf> {
        let query = "SELECT folders FROM agent WHERE meta::id(id) = $agent_id";
        let results: Result<Vec<serde_json::Value>, _> = self
            .db
            .query_json_with_params(
                query,
                vec![("agent_id".to_string(), serde_json::json!(agent_id))],
            )
            .await;

        match results {
            Ok(rows) => {
                if let Some(row) = rows.into_iter().next() {
                    if let Some(folders) = row["folders"].as_array() {
                        return folders
                            .iter()
                            .filter_map(|v| {
                                let path_str = v.as_str()?;
                                let path = PathBuf::from(path_str);
                                match path.canonicalize() {
                                    Ok(canonical) if canonical.is_dir() => Some(canonical),
                                    Ok(_) => {
                                        warn!(path = %path_str, agent_id = %agent_id, "Folder path is not a directory, skipping");
                                        None
                                    }
                                    Err(e) => {
                                        warn!(path = %path_str, agent_id = %agent_id, error = %e, "Cannot resolve folder path, skipping");
                                        None
                                    }
                                }
                            })
                            .collect();
                    }
                }
                Vec::new()
            }
            Err(e) => {
                warn!(agent_id = %agent_id, error = %e, "Failed to resolve agent folders, defaulting to empty");
                Vec::new()
            }
        }
    }

    /// Creates a tool instance by name.
    ///
    /// # Arguments
    /// * `tool_name` - Tool identifier (e.g., "MemoryTool", "TodoTool")
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tool
    /// * `app_handle` - Optional Tauri app handle for event emission
    ///
    /// # Returns
    /// * `Ok(Arc<dyn Tool>)` - Tool instance ready for use
    /// * `Err(String)` - Error message if tool is unknown
    pub async fn create_tool(
        &self,
        tool_name: &str,
        workflow_id: Option<String>,
        agent_id: String,
        app_handle: Option<tauri::AppHandle>,
    ) -> Result<Arc<dyn Tool>, String> {
        // Validate tool exists in registry
        crate::tools::registry::TOOL_REGISTRY.validate(tool_name)?;
        debug!(
            tool_name = %tool_name,
            workflow_id = ?workflow_id,
            agent_id = %agent_id,
            "Creating tool instance"
        );

        match tool_name {
            "MemoryTool" => {
                let embedding_service = self.get_embedding_service().await;
                let has_embedding = embedding_service.is_some();
                debug!(
                    has_embedding = has_embedding,
                    "Creating MemoryTool with current embedding state"
                );
                let tool =
                    MemoryTool::new(self.db.clone(), embedding_service, workflow_id, agent_id);
                info!(has_embedding = has_embedding, "MemoryTool instance created");
                Ok(Arc::new(tool))
            }

            "TodoTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                // Default to primary agent when created via create_tool() (backward compat)
                let tool = TodoTool::new(self.db.clone(), wf_id, agent_id, app_handle, true);
                info!("TodoTool instance created");
                Ok(Arc::new(tool))
            }

            "CalculatorTool" => {
                let tool = CalculatorTool::new();
                info!("CalculatorTool instance created");
                Ok(Arc::new(tool))
            }

            "UserQuestionTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = UserQuestionTool::new(self.db.clone(), wf_id, agent_id, app_handle);
                info!("UserQuestionTool instance created");
                Ok(Arc::new(tool))
            }

            "ReadSkillTool" => {
                let agent_skills = self.resolve_agent_skills(&agent_id).await;
                debug!(
                    agent_id = %agent_id,
                    skills_count = agent_skills.len(),
                    "Creating ReadSkillTool with resolved agent skills"
                );
                let tool = ReadSkillTool::new(self.db.clone(), agent_skills);
                info!("ReadSkillTool instance created");
                Ok(Arc::new(tool))
            }

            "FileManagerTool" => {
                let folders = self.resolve_agent_folders(&agent_id).await;
                debug!(
                    agent_id = %agent_id,
                    folders_count = folders.len(),
                    "Creating FileManagerTool with resolved agent folders"
                );
                let tool = FileManagerTool::new(folders);
                info!("FileManagerTool instance created");
                Ok(Arc::new(tool))
            }

            _ => {
                warn!(tool_name = %tool_name, "Unknown tool requested");
                Err(format!(
                    "Unknown tool: '{}'. Available tools: MemoryTool, TodoTool, CalculatorTool, UserQuestionTool, ReadSkillTool, FileManagerTool",
                    tool_name
                ))
            }
        }
    }

    /// Creates multiple tools from a list of names.
    ///
    /// # Arguments
    /// * `tool_names` - List of tool identifiers
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tools
    /// * `app_handle` - Optional Tauri app handle for event emission
    ///
    /// # Returns
    /// Vector of successfully created tools. Failed tools are logged but skipped.
    pub async fn create_tools(
        &self,
        tool_names: &[String],
        workflow_id: Option<String>,
        agent_id: String,
        app_handle: Option<tauri::AppHandle>,
    ) -> Vec<Arc<dyn Tool>> {
        let mut tools = Vec::new();

        for name in tool_names {
            match self
                .create_tool(
                    name,
                    workflow_id.clone(),
                    agent_id.clone(),
                    app_handle.clone(),
                )
                .await
            {
                Ok(tool) => {
                    tools.push(tool);
                }
                Err(e) => {
                    warn!(
                        tool_name = %name,
                        error = %e,
                        "Failed to create tool, skipping"
                    );
                }
            }
        }

        debug!(
            requested = tool_names.len(),
            created = tools.len(),
            "Tool batch creation completed"
        );

        tools
    }

    /// Creates a tool instance with AgentToolContext.
    ///
    /// This method is used for tools that need access to the agent system,
    /// such as SpawnAgentTool, DelegateTaskTool, and ParallelTasksTool.
    ///
    /// # Arguments
    /// * `tool_name` - Tool identifier
    /// * `workflow_id` - Workflow ID for scoping
    /// * `agent_id` - Agent ID using the tool
    /// * `context` - AgentToolContext providing system dependencies
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Returns
    /// * `Ok(Arc<dyn Tool>)` - Tool instance ready for use
    /// * `Err(String)` - Error message if tool is unknown or cannot be created
    ///
    /// # Sub-Agent Constraints
    ///
    /// When `is_primary_agent` is `false`, sub-agent tools (SpawnAgentTool,
    /// DelegateTaskTool, ParallelTasksTool) will NOT be created. This enforces
    /// the single-level constraint where sub-agents cannot spawn other sub-agents.
    pub async fn create_tool_with_context(
        &self,
        tool_name: &str,
        workflow_id: Option<String>,
        agent_id: String,
        context: AgentToolContext,
        is_primary_agent: bool,
    ) -> Result<Arc<dyn Tool>, String> {
        debug!(
            tool_name = %tool_name,
            workflow_id = ?workflow_id,
            agent_id = %agent_id,
            is_primary_agent = is_primary_agent,
            "Creating tool instance with context"
        );

        // Check if this is a sub-agent tool and enforce constraints
        if Self::requires_context(tool_name) && !is_primary_agent {
            warn!(
                tool_name = %tool_name,
                agent_id = %agent_id,
                "Sub-agent attempted to access sub-agent tool - denied"
            );
            return Err(format!(
                "Tool '{}' is only available to the primary workflow agent. \
                 Sub-agents cannot spawn other sub-agents or delegate tasks.",
                tool_name
            ));
        }

        match tool_name {
            // TodoTool needs is_primary_agent for scoping (sub-agents only see own tasks)
            "TodoTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = TodoTool::new(
                    self.db.clone(),
                    wf_id,
                    agent_id,
                    context.app_handle.clone(),
                    is_primary_agent,
                );
                info!(
                    is_primary_agent = is_primary_agent,
                    "TodoTool instance created with context"
                );
                Ok(Arc::new(tool))
            }

            // Other basic tools (delegate to create_tool)
            "MemoryTool" | "CalculatorTool" | "UserQuestionTool" | "ReadSkillTool"
            | "FileManagerTool" => {
                let app_handle = context.app_handle.clone();
                self.create_tool(tool_name, workflow_id, agent_id, app_handle)
                    .await
            }

            // Sub-agent tools (require context)
            "SpawnAgentTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = SpawnAgentTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("SpawnAgentTool instance created");
                Ok(Arc::new(tool))
            }

            "DelegateTaskTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = DelegateTaskTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("DelegateTaskTool instance created");
                Ok(Arc::new(tool))
            }

            "ParallelTasksTool" => {
                let wf_id = workflow_id.unwrap_or_else(|| "default".to_string());
                let tool = ParallelTasksTool::new(
                    self.db.clone(),
                    context,
                    agent_id,
                    wf_id,
                    is_primary_agent,
                );
                info!("ParallelTasksTool instance created");
                Ok(Arc::new(tool))
            }

            _ => {
                warn!(tool_name = %tool_name, "Unknown tool requested");
                Err(format!(
                    "Unknown tool: '{}'. Available tools: {:?}",
                    tool_name,
                    Self::available_tools()
                ))
            }
        }
    }

    /// Creates multiple tools, handling both basic and context-aware tools.
    ///
    /// # Arguments
    /// * `tool_names` - List of tool identifiers
    /// * `workflow_id` - Optional workflow ID for scoping
    /// * `agent_id` - Agent ID using the tools
    /// * `context` - Optional AgentToolContext for sub-agent tools
    /// * `is_primary_agent` - Whether this is the primary workflow agent
    ///
    /// # Returns
    /// Vector of successfully created tools. Failed tools are logged but skipped.
    ///
    /// # Primary Agent Behavior
    ///
    /// When `is_primary_agent` is true and `context` is provided, this method
    /// AUTOMATICALLY adds sub-agent tools (SpawnAgentTool, DelegateTaskTool,
    /// ParallelTasksTool) even if they are not in `tool_names`. This ensures
    /// the primary workflow agent always has access to orchestration capabilities.
    pub async fn create_tools_with_context(
        &self,
        tool_names: &[String],
        workflow_id: Option<String>,
        agent_id: String,
        context: Option<AgentToolContext>,
        is_primary_agent: bool,
    ) -> Vec<Arc<dyn Tool>> {
        let mut tools = Vec::new();

        // Extract app_handle from context if available
        let app_handle = context.as_ref().and_then(|ctx| ctx.app_handle.clone());

        // First, create tools from the provided tool_names list
        for name in tool_names {
            let result = if Self::requires_context(name) {
                if let Some(ctx) = &context {
                    self.create_tool_with_context(
                        name,
                        workflow_id.clone(),
                        agent_id.clone(),
                        ctx.clone(),
                        is_primary_agent,
                    )
                    .await
                } else {
                    warn!(
                        tool_name = %name,
                        "Sub-agent tool requested without context - skipping"
                    );
                    Err("AgentToolContext required for sub-agent tools".to_string())
                }
            } else {
                self.create_tool(
                    name,
                    workflow_id.clone(),
                    agent_id.clone(),
                    app_handle.clone(),
                )
                .await
            };

            match result {
                Ok(tool) => {
                    tools.push(tool);
                }
                Err(e) => {
                    warn!(
                        tool_name = %name,
                        error = %e,
                        "Failed to create tool, skipping"
                    );
                }
            }
        }

        // For primary agents with context, automatically add sub-agent tools
        // if they weren't already included in tool_names
        if is_primary_agent {
            if let Some(ctx) = &context {
                let sub_agent_tool_names = Self::sub_agent_tools();
                for sub_tool_name in sub_agent_tool_names {
                    // Skip if already in the list
                    if tool_names.iter().any(|t| t == sub_tool_name) {
                        continue;
                    }

                    match self
                        .create_tool_with_context(
                            sub_tool_name,
                            workflow_id.clone(),
                            agent_id.clone(),
                            ctx.clone(),
                            true, // is_primary_agent
                        )
                        .await
                    {
                        Ok(tool) => {
                            info!(
                                tool_name = %sub_tool_name,
                                agent_id = %agent_id,
                                "Auto-added sub-agent tool for primary agent"
                            );
                            tools.push(tool);
                        }
                        Err(e) => {
                            warn!(
                                tool_name = %sub_tool_name,
                                error = %e,
                                "Failed to auto-add sub-agent tool"
                            );
                        }
                    }
                }
            }
        }

        debug!(
            requested = tool_names.len(),
            created = tools.len(),
            is_primary_agent = is_primary_agent,
            "Tool batch creation with context completed"
        );

        tools
    }
}
