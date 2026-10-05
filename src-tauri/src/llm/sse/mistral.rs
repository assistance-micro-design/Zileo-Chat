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
//! Native Mistral typed-content accumulator (`TextChunk | ThinkChunk`).
use super::shared::{DeltaAccumulator, ToolCallAcc, KNOWN_DELTA_KEYS};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tracing::info;

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
