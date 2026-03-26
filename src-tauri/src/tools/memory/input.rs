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

//! Structured input parsing and validation for MemoryTool operations.

use crate::tools::constants::memory::VALID_TYPES;
use crate::tools::{ToolError, ToolResult};
use serde_json::Value;

/// Parsed and typed memory operation input.
///
/// This struct reduces the cyclomatic complexity of `validate_input()` and `execute()` by:
/// 1. Extracting JSON parsing into `from_json()`
/// 2. Delegating validation to per-operation methods
/// 3. Providing typed fields for direct use in `execute()`
#[derive(Debug)]
pub struct MemoryInput {
    pub operation: String,
    pub workflow_id: Option<String>,
    pub memory_type: Option<String>,
    pub content: Option<String>,
    pub memory_id: Option<String>,
    pub query: Option<String>,
    pub type_filter: Option<String>,
    pub threshold: Option<f64>,
    pub limit: Option<usize>,
    pub scope: Option<String>,
    pub mode: Option<String>,
    pub metadata: Option<Value>,
    pub tags: Option<Vec<String>>,
}

impl MemoryInput {
    /// Parses JSON input into typed struct.
    pub fn from_json(input: &Value) -> ToolResult<Self> {
        if !input.is_object() {
            return Err(ToolError::InvalidInput(
                "Input must be an object".to_string(),
            ));
        }

        let operation = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("Missing operation field".to_string()))?
            .to_string();

