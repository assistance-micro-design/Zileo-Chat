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

//! Provider-specific Tool Adapters
//!
//! This module contains adapter implementations for different LLM providers.
//! Each adapter handles the format differences for JSON function calling.
//!
//! ## Supported Providers
//!
//! - **Mistral**: Full function calling support with `tool_call_id`
//! - **Ollama**: OpenAI-compatible format (no native tool_call_id)

mod mistral_adapter;
mod ollama_adapter;

pub use mistral_adapter::MistralToolAdapter;
pub use ollama_adapter::OllamaToolAdapter;

#[cfg(test)]
mod tests;
