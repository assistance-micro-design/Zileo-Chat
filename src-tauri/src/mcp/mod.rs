// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! MCP (Model Context Protocol) Module
//!
//! This module provides MCP client functionality for Zileo-Chat-3,
//! enabling agents to use tools from external MCP servers.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────┐
//! │            MCPManager                   │
//! │  - Server registry                      │
//! │  - Lifecycle management                 │
//! │  - Tool routing                         │
//! └─────────────────┬───────────────────────┘
//!                   │
//!     ┌─────────────┼─────────────┐
//!     ↓             ↓             ↓
//! ┌───────────┐ ┌───────────┐ ┌───────────┐
//! │MCPClient  │ │MCPClient  │ │MCPClient  │
//! │ (stdio)   │ │ (stdio)   │ │ (stdio)   │
//! └─────┬─────┘ └─────┬─────┘ └─────┬─────┘
//!       │             │             │
//! ┌─────┴─────┐ ┌─────┴─────┐ ┌─────┴─────┐
//! │MCP Server │ │MCP Server │ │MCP Server │
//! │ (Docker)  │ │  (NPX)    │ │  (UVX)    │
//! └───────────┘ └───────────┘ └───────────┘
//! ```
//!
//! ## Modules
//!
//! - [`error`]: MCP-specific error types
//! - [`protocol`]: JSON-RPC 2.0 and MCP protocol types
//!
//! ## Future Modules (Phase 2)
//!
//! - `client`: MCPClient for JSON-RPC communication
//! - `server_handle`: Process spawning and lifecycle management
//! - `manager`: MCPManager for server registry and coordination
//!
//! ## Usage (Phase 2)
//!
//! ```rust,ignore
//! use zileo_chat::mcp::{MCPManager, MCPError};
//!
//! // Initialize manager
//! let manager = MCPManager::new(db.clone()).await?;
//!
//! // Spawn a server
//! let server = manager.spawn_server(config).await?;
//!
//! // Call a tool
//! let result = manager.call_tool("serena", "find_symbol", args).await?;
//! ```

pub mod error;
pub mod protocol;

// Re-export commonly used types
pub use error::{MCPError, MCPResult};
pub use protocol::{
    JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, MCPClientCapabilities, MCPClientInfo,
    MCPContent, MCPInitializeParams, MCPInitializeResult, MCPResourceContent,
    MCPResourceDefinition, MCPResourcesListResult, MCPServerCapabilities, MCPServerInfo,
    MCPToolCallParams, MCPToolCallResponse, MCPToolDefinition, MCPToolsListResult, MCP_CLIENT_NAME,
    MCP_CLIENT_VERSION, MCP_PROTOCOL_VERSION,
};
