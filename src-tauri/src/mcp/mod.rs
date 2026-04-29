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

// MCP modules contain public API items used by the lib crate and tests,
// but not all are reachable from the binary target. The allow(dead_code)
// prevents false positives in `cargo clippy --all-targets`.
#[allow(dead_code)]
pub mod circuit_breaker;
#[allow(dead_code)]
pub mod client;
pub mod error;
#[allow(dead_code)]
pub mod helpers;
#[allow(dead_code)]
pub mod http_auth;
#[allow(dead_code)]
pub mod http_handle;
pub mod manager;
#[allow(dead_code)]
pub mod protocol;
#[allow(dead_code)]
pub mod redact;
#[allow(dead_code)]
pub mod secrets;
#[allow(dead_code)]
pub mod server_handle;

// Re-export commonly used types
pub use error::{MCPError, MCPResult};
pub use manager::MCPManager;
#[allow(unused_imports)]
pub use protocol::{
    JsonRpcRequest, JsonRpcResponse, MCPContent, MCPInitializeParams, MCPInitializeResult,
    MCPResourcesListResult, MCPToolCallParams, MCPToolCallResponse, MCPToolsListResult,
};
