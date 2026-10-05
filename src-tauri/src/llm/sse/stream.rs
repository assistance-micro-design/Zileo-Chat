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
//! `collect_sse_to_json`: drain an SSE response into one JSON value.
use super::mistral::MistralAccumulator;
use super::openai::OpenAiAccumulator;
use super::parser::{describe_sse_read_error, SseParser, MAX_SSE_PAYLOAD_BYTES};
use super::shared::{DeltaAccumulator, ProviderWireFormat};
use crate::llm::provider::LLMError;
use serde_json::Value;
use tracing::info;

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
        let bytes = item.map_err(|e| {
            LLMError::RequestFailed(describe_sse_read_error(e.is_timeout(), &e.to_string()))
        })?;
        let parsed = parser.feed(&bytes)?;
        for payload in parsed.events {
            event_count += 1;
            if payload.len() > MAX_SSE_PAYLOAD_BYTES {
                return Err(LLMError::ResponseTooLarge {
                    what: "single payload",
                });
            }
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
