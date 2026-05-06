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

use super::provider::LLMError;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tracing::info;

/// Delta keys recognized by the chat-completions accumulators. Any key in a
/// chunk's `delta` outside this allowlist is reported once at the end of the
/// stream so we can extend the accumulator when a new provider field shows
/// up (e.g. `reasoning_text`, `chain_of_thought`, ...).
///
/// Shared between [`OpenAiAccumulator`] and [`MistralAccumulator`]: the
/// surrounding fields are identical between the two wire formats — only
/// `delta.content` differs (string vs typed object), and that's handled by
/// the per-accumulator ingest logic, not this allowlist.
const KNOWN_DELTA_KEYS: &[&str] = &[
    "role",
    "content",
    "reasoning",
    "reasoning_content",
    "reasoning_details",
    "thinking",
    "tool_calls",
    "function_call", // OpenAI legacy
    "refusal",
    "audio",
];

/// Selects the per-chunk delta accumulator that matches a provider's wire
/// format.
///
/// The accumulator is the only thing that varies between providers. Once the
/// JSON is reconstructed by [`finalize`](DeltaAccumulator::finalize), the rest
/// of the pipeline is provider-agnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProviderWireFormat {
    /// `delta.content` is always a `string` (or absent). Used by OpenRouter,
    /// vLLM, RouterLab, LM Studio, and any other OpenAI-compatible provider.
    OpenAi,
    /// `delta.content` is an object typed `TextChunk | ThinkChunk`, and the
    /// non-stream `message.content` is an **array** mixing both. Native
    /// Mistral cloud only.
    Mistral,
}

/// Trait shared by the two delta accumulators. Internal to this module; see
/// [`OpenAiAccumulator`] and [`MistralAccumulator`].
pub(crate) trait DeltaAccumulator {
    /// Ingest a single SSE event payload (the JSON object that follows
    /// `data:`). The `[DONE]` terminator is handled by the caller and never
    /// reaches this method.
    fn ingest(&mut self, chunk: &Value);

    /// Drain the accumulator and return the reconstructed JSON response,
    /// shaped exactly like the provider's non-streaming body.
    fn finalize(self: Box<Self>) -> Value;
}

/// Accumulates fragments of a single tool call across multiple deltas.
///
/// OpenAI's spec says `id` and `function.name` arrive in the first chunk
/// only, while `function.arguments` is concatenated across subsequent
/// chunks for the same `index`.
#[derive(Debug, Default)]
struct ToolCallAcc {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    /// Tool call type (almost always `"function"`); preserved verbatim if
    /// present so finalize can echo it back.
    call_type: Option<String>,
}

impl ToolCallAcc {
    fn ingest(&mut self, tc: &Value) {
        if self.id.is_none() {
            if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                self.id = Some(id.to_string());
            }
        }
        if self.call_type.is_none() {
            if let Some(t) = tc.get("type").and_then(|v| v.as_str()) {
                self.call_type = Some(t.to_string());
            }
        }
        if let Some(func) = tc.get("function") {
            if self.name.is_none() {
                if let Some(name) = func.get("name").and_then(|v| v.as_str()) {
                    if !name.is_empty() {
                        self.name = Some(name.to_string());
                    }
                }
            }
            if let Some(args) = func.get("arguments").and_then(|v| v.as_str()) {
                self.arguments.push_str(args);
            }
        }
    }

    fn finalize(self) -> Value {
        json!({
            "id": self.id.unwrap_or_default(),
            "type": self.call_type.unwrap_or_else(|| "function".to_string()),
            "function": {
                "name": self.name.unwrap_or_default(),
                "arguments": self.arguments,
            }
        })
    }
}

/// Accumulator for OpenAI-style SSE responses (string `delta.content`).
///
/// Handles all known reasoning surfaces simultaneously:
/// - `delta.reasoning` (vLLM)
/// - `delta.reasoning_content` (LM Studio alias, same bucket as `reasoning`)
/// - `delta.reasoning_details[]` (OpenRouter typed array, separate bucket)
/// - `delta.thinking` (Ollama-style proxies relayed through an OpenAI-compat
///   front-end, separate bucket)
/// - `<think>...</think>` tags inline in `delta.content` (Kimi, DeepSeek,
///   QwQ) — left in the accumulated `content` string so
///   [`crate::llm::utils::extract_thinking_from_message`] can split them at
///   extraction time.
#[derive(Debug, Default)]
pub(crate) struct OpenAiAccumulator {
    id: Option<String>,
    model: Option<String>,
    created: Option<i64>,
    system_fingerprint: Option<String>,
    role: Option<String>,
    content: String,
    reasoning_str: String,
    reasoning_details: Vec<Value>,
    /// Concatenation of `delta.thinking` string fragments. Some Ollama-style
    /// proxies (and a handful of forks) put the reasoning trace there
    /// instead of `delta.reasoning`; mirroring it back as `message.thinking`
    /// lets the shared extractor pick it up.
    thinking_str: String,
    tool_calls: BTreeMap<usize, ToolCallAcc>,
    finish_reason: Option<String>,
    usage: Option<Value>,
    /// Set of delta keys observed but not in [`KNOWN_DELTA_KEYS`].
    /// Reported once at finalize so we can extend the accumulator when a
    /// new provider exposes reasoning under an unfamiliar field name.
    unknown_delta_keys: BTreeSet<String>,
}

impl OpenAiAccumulator {
    pub(crate) fn new() -> Self {
        Self::default()
    }
}

impl DeltaAccumulator for OpenAiAccumulator {
    fn ingest(&mut self, chunk: &Value) {
        // Top-level metadata: only the first chunk usually carries these.
        if self.id.is_none() {
            if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
                self.id = Some(id.to_string());
            }
        }
        if self.model.is_none() {
            if let Some(model) = chunk.get("model").and_then(|v| v.as_str()) {
                self.model = Some(model.to_string());
            }
        }
        if self.created.is_none() {
            if let Some(created) = chunk.get("created").and_then(|v| v.as_i64()) {
                self.created = Some(created);
            }
        }
        if self.system_fingerprint.is_none() {
            if let Some(fp) = chunk.get("system_fingerprint").and_then(|v| v.as_str()) {
                self.system_fingerprint = Some(fp.to_string());
            }
        }

