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

//! Data models for the Zileo-Chat application.
//!
//! Types are organized by domain. Consumers should import from the specific
//! submodule (e.g., `models::streaming::StreamChunk`) or use the re-exports
//! below for the most commonly used types.

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
pub mod workflow_folder;

// Re-exports: only types that are actually imported via `crate::models::X`
// elsewhere in the codebase. All other types should be imported from their
// specific submodule (e.g., `crate::models::streaming::events`).

pub use agent::{
    AgentConfig, AgentConfigCreate, AgentConfigUpdate, AgentSummary, LLMConfig, Lifecycle,
};
pub use chat_block::{merge_into_chat_blocks, ChatBlock};
pub use embedding::{
    CategoryTokenStats, EmbeddingConfigSettings, EmbeddingTestResult, ExportFormat, ImportResult,
    MemoryStats, MemoryTokenStats, RegenerateResult,
};
pub use memory::{Memory, MemoryCreate, MemoryCreateWithEmbedding, MemorySearchResult, MemoryType};
pub use message::{Message, MessageCreate, PaginatedMessages};
pub use prompt::Prompt;
pub use streaming::{StreamChunk, WorkflowComplete};
pub use thinking_step::{ThinkingStep, ThinkingStepCreate};
pub use tool_execution::{ToolExecution, ToolExecutionCreate};
pub use user_question::{
    QuestionOption, UserQuestion, UserQuestionCreate, UserQuestionStreamPayload,
};
pub use validation::{
    AuditBucket, AuditConfig, AuditDecision, AuditFilter, AuditStats, DecidedBy,
    PartialAuditConfig, PartialRiskThresholds, PartialSelectiveConfig, RiskLevel,
    RiskThresholdConfig, SelectiveValidationConfig, TimeoutBehavior,
    UpdateValidationSettingsRequest, ValidationAuditEntry, ValidationMode, ValidationRequest,
    ValidationRequestCreate, ValidationSettings, ValidationStatus, ValidationType,
};
pub use workflow::{
    Workflow, WorkflowCreate, WorkflowFullState, WorkflowMetrics, WorkflowResult, WorkflowStatus,
    WorkflowToolExecution,
};
pub use workflow_folder::{WorkflowFolder, WorkflowFolderCreate};
