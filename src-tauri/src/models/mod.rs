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

pub mod agent;
pub mod chat_block;
pub mod custom_provider;
pub mod embedding;
pub mod function_calling;
pub mod import_export;
pub mod llm_models;
pub mod mcp;
pub mod memory;
pub mod message;
pub mod prompt;
pub mod serde_utils;
pub mod skill;
pub mod streaming;
pub mod sub_agent;
pub mod task;
pub mod thinking_step;
pub mod tool_execution;
pub mod user_question;
pub mod validation;
pub mod workflow;

pub use agent::{
    AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, LLMConfig, Lifecycle,
};
pub use workflow::{
    Workflow, WorkflowCreate, WorkflowFullState, WorkflowMetrics, WorkflowResult, WorkflowStatus,
    WorkflowToolExecution,
};

#[allow(unused_imports)]
pub use memory::{Memory, MemoryCreate, MemoryDescribeResult, MemorySearchResult, MemoryType};
// Re-export memory types with embedding support
#[allow(unused_imports)]
pub use memory::MemoryCreateWithEmbedding;
pub use streaming::{StreamChunk, WorkflowComplete};
// Re-export task types
#[allow(unused_imports)]
pub use task::{Task, TaskCreate, TaskPriority, TaskStatus, TaskUpdate};
// ValidationMode and TimeoutBehavior are used by frontend via UpdateValidationSettingsRequest
#[allow(unused_imports)]
pub use validation::{
    AuditConfig, PartialAuditConfig, PartialRiskThresholds, PartialSelectiveConfig, RiskLevel,
    RiskThresholdConfig, SelectiveValidationConfig, TimeoutBehavior,
    UpdateValidationSettingsRequest, ValidationMode, ValidationRequest, ValidationRequestCreate,
    ValidationSettings, ValidationStatus, ValidationType,
};

// Re-export RAG/streaming types
#[allow(unused_imports)]
pub use memory::MemoryWithEmbedding;
#[allow(unused_imports)]
pub use streaming::{ChunkType, CompletionStatus};

// Re-export MCP types
#[allow(unused_imports)]
pub use mcp::{
    MCPCallLog, MCPDeploymentMethod, MCPResource, MCPServer, MCPServerConfig, MCPServerCreate,
    MCPServerStatus, MCPTestResult, MCPTool, MCPToolCallRequest, MCPToolCallResult,
};

#[allow(unused_imports)]
pub use agent::{Agent, AgentStatus};

// Re-export message types
pub use message::{Message, MessageCreate, PaginatedMessages};
// Re-export MessageRole for future use (currently used in commands/message.rs validation)
#[allow(unused_imports)]
pub use message::MessageRole;

// Re-export tool execution types
pub use tool_execution::{ToolExecution, ToolExecutionCreate};
// Re-export ToolType for future use (currently unused in commands)
#[allow(unused_imports)]
pub use tool_execution::ToolType;

// Re-export thinking step types
pub use thinking_step::{ThinkingStep, ThinkingStepCreate};

// Re-export chat block types
pub use chat_block::{merge_into_chat_blocks, ChatBlock};
// ChatBlockType is used by frontend via IPC
#[allow(unused_imports)]
pub use chat_block::ChatBlockType;

// Re-export sub-agent types
#[allow(unused_imports)]
pub use sub_agent::{
    DelegateResult, ParallelBatchResult, ParallelTaskResult, SubAgentExecution,
    SubAgentExecutionComplete, SubAgentExecutionCreate, SubAgentMetrics, SubAgentSpawnResult,
    SubAgentStatus,
};
// Re-export sub-agent constants
#[allow(unused_imports)]
pub use sub_agent::constants;

// Re-export LLM model types for CRUD operations
#[allow(unused_imports)]
pub use llm_models::get_all_builtin_models;
#[allow(unused_imports)]
pub use llm_models::{
    ConnectionTestResult, CreateModelRequest, LLMModel, ProviderSettings, UpdateModelRequest,
};
// ProviderType canonical location: llm/provider.rs
#[allow(unused_imports)]
pub use crate::llm::ProviderType;

// Re-export embedding settings types
pub use embedding::{
    CategoryTokenStats, EmbeddingConfigSettings, EmbeddingTestResult, ExportFormat, ImportResult,
    MemoryStats, MemoryTokenStats, RegenerateResult,
};

// Re-export prompt library types for Prompt Library feature
#[allow(unused_imports)] // Used in commands/prompt.rs and frontend integration
pub use prompt::{
    Prompt, PromptCategory, PromptCreate, PromptSummary, PromptUpdate, PromptVariable,
    MAX_PROMPT_CONTENT_LEN, MAX_PROMPT_DESCRIPTION_LEN, MAX_PROMPT_NAME_LEN,
};

// Re-export skill types for Skill feature
#[allow(unused_imports)]
pub use skill::{
    Skill, SkillCategory, SkillCreate, SkillSummary, SkillUpdate, MAX_SKILL_CONTENT_LEN,
    MAX_SKILL_DESCRIPTION_LEN, MAX_SKILL_NAME_LEN,
};

// Re-export function calling types for JSON-based tool calling (replacing XML)
// Exported for public API but consumed directly from submodule internally
#[allow(unused_imports)]
pub use function_calling::{
    AssistantToolCall, AssistantToolCallFunction, ChatMessage, FunctionCall, FunctionCallResult,
    ToolChoiceMode,
};

// Re-export user question types for UserQuestionTool implementation
#[allow(unused_imports)]
pub use user_question::{
    QuestionOption, QuestionStatus, QuestionType, UserQuestion, UserQuestionCreate,
    UserQuestionStreamPayload,
};