        // Final usage chunk: `choices: []`, only `usage` populated. Save it.
        if let Some(usage) = chunk.get("usage") {
            if !usage.is_null() {
                self.usage = Some(usage.clone());
            }
        }

        let Some(choice) = chunk.pointer("/choices/0") else {
            return;
        };

        if let Some(reason) = choice.get("finish_reason") {
            if !reason.is_null() {
                if let Some(s) = reason.as_str() {
                    self.finish_reason = Some(s.to_string());
                }
            }
        }

        let Some(delta) = choice.get("delta") else {
            return;
        };

        if self.role.is_none() {
            if let Some(role) = delta.get("role").and_then(|v| v.as_str()) {
                self.role = Some(role.to_string());
            }
        }

        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            self.content.push_str(content);
        }

        // Two reasoning string aliases, same bucket: `reasoning` (vLLM) and
        // `reasoning_content` (LM Studio and forks).
        for key in ["reasoning", "reasoning_content"] {
            if let Some(piece) = delta.get(key).and_then(|v| v.as_str()) {
                self.reasoning_str.push_str(piece);
            }
        }

        // OpenRouter typed array: extend in arrival order, opaque payload.
        if let Some(details) = delta.get("reasoning_details").and_then(|v| v.as_array()) {
            for item in details {
                self.reasoning_details.push(item.clone());
            }
        }

        // Ollama-style proxies relayed through an OpenAI-compat front-end
        // (and a few custom-provider forks) put the reasoning trace at
        // `delta.thinking`. Mirror it back as `message.thinking` so the
        // shared extractor surfaces it in the Reflexion UI block.
        if let Some(piece) = delta.get("thinking").and_then(|v| v.as_str()) {
            self.thinking_str.push_str(piece);
        }

        if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tcs {
                let index = tc
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(0);
                self.tool_calls.entry(index).or_default().ingest(tc);
            }
        }

        // Track any delta key we don't already handle so a missing-thinking
        // bug can be triaged from logs (a custom provider may publish
        // reasoning under a name not yet in `KNOWN_DELTA_KEYS`).
        if let Some(obj) = delta.as_object() {
            for key in obj.keys() {
                if !KNOWN_DELTA_KEYS.iter().any(|k| *k == key) {
                    self.unknown_delta_keys.insert(key.clone());
                }
            }
        }
    }

    fn finalize(self: Box<Self>) -> Value {
        let Self {
            id,
            model,
            created,
            system_fingerprint,
            role,
            content,
            reasoning_str,
            reasoning_details,
            thinking_str,
            tool_calls,
            finish_reason,
            usage,
            unknown_delta_keys,
        } = *self;

        if !unknown_delta_keys.is_empty() {
            info!(
                provider_format = "openai",
                unknown_delta_keys = ?unknown_delta_keys,
                "SSE delta contained keys outside the known allowlist — \
                 reasoning may be exposed under one of these. Extend \
                 KNOWN_DELTA_KEYS if a missing-thinking bug is reported."
            );
        }

        let mut message = serde_json::Map::new();
        message.insert(
            "role".to_string(),
            Value::String(role.unwrap_or_else(|| "assistant".to_string())),
        );
        message.insert("content".to_string(), Value::String(content));

        if !tool_calls.is_empty() {
            let calls: Vec<Value> = tool_calls.into_values().map(|tc| tc.finalize()).collect();
            message.insert("tool_calls".to_string(), Value::Array(calls));
        }

        if !reasoning_str.is_empty() {
            message.insert("reasoning".to_string(), Value::String(reasoning_str));
        }

        if !reasoning_details.is_empty() {
            message.insert(
                "reasoning_details".to_string(),
                Value::Array(reasoning_details),
            );
        }

        if !thinking_str.is_empty() {
            message.insert("thinking".to_string(), Value::String(thinking_str));
        }

        let mut choice = serde_json::Map::new();
        choice.insert("index".to_string(), json!(0));
        choice.insert("message".to_string(), Value::Object(message));
        choice.insert(
            "finish_reason".to_string(),
            finish_reason.map(Value::String).unwrap_or(Value::Null),
        );

        let mut out = serde_json::Map::new();
        if let Some(id) = id {
            out.insert("id".to_string(), Value::String(id));
        }
        if let Some(model) = model {
            out.insert("model".to_string(), Value::String(model));
        }
        if let Some(created) = created {
            out.insert("created".to_string(), json!(created));
        }
        if let Some(fp) = system_fingerprint {
            out.insert("system_fingerprint".to_string(), Value::String(fp));
        }
        out.insert(
            "choices".to_string(),
            Value::Array(vec![Value::Object(choice)]),
        );
        if let Some(usage) = usage {
            out.insert("usage".to_string(), usage);
        }
        Value::Object(out)
    }
}

/// Active content slot while reconstructing Mistral's typed-content array.
#[derive(Debug)]
enum MistralContentSlot {
    /// Currently aggregating consecutive `TextChunk` deltas into one block.
    Text(String),
    /// Currently aggregating an open `ThinkChunk` (its inner `thinking` array
    /// is itself made of `TextChunk`s, which we concatenate by appending text
    /// items in arrival order).
    Thinking { items: Vec<Value> },
}

