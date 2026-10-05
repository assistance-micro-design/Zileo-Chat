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
//! Shared SSE vocabulary: delta allowlist, wire format, accumulator trait, tool-call fold.
use serde_json::{json, Value};

/// Delta keys recognized by the chat-completions accumulators. Any key in a
/// chunk's `delta` outside this allowlist is reported once at the end of the
/// stream so we can extend the accumulator when a new provider field shows
/// up (e.g. `reasoning_text`, `chain_of_thought`, ...).
///
/// Shared between [`OpenAiAccumulator`] and [`MistralAccumulator`]: the
/// surrounding fields are identical between the two wire formats — only
/// `delta.content` differs (string vs typed object), and that's handled by
/// the per-accumulator ingest logic, not this allowlist.
pub(crate) const KNOWN_DELTA_KEYS: &[&str] = &[
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
pub(crate) struct ToolCallAcc {
    id: Option<String>,
    name: Option<String>,
    arguments: String,
    /// Tool call type (almost always `"function"`); preserved verbatim if
    /// present so finalize can echo it back.
    call_type: Option<String>,
}

impl ToolCallAcc {
    pub(crate) fn ingest(&mut self, tc: &Value) {
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

    pub(crate) fn finalize(self) -> Value {
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
