// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Agent Tools Framework
//!
//! Provides the trait and types for agent-callable tools.
//!
//! # Overview
//!
//! Tools are the primary mechanism for agents to interact with external systems:
//! - **Local Tools**: Rust-native tools (SurrealDBTool, TodoTool)
//! - **MCP Tools**: Remote tools via MCP protocol
//!
//! # Tool Trait
//!
//! All tools implement the [`Tool`] trait which provides:
//! - `definition()` - Returns metadata for LLM understanding
//! - `execute()` - Performs the tool action with JSON input/output
//! - `validate_input()` - Validates parameters before execution
//! - `requires_confirmation()` - Whether human confirmation is needed
//!
//! # Available Tools
//!
//! | Module | Tool | Description |
//! |--------|------|-------------|
//! | [`db`] | `SurrealDBTool` | Direct CRUD operations on SurrealDB |
//! | [`db`] | `QueryBuilderTool` | SurrealQL query generation |
//! | [`db`] | `AnalyticsTool` | Aggregations and analytics |
//! | [`memory`] | `MemoryTool` | Contextual memory with semantic search |
//! | [`todo`] | `TodoTool` | Task management for workflow decomposition |
//!
//! # Example
//!
//! ```ignore
//! use crate::tools::{Tool, TodoTool};
//! use serde_json::json;
//!
//! let tool = TodoTool::new(db, workflow_id, agent_id);
//! let result = tool.execute(json!({
//!     "operation": "create",
//!     "name": "Analyze code",
//!     "priority": 1
//! })).await?;
//! ```

pub mod db;
pub mod factory;
pub mod memory;
pub mod todo;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

// Re-export tools for agent integration
#[allow(unused_imports)]
pub use db::{AnalyticsTool, QueryBuilderTool, SurrealDBTool};
#[allow(unused_imports)]
pub use factory::ToolFactory;
#[allow(unused_imports)]
pub use memory::MemoryTool;
#[allow(unused_imports)]
pub use todo::TodoTool;

/// Tool definition metadata for LLM understanding.
///
/// Contains all information needed for an LLM to understand when and how
/// to use a tool. The description is critical for tool selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique tool identifier (e.g., "TodoTool")
    pub id: String,
    /// Human-readable name
    pub name: String,
    /// Description for LLM (critical for tool selection)
    pub description: String,
    /// JSON Schema for input validation (OpenAPI 3.0 format)
    pub input_schema: Value,
    /// JSON Schema for output format
    pub output_schema: Value,
    /// Whether the tool requires human confirmation before execution
    pub requires_confirmation: bool,
}

/// Tool execution result type.
#[allow(dead_code)]
pub type ToolResult<T> = Result<T, ToolError>;

/// Errors that can occur during tool execution.
///
/// Each variant provides structured, actionable feedback for agents:
/// - Clear explanation of what went wrong
/// - Suggestion for how to fix the issue
/// - Context about the operation that failed
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum ToolError {
    /// Invalid input parameters - check the input_schema for valid format
    InvalidInput(String),
    /// Tool execution failed - operation could not complete
    ExecutionFailed(String),
    /// Resource not found - verify the ID exists before referencing
    NotFound(String),
    /// Permission denied - operation requires elevated permissions or confirmation
    PermissionDenied(String),
    /// Operation timed out - consider breaking into smaller operations
    Timeout(String),
    /// Database error - persistence layer issue, may be transient
    DatabaseError(String),
    /// Validation failed - input does not meet business rules
    ValidationFailed(String),
    /// Dependency error - required resource is missing or in wrong state
    DependencyError(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(msg) => write!(
                f,
                "[INVALID_INPUT] {}. Check the tool's input_schema for required fields and valid formats.",
                msg
            ),
            Self::ExecutionFailed(msg) => write!(
                f,
                "[EXECUTION_FAILED] {}. The operation could not complete. Review the parameters and retry.",
                msg
            ),
            Self::NotFound(msg) => write!(
                f,
                "[NOT_FOUND] {}. The requested resource does not exist. Verify the ID is correct or use 'list' to find valid IDs.",
                msg
            ),
            Self::PermissionDenied(msg) => write!(
                f,
                "[PERMISSION_DENIED] {}. This operation requires confirmation or elevated permissions.",
                msg
            ),
            Self::Timeout(msg) => write!(
                f,
                "[TIMEOUT] {}. Operation took too long. Consider breaking into smaller tasks or increasing timeout.",
                msg
            ),
            Self::DatabaseError(msg) => write!(
                f,
                "[DATABASE_ERROR] {}. Persistence layer error. This may be transient - retry after a moment.",
                msg
            ),
            Self::ValidationFailed(msg) => write!(
                f,
                "[VALIDATION_FAILED] {}. Input does not meet requirements. Check constraints: name <= 128 chars, description <= 1000 chars, priority 1-5.",
                msg
            ),
            Self::DependencyError(msg) => write!(
                f,
                "[DEPENDENCY_ERROR] {}. A required resource is missing or in wrong state. Check dependencies are completed first.",
                msg
            ),
        }
    }
}