/// Accumulator for native Mistral SSE responses, where `delta.content` is an
/// object typed `TextChunk | ThinkChunk`.
///
/// Reconstructs `message.content` as the array shape Mistral returns when
/// non-streaming (e.g. `[ThinkChunk, TextChunk]`).
///
/// Two thinking variants observed in production:
/// - **Magistral** (and follow-ups): `ThinkChunk.thinking` is an **array** of
///   `TextChunk`s (`[{type:"text", text:"..."}, ...]`).
/// - **mistral-small-3.5 / mistral-medium-3.5** with `reasoning_effort`:
///   `ThinkChunk.thinking` is a **plain string** (`"..."`).
///
/// Both are normalized into the array form at finalize time so
/// [`crate::llm::utils::extract_thinking_from_message`] (which already
/// handles both shapes) sees a consistent input.
///
/// Defensive fallbacks: if a delta carries top-level reasoning surfaces
/// (`delta.reasoning`, `delta.reasoning_content`, `delta.reasoning_details`,
/// `delta.thinking`), they are captured too. Some Mistral routes (notably
/// when relayed via OpenRouter or vLLM-shaped forks) expose reasoning
/// outside the `content` array, and dropping it would silently break the
/// "Reflexion" UI block.
#[derive(Debug, Default)]
pub(crate) struct MistralAccumulator {
    id: Option<String>,
    model: Option<String>,
    created: Option<i64>,
    role: Option<String>,
    content_blocks: Vec<Value>,
    /// Active block being aggregated. Flushed to `content_blocks` when the
    /// type changes or a `ThinkChunk { closed: true }` arrives.
    current: Option<MistralContentSlot>,
    tool_calls: BTreeMap<usize, ToolCallAcc>,
    finish_reason: Option<String>,
    usage: Option<Value>,
    /// Concatenation of any `delta.reasoning` and `delta.reasoning_content`
    /// fragments seen at the top of the delta (NOT inside `content`).
    /// Emitted as `message.reasoning` at finalize.
    reasoning_str: String,
    /// `delta.reasoning_details[]` items appended in arrival order.
    reasoning_details: Vec<Value>,
    /// Concatenation of any `delta.thinking` string fragments at the top of
    /// the delta. Emitted as `message.thinking` at finalize (read by
    /// `extract_thinking_from_message`).
    thinking_str: String,
    /// Set of delta keys observed but not in [`KNOWN_DELTA_KEYS`].
    /// Reported once at finalize for diagnostic use.
    unknown_delta_keys: BTreeSet<String>,
}

/// Append text to the trailing `TextChunk` of `items`, merging if the last
/// item is already text. Used by [`MistralAccumulator`] to normalize both
/// the array-form and string-form of `ThinkChunk.thinking`.
fn append_thinking_text(items: &mut Vec<Value>, text: &str) {
    if text.is_empty() {
        return;
    }
    if let Some(last) = items.last_mut() {
        if last.get("type").and_then(|v| v.as_str()) == Some("text") {
            if let Some(existing) = last
                .get("text")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
            {
                *last = json!({
                    "type": "text",
                    "text": format!("{}{}", existing, text),
                });
                return;
            }
        }
    }
    items.push(json!({"type": "text", "text": text}));
}

impl MistralAccumulator {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn flush_current(&mut self) {
        if let Some(slot) = self.current.take() {
            match slot {
                MistralContentSlot::Text(text) => {
                    if !text.is_empty() {
                        self.content_blocks.push(json!({
                            "type": "text",
                            "text": text,
                        }));
                    }
                }
                MistralContentSlot::Thinking { items } => {
                    self.content_blocks.push(json!({
                        "type": "thinking",
                        "thinking": items,
                    }));
                }
            }
        }
    }

    fn ingest_content_object(&mut self, content: &Value) {
        let chunk_type = content.get("type").and_then(|v| v.as_str()).unwrap_or("");
        match chunk_type {
            "text" => {
                let text = content.get("text").and_then(|v| v.as_str()).unwrap_or("");
                match &mut self.current {
                    Some(MistralContentSlot::Text(buf)) => buf.push_str(text),
                    Some(MistralContentSlot::Thinking { .. }) => {
                        // Type change: flush the open Thinking, start Text.
                        self.flush_current();
                        self.current = Some(MistralContentSlot::Text(text.to_string()));
                    }
                    None => {
                        self.current = Some(MistralContentSlot::Text(text.to_string()));
                    }
                }
            }
            "thinking" => {
                if !matches!(self.current, Some(MistralContentSlot::Thinking { .. })) {
                    // Type change (or first chunk): flush whatever was open.
                    self.flush_current();
                    self.current = Some(MistralContentSlot::Thinking { items: Vec::new() });
                }
                if let Some(MistralContentSlot::Thinking { items }) = &mut self.current {
                    match content.get("thinking") {
                        Some(Value::Array(inner)) => {
                            // Magistral / Mistral 3.x array form: thinking is
                            // a list of TextChunk-like items. Concatenate
                            // consecutive `text` items into one.
                            for item in inner {
                                if item.get("type").and_then(|v| v.as_str()) == Some("text") {
                                    let text =
                                        item.get("text").and_then(|v| v.as_str()).unwrap_or("");
                                    append_thinking_text(items, text);
                                } else {
                                    items.push(item.clone());
                                }
                            }
                        }
                        Some(Value::String(s)) => {
                            // mistral-small-3.5 / mistral-medium-3.5 with
                            // reasoning_effort: thinking arrives as a plain
                            // string. Normalize into the array form so the
                            // finalize shape stays uniform across variants.
                            append_thinking_text(items, s);
                        }
                        _ => {}
                    }
                }
                if content.get("closed").and_then(|v| v.as_bool()) == Some(true) {
                    self.flush_current();
                }
            }
            _ => {
                // Unknown chunk type: pass-through as-is to avoid silently
                // dropping content (defense in depth).
                self.flush_current();
                self.content_blocks.push(content.clone());
            }
        }
    }
}

