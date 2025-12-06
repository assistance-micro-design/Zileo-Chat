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

//! Todo Tool - Task management for agents.
//!
//! Allows agents to create, update, and manage workflow tasks.
//!
//! # Overview
//!
//! The TodoTool provides task management capabilities for agents:
//! - Create tasks with name, description, and priority
//! - Update task status (pending/in_progress/completed/blocked)
//! - Query tasks by status or workflow
//! - Mark tasks as completed with timing metrics
//!
//! # Integration
//!
//! Add "TodoTool" to an agent's tools configuration:
//! ```toml
//! [tools]
//! enabled = ["TodoTool", "MemoryTool"]
//! ```
//!
//! # Example
//!
//! ```ignore
//! use crate::tools::TodoTool;
//!
//! let tool = TodoTool::new(db, "workflow_123".into(), "agent_001".into());
//! let result = tool.execute(json!({
//!     "operation": "create",
//!     "name": "Analyze code",
//!     "description": "Deep analysis of the codebase",
//!     "priority": 1
//! })).await?;
//! ```

mod tool;

pub use tool::TodoTool;
