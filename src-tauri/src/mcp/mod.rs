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

//! MCP (Model Context Protocol) Module
//!
//! This module provides MCP client functionality for Zileo Chat,
//! enabling agents to use tools from external MCP servers.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────┐
//! │               MCPManager                        │
//! │  - Server registry                              │
//! │  - Lifecycle management                         │
//! │  - Tool routing                                 │
//! └───────────────────┬─────────────────────────────┘
//!                     │
//!     ┌───────────────┼───────────────┬─────────────┐
//!     ↓               ↓               ↓             ↓
//! ┌───────────┐ ┌───────────┐ ┌───────────┐ ┌───────────┐
//! │MCPClient  │ │MCPClient  │ │MCPClient  │ │MCPClient  │
//! │ (stdio)   │ │ (stdio)   │ │ (stdio)   │ │ (http)    │
//! └─────┬─────┘ └─────┬─────┘ └─────┬─────┘ └─────┬─────┘
//!       │             │             │             │
//! ┌─────┴─────┐ ┌─────┴─────┐ ┌─────┴─────┐ ┌─────┴─────┐
//! │MCP Server │ │MCP Server │ │MCP Server │ │MCP Server │
//! │ (Docker)  │ │  (NPX)    │ │  (UVX)    │ │  (HTTP)   │
//! └───────────┘ └───────────┘ └───────────┘ └───────────┘
//! ```
//!
//! ## Modules
//!
//! - [`error`]: MCP-specific error types
//! - [`protocol`]: JSON-RPC 2.0 and MCP protocol types
//! - [`server_handle`]: Process spawning and lifecycle management (stdio transport)
//! - [`http_handle`]: HTTP/SSE transport for remote MCP servers
//! - [`client`]: High-level MCP client interface
//! - [`manager`]: MCPManager for server registry and coordination

pub mod circuit_breaker;
pub mod client;
pub mod docker_guard;
pub mod error;
pub mod helpers;
pub mod http_auth;
pub mod http_handle;
pub mod manager;
pub mod network_settings;
pub mod protocol;
pub mod redact;
pub mod secrets;
pub mod server_handle;
pub mod ssrf;

// Re-export commonly used types
pub use error::{MCPError, MCPResult};
pub use manager::MCPManager;
pub use protocol::{
    JsonRpcRequest, JsonRpcResponse, MCPInitializeParams, MCPInitializeResult,
    MCPResourcesListResult, MCPToolCallParams, MCPToolCallResponse, MCPToolsListResult,
};
