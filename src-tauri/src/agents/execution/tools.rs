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

//! Tool management for LLM agent execution.
//!
//! Handles tool creation, definition collection, and individual tool execution
//! for both local tools and MCP tools.
//! Submodules (split of the former monolithic `tools.rs`):
//! - `governance` - pure *Manager write classification + decision
//! - `collection` - MCP tool definition/summary collection
//! - `factory` - local tool instantiation
//! - `definitions` - definition registry + `FunctionCallContext`
//! - `permissions` - detached MCP allowlist + run budgets
//! - `validation` - *Manager ownership + write gate
//! - `dispatcher` - `execute_function_call` orchestration

pub(crate) mod collection;
pub(crate) mod definitions;
pub(crate) mod dispatcher;
pub(crate) mod factory;
pub(crate) mod governance;
pub(crate) mod permissions;
#[cfg(test)]
mod tests;
pub(crate) mod validation;

pub(crate) use collection::*;
pub(crate) use definitions::*;
pub(crate) use dispatcher::*;
pub(crate) use factory::*;
#[allow(unused_imports)]
pub(crate) use governance::*;
pub(crate) use permissions::*;
#[allow(unused_imports)]
pub(crate) use validation::*;
