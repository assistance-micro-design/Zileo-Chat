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

//! MCP-specific error types
//!
//! This module defines error types for MCP operations including
//! process spawning, JSON-RPC communication, and protocol errors.

use std::fmt;

/// MCP operation error
///
/// Represents all possible errors that can occur during MCP operations.
#[derive(Debug)]
pub enum MCPError {
    /// Failed to spawn the MCP server process
    ProcessSpawnFailed {
        /// Command that was attempted
        command: String,
        /// Underlying error message
        message: String,
    },
    /// Failed to connect to the MCP server
    ConnectionFailed {
        /// Server name
        server: String,
        /// Underlying error message
        message: String,
    },
    /// JSON-RPC protocol error
    ProtocolError {
        /// Error code from JSON-RPC
        code: i32,
        /// Error message
        message: String,
    },
    /// Server initialization failed
    InitializationFailed {
        /// Server name
        server: String,
        /// Underlying error message
        message: String,
    },
    /// Requested tool was not found on the server
    ToolNotFound {
        /// Server name
        server: String,
        /// Tool name that was requested
        tool: String,
    },
    /// Requested server was not found in the registry
    ServerNotFound {
        /// Server name that was requested
        server: String,
    },
    /// Server is not in a running state
    ServerNotRunning {
        /// Server name
        server: String,
        /// Current status
        status: String,
    },
    /// Operation timed out
    Timeout {
        /// Operation that timed out
        operation: String,
        /// Timeout duration in milliseconds
        timeout_ms: u64,
    },
    /// I/O error during communication
    IoError {
        /// Context of the I/O operation
        context: String,
        /// Underlying error message
        message: String,
    },
    /// JSON serialization/deserialization error
    SerializationError {
        /// Context of the serialization
        context: String,
        /// Underlying error message
        message: String,
    },
    /// Database operation failed
    DatabaseError {
        /// Database operation context
        context: String,
        /// Underlying error message
        message: String,
    },
    /// Server already exists
    ServerAlreadyExists {
        /// Server name or ID
        server: String,
    },
    /// Invalid configuration
    InvalidConfig {
        /// Configuration field that is invalid
        field: String,
        /// Reason for invalidity
        reason: String,
    },
    /// Circuit breaker is open (server unhealthy)
    CircuitBreakerOpen {
        /// Server name
        server: String,
        /// Remaining cooldown in seconds before retry
        cooldown_remaining_secs: u64,
    },
}

impl fmt::Display for MCPError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MCPError::ProcessSpawnFailed { command, message } => {
                write!(f, "Failed to spawn MCP process '{}': {}", command, message)
            }
            MCPError::ConnectionFailed { server, message } => {
                write!(
                    f,
                    "Failed to connect to MCP server '{}': {}",
                    server, message
                )
            }
            MCPError::ProtocolError { code, message } => {
                write!(f, "JSON-RPC protocol error (code {}): {}", code, message)
            }
            MCPError::InitializationFailed { server, message } => {
                write!(
                    f,
                    "Failed to initialize MCP server '{}': {}",
                    server, message
                )
            }
            MCPError::ToolNotFound { server, tool } => {
                write!(f, "Tool '{}' not found on MCP server '{}'", tool, server)
            }
            MCPError::ServerNotFound { server } => {
                write!(f, "MCP server '{}' not found in registry", server)
            }
            MCPError::ServerNotRunning { server, status } => {
                write!(
                    f,
                    "MCP server '{}' is not running (status: {})",
                    server, status
                )
            }
            MCPError::Timeout {
                operation,
                timeout_ms,
            } => {
                write!(
                    f,
                    "Operation '{}' timed out after {}ms",
                    operation, timeout_ms
                )
            }
            MCPError::IoError { context, message } => {
                write!(f, "I/O error during {}: {}", context, message)
            }
            MCPError::SerializationError { context, message } => {
                write!(f, "Serialization error during {}: {}", context, message)
            }
            MCPError::DatabaseError { context, message } => {
                write!(f, "Database error during {}: {}", context, message)
            }
            MCPError::ServerAlreadyExists { server } => {
                write!(f, "MCP server '{}' already exists", server)
            }
            MCPError::InvalidConfig { field, reason } => {
                write!(f, "Invalid MCP configuration for '{}': {}", field, reason)
            }
            MCPError::CircuitBreakerOpen {
                server,
                cooldown_remaining_secs,
            } => {
                write!(
                    f,
                    "Circuit breaker open for MCP server '{}': retry in {}s",
                    server, cooldown_remaining_secs
                )
            }
        }
    }
}

impl std::error::Error for MCPError {}

impl From<std::io::Error> for MCPError {
    fn from(err: std::io::Error) -> Self {
        MCPError::IoError {
            context: "I/O operation".to_string(),
            message: err.to_string(),
        }
    }
}

impl From<serde_json::Error> for MCPError {
    fn from(err: serde_json::Error) -> Self {
        MCPError::SerializationError {
            context: "JSON processing".to_string(),
            message: err.to_string(),
        }
    }
}

/// Convert MCPError to a String for Tauri command error handling
impl From<MCPError> for String {
    fn from(err: MCPError) -> Self {
        err.to_string()
    }
}

/// Result type alias for MCP operations
pub type MCPResult<T> = Result<T, MCPError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_spawn_error_display() {
        let err = MCPError::ProcessSpawnFailed {
            command: "docker".to_string(),
            message: "Permission denied".to_string(),
        };
        assert!(err.to_string().contains("docker"));
        assert!(err.to_string().contains("Permission denied"));
    }

    #[test]
    fn test_connection_failed_display() {
        let err = MCPError::ConnectionFailed {
            server: "serena".to_string(),
            message: "Connection refused".to_string(),
        };
        assert!(err.to_string().contains("serena"));
        assert!(err.to_string().contains("Connection refused"));
    }

    #[test]
    fn test_protocol_error_display() {
        let err = MCPError::ProtocolError {
            code: -32600,
            message: "Invalid Request".to_string(),
        };
        assert!(err.to_string().contains("-32600"));
        assert!(err.to_string().contains("Invalid Request"));
    }

    #[test]
    fn test_tool_not_found_display() {
        let err = MCPError::ToolNotFound {
            server: "serena".to_string(),
            tool: "find_symbol".to_string(),
        };
        assert!(err.to_string().contains("find_symbol"));
        assert!(err.to_string().contains("serena"));
    }

    #[test]
    fn test_timeout_display() {
        let err = MCPError::Timeout {
            operation: "initialize".to_string(),
            timeout_ms: 30000,
        };
        assert!(err.to_string().contains("initialize"));
        assert!(err.to_string().contains("30000"));
    }

    #[test]
    fn test_invalid_config_display() {
        let err = MCPError::InvalidConfig {
            field: "server_name".to_string(),
            reason: "Contains invalid characters".to_string(),
        };
        assert!(err.to_string().contains("server_name"));
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_error_to_string_conversion() {
        let err = MCPError::ServerNotFound {
            server: "test".to_string(),
        };
        let s: String = err.into();
        assert!(s.contains("test"));
        assert!(s.contains("not found"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let mcp_err: MCPError = io_err.into();
        match mcp_err {
            MCPError::IoError { context, message } => {
                assert!(context.contains("I/O"));
                assert!(message.contains("File not found"));
            }
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_serde_error_conversion() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid json").unwrap_err();
        let mcp_err: MCPError = json_err.into();
        match mcp_err {
            MCPError::SerializationError {
                context,
                message: _,
            } => {
                assert!(context.contains("JSON"));
            }
            _ => panic!("Expected SerializationError variant"),
        }
    }
}