impl DeltaAccumulator for MistralAccumulator {
    fn ingest(&mut self, chunk: &Value) {
        if self.id.is_none() {
            if let Some(id) = chunk.get("id").and_then(|v| v.as_str()) {
                self.id = Some(id.to_string());
            }
        }
        if self.model.is_none() {
            if let Some(model) = chunk.get("model").and_then(|v| v.as_str()) {
                self.model = Some(model.to_string());
            }
        }
        if self.created.is_none() {
            if let Some(created) = chunk.get("created").and_then(|v| v.as_i64()) {
                self.created = Some(created);
            }
        }

        if let Some(usage) = chunk.get("usage") {
            if !usage.is_null() {
                self.usage = Some(usage.clone());
            }
        }

        let Some(choice) = chunk.pointer("/choices/0") else {
            return;
        };

        if let Some(reason) = choice.get("finish_reason") {
            if !reason.is_null() {
                if let Some(s) = reason.as_str() {
                    self.finish_reason = Some(s.to_string());
                }
            }
        }

        let Some(delta) = choice.get("delta") else {
            return;
        };

        if self.role.is_none() {
            if let Some(role) = delta.get("role").and_then(|v| v.as_str()) {
                self.role = Some(role.to_string());
            }
        }

        if let Some(content) = delta.get("content") {
            match content {
                Value::Object(_) => self.ingest_content_object(content),
                Value::String(s) => {
                    // Some Mistral models / older endpoints still emit a plain
                    // string. Treat it like a TextChunk to stay compatible.
                    let synthesized = json!({"type": "text", "text": s});
                    self.ingest_content_object(&synthesized);
                }
                Value::Array(items) => {
                    for item in items {
                        self.ingest_content_object(item);
                    }
                }
                _ => {}
            }
        }

        // Defensive capture of reasoning surfaces published outside `content`.
        // Some Mistral routes (relayed through OpenRouter or vLLM-shaped
        // forks) expose reasoning at `delta.reasoning` / `delta.reasoning_content`
        // / `delta.reasoning_details[]` / `delta.thinking` instead of (or in
        // addition to) `delta.content` ThinkChunks. Without this, reasoning
        // never reaches the Reflexion UI block.
        for key in ["reasoning", "reasoning_content"] {
            if let Some(piece) = delta.get(key).and_then(|v| v.as_str()) {
                self.reasoning_str.push_str(piece);
            }
        }
        if let Some(details) = delta.get("reasoning_details").and_then(|v| v.as_array()) {
            for item in details {
                self.reasoning_details.push(item.clone());
            }
        }
        if let Some(piece) = delta.get("thinking").and_then(|v| v.as_str()) {
            self.thinking_str.push_str(piece);
        }

        if let Some(tcs) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            for tc in tcs {
                let index = tc
                    .get("index")
                    .and_then(|v| v.as_u64())
                    .map(|n| n as usize)
                    .unwrap_or(0);
                self.tool_calls.entry(index).or_default().ingest(tc);
            }
        }

        if let Some(obj) = delta.as_object() {
            for key in obj.keys() {
                if !KNOWN_DELTA_KEYS.iter().any(|k| *k == key) {
                    self.unknown_delta_keys.insert(key.clone());
                }
            }
        }
    }

    fn finalize(mut self: Box<Self>) -> Value {
        self.flush_current();
        let Self {
            id,
            model,
            created,
            role,
            content_blocks,
            current: _,
            tool_calls,
            finish_reason,
            usage,
            reasoning_str,
            reasoning_details,
            thinking_str,
            unknown_delta_keys,
        } = *self;

        if !unknown_delta_keys.is_empty() {
            info!(
                provider_format = "mistral",
                unknown_delta_keys = ?unknown_delta_keys,
                "SSE delta contained keys outside the known allowlist — \
                 reasoning may be exposed under one of these. Extend \
                 KNOWN_DELTA_KEYS if a missing-thinking bug is reported."
            );
        }

        let mut message = serde_json::Map::new();
        message.insert(
            "role".to_string(),
            Value::String(role.unwrap_or_else(|| "assistant".to_string())),
        );
        message.insert("content".to_string(), Value::Array(content_blocks));

        if !tool_calls.is_empty() {
            let calls: Vec<Value> = tool_calls.into_values().map(|tc| tc.finalize()).collect();
            message.insert("tool_calls".to_string(), Value::Array(calls));
        }

        // Surface defensive reasoning captures so
        // `extract_thinking_from_message` can find them. The function checks
        // `reasoning` / `reasoning_content` / `reasoning_details` / `thinking`
        // before falling back to the content array.
        if !reasoning_str.is_empty() {
            message.insert("reasoning".to_string(), Value::String(reasoning_str));
        }
        if !reasoning_details.is_empty() {
            message.insert(
                "reasoning_details".to_string(),
                Value::Array(reasoning_details),
            );
        }
        if !thinking_str.is_empty() {
            message.insert("thinking".to_string(), Value::String(thinking_str));
        }

        let mut choice = serde_json::Map::new();
        choice.insert("index".to_string(), json!(0));
        choice.insert("message".to_string(), Value::Object(message));
        choice.insert(
            "finish_reason".to_string(),
            finish_reason.map(Value::String).unwrap_or(Value::Null),
        );

        let mut out = serde_json::Map::new();
        if let Some(id) = id {
            out.insert("id".to_string(), Value::String(id));
        }
        if let Some(model) = model {
            out.insert("model".to_string(), Value::String(model));
        }
        if let Some(created) = created {
            out.insert("created".to_string(), json!(created));
        }
        out.insert(
            "choices".to_string(),
            Value::Array(vec![Value::Object(choice)]),
        );
        if let Some(usage) = usage {
            out.insert("usage".to_string(), usage);
        }
        Value::Object(out)
    }
}

/// Outcome of feeding bytes to the line-oriented SSE parser.
#[derive(Debug, Default)]
struct SseParseResult {
    /// Parsed `data:` payloads ready to feed the accumulator.
    events: Vec<String>,
    /// Whether the terminating `data: [DONE]` marker was seen.
    done: bool,
}

/// Stateful line-oriented SSE parser.
///
/// Buffers incoming bytes (which may arrive split across TCP frames), splits
/// on `\n\n` event boundaries, then within each event extracts every
/// `data:` line. Comment lines (`:`-prefixed, used as keep-alives) are
/// silently dropped. The `[DONE]` sentinel is recognized verbatim and
/// stops accumulation.
#[derive(Debug, Default)]
struct SseParser {
    buffer: String,
}

impl SseParser {
    fn new() -> Self {
        Self::default()
    }

    /// Append a chunk of bytes (lossy UTF-8 conversion) and return any
    /// completed events.
    fn feed(&mut self, bytes: &[u8]) -> SseParseResult {
        // SSE is required to be UTF-8. Use lossy to avoid panicking on the
        // off chance a proxy hands us a malformed byte; lost chars are
        // replaced with U+FFFD which won't parse as JSON and will be
        // ignored downstream.
        self.buffer.push_str(&String::from_utf8_lossy(bytes));
        let mut result = SseParseResult::default();

        while let Some(boundary) = find_event_boundary(&self.buffer) {
            let (event_block, _len) = boundary;
            let raw_event = self.buffer[..event_block].to_string();
            // Drop the consumed event including its terminator.
            let consumed = event_block + boundary.1;
            self.buffer.drain(..consumed);

            if let Some(payload) = parse_event_block(&raw_event) {
                if payload.trim() == "[DONE]" {
                    result.done = true;
                    return result;
                }
                result.events.push(payload);
            }
        }

        result
    }
}

