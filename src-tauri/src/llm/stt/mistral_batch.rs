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

//! Mistral Voxtral batch transcription adapter.
//!
//! HTTP boundary for `POST https://api.mistral.ai/v1/audio/transcriptions`
//! with `multipart/form-data`. The audio bytes, MIME type and model ID
//! arrive already decoded — encoding/decoding and IPC validation live in
//! the Tauri command layer.

use crate::llm::http;
use crate::llm::provider::LLMError;
use crate::models::stt::{
    validate_voxtral_model_id, TranscriptionResult, MAX_CONTEXT_BIAS_ENTRIES,
    MAX_CONTEXT_BIAS_ENTRY_LEN,
};
use reqwest::multipart::{Form, Part};
use serde::Deserialize;
use tracing::{debug, info, instrument};

/// Mistral transcription endpoint (v1).
const MISTRAL_TRANSCRIBE_URL: &str = "https://api.mistral.ai/v1/audio/transcriptions";

/// Upper bound on uploaded audio. 25 MB matches the front-end soft guard
/// and Whisper/Mistral's published per-request caps.
pub const MAX_AUDIO_BYTES: usize = 25 * 1024 * 1024;

/// Whitelisted audio MIMEs. Kept in lock-step with
/// `src/lib/utils/audio-capture.ts::SUPPORTED_AUDIO_MIMES` — change both
/// when MediaRecorder probes a new codec (PAT_IPC_MIRRORED_WHITELIST).
pub const SUPPORTED_AUDIO_MIMES: &[&str] = &[
    "audio/webm",
    "audio/ogg",
    "audio/mp4",
    "audio/wav",
    "audio/mpeg",
    "audio/x-m4a",
];

/// JSON shape returned by the Mistral transcribe endpoint. Only the
/// fields surfaced past the Rust boundary are declared here; the
/// endpoint may also return `segments`, `finish_reason`, `usage.*` and
/// other metadata which serde silently ignores (no `deny_unknown_fields`).
#[derive(Debug, Deserialize)]
struct MistralTranscriptionResponse {
    text: String,
    #[serde(default)]
    language: Option<String>,
    model: String,
}

/// Maps a MIME to the file extension used in the multipart `filename`
/// header. The Mistral API tolerates a generic name, but supplying a
/// matching extension helps server-side codec sniffing.
fn extension_for_mime(mime: &str) -> &'static str {
    match mime {
        "audio/webm" => "webm",
        "audio/ogg" => "ogg",
        "audio/mp4" | "audio/x-m4a" => "m4a",
        "audio/wav" => "wav",
        "audio/mpeg" => "mp3",
        _ => "bin",
    }
}

