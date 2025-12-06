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

//! # Multi-Agent System Module
//!
//! This module provides the infrastructure for the multi-agent architecture.
//!
//! ## Overview
//!
//! The agent system consists of:
//! - [`core::agent::Agent`] - Trait defining agent interface
//! - [`core::AgentRegistry`] - Thread-safe registry for agent discovery
//! - [`core::AgentOrchestrator`] - Coordinator for agent task execution
//! - [`SimpleAgent`] - Base implementation for demonstration
//!
//! ## Agent Lifecycle
//!
//! Agents can be:
//! - **Permanent**: Long-lived agents registered at startup
//! - **Temporary**: Short-lived agents created for specific tasks
//!
//! ## Communication
//!
//! Agents communicate via Markdown reports, providing:
//! - Human-readable output
//! - Machine-parsable structure
//! - Standardized metrics
//!
//! ## Agent Types
//!
//! - [`SimpleAgent`] - Base implementation for demonstration (no LLM calls)
//! - [`LLMAgent`] - Agent that uses real LLM calls via ProviderManager

pub mod core;
pub mod llm_agent;
pub mod simple_agent;

pub use llm_agent::LLMAgent;
// SimpleAgent is used in tests only
#[allow(unused_imports)]
pub use simple_agent::SimpleAgent;