/// Locate the first `\n\n` (or `\r\n\r\n`) terminator in the buffer.
///
/// Returns `(event_len, terminator_len)` so the caller can drain both. None
/// when no full event is present yet.
fn find_event_boundary(buf: &str) -> Option<(usize, usize)> {
    if let Some(idx) = buf.find("\r\n\r\n") {
        Some((idx, 4))
    } else {
        buf.find("\n\n").map(|idx| (idx, 2))
    }
}

/// Extract the concatenated `data:` lines of one SSE event, ignoring
/// comments and other field names. Returns `None` if no `data:` line was
/// found (e.g. pure keep-alive or `event:` only).
fn parse_event_block(event: &str) -> Option<String> {
    let mut data = String::new();
    let mut has_data = false;
    for raw_line in event.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        if let Some(rest) = line.strip_prefix("data:") {
            if has_data {
                data.push('\n');
            }
            // Per spec, an optional single space after `data:` is stripped.
            let payload = rest.strip_prefix(' ').unwrap_or(rest);
            data.push_str(payload);
            has_data = true;
        }
        // Other field names (`event:`, `id:`, `retry:`) are intentionally
        // ignored — chat completions APIs only use `data:`.
    }
    if has_data {
        Some(data)
    } else {
        None
    }
}

/// Read an SSE response body and reconstruct the JSON the provider would
/// have returned non-streaming.
///
/// Honors `[DONE]` as the end-of-stream marker. Stops early on stream
/// errors, returning whatever was accumulated so far would mask the failure
/// — instead we surface [`LLMError::RequestFailed`] with the underlying
/// `reqwest` error.
pub(crate) async fn collect_sse_to_json(
    response: reqwest::Response,
    format: ProviderWireFormat,
) -> Result<Value, LLMError> {
    use futures_util::StreamExt;

    let mut accumulator: Box<dyn DeltaAccumulator + Send> = match format {
        ProviderWireFormat::OpenAi => Box::new(OpenAiAccumulator::new()),
        ProviderWireFormat::Mistral => Box::new(MistralAccumulator::new()),
    };

    let mut parser = SseParser::new();
    let mut stream = response.bytes_stream();
    let mut event_count: usize = 0;

    while let Some(item) = stream.next().await {
        let bytes =
            item.map_err(|e| LLMError::RequestFailed(format!("SSE stream read failed: {}", e)))?;
        let parsed = parser.feed(&bytes);
        for payload in parsed.events {
            event_count += 1;
            match serde_json::from_str::<Value>(&payload) {
                Ok(chunk) => accumulator.ingest(&chunk),
                Err(e) => {
                    return Err(LLMError::RequestFailed(format!(
                        "Failed to parse SSE chunk JSON: {} (payload prefix: {})",
                        e,
                        crate::tools::utils::safe_truncate(&payload, 200, true)
                    )));
                }
            }
        }
        if parsed.done {
            break;
        }
    }

    let result = accumulator.finalize();

    // Diagnostic summary so a missing thinking / cached tokens regression can
    // be triaged from logs alone, without leaking user content. Only boolean
    // presence flags, lengths, and finish_reason are emitted.
    let message = result.pointer("/choices/0/message");
    let content_shape = match message.and_then(|m| m.get("content")) {
        Some(Value::String(s)) => format!("string({})", s.len()),
        Some(Value::Array(arr)) => format!(
            "array({}, types={:?})",
            arr.len(),
            arr.iter()
                .filter_map(|b| b.get("type").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
        ),
        Some(Value::Null) | None => "missing".to_string(),
        _ => "other".to_string(),
    };
    // Detect `<think>` tags inline (Kimi/DeepSeek/QwQ format) so the log
    // surfaces this case explicitly: the tags are inside `content`, so the
    // earlier presence flags (`has_reasoning`, ...) all read false even
    // when reasoning IS present.
    let has_think_tags_in_content = message
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .map(|s| s.contains("<think>") || s.contains("</think>"))
        .unwrap_or(false);

    info!(
        format = ?format,
        sse_events = event_count,
        content_shape = %content_shape,
        has_reasoning = message.map(|m| m.get("reasoning").is_some()).unwrap_or(false),
        has_reasoning_details = message
            .map(|m| m.get("reasoning_details").is_some())
            .unwrap_or(false),
        has_thinking_field = message.map(|m| m.get("thinking").is_some()).unwrap_or(false),
        has_think_tags_in_content,
        has_tool_calls = message.map(|m| m.get("tool_calls").is_some()).unwrap_or(false),
        finish_reason = ?result.pointer("/choices/0/finish_reason"),
        usage_present = result.get("usage").is_some(),
        cached_tokens = ?result.pointer("/usage/prompt_tokens_details/cached_tokens"),
        cache_write_tokens = ?result.pointer("/usage/prompt_tokens_details/cache_write_tokens"),
        reasoning_tokens = ?result.pointer("/usage/completion_tokens_details/reasoning_tokens"),
        cost = ?result.pointer("/usage/cost"),
        "SSE collected and finalized"
    );

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ingest_all<A: DeltaAccumulator>(mut acc: A, chunks: Vec<Value>) -> Value {
        for c in &chunks {
            acc.ingest(c);
        }
        Box::new(acc).finalize()
    }

    // ---------------- OpenAiAccumulator ----------------

    #[test]
    fn test_openai_accumulate_simple_content() {
        let chunks = vec![
            json!({"id":"x","model":"m","choices":[{"index":0,"delta":{"role":"assistant","content":"He"},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":"llo"},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":" world"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
        assert_eq!(out["choices"][0]["finish_reason"], "stop");
        assert_eq!(out["id"], "x");
        assert_eq!(out["model"], "m");
    }

    #[test]
    fn test_openai_accumulate_reasoning_string_separate() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning":"Let me "},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning":"think..."},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":"Answer"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["reasoning"], "Let me think...");
        assert_eq!(out["choices"][0]["message"]["content"], "Answer");
    }

    #[test]
    fn test_openai_accumulate_reasoning_content_alias() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_content":"part1 "}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_content":"part2"}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"X"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["reasoning"], "part1 part2");
    }

    #[test]
    fn test_openai_accumulate_reasoning_details_array() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"step 1...","index":0}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"step 2...","index":1}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"step 3...","index":2}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"final"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        let details = out["choices"][0]["message"]["reasoning_details"]
            .as_array()
            .unwrap();
        assert_eq!(details.len(), 3);
        assert_eq!(details[0]["text"], "step 1...");
        assert_eq!(details[2]["text"], "step 3...");
    }

    #[test]
    fn test_openai_accumulate_reasoning_redacted_passthrough() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.encrypted","text":"[REDACTED]","signature":"abc"}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        let details = out["choices"][0]["message"]["reasoning_details"]
            .as_array()
            .unwrap();
        assert_eq!(details[0]["text"], "[REDACTED]");
        assert_eq!(details[0]["signature"], "abc");
    }

    /// Custom-provider regression: a few OpenAI-compat front-ends (Ollama
    /// behind a reverse-proxy, certain forks) publish the reasoning trace at
    /// `delta.thinking` instead of `delta.reasoning`. Without this bucket,
    /// the Reflexion UI block never appears for those custom providers.
    #[test]
    fn test_openai_accumulate_thinking_field_string() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"thinking":"step 1 "}}]}),
            json!({"choices":[{"index":0,"delta":{"thinking":"step 2"}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"Answer."},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["thinking"], "step 1 step 2");
        let message = out.pointer("/choices/0/message").unwrap();
        let thinking = crate::llm::utils::extract_thinking_from_message(message);
        assert_eq!(thinking.as_deref(), Some("step 1 step 2"));
    }

    #[test]
    fn test_openai_accumulate_reasoning_str_and_details_coexist() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning":"raw "}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[{"type":"reasoning.text","text":"typed"}]}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning":"more"}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"a"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["reasoning"], "raw more");
        assert_eq!(
            out["choices"][0]["message"]["reasoning_details"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn test_openai_accumulate_tool_calls_single() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"call_1","type":"function","function":{"name":"my_tool","arguments":""}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"function":{"arguments":"{\"a\":"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"function":{"arguments":"1}"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        let calls = out["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["type"], "function");
        assert_eq!(calls[0]["function"]["name"], "my_tool");
        assert_eq!(calls[0]["function"]["arguments"], "{\"a\":1}");
        // Arguments must be JSON-parsable.
        let _: Value =
            serde_json::from_str(calls[0]["function"]["arguments"].as_str().unwrap()).unwrap();
        assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
    }

    #[test]
    fn test_openai_accumulate_tool_calls_interleaved() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"a","type":"function","function":{"name":"first","arguments":"{\""}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":1,"id":"b","type":"function","function":{"name":"second","arguments":"{\""}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"function":{"arguments":"x\":1}"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":1,"function":{"arguments":"y\":2}"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        let calls = out["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0]["id"], "a");
        assert_eq!(calls[0]["function"]["name"], "first");
        assert_eq!(calls[0]["function"]["arguments"], "{\"x\":1}");
        assert_eq!(calls[1]["id"], "b");
        assert_eq!(calls[1]["function"]["name"], "second");
        assert_eq!(calls[1]["function"]["arguments"], "{\"y\":2}");
    }

    #[test]
    fn test_openai_finalize_omits_empty_tool_calls() {
        let chunks =
            vec![json!({"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":"stop"}]})];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert!(out["choices"][0]["message"].get("tool_calls").is_none());
    }

    #[test]
    fn test_openai_finalize_includes_usage_when_present() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":"hi"},"finish_reason":"stop"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":12,"completion_tokens":5,"total_tokens":17}}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["usage"]["prompt_tokens"], 12);
        assert_eq!(out["usage"]["completion_tokens"], 5);
    }

    /// End-to-end check that the OpenRouter prompt caching breakpoints
    /// applied via `cache_control.rs` survive the streaming round-trip.
    /// The provider replies with the detailed `usage.prompt_tokens_details.*`
    /// shape (cached_tokens + cache_write_tokens) and the accumulator must
    /// preserve every sub-pointer so `tool_adapter::extract_usage` reads
    /// the same values it would have read non-streaming. Without this the
    /// cost calculation would silently fall back to "no cache hits".
    #[test]
    fn test_openai_finalize_preserves_cache_control_usage_details() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":"ok"},"finish_reason":"stop"}]}),
            // Final usage chunk modeled on a real OpenRouter response for
            // Anthropic Claude with prompt caching active.
            json!({
                "choices": [],
                "usage": {
                    "prompt_tokens": 5000,
                    "completion_tokens": 800,
                    "total_tokens": 5800,
                    "prompt_tokens_details": {
                        "cached_tokens": 4200,
                        "cache_write_tokens": 600,
                        "audio_tokens": 0
                    },
                    "completion_tokens_details": {
                        "reasoning_tokens": 250
                    },
                    "cost": 0.012345
                }
            }),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        // Every pointer read by `tool_adapter::extract_usage` must resolve.
        assert_eq!(out.pointer("/usage/prompt_tokens").unwrap(), 5000);
        assert_eq!(out.pointer("/usage/completion_tokens").unwrap(), 800);
        assert_eq!(
            out.pointer("/usage/prompt_tokens_details/cached_tokens")
                .unwrap(),
            4200
        );
        assert_eq!(
            out.pointer("/usage/prompt_tokens_details/cache_write_tokens")
                .unwrap(),
            600
        );
        assert_eq!(
            out.pointer("/usage/completion_tokens_details/reasoning_tokens")
                .unwrap(),
            250
        );
        assert!((out.pointer("/usage/cost").unwrap().as_f64().unwrap() - 0.012345).abs() < 1e-9);
    }

    /// Regression: when the assistant turn carries `reasoning_details` (the
    /// Anthropic-via-OpenRouter case where each thinking block has a
    /// cryptographic `signature`), the accumulator must surface it on
    /// `message.reasoning_details` so the next iteration can echo the
    /// blocks back unchanged. Without this the prompt-cache breakpoint
    /// applied by `cache_control.rs` would land on an assistant message
    /// missing its signed reasoning, and OpenRouter would reject the
    /// follow-up request.
    #[test]
    fn test_openai_finalize_reasoning_details_round_trip_for_cache_echo() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"Step A","signature":"sig1"}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"Step B","signature":"sig2"}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"content":"Answer"},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        let details = out
            .pointer("/choices/0/message/reasoning_details")
            .and_then(|v| v.as_array())
            .expect("reasoning_details must be present at the message level");
        assert_eq!(details.len(), 2);
        // Both signatures preserved verbatim — OpenRouter validates these
        // when the assistant message is replayed in the next turn.
        assert_eq!(details[0]["signature"], "sig1");
        assert_eq!(details[1]["signature"], "sig2");
    }

    #[test]
    fn test_openai_finalize_omits_usage_when_absent() {
        let chunks =
            vec![json!({"choices":[{"index":0,"delta":{"content":"x"},"finish_reason":"stop"}]})];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert!(out.get("usage").is_none());
    }

    #[test]
    fn test_openai_finish_reason_takes_last_non_null() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":"a"},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":"b"},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        ];
        let out = ingest_all(OpenAiAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["finish_reason"], "tool_calls");
    }

    // ---------------- MistralAccumulator ----------------

    #[test]
    fn test_mistral_accumulate_text_chunk_concat() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"role":"assistant","content":{"type":"text","text":"Hel"}},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"lo"}},"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":" world"}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "text");
        assert_eq!(content[0]["text"], "Hello world");
    }

    #[test]
    fn test_mistral_accumulate_thinking_chunk_open_then_closed() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":false,
                "thinking":[{"type":"text","text":"ref"}]
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":false,
                "thinking":[{"type":"text","text":"lexion..."}]
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":true,
                "thinking":[]
            }},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 1);
        assert_eq!(content[0]["type"], "thinking");
        let inner = content[0]["thinking"].as_array().unwrap();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0]["text"], "reflexion...");
    }

    #[test]
    fn test_mistral_accumulate_thinking_then_text() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":true,
                "thinking":[{"type":"text","text":"plan"}]
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"text","text":"answer"
            }},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[1]["text"], "answer");
    }

    #[test]
    fn test_mistral_tool_calls_are_openai_compat() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"id":"call_1","type":"function","function":{"name":"f","arguments":"{"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"tool_calls":[
                {"index":0,"function":{"arguments":"\"k\":1}"}}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{},"finish_reason":"tool_calls"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let calls = out["choices"][0]["message"]["tool_calls"]
            .as_array()
            .unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["id"], "call_1");
        assert_eq!(calls[0]["function"]["name"], "f");
        assert_eq!(calls[0]["function"]["arguments"], "{\"k\":1}");
    }

    /// Regression: mistral-small-3.5 / mistral-medium-3.5 ship the
    /// `ThinkChunk.thinking` payload as a **plain string** instead of the
    /// Magistral array form. Without explicit handling, the original
    /// accumulator silently dropped the entire thinking text — the
    /// Reflexion UI block never appeared in the frontend.
    #[test]
    fn test_mistral_accumulate_thinking_string_form() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":false,
                "thinking":"L'utilisateur "
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":false,
                "thinking":"demande X."
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":true,
                "thinking":""
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"text","text":"Voici la reponse."
            }},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(
            content.len(),
            2,
            "expected exactly one Thinking block + one Text block, got: {:?}",
            content
        );
        assert_eq!(content[0]["type"], "thinking");
        // Normalized into the array form so extract_thinking_from_message can
        // walk the content array uniformly.
        let inner = content[0]["thinking"].as_array().unwrap();
        assert_eq!(inner.len(), 1);
        assert_eq!(inner[0]["text"], "L'utilisateur demande X.");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[1]["text"], "Voici la reponse.");
    }

    /// Regression: some Mistral routes (notably when the model is relayed
    /// through OpenRouter or a vLLM-shaped fork) publish reasoning at
    /// `delta.reasoning` instead of `delta.content` ThinkChunks. The
    /// accumulator must mirror it back as `message.reasoning` so the
    /// shared extractor surfaces it.
    #[test]
    fn test_mistral_accumulate_top_level_reasoning_field() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning":"Step 1. "}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning":"Step 2."}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"answer"}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["reasoning"], "Step 1. Step 2.");
    }

    /// Same as above but for `reasoning_content` (LM Studio / vLLM alias).
    #[test]
    fn test_mistral_accumulate_top_level_reasoning_content_alias() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_content":"part1 "}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_content":"part2"}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"x"}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        assert_eq!(out["choices"][0]["message"]["reasoning"], "part1 part2");
    }

    /// Same as above but for the `delta.thinking` string variant
    /// (Ollama-relayed Mistral and some proxies).
    #[test]
    fn test_mistral_accumulate_top_level_thinking_field() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"thinking":"reflexion "}}]}),
            json!({"choices":[{"index":0,"delta":{"thinking":"interne"}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"final"}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        assert_eq!(
            out["choices"][0]["message"]["thinking"],
            "reflexion interne"
        );
    }

    /// `delta.reasoning_details[]` array form (OpenRouter-relayed Mistral).
    #[test]
    fn test_mistral_accumulate_top_level_reasoning_details_array() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"step A"}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"reasoning_details":[
                {"type":"reasoning.text","text":"step B"}
            ]}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"x"}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let details = out["choices"][0]["message"]["reasoning_details"]
            .as_array()
            .unwrap();
        assert_eq!(details.len(), 2);
        assert_eq!(details[0]["text"], "step A");
        assert_eq!(details[1]["text"], "step B");
    }

    /// End-to-end check that the finalized JSON is consumed correctly by
    /// the shared `extract_thinking_from_message` for the new mistral-small/
    /// medium 3.5 string-form thinking. This is the test that would have
    /// caught the regression from the user's report.
    #[test]
    fn test_mistral_string_form_thinking_is_extracted_by_helper() {
        let chunks = vec![
            json!({"choices":[{"index":0,"delta":{"content":{
                "type":"thinking","closed":true,
                "thinking":"Internal trace."
            }}}]}),
            json!({"choices":[{"index":0,"delta":{"content":{"type":"text","text":"OK."}},"finish_reason":"stop"}]}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        let message = out.pointer("/choices/0/message").unwrap();
        let thinking = crate::llm::utils::extract_thinking_from_message(message);
        assert_eq!(thinking.as_deref(), Some("Internal trace."));
    }

    #[test]
    fn test_mistral_finalize_matches_non_stream_shape() {
        // Sample inspired by Mistral's non-stream `reasoning_effort: high` body.
        let chunks = vec![
            json!({"id":"chat-1","model":"magistral-medium","created":1234,"choices":[{"index":0,"delta":{
                "role":"assistant",
                "content":{"type":"thinking","closed":true,"thinking":[{"type":"text","text":"r1"}]}
            },"finish_reason":null}]}),
            json!({"choices":[{"index":0,"delta":{
                "content":{"type":"text","text":"final"}
            },"finish_reason":"stop"}]}),
            json!({"choices":[],"usage":{"prompt_tokens":7,"completion_tokens":3}}),
        ];
        let out = ingest_all(MistralAccumulator::new(), chunks);
        // Same pointers as the non-stream body. Adapters look at:
        // /choices/0/message/content (array), /usage/prompt_tokens, etc.
        assert_eq!(out["id"], "chat-1");
        assert_eq!(out["model"], "magistral-medium");
        assert_eq!(out["choices"][0]["message"]["role"], "assistant");
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(out["usage"]["prompt_tokens"], 7);
        assert_eq!(out["usage"]["completion_tokens"], 3);
        assert_eq!(out["choices"][0]["finish_reason"], "stop");
    }

    // ---------------- Parser SSE ----------------

    #[test]
    fn test_parse_data_line_simple() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"a\":1}\n\n");
        assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
        assert!(!r.done);
    }

    #[test]
    fn test_parse_handles_chunked_event() {
        let mut p = SseParser::new();
        let r1 = p.feed(b"data: {\"a\"");
        assert!(r1.events.is_empty());
        let r2 = p.feed(b":1}\n\n");
        assert_eq!(r2.events, vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn test_parse_done_terminator() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"a\":1}\n\ndata: [DONE]\n\n");
        assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
        assert!(r.done);
    }

    #[test]
    fn test_parse_ignores_comments_and_keepalive() {
        let mut p = SseParser::new();
        let r = p.feed(b": keepalive\n\ndata: {\"x\":2}\n\n");
        assert_eq!(r.events, vec!["{\"x\":2}".to_string()]);
    }

    #[test]
    fn test_parse_handles_multi_event_in_one_chunk() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"a\":1}\n\ndata: {\"b\":2}\n\n");
        assert_eq!(
            r.events,
            vec!["{\"a\":1}".to_string(), "{\"b\":2}".to_string()]
        );
    }

    #[test]
    fn test_parse_handles_crlf() {
        let mut p = SseParser::new();
        let r = p.feed(b"data: {\"a\":1}\r\n\r\n");
        assert_eq!(r.events, vec!["{\"a\":1}".to_string()]);
    }

    #[test]
    fn test_parse_strips_optional_space_after_colon() {
        let mut p = SseParser::new();
        // Per spec, "data: x" and "data:x" are both valid.
        let r1 = p.feed(b"data:no_space\n\n");
        assert_eq!(r1.events, vec!["no_space".to_string()]);
        let r2 = p.feed(b"data: with_space\n\n");
        assert_eq!(r2.events, vec!["with_space".to_string()]);
    }

    // ---------------- collect_sse_to_json (integration) ----------------

    /// Build a `reqwest::Response` from an in-memory SSE body without doing
    /// any network I/O. Lets the integration tests below exercise the full
    /// `bytes_stream` + parser + accumulator pipeline.
    fn fake_sse_response(body: &'static str) -> reqwest::Response {
        let http_resp = http::Response::builder()
            .status(200)
            .header("content-type", "text/event-stream")
            .body(body)
            .expect("test http::Response");
        reqwest::Response::from(http_resp)
    }

    #[tokio::test]
    async fn test_collect_sse_to_json_openai_full_round_trip() {
        let body = concat!(
            "data: {\"id\":\"x\",\"model\":\"m\",\"choices\":[{\"index\":0,\"delta\":{\"role\":\"assistant\",\"content\":\"He\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"llo\"},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\" world\"},\"finish_reason\":\"stop\"}]}\n\n",
            "data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2}}\n\n",
            "data: [DONE]\n\n",
        );
        let resp = fake_sse_response(body);
        let out = collect_sse_to_json(resp, ProviderWireFormat::OpenAi)
            .await
            .unwrap();
        assert_eq!(out["choices"][0]["message"]["content"], "Hello world");
        assert_eq!(out["choices"][0]["finish_reason"], "stop");
        assert_eq!(out["usage"]["prompt_tokens"], 4);
        assert_eq!(out["usage"]["completion_tokens"], 2);
    }

    #[tokio::test]
    async fn test_collect_sse_to_json_mistral_thinking_then_text() {
        let body = concat!(
            "data: {\"id\":\"x\",\"model\":\"magistral\",\"choices\":[{\"index\":0,\"delta\":{",
            "\"role\":\"assistant\",\"content\":{\"type\":\"thinking\",\"closed\":true,",
            "\"thinking\":[{\"type\":\"text\",\"text\":\"plan\"}]}},\"finish_reason\":null}]}\n\n",
            "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":{\"type\":\"text\",\"text\":\"ok\"}},\"finish_reason\":\"stop\"}]}\n\n",
            "data: [DONE]\n\n",
        );
        let resp = fake_sse_response(body);
        let out = collect_sse_to_json(resp, ProviderWireFormat::Mistral)
            .await
            .unwrap();
        let content = out["choices"][0]["message"]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[1]["type"], "text");
        assert_eq!(content[1]["text"], "ok");
    }

    #[tokio::test]
    async fn test_collect_sse_to_json_rejects_malformed_chunk() {
        let body = "data: {\"oops\": notjson}\n\n";
        let resp = fake_sse_response(body);
        let err = collect_sse_to_json(resp, ProviderWireFormat::OpenAi)
            .await
            .unwrap_err();
        // Surface a structured `RequestFailed` error rather than panicking
        // or returning a half-baked accumulator.
        assert!(matches!(err, LLMError::RequestFailed(_)));
    }
}
