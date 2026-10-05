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
//! OpenAI-compatible delta accumulator (`delta.content` is a string).
use super::shared::{DeltaAccumulator, ToolCallAcc, KNOWN_DELTA_KEYS};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use tracing::info;

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
    /// Wire field the reasoning string arrived under (`reasoning` for vLLM,
    /// `reasoning_content` for DeepSeek / LM Studio). Preserved so the
    /// reassembled `message` echoes it back under the *same* name on the next
    /// tool-loop turn: DeepSeek (incl. via RouterLab) rejects an echoed turn
    /// whose reasoning is renamed to `reasoning` with HTTP 400, and only
    /// accepts `reasoning_content`. First key seen wins.
    reasoning_field: Option<&'static str>,
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
        // `reasoning_content` (DeepSeek, LM Studio and forks). The source key
        // is remembered (first wins) so finalize can echo it back unchanged —
        // DeepSeek rejects a turn whose reasoning is renamed to `reasoning`.
        for key in ["reasoning", "reasoning_content"] {
            if let Some(piece) = delta.get(key).and_then(|v| v.as_str()) {
                self.reasoning_str.push_str(piece);
                self.reasoning_field.get_or_insert(key);
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
            reasoning_field,
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
            // Echo the reasoning under the field name it arrived in so the next
            // tool-loop turn replays it verbatim (DeepSeek requires
            // `reasoning_content`; vLLM uses `reasoning`). Defaults to
            // `reasoning` when the source key was somehow not recorded.
            message.insert(
                reasoning_field.unwrap_or("reasoning").to_string(),
                Value::String(reasoning_str),
            );
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
