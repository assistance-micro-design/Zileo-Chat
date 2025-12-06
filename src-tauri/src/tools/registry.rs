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

//! Centralized registry for tool discovery and validation.

use std::collections::HashMap;

/// Categories of tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    Basic,
    SubAgent,
}

/// Metadata for a registered tool.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ToolMetadata {
    pub name: &'static str,
    pub category: ToolCategory,
    pub requires_context: bool,
}

/// Centralized registry for all available tools.
pub struct ToolRegistry {
    tools: HashMap<&'static str, ToolMetadata>,
}

impl ToolRegistry {
    /// Creates a new registry with all tools registered.
    pub fn new() -> Self {
        let mut tools = HashMap::new();

        // Basic tools
        tools.insert(
            "MemoryTool",
            ToolMetadata {
                name: "MemoryTool",
                category: ToolCategory::Basic,
                requires_context: false,
            },
        );
        tools.insert(
            "TodoTool",
            ToolMetadata {
                name: "TodoTool",
                category: ToolCategory::Basic,
                requires_context: false,
            },
        );
        tools.insert(
            "CalculatorTool",
            ToolMetadata {
                name: "CalculatorTool",
                category: ToolCategory::Basic,
                requires_context: false,
            },
        );
        tools.insert(
            "UserQuestionTool",
            ToolMetadata {
                name: "UserQuestionTool",
                category: ToolCategory::Basic,
                requires_context: false,
            },
        );

        // Sub-agent tools
        tools.insert(
            "SpawnAgentTool",
            ToolMetadata {
                name: "SpawnAgentTool",
                category: ToolCategory::SubAgent,
                requires_context: true,
            },
        );
        tools.insert(
            "DelegateTaskTool",
            ToolMetadata {
                name: "DelegateTaskTool",
                category: ToolCategory::SubAgent,
                requires_context: true,
            },
        );
        tools.insert(
            "ParallelTasksTool",
            ToolMetadata {
                name: "ParallelTasksTool",
                category: ToolCategory::SubAgent,
                requires_context: true,
            },
        );

        Self { tools }
    }

    /// Checks if a tool exists.
    pub fn has_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// Returns metadata for a tool.
    #[allow(dead_code)]
    pub fn get(&self, name: &str) -> Option<&ToolMetadata> {
        self.tools.get(name)
    }

    /// Checks if a tool requires AgentToolContext.
    pub fn requires_context(&self, name: &str) -> bool {
        self.tools
            .get(name)
            .map(|m| m.requires_context)
            .unwrap_or(false)
    }

    /// Returns all tool names.
    pub fn available_tools(&self) -> Vec<&'static str> {
        self.tools.keys().copied().collect()
    }

    /// Returns basic tools (no context needed).
    pub fn basic_tools(&self) -> Vec<&'static str> {
        self.tools
            .iter()
            .filter(|(_, m)| m.category == ToolCategory::Basic)
            .map(|(name, _)| *name)
            .collect()
    }

    /// Returns sub-agent tools (context needed).
    pub fn sub_agent_tools(&self) -> Vec<&'static str> {
        self.tools
            .iter()
            .filter(|(_, m)| m.category == ToolCategory::SubAgent)
            .map(|(name, _)| *name)
            .collect()
    }

    /// Validates a tool name, returns error if unknown.
    pub fn validate(&self, name: &str) -> Result<&ToolMetadata, String> {
        self.tools.get(name).ok_or_else(|| {
            format!(
                "Unknown tool: '{}'. Available tools: {:?}",
                name,
                self.available_tools()
            )
        })
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton
lazy_static::lazy_static! {
    pub static ref TOOL_REGISTRY: ToolRegistry = ToolRegistry::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_has_all_basic_tools() {
        assert!(TOOL_REGISTRY.has_tool("MemoryTool"));
        assert!(TOOL_REGISTRY.has_tool("TodoTool"));
        assert!(TOOL_REGISTRY.has_tool("CalculatorTool"));
    }

    #[test]
    fn test_registry_has_all_sub_agent_tools() {
        assert!(TOOL_REGISTRY.has_tool("SpawnAgentTool"));
        assert!(TOOL_REGISTRY.has_tool("DelegateTaskTool"));
        assert!(TOOL_REGISTRY.has_tool("ParallelTasksTool"));
    }

    #[test]
    fn test_registry_unknown_tool() {
        assert!(!TOOL_REGISTRY.has_tool("UnknownTool"));
    }

    #[test]
    fn test_registry_validate_valid() {
        let result = TOOL_REGISTRY.validate("MemoryTool");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "MemoryTool");
    }

    #[test]
    fn test_registry_validate_invalid() {
        let result = TOOL_REGISTRY.validate("FakeTool");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown tool"));
    }

    #[test]
    fn test_registry_categories() {
        let basic = TOOL_REGISTRY.basic_tools();
        assert!(basic.contains(&"MemoryTool"));
        assert!(basic.contains(&"TodoTool"));
        assert!(basic.contains(&"CalculatorTool"));
        assert!(basic.contains(&"UserQuestionTool"));
        assert!(!basic.contains(&"SpawnAgentTool"));
        assert_eq!(basic.len(), 4);

        let sub_agent = TOOL_REGISTRY.sub_agent_tools();
        assert!(sub_agent.contains(&"SpawnAgentTool"));
        assert!(sub_agent.contains(&"DelegateTaskTool"));
        assert!(sub_agent.contains(&"ParallelTasksTool"));
        assert!(!sub_agent.contains(&"MemoryTool"));
        assert_eq!(sub_agent.len(), 3);
    }

    #[test]
    fn test_registry_requires_context() {
        assert!(!TOOL_REGISTRY.requires_context("MemoryTool"));
        assert!(!TOOL_REGISTRY.requires_context("TodoTool"));
        assert!(!TOOL_REGISTRY.requires_context("UserQuestionTool"));
        assert!(TOOL_REGISTRY.requires_context("SpawnAgentTool"));
        assert!(TOOL_REGISTRY.requires_context("DelegateTaskTool"));
        assert!(TOOL_REGISTRY.requires_context("ParallelTasksTool"));
    }

    #[test]
    fn test_registry_available_tools_count() {
        let all = TOOL_REGISTRY.available_tools();
        assert_eq!(all.len(), 7); // 4 basic + 3 sub-agent
    }

    #[test]
    fn test_registry_calculator_tool() {
        assert!(TOOL_REGISTRY.has_tool("CalculatorTool"));
        let metadata = TOOL_REGISTRY.get("CalculatorTool").unwrap();
        assert_eq!(metadata.name, "CalculatorTool");
        assert_eq!(metadata.category, ToolCategory::Basic);
        assert!(!metadata.requires_context);
    }
}
