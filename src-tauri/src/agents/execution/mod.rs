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

//! Execution engine for LLM agents.
//!
//! - [`tool_loop`] - Orchestrators (simple and tool-augmented)
//! - [`iteration`] - Single iteration of the tool loop
//! - [`reasoning`] - Reasoning emission helpers
//! - [`completion`] - Report enforcement + report content building
//! - [`tools`] - Tool creation, collection, and execution
//! - [`sequence_tracker`] - Atomic monotonic counter for ordering blocks

pub(crate) mod completion;
pub(crate) mod iteration;
pub(crate) mod reasoning;
pub(crate) mod sequence_tracker;
pub(crate) mod tool_loop;
pub(crate) mod tools;
