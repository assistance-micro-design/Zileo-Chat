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

//! Validation request methods for different operation types.
//!
//! Contains `request_*_validation()` and `*_details()` methods for:
//! - Sub-agent operations (spawn, delegate, parallel)
//! - Local tool execution
//! - MCP server tool calls
//! - File operations

use super::validation_helper::{should_require_validation, ValidationHelper};
use crate::models::streaming::SubAgentOperationType;
use crate::models::{RiskLevel, ValidationType};
use crate::tools::ToolError;
use serde_json::Value;
use tracing::info;
use uuid::Uuid;

use super::utils::safe_truncate;

impl ValidationHelper {
    /// Requests validation for a sub-agent operation.
    ///
    /// Checks ValidationSettings, then delegates to `create_and_wait_validation()`.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `operation_type` - Type of sub-agent operation
    /// * `operation_description` - Human-readable operation description
    /// * `details` - Additional details about the operation (JSON)
    /// * `risk_level` - Risk assessment for the operation
    ///
    /// # Returns
    /// * `Ok(())` - If operation was approved (or validation was skipped)
    /// * `Err(ToolError::PermissionDenied)` - If operation was rejected
    /// * `Err(ToolError::Timeout)` - If validation timed out
    #[allow(clippy::too_many_arguments)]
    pub async fn request_validation(
        &self,
        workflow_id: &str,
        operation_type: SubAgentOperationType,
        operation_description: &str,
        details: Value,
        risk_level: RiskLevel,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;

        if !should_require_validation(&settings, &ValidationType::SubAgent, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                operation_type = %operation_type,
                "Skipping validation based on settings (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = Uuid::new_v4().to_string();

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            operation_type = %operation_type,
            "Creating validation request for sub-agent operation"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::SubAgent,
            operation_description,
            details,
            risk_level,
        )
        .await
    }

    /// Determines the risk level based on operation type.
    ///
    /// # Risk Level Guidelines
    /// - `Low`: Read-only operations, listing
    /// - `Medium`: Single agent spawn/delegate
    /// - `High`: Parallel execution, multiple agents
    pub fn determine_risk_level(operation_type: &SubAgentOperationType) -> RiskLevel {
        match operation_type {
            SubAgentOperationType::Spawn => RiskLevel::Medium,
            SubAgentOperationType::Delegate => RiskLevel::Medium,
            SubAgentOperationType::ParallelBatch => RiskLevel::High,
        }
    }

    /// Creates operation details JSON for spawn operation.
    pub fn spawn_details(
        name: &str,
        prompt: &str,
        tools: &[String],
        mcp_servers: &[String],
    ) -> Value {
        serde_json::json!({
            "sub_agent_name": name,
            "prompt_preview": safe_truncate(prompt, 200, true),
            "prompt_length": prompt.len(),
            "tools": tools,
            "mcp_servers": mcp_servers
        })
    }

    /// Creates operation details JSON for delegate operation.
    pub fn delegate_details(target_agent_id: &str, target_agent_name: &str, prompt: &str) -> Value {
        serde_json::json!({
            "target_agent_id": target_agent_id,
            "target_agent_name": target_agent_name,
            "prompt_preview": safe_truncate(prompt, 200, true),
            "prompt_length": prompt.len()
        })
    }

    /// Creates operation details JSON for parallel batch operation.
    pub fn parallel_details(tasks: &[(String, String)]) -> Value {
        let task_list: Vec<Value> = tasks
            .iter()
            .map(|(agent_id, prompt)| {
                serde_json::json!({
                    "agent_id": agent_id,
                    "prompt_preview": safe_truncate(prompt, 100, true)
                })
            })
            .collect();

        serde_json::json!({
            "task_count": tasks.len(),
            "tasks": task_list
        })
    }

    /// Requests validation for a local tool execution.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `tool_name` - Name of the tool being executed
    /// * `operation` - Operation being performed (e.g., "add", "delete")
    /// * `arguments` - Tool arguments (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_tool_validation(
        &self,
        workflow_id: &str,
        tool_name: &str,
        operation: &str,
        arguments: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        let risk_level = RiskLevel::Low; // Local tools are generally low risk

