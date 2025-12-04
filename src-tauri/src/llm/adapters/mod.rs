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
