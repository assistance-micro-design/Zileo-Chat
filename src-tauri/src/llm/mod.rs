// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! # LLM Integration Module
//!
//! This module provides the LLM provider abstraction layer using rig-core.
//! It supports multiple providers (Mistral, Ollama) with a unified interface.
//!
//! ## Architecture
//!
//! - [`LLMProvider`] - Trait defining the common interface for all providers
//! - [`ProviderManager`] - Manages provider instances and configuration
//! - [`MistralProvider`] - Mistral AI cloud API integration
//! - [`OllamaProvider`] - Local Ollama server integration
//!
//! ## Usage
//!
//! ```rust,ignore
//! use zileo_chat::llm::{ProviderManager, ProviderType};
//!
//! let manager = ProviderManager::new();
//! manager.set_provider(ProviderType::Mistral, "api-key").await?;
//! let response = manager.complete("Hello", "mistral-large").await?;
//! ```

mod manager;
mod mistral;
mod ollama;
mod provider;

pub use manager::ProviderManager;
pub use provider::{LLMError, ProviderType};

// Re-export for future use (tools, external integrations)
#[allow(unused_imports)]
pub use mistral::MistralProvider;
#[allow(unused_imports)]
pub use ollama::OllamaProvider;
#[allow(unused_imports)]
pub use provider::{LLMProvider, LLMResponse};
