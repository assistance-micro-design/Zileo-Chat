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

//! Server-Sent Events (SSE) reader for OpenAI-style streaming chat completions.
//!
//! The sole purpose of this module is to defeat Cloudflare's ~100s origin-idle
//! timeout (HTTP 524) on slow thinking models hosted behind proxied providers
//! (Mistral cloud, OpenRouter, RouterLab, ...). It does NOT expose streaming
//! to the UI, does NOT emit `StreamChunk`s, and does NOT change the
//! adapter contract.
//!
//! [`collect_sse_to_json`] consumes a `reqwest::Response` body as an SSE
//! stream, accumulates the per-chunk deltas into a single
//! `serde_json::Value` that is **structurally identical** to the
//! non-streaming JSON each provider would have returned, then hands it back
//! to [`super::tool_format::send_tool_completion`]. The downstream adapters
//! (`tool_adapter`, `extract_*`, `parse_tool_calls`, `build_assistant_message`)
//! consume the result without modification.
//!
//! Two wire formats are supported, selected by [`ProviderWireFormat`]:
//!
//! - [`ProviderWireFormat::OpenAi`] — `delta.content` is a `string`. Used by
//!   any OpenAI-compatible provider (OpenRouter, vLLM, RouterLab, ...).
//! - [`ProviderWireFormat::Mistral`] — `delta.content` is an object typed
//!   `TextChunk | ThinkChunk`. Native Mistral cloud only.
//!
//! Reasoning surfaces (3 known wire formats: `delta.reasoning` string,
//! `delta.reasoning_content` alias, `delta.reasoning_details[]` array) are
//! handled by the OpenAI accumulator with two independent buckets so they can
//! coexist on a single response.

// Submodules (split of the former monolithic `sse.rs`):
// - `shared` - delta allowlist, wire format, accumulator trait, tool-call fold
// - `openai` - OpenAI-compatible accumulator
// - `mistral` - native Mistral typed-content accumulator
// - `parser` - bounded line-oriented SSE parser
// - `stream` - `collect_sse_to_json`

pub(crate) mod mistral;
pub(crate) mod openai;
pub(crate) mod parser;
pub(crate) mod shared;
pub(crate) mod stream;
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub(crate) use mistral::*;
#[allow(unused_imports)]
pub(crate) use openai::*;
#[allow(unused_imports)]
pub(crate) use parser::*;
pub(crate) use shared::*;
pub(crate) use stream::*;
