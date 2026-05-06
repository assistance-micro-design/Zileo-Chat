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
//! let manager = ProviderManager::new()?;
//! manager.set_provider(ProviderType::Mistral, "api-key").await?;
//! let response = manager.complete("Hello", "mistral-large").await?;
//! ```

pub mod adapters;
mod cache_control;
pub mod circuit_breaker;
pub mod embedding;
pub mod http;
mod manager;
mod mistral;
mod ollama;
pub mod openai_compatible;
pub mod pricing;
mod provider;
pub mod retry;
pub(crate) mod sse;
pub mod tool_adapter;
pub(crate) mod tool_format;
pub mod utils;

pub use manager::ProviderManager;
pub use ollama::DEFAULT_OLLAMA_URL;
pub use provider::{CompletionParams, LLMError, ProviderType, ToolCompletionParams};
