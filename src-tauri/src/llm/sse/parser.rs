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
//! Line-oriented SSE parser with buffer/payload bounds.
use crate::llm::provider::LLMError;

/// Outcome of feeding bytes to the line-oriented SSE parser.
#[derive(Debug, Default)]
pub(crate) struct SseParseResult {
    /// Parsed `data:` payloads ready to feed the accumulator.
    pub(crate) events: Vec<String>,
    /// Whether the terminating `data: [DONE]` marker was seen.
    pub(crate) done: bool,
}

/// Stateful line-oriented SSE parser.
///
/// Maximum size of the [`SseParser`] internal buffer before we abort the
/// stream. A misbehaving upstream that never sends an event terminator
/// (`\n\n`) would otherwise let the buffer grow without bound.
pub(crate) const MAX_SSE_BUFFER_BYTES: usize = 16 * 1024 * 1024;

/// Maximum size of a single SSE `data:` payload before it is parsed as JSON.
/// 4 MiB is well above any realistic single-chunk completion (largest reported
/// thinking-model chunks are ~150 KB) while still bounding `serde_json::from_str`
/// CPU cost.
pub(crate) const MAX_SSE_PAYLOAD_BYTES: usize = 4 * 1024 * 1024;

/// Buffers incoming bytes (which may arrive split across TCP frames), splits
/// on `\n\n` event boundaries, then within each event extracts every
/// `data:` line. Comment lines (`:`-prefixed, used as keep-alives) are
/// silently dropped. The `[DONE]` sentinel is recognized verbatim and
/// stops accumulation.
#[derive(Debug, Default)]
pub(crate) struct SseParser {
    buffer: String,
}

impl SseParser {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Append a chunk of bytes (lossy UTF-8 conversion) and return any
    /// completed events. Errors with [`LLMError::ResponseTooLarge`] when the
    /// internal buffer would exceed [`MAX_SSE_BUFFER_BYTES`].
    pub(crate) fn feed(&mut self, bytes: &[u8]) -> Result<SseParseResult, LLMError> {
        // SSE is required to be UTF-8. Use lossy to avoid panicking on the
        // off chance a proxy hands us a malformed byte; lost chars are
        // replaced with U+FFFD which won't parse as JSON and will be
        // ignored downstream.
        self.buffer.push_str(&String::from_utf8_lossy(bytes));

        if self.buffer.len() > MAX_SSE_BUFFER_BYTES {
            return Err(LLMError::ResponseTooLarge { what: "SSE buffer" });
        }

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
                    return Ok(result);
                }
                result.events.push(payload);
            }
        }

        Ok(result)
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

/// Builds a user-facing message for a transport-level failure while reading
/// the SSE response body.
///
/// `reqwest` collapses both a per-read timeout and a mid-stream connection
/// drop into the opaque `Display` "error decoding response body", which gives
/// the user no actionable hint. A read timeout is by far the most common cause
/// with reasoning models (DeepSeek V4 pro/flash): they emit the full thinking
/// trace before any answer token, so the body can stay silent longer than
/// `DEFAULT_READ_TIMEOUT_SECS`. We split that case out explicitly.
pub(crate) fn describe_sse_read_error(is_timeout: bool, source: &str) -> String {
    if is_timeout {
        format!(
            "SSE stream read timed out after {}s with no data: the model stayed \
             silent too long, which is common with reasoning models that emit \
             their entire thinking trace before any token. Underlying error: {}",
            crate::constants::llm_http::DEFAULT_READ_TIMEOUT_SECS,
            source
        )
    } else {
        format!("SSE stream read failed: {}", source)
    }
}
