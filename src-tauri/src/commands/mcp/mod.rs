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

//! MCP (Model Context Protocol) Tauri commands
//!
//! Provides IPC commands for managing MCP server configurations,
//! server lifecycle (start/stop), and tool execution.
//!
//! ## Commands
//!
//! ### Configuration (crud)
//! - `list_mcp_servers` - List all configured MCP servers
//! - `get_mcp_server` - Get a single MCP server by ID
//! - `create_mcp_server` - Create a new MCP server configuration
//! - `update_mcp_server` - Update an existing MCP server
//! - `delete_mcp_server` - Delete an MCP server configuration
//!
//! ### Lifecycle
//! - `start_mcp_server` - Start an MCP server
//! - `stop_mcp_server` - Stop a running MCP server
//! - `test_mcp_server` - Test MCP server connection
//!
//! ### Tools
//! - `list_mcp_tools` - List available tools from a server
//! - `call_mcp_tool` - Execute a tool on an MCP server
//! - `get_mcp_latency_metrics` - Get latency metrics for MCP servers

pub mod crud;
pub mod lifecycle;
pub mod tools;
pub mod validation;

#[cfg(test)]
mod tests;
