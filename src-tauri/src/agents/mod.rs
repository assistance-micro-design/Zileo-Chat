// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

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
