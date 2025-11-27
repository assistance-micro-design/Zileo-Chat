// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

pub mod agent;
pub mod embedding;
pub mod llm_models;
pub mod mcp;
pub mod memory;
pub mod message;
pub mod serde_utils;
pub mod streaming;
pub mod task;
pub mod thinking_step;
pub mod tool_execution;
pub mod validation;
pub mod workflow;

pub use agent::{
    AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, LLMConfig, Lifecycle,
    KNOWN_TOOLS,
};
pub use workflow::{
    Workflow, WorkflowCreate, WorkflowFullState, WorkflowMetrics, WorkflowResult, WorkflowStatus,
    WorkflowToolExecution,
};

// Re-export types for Phase 5 implementation
pub use memory::{Memory, MemoryCreate, MemorySearchResult, MemoryType};
// Re-export types for Memory Tool Phase 3 implementation (currently unused)
#[allow(unused_imports)]
pub use memory::MemoryCreateWithEmbedding;
pub use streaming::{StreamChunk, WorkflowComplete};
// Re-export task types for Phase 2 Commands implementation (currently unused)
#[allow(unused_imports)]
pub use task::{Task, TaskCreate, TaskPriority, TaskStatus, TaskUpdate};
pub use validation::{
    RiskLevel, ValidationRequest, ValidationRequestCreate, ValidationStatus, ValidationType,
};

// Re-export types for future RAG/streaming phases (currently unused)
#[allow(unused_imports)]
pub use memory::MemoryWithEmbedding;
#[allow(unused_imports)]
pub use streaming::{ChunkType, CompletionStatus};
#[allow(unused_imports)]
pub use validation::ValidationMode;

// Re-export MCP types for future phases (Phase 2: MCP Client, Phase 3: Commands)
#[allow(unused_imports)]
pub use mcp::{
    MCPCallLog, MCPDeploymentMethod, MCPResource, MCPServer, MCPServerConfig, MCPServerCreate,
    MCPServerStatus, MCPTestResult, MCPTool, MCPToolCallRequest, MCPToolCallResult,
};

// Re-export types for future phases
#[allow(unused_imports)]
pub use agent::{Agent, AgentStatus};

// Re-export message types for Phase 6 Message Persistence
pub use message::{Message, MessageCreate, PaginatedMessages};
// Re-export MessageRole for future use (currently used in commands/message.rs validation)
#[allow(unused_imports)]
pub use message::MessageRole;

// Re-export tool execution types for Phase 3 Tool Execution Persistence
pub use tool_execution::{ToolExecution, ToolExecutionCreate};
// Re-export ToolType for future use (currently unused in commands)
#[allow(unused_imports)]
pub use tool_execution::ToolType;

// Re-export thinking step types for Phase 4 Thinking Steps Persistence
pub use thinking_step::{ThinkingStep, ThinkingStepCreate};

// Re-export LLM model types for CRUD operations (Phase 2 will use these)
#[allow(unused_imports)]
pub use llm_models::{
    ConnectionTestResult, CreateModelRequest, LLMModel, ProviderSettings, ProviderType,
    UpdateModelRequest,
};
// Re-export builtin model data (Phase 2 will use these for seeding)
#[allow(unused_imports)]
pub use llm_models::{get_all_builtin_models, MISTRAL_BUILTIN_MODELS, OLLAMA_BUILTIN_MODELS};

// Re-export embedding settings types for Phase 5 implementation
pub use embedding::{
    EmbeddingConfigSettings, EmbeddingTestResult, ExportFormat, ImportResult, MemoryStats,
    RegenerateResult,
};