        if !should_require_validation(&settings, &ValidationType::Tool, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                tool_name = %tool_name,
                "Skipping validation for local tool (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = uuid::Uuid::new_v4().to_string();
        let details = Self::tool_details(tool_name, operation, &arguments);
        let description = format!("Execute {} tool: {}", tool_name, operation);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            tool_name = %tool_name,
            "Creating validation request for local tool"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::Tool,
            &description,
            details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for tool execution.
    pub fn tool_details(tool_name: &str, operation: &str, arguments: &Value) -> Value {
        serde_json::json!({
            "tool_name": tool_name,
            "operation": operation,
            "arguments_preview": safe_truncate(&arguments.to_string(), 200, true)
        })
    }

    /// Requests validation for an MCP server tool call.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `server_name` - MCP server name
    /// * `tool_name` - Tool name on the server
    /// * `arguments` - Tool arguments (JSON)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_mcp_validation(
        &self,
        workflow_id: &str,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        let risk_level = RiskLevel::Medium; // MCP calls are medium risk (external system)

        if !should_require_validation(&settings, &ValidationType::Mcp, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                server_name = %server_name,
                tool_name = %tool_name,
                "Skipping validation for MCP tool (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = uuid::Uuid::new_v4().to_string();
        let details = Self::mcp_details(server_name, tool_name, &arguments);
        let description = format!("Call MCP server '{}': {}", server_name, tool_name);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            server_name = %server_name,
            tool_name = %tool_name,
            "Creating validation request for MCP tool"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::Mcp,
            &description,
            details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for MCP tool call.
    pub fn mcp_details(server_name: &str, tool_name: &str, arguments: &Value) -> Value {
        serde_json::json!({
            "server_name": server_name,
            "tool_name": tool_name,
            "arguments_preview": safe_truncate(&arguments.to_string(), 200, true)
        })
    }

    /// Requests validation for a destructive file operation.
    ///
    /// # Arguments
    /// * `workflow_id` - Associated workflow ID
    /// * `operation` - File operation name (e.g., "delete", "write", "move")
    /// * `path` - File/directory path being operated on
    /// * `details` - Additional details (destination, pattern, etc.)
    ///
    /// # Returns
    /// * `Ok(())` - If approved or validation skipped
    /// * `Err(ToolError)` - If rejected or error
    pub async fn request_file_validation(
        &self,
        workflow_id: &str,
        operation: &str,
        path: &str,
        details: Value,
    ) -> Result<(), ToolError> {
        let settings = self.load_validation_settings().await;
        // Delete is high risk (permanent data loss), other destructive ops are medium
        let risk_level = if operation == "delete" {
            RiskLevel::High
        } else {
            RiskLevel::Medium
        };

        if !should_require_validation(&settings, &ValidationType::FileOp, &risk_level) {
            info!(
                workflow_id = %workflow_id,
                operation = %operation,
                path = %path,
                risk_level = %risk_level,
                "Skipping validation for file operation (mode: {:?})",
                settings.mode
            );
            return Ok(());
        }

        let validation_id = Uuid::new_v4().to_string();
        let full_details = Self::file_op_details(operation, path, &details);
        let description = format!("File operation '{}' on: {}", operation, path);

        info!(
            validation_id = %validation_id,
            workflow_id = %workflow_id,
            operation = %operation,
            "Creating validation request for file operation"
        );

        self.create_and_wait_validation(
            &validation_id,
            workflow_id,
            ValidationType::FileOp,
            &description,
            full_details,
            risk_level,
        )
        .await
    }

    /// Creates operation details JSON for file operations.
    pub fn file_op_details(operation: &str, path: &str, extra: &Value) -> Value {
        serde_json::json!({
            "operation": operation,
            "path": path,
            "details": extra
        })
    }
}