impl std::error::Error for ToolError {}

impl From<String> for ToolError {
    fn from(s: String) -> Self {
        ToolError::ExecutionFailed(s)
    }
}

/// Tool trait - unified interface for all agent tools.
///
/// All tools must implement this trait to be usable by agents.
/// The trait is async-safe and thread-safe (`Send + Sync`).
///
/// # Implementation Guide
///
/// 1. **definition()**: Return comprehensive metadata including JSON schemas
/// 2. **execute()**: Handle all operations via operation dispatch
/// 3. **validate_input()**: Validate against JSON schema before execution
/// 4. **requires_confirmation()**: Return true for destructive operations
///
/// # Example Implementation
///
/// ```ignore
/// #[async_trait]
/// impl Tool for MyTool {
///     fn definition(&self) -> ToolDefinition {
///         ToolDefinition {
///             id: "MyTool".to_string(),
///             name: "My Tool".to_string(),
///             description: "Does something useful".to_string(),
///             input_schema: json!({...}),
///             output_schema: json!({...}),
///             requires_confirmation: false,
///         }
///     }
///
///     async fn execute(&self, input: Value) -> ToolResult<Value> {
///         self.validate_input(&input)?;
///         // ... execute logic
///         Ok(json!({ "success": true }))
///     }
///
///     fn validate_input(&self, input: &Value) -> ToolResult<()> {
///         // ... validation logic
///         Ok(())
///     }
/// }
/// ```
#[allow(dead_code)]
#[async_trait]
pub trait Tool: Send + Sync {
    /// Returns tool definition with description for LLM.
    ///
    /// The description should clearly explain:
    /// - What the tool does
    /// - When to use it
    /// - What operations are available
    /// - Best practices for usage
    fn definition(&self) -> ToolDefinition;

    /// Executes the tool with JSON input.
    ///
    /// # Arguments
    /// * `input` - JSON object with operation and parameters
    ///
    /// # Returns
    /// * `Ok(Value)` - JSON result on success
    /// * `Err(ToolError)` - Error description on failure
    async fn execute(&self, input: Value) -> ToolResult<Value>;

    /// Validates input before execution.
    ///
    /// Should validate:
    /// - Required fields are present
    /// - Field types match schema
    /// - Values are within valid ranges
    /// - Operation-specific requirements
    fn validate_input(&self, input: &Value) -> ToolResult<()>;

    /// Returns true if tool requires human confirmation.
    ///
    /// Override to return `true` for:
    /// - Destructive operations (delete, drop)
    /// - External system modifications
    /// - Operations with side effects
    fn requires_confirmation(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_error_display() {
        // Test that error messages contain the key information
        let error = ToolError::InvalidInput("missing 'name' field".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[INVALID_INPUT]"));
        assert!(msg.contains("missing 'name' field"));
        assert!(msg.contains("input_schema"));

        let error = ToolError::NotFound("task_123".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[NOT_FOUND]"));
        assert!(msg.contains("task_123"));
        assert!(msg.contains("use 'list'"));

        let error = ToolError::ValidationFailed("priority must be 1-5".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[VALIDATION_FAILED]"));
        assert!(msg.contains("priority must be 1-5"));

        let error = ToolError::DependencyError("task_001 is not completed".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[DEPENDENCY_ERROR]"));
        assert!(msg.contains("task_001"));

        let error = ToolError::DatabaseError("connection lost".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[DATABASE_ERROR]"));
        assert!(msg.contains("transient"));

        let error = ToolError::Timeout("operation exceeded 30s".to_string());
        let msg = error.to_string();
        assert!(msg.contains("[TIMEOUT]"));
        assert!(msg.contains("smaller tasks"));
    }

    #[test]
    fn test_tool_definition_serialization() {
        let definition = ToolDefinition {
            id: "TestTool".to_string(),
            name: "Test Tool".to_string(),
            description: "A test tool".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "operation": {"type": "string"}
                }
            }),
            output_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "success": {"type": "boolean"}
                }
            }),
            requires_confirmation: false,
        };

        let json = serde_json::to_string(&definition).unwrap();
        assert!(json.contains("\"id\":\"TestTool\""));
        assert!(json.contains("\"requires_confirmation\":false"));

        let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.id, "TestTool");
        assert_eq!(deserialized.name, "Test Tool");
    }

    #[test]
    fn test_tool_error_from_string() {
        let error: ToolError = "test error".to_string().into();
        assert!(matches!(error, ToolError::ExecutionFailed(_)));
        let msg = error.to_string();
        assert!(msg.contains("[EXECUTION_FAILED]"));
        assert!(msg.contains("test error"));
    }
}