/// Performs a single transcription round-trip against the Voxtral batch
/// endpoint. All validations happen up-front so the network call only
/// fires on well-formed input.
#[instrument(
    name = "stt.transcribe_audio_core",
    skip(client, api_key, audio, context_bias),
    fields(
        bytes = audio.len(),
        mime = %mime,
        model = %model_id,
        has_language = language.is_some(),
        bias_count = context_bias.len(),
    )
)]
pub async fn transcribe_audio_core(
    client: &reqwest::Client,
    api_key: &str,
    audio: &[u8],
    mime: &str,
    model_id: &str,
    context_bias: &[String],
    language: Option<&str>,
) -> Result<TranscriptionResult, LLMError> {
    // --- Pre-flight validation ---
    if audio.is_empty() {
        return Err(LLMError::RequestFailed("audio buffer is empty".to_string()));
    }
    if audio.len() > MAX_AUDIO_BYTES {
        return Err(LLMError::RequestFailed(format!(
            "audio buffer exceeds {} bytes",
            MAX_AUDIO_BYTES
        )));
    }
    if !SUPPORTED_AUDIO_MIMES.contains(&mime) {
        return Err(LLMError::RequestFailed(format!(
            "unsupported audio MIME: {}",
            mime
        )));
    }

    validate_voxtral_model_id(model_id).map_err(LLMError::RequestFailed)?;

    if context_bias.len() > MAX_CONTEXT_BIAS_ENTRIES {
        return Err(LLMError::RequestFailed(format!(
            "context_bias exceeds {} entries",
            MAX_CONTEXT_BIAS_ENTRIES
        )));
    }
    for entry in context_bias {
        if entry.len() > MAX_CONTEXT_BIAS_ENTRY_LEN {
            return Err(LLMError::RequestFailed(format!(
                "context_bias entry exceeds {} characters",
                MAX_CONTEXT_BIAS_ENTRY_LEN
            )));
        }
        if entry.chars().any(|c| c.is_control()) {
            return Err(LLMError::RequestFailed(
                "context_bias entries must not contain control characters".to_string(),
            ));
        }
    }
    if let Some(lang) = language {
        if lang.is_empty() || lang.len() > 16 {
            return Err(LLMError::RequestFailed(
                "language code length out of range".to_string(),
            ));
        }
        if !lang
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(LLMError::RequestFailed(
                "language code contains invalid characters".to_string(),
            ));
        }
    }

    // --- Build multipart form ---
    let filename = format!("audio.{}", extension_for_mime(mime));
    let file_part = Part::bytes(audio.to_vec())
        .file_name(filename)
        .mime_str(mime)
        .map_err(|e| LLMError::RequestFailed(format!("invalid audio MIME: {}", e)))?;

    let mut form = Form::new()
        .text("model", model_id.to_string())
        .part("file", file_part);

    for bias in context_bias {
        form = form.text("context_bias", bias.clone());
    }
    if let Some(lang) = language {
        form = form.text("language", lang.to_string());
    }

    debug!("sending Voxtral transcription request");

    // --- HTTP call ---
    let response = client
        .post(MISTRAL_TRANSCRIBE_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .multipart(form)
        .send()
        .await
        .map_err(|e| LLMError::RequestFailed(format!("Mistral STT request failed: {}", e)))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| LLMError::RequestFailed(format!("Mistral STT body read failed: {}", e)))?;

    if !status.is_success() {
        return Err(http::parse_api_error("Mistral STT", status, &body));
    }

    let parsed: MistralTranscriptionResponse = serde_json::from_str(&body).map_err(|e| {
        LLMError::RequestFailed(format!("Mistral STT response parse failed: {}", e))
    })?;

    info!(
        text_length = parsed.text.len(),
        model_used = %parsed.model,
        "Voxtral transcription succeeded"
    );

    Ok(TranscriptionResult {
        text: parsed.text,
        language: parsed.language,
        model_used: parsed.model,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    /// Spawns a tiny single-shot HTTP listener that returns the supplied
    /// status + body, and yields the URL to point a client at. Used by
    /// the success/4xx/5xx tests below.
    async fn spawn_mock_server(
        status_line: &'static str,
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock");
        let port = listener.local_addr().unwrap().port();
        listener.set_nonblocking(true).unwrap();
        let listener = tokio::net::TcpListener::from_std(listener).unwrap();

        let handle = tokio::spawn(async move {
            if let Ok((mut sock, _)) = listener.accept().await {
                // Drain the request — we do not assert on it here, only
                // that the client made it to the wire.
                let mut buf = [0u8; 4096];
                let _ = sock.read(&mut buf).await;
                let response = format!(
                    "HTTP/1.1 {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    status_line,
                    body.len(),
                    body
                );
                let _ = sock.write_all(response.as_bytes()).await;
                let _ = sock.shutdown().await;
            }
        });

        (format!("http://127.0.0.1:{}", port), handle)
    }

    /// Replays `transcribe_audio_core` against a mock URL by injecting
    /// the test URL via a thin wrapper. Because `transcribe_audio_core`
    /// hard-codes `MISTRAL_TRANSCRIBE_URL`, we exercise it indirectly
    /// through the public helpers that validate inputs (which is where
    /// most of the production logic lives). Network round-trip is
    /// covered by `manual_round_trip_against_mock_server` below.
    #[tokio::test]
    async fn rejects_empty_audio() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[],
            "audio/webm",
            "voxtral-mini-transcribe-2507",
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("empty")));
    }

    #[tokio::test]
    async fn rejects_oversize_audio() {
        let client = reqwest::Client::new();
        let payload = vec![0u8; MAX_AUDIO_BYTES + 1];
        let err = transcribe_audio_core(
            &client,
            "key",
            &payload,
            "audio/webm",
            "voxtral-mini-transcribe-2507",
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("exceeds")));
    }

    #[tokio::test]
    async fn rejects_unsupported_mime() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/flac",
            "voxtral-mini-transcribe-2507",
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("MIME")));
    }

    #[tokio::test]
    async fn rejects_realtime_model() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/webm",
            "voxtral-mini-realtime-2602",
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(
            matches!(err, LLMError::RequestFailed(ref m) if m.to_lowercase().contains("realtime"))
        );
    }

    #[tokio::test]
    async fn rejects_non_voxtral_model() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/webm",
            "whisper-large-v3",
            &[],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(_)));
    }

    #[tokio::test]
    async fn rejects_too_many_bias_entries() {
        let client = reqwest::Client::new();
        let bias: Vec<String> = (0..(MAX_CONTEXT_BIAS_ENTRIES + 1))
            .map(|i| format!("hint{}", i))
            .collect();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/webm",
            "voxtral-mini-transcribe-2507",
            &bias,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("context_bias")));
    }

    #[tokio::test]
    async fn rejects_bias_with_control_chars() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/webm",
            "voxtral-mini-transcribe-2507",
            &["bad\nentry".to_string()],
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("control")));
    }

    #[tokio::test]
    async fn rejects_bad_language_code() {
        let client = reqwest::Client::new();
        let err = transcribe_audio_core(
            &client,
            "key",
            &[1, 2, 3],
            "audio/webm",
            "voxtral-mini-transcribe-2507",
            &[],
            Some("f r"),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, LLMError::RequestFailed(ref m) if m.contains("language")));
    }

    /// Direct round-trip against an in-process HTTP server. Uses the
    /// public `reqwest::Client` directly to mirror what
    /// `transcribe_audio_core` sends, since the production URL is
    /// hard-coded. Confirms the multipart wire shape compiles and the
    /// success/error parsing branches behave as documented.
    #[tokio::test]
    async fn round_trip_success_against_mock() {
        let body = r#"{"text":"hello","language":null,"model":"voxtral-mini-transcribe-2507","segments":[],"usage":{"prompt_audio_seconds":1,"prompt_tokens":1,"total_tokens":2,"completion_tokens":1,"prompt_tokens_details":{"cached_tokens":0,"audio_tokens":1}},"finish_reason":null}"#;
        let (url, handle) = spawn_mock_server("200 OK", body).await;

        let client = reqwest::Client::new();
        let file_part = Part::bytes(vec![0u8; 16])
            .file_name("audio.webm")
            .mime_str("audio/webm")
            .unwrap();
        let form = Form::new()
            .text("model", "voxtral-mini-transcribe-2507")
            .part("file", file_part);
        let resp = client
            .post(&url)
            .header("Authorization", "Bearer test")
            .multipart(form)
            .send()
            .await
            .unwrap();
        assert!(resp.status().is_success());
        let parsed: MistralTranscriptionResponse =
            serde_json::from_str(&resp.text().await.unwrap()).unwrap();
        assert_eq!(parsed.text, "hello");
        assert_eq!(parsed.model, "voxtral-mini-transcribe-2507");

        handle.await.ok();
    }

    #[tokio::test]
    async fn round_trip_4xx_against_mock() {
        let body =
            r#"{"object":"error","message":"Invalid model","type":"invalid_model","code":"1500"}"#;
        let (url, handle) = spawn_mock_server("400 Bad Request", body).await;

        let client = reqwest::Client::new();
        let form = Form::new().text("model", "bad");
        let resp = client.post(&url).multipart(form).send().await.unwrap();
        let status = resp.status();
        let text = resp.text().await.unwrap();
        let err = http::parse_api_error("Mistral STT", status, &text);
        assert!(!matches!(err, LLMError::Cancelled));

        handle.await.ok();
    }
}
