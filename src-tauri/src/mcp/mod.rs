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
//! - [`server_handle`]: Process spawning and lifecycle management
//! - [`client`]: High-level MCP client interface
//! - [`manager`]: MCPManager for server registry and coordination
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::mcp::{MCPManager, MCPError};
//!
//! // Initialize manager with database
//! let manager = MCPManager::new(db.clone()).await?;
//!
//! // Load servers from database
//! manager.load_from_db().await?;
//!
//! // Or spawn a new server
//! let server = manager.spawn_server(config).await?;
//!
//! // Call a tool
//! let result = manager.call_tool("serena", "find_symbol", args).await?;
//!
//! // Shutdown all servers
//! manager.shutdown().await?;
//! ```

// Allow dead_code for Phase 2 - methods will be used in Phase 3 (Tauri Commands)
#[allow(dead_code)]
pub mod client;
pub mod error;
#[allow(dead_code)]
pub mod manager;
#[allow(dead_code)]
pub mod protocol;
#[allow(dead_code)]
pub mod server_handle;

// Re-export commonly used types
pub use error::{MCPError, MCPResult};
#[allow(unused_imports)]
pub use protocol::{
    JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, MCPClientCapabilities, MCPClientInfo,
    MCPContent, MCPInitializeParams, MCPInitializeResult, MCPResourceContent,
    MCPResourceDefinition, MCPResourcesListResult, MCPServerCapabilities, MCPServerInfo,
    MCPToolCallParams, MCPToolCallResponse, MCPToolDefinition, MCPToolsListResult, MCP_CLIENT_NAME,
    MCP_CLIENT_VERSION, MCP_PROTOCOL_VERSION,
};

// Re-export high-level types for convenience (will be used in Phase 3)
#[allow(unused_imports)]
pub use client::MCPClient;
pub use manager::MCPManager;
#[allow(unused_imports)]
pub use server_handle::MCPServerHandle;
