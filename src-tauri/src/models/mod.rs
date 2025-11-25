// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

pub mod agent;
pub mod memory;
pub mod message;
pub mod serde_utils;
pub mod streaming;
pub mod validation;
pub mod workflow;

pub use agent::{AgentConfig, LLMConfig, Lifecycle};
pub use workflow::{Workflow, WorkflowCreate, WorkflowMetrics, WorkflowResult, WorkflowStatus};

// Re-export types for Phase 5 implementation
pub use memory::{Memory, MemoryCreate, MemorySearchResult, MemoryType};
pub use streaming::{StreamChunk, WorkflowComplete};
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

// Re-export types for future phases
#[allow(unused_imports)]
pub use agent::{Agent, AgentStatus};
#[allow(unused_imports)]
pub use message::{Message, MessageRole};
