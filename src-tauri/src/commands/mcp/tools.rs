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

//! MCP tool commands
//!
//! Tauri commands for listing tools, calling tools, and querying latency metrics.

use crate::commands::mcp::validation::{validate_mcp_server_id, validate_tool_name};
use crate::models::mcp::{MCPLatencyMetrics, MCPTool, MCPToolCallRequest, MCPToolCallResult};
use crate::security::serialize_for_query;
use crate::state::AppState;
use tauri::State;
use tracing::{error, info, instrument};

/// Lists available tools from an MCP server.
///
/// Returns the tools discovered during server initialization.
/// The server must be running for this to return tools.
///
/// # Arguments
///
/// * `server_name` - The name/ID of the MCP server
///
/// # Returns
///
/// A vector of [`MCPTool`] objects describing available tools.
///
/// # Errors
///
/// Returns an error if:
/// - The server name is invalid
/// - The server is not found
/// - The server is not running
#[tauri::command]
#[instrument(name = "list_mcp_tools", skip(state), fields(server = %server_name))]
pub async fn list_mcp_tools(
    server_name: String,
    state: State<'_, AppState>,
) -> Result<Vec<MCPTool>, String> {
    let validated_name = validate_mcp_server_id(&server_name)?;
    info!(server = %validated_name, "Listing MCP tools");

    let tools = state.mcp_manager.list_server_tools(&validated_name).await;

    info!(
        server = %validated_name,
        count = tools.len(),
        "MCP tools listed"
    );
    Ok(tools)
}

/// Calls a tool on an MCP server.
///
/// Executes the specified tool with the provided arguments.
/// The tool call is logged for auditing purposes.
///
/// # Arguments
///
/// * `request` - The tool call request containing server name, tool name, and arguments
///
/// # Returns
///
/// An [`MCPToolCallResult`] containing the tool output or error.
///
/// # Errors
///
/// Returns an error if:
/// - The request is invalid
/// - The server is not found or not running
/// - The tool is not found
/// - Tool execution fails
#[tauri::command]
#[instrument(name = "call_mcp_tool", skip(state, request), fields(server, tool))]
pub async fn call_mcp_tool(
    request: MCPToolCallRequest,
    state: State<'_, AppState>,
) -> Result<MCPToolCallResult, String> {
    let validated_server = validate_mcp_server_id(&request.server_name)?;
    let validated_tool = validate_tool_name(&request.tool_name)?;

    tracing::Span::current().record("server", &validated_server);
    tracing::Span::current().record("tool", &validated_tool);

    info!(
        server = %validated_server,
        tool = %validated_tool,
        "Calling MCP tool"
    );

    let validated_request = MCPToolCallRequest {
        server_name: validated_server.clone(),
        tool_name: validated_tool.clone(),
        arguments: request.arguments,
    };

    let result = state
        .mcp_manager
        .call_tool_request(validated_request)
        .await
        .map_err(|e| {
            error!(error = %e, "MCP tool call failed");
            format!("MCP tool call failed: {}", e)
        })?;

    info!(
        server = %validated_server,
        tool = %validated_tool,
        success = result.success,
        duration_ms = result.duration_ms,
        "MCP tool call completed"
    );
    Ok(result)
}

/// Returns latency percentile metrics (p50, p95, p99) for MCP servers.
///
/// Queries the `mcp_call_log` table for the last hour of data and
/// calculates percentile latencies per server. This provides insight
/// into MCP tool call performance over time.
///
/// # Arguments
///
/// * `server_name` - Optional filter to a specific server (None = all servers)
///
/// # Returns
///
/// Vector of [`MCPLatencyMetrics`] with percentile values per server.
///
/// # Errors
///
/// Returns an error if:
/// - The server name is invalid (if provided)
/// - Database query fails
#[tauri::command]
#[instrument(name = "get_mcp_latency_metrics", skip(state))]
pub async fn get_mcp_latency_metrics(
    server_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<MCPLatencyMetrics>, String> {
    // Validate server_name if provided
    let filter = server_name
        .map(|n| validate_mcp_server_id(&n))
        .transpose()?;

    info!(server = ?filter, "Fetching MCP latency metrics");

    // Build query using SurrealDB percentile function
    let query = match &filter {
        Some(name) => {
            // Use JSON string encoding for proper escaping
            let json_name = serialize_for_query(name, "server_name")?;
            format!(
                r#"SELECT
                    server_name,
                    math::percentile(duration_ms, 0.50) AS p50_ms,
                    math::percentile(duration_ms, 0.95) AS p95_ms,
                    math::percentile(duration_ms, 0.99) AS p99_ms,
                    count() AS total_calls
                FROM mcp_call_log
                WHERE timestamp > time::now() - 1h AND server_name = {}
                GROUP BY server_name"#,
                json_name
            )
        }
        None => r#"SELECT
            server_name,
            math::percentile(duration_ms, 0.50) AS p50_ms,
            math::percentile(duration_ms, 0.95) AS p95_ms,
            math::percentile(duration_ms, 0.99) AS p99_ms,
            count() AS total_calls
        FROM mcp_call_log
        WHERE timestamp > time::now() - 1h
        GROUP BY server_name"#
            .to_string(),
    };

    let mut response = state.db.db.query(&query).await.map_err(|e| {
        error!(error = %e, "Failed to query MCP latency metrics");
        format!("Failed to query MCP latency metrics: {}", e)
    })?;

    let metrics: Vec<MCPLatencyMetrics> = response.take(0).map_err(|e| {
        error!(error = %e, "Failed to parse MCP latency metrics");
        format!("Failed to parse MCP latency metrics: {}", e)
    })?;

    info!(
        server = ?filter,
        count = metrics.len(),
        "MCP latency metrics retrieved"
    );

    Ok(metrics)
}
