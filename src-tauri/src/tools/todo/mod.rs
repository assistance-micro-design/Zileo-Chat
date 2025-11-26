// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
//! enabled = ["TodoTool", "SurrealDBTool"]
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
