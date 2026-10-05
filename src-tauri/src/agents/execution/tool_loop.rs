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

//! Tool execution loop for LLM agents.
//!
//! This module is the orchestrator only. The hot logic lives in sibling
//! modules:
//! - [`super::reasoning`]: emit_progress / emit_reasoning + small format helpers
//! - [`super::completion`]: report enforcement + report content building
//! - [`super::iteration`]: a single pass of the LLM-call → tool-execute loop

// Submodules (split of the former monolithic `tool_loop.rs`):
// - `metrics` - pricing cache, token tracking, metric projection
// - `context` - grouped loop dependencies
// - `init` - initial message assembly
// - `policy` - tool-choice policy + capability lookup
// - `runner` - `execute_simple` + `execute_with_tools`

pub(crate) mod context;
pub(crate) mod init;
pub(crate) mod metrics;
pub(crate) mod policy;
pub(crate) mod runner;
#[cfg(test)]
mod tests;

pub(crate) use context::*;
#[allow(unused_imports)]
pub(crate) use init::*;
pub(crate) use metrics::*;
#[allow(unused_imports)]
pub(crate) use policy::*;
pub(crate) use runner::*;
