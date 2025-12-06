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

//! # User Question Tool Module
//!
//! This module provides the UserQuestionTool for LLM agents to ask questions
//! to users via a modal interface.
//!
//! ## Features
//!
//! - Ask users questions with multiple response types (checkbox, text, mixed)
//! - Progressive polling with no timeout
//! - Tauri event-based communication
//! - Workflow-scoped question tracking
//!
//! ## Usage
//!
//! ```rust,ignore
//! use crate::tools::user_question::UserQuestionTool;
//!
//! let tool = UserQuestionTool::new(
//!     db.clone(),
//!     "workflow_123".to_string(),
//!     "agent_id".to_string(),
//!     Some(app_handle),
//! );
//!
//! let result = tool.execute(json!({
//!     "operation": "ask",
//!     "question": "Which file should I process?",
//!     "questionType": "checkbox",
//!     "options": [
//!         {"id": "file1", "label": "config.json"},
//!         {"id": "file2", "label": "data.csv"}
//!     ]
//! })).await?;
//! ```

mod tool;

pub use tool::UserQuestionTool;