        // Parse tags array if present
        let tags = input["tags"].as_array().map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        });

        Ok(Self {
            operation,
            workflow_id: input["workflow_id"].as_str().map(String::from),
            memory_type: input["type"].as_str().map(String::from),
            content: input["content"].as_str().map(String::from),
            memory_id: input["memory_id"].as_str().map(String::from),
            query: input["query"].as_str().map(String::from),
            type_filter: input["type_filter"].as_str().map(String::from),
            threshold: input["threshold"].as_f64(),
            limit: input["limit"].as_u64().map(|v| v as usize),
            scope: input["scope"].as_str().map(String::from),
            mode: input["mode"].as_str().map(String::from),
            metadata: input.get("metadata").cloned(),
            tags,
        })
    }

    /// Validates input based on operation type.
    pub fn validate(&self) -> ToolResult<()> {
        match self.operation.as_str() {
            "describe" => Ok(()),
            "add" => self.validate_add(),
            "get" | "delete" => self.validate_get_or_delete(),
            "list" => self.validate_type_filter(),
            "search" => self.validate_search(),
            "clear_by_type" => self.validate_clear_by_type(),
            _ => Err(ToolError::InvalidInput(format!(
                "Unknown operation: '{}'. Valid operations: describe, add, get, list, search, delete, clear_by_type",
                self.operation
            ))),
        }
    }

    /// Validates add operation.
    fn validate_add(&self) -> ToolResult<()> {
        if self.memory_type.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'type' for add operation. Valid types: user_pref, context, knowledge, decision".to_string(),
            ));
        }
        if self.content.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'content' for add operation".to_string(),
            ));
        }
        // Validate type value
        if let Some(ref type_str) = self.memory_type {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }

    /// Validates get or delete operation.
    fn validate_get_or_delete(&self) -> ToolResult<()> {
        if self.memory_id.is_none() {
            return Err(ToolError::InvalidInput(format!(
                "Missing 'memory_id' for {} operation",
                self.operation
            )));
        }
        Ok(())
    }

    /// Validates type_filter if present (shared by list and search).
    fn validate_type_filter(&self) -> ToolResult<()> {
        if let Some(ref type_str) = self.type_filter {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type_filter '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }

    /// Validates search operation.
    fn validate_search(&self) -> ToolResult<()> {
        if self.query.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'query' for search operation".to_string(),
            ));
        }
        // Validate type_filter if provided
        self.validate_type_filter()?;
        // Validate threshold if provided
        if let Some(threshold) = self.threshold {
            if !(0.0..=1.0).contains(&threshold) {
                return Err(ToolError::ValidationFailed(format!(
                    "Threshold {} must be between 0 and 1",
                    threshold
                )));
            }
        }
        Ok(())
    }

    /// Validates clear_by_type operation.
    fn validate_clear_by_type(&self) -> ToolResult<()> {
        if self.memory_type.is_none() {
            return Err(ToolError::InvalidInput(
                "Missing 'type' for clear_by_type operation. Valid types: user_pref, context, knowledge, decision".to_string(),
            ));
        }
        // Validate type value
        if let Some(ref type_str) = self.memory_type {
            if !VALID_TYPES.contains(&type_str.as_str()) {
                return Err(ToolError::ValidationFailed(format!(
                    "Invalid type '{}'. Valid types: user_pref, context, knowledge, decision",
                    type_str
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_json_valid_add() {
        let input = serde_json::json!({
            "operation": "add",
            "type": "knowledge",
            "content": "Test content",
            "tags": ["tag1", "tag2"]
        });
        let parsed = MemoryInput::from_json(&input).unwrap();
        assert_eq!(parsed.operation, "add");
        assert_eq!(parsed.memory_type.as_deref(), Some("knowledge"));
        assert_eq!(parsed.content.as_deref(), Some("Test content"));
        assert_eq!(parsed.tags.as_ref().map(|t| t.len()), Some(2));
    }

    #[test]
    fn test_from_json_rejects_non_object() {
        let result = MemoryInput::from_json(&serde_json::json!("not an object"));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => assert!(msg.contains("Input must be an object")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_from_json_rejects_missing_operation() {
        let result = MemoryInput::from_json(&serde_json::json!({"type": "knowledge"}));
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => assert!(msg.contains("Missing operation field")),
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_rejects_unknown_operation() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "unknown_op"
        }))
        .unwrap();
        let result = input.validate();
        assert!(result.is_err());
        match result {
            Err(ToolError::InvalidInput(msg)) => {
                assert!(msg.contains("Unknown operation"));
                assert!(msg.contains("unknown_op"));
            }
            _ => panic!("Expected InvalidInput error"),
        }
    }

    #[test]
    fn test_validate_add_missing_type() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "content": "Test"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_add_missing_content() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "type": "knowledge"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_add_invalid_type() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "add",
            "type": "invalid_type",
            "content": "Test"
        }))
        .unwrap();
        match input.validate() {
            Err(ToolError::ValidationFailed(msg)) => assert!(msg.contains("Invalid type")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_validate_add_all_valid_types() {
        for memory_type in &["user_pref", "context", "knowledge", "decision"] {
            let input = MemoryInput::from_json(&serde_json::json!({
                "operation": "add",
                "type": memory_type,
                "content": "Test content"
            }))
            .unwrap();
            assert!(
                input.validate().is_ok(),
                "Type '{}' should be valid",
                memory_type
            );
        }
    }

    #[test]
    fn test_validate_get_missing_id() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "get"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_delete_missing_id() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "delete"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_search_missing_query() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "search"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_search_threshold_out_of_range() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "search",
            "query": "test",
            "threshold": 1.5
        }))
        .unwrap();
        match input.validate() {
            Err(ToolError::ValidationFailed(msg)) => {
                assert!(msg.contains("Threshold"));
                assert!(msg.contains("must be between 0 and 1"));
            }
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_validate_search_threshold_boundary_values() {
        for threshold in &[0.0, 0.5, 1.0] {
            let input = MemoryInput::from_json(&serde_json::json!({
                "operation": "search",
                "query": "test",
                "threshold": threshold
            }))
            .unwrap();
            assert!(
                input.validate().is_ok(),
                "Threshold {} should be valid",
                threshold
            );
        }
    }

    #[test]
    fn test_validate_list_invalid_type_filter() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "list",
            "type_filter": "invalid_filter"
        }))
        .unwrap();
        match input.validate() {
            Err(ToolError::ValidationFailed(msg)) => assert!(msg.contains("Invalid type_filter")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_validate_clear_by_type_missing_type() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "clear_by_type"
        }))
        .unwrap();
        assert!(input.validate().is_err());
    }

    #[test]
    fn test_validate_clear_by_type_invalid_type() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "clear_by_type",
            "type": "nonexistent"
        }))
        .unwrap();
        match input.validate() {
            Err(ToolError::ValidationFailed(msg)) => assert!(msg.contains("Invalid type")),
            _ => panic!("Expected ValidationFailed error"),
        }
    }

    #[test]
    fn test_validate_describe_valid() {
        let input = MemoryInput::from_json(&serde_json::json!({
            "operation": "describe"
        }))
        .unwrap();
        assert!(input.validate().is_ok());
    }
}
