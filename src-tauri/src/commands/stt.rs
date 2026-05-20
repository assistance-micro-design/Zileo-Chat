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

//! Tauri command surface for the dictation pipeline.
//!
//! The frontend invokes `transcribe_audio` once per push-to-talk session
//! with the recorded blob encoded as base64. The command decodes it,
//! pulls the Mistral key from the keystore, and delegates to the
//! provider-agnostic `transcribe_audio_core`.

use crate::commands::SecureKeyStore;
use crate::llm::stt::transcribe_audio_core;
use crate::models::stt::TranscriptionResult;
use crate::state::AppState;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use tauri::State;
use tracing::{error, info, instrument, warn};

/// Maximum size for the base64-encoded payload (in characters).
///
/// Base64 inflates binary by 4/3 (every 3 bytes -> 4 ASCII chars), so a
/// 25 MB binary blob becomes ~33.33 MB of ASCII. Rounded up to 36 MB to
/// absorb the padding bytes and a small margin, ensuring legitimate
/// payloads at the binary cap are not rejected by the encoding overhead.
const MAX_AUDIO_BASE64_LEN: usize = 36 * 1024 * 1024;

/// Transcribes an audio blob via Mistral Voxtral.
///
/// # Arguments
/// * `audio_base64` - Base64-encoded audio bytes (no `data:` prefix).
/// * `mime_type` - MIME of the encoded audio (must be whitelisted).
/// * `context_bias` - Optional vocabulary hints forwarded to Voxtral.
/// * `language_override` - BCP-47 short code (e.g. `fr`) or `None` for auto.
/// * `model_id` - Voxtral model identifier configured in Settings.
///
/// # Returns
/// The transcription text plus metadata.
///
/// # Errors
/// Returns a human-readable error string suitable for surfacing via
/// toast on the frontend. The Mistral API key is never echoed.
#[tauri::command]
#[instrument(
    name = "transcribe_audio",
    skip(audio_base64, keystore, _state, context_bias),
    fields(
        bytes_b64 = audio_base64.len(),
        mime = %mime_type,
        model = %model_id,
        bias_count = context_bias.len(),
        has_language = language_override.is_some(),
    )
)]
pub async fn transcribe_audio(
    audio_base64: String,
    mime_type: String,
    context_bias: Vec<String>,
    language_override: Option<String>,
    model_id: String,
    keystore: State<'_, SecureKeyStore>,
    _state: State<'_, AppState>,
) -> Result<TranscriptionResult, String> {
    if audio_base64.is_empty() {
        return Err("audio_base64 is empty".to_string());
    }
    if audio_base64.len() > MAX_AUDIO_BASE64_LEN {
        return Err("audio payload too large".to_string());
    }

    let audio = BASE64.decode(audio_base64.as_bytes()).map_err(|e| {
        warn!(error = %e, "Failed to decode audio base64");
        "Failed to decode audio payload".to_string()
    })?;

    let api_key = keystore.get_key("Mistral").ok_or_else(|| {
        warn!("Mistral API key not configured for STT");
        "Mistral API key is not configured. Add it in Settings > Providers.".to_string()
    })?;
    if api_key.is_empty() {
        return Err("Mistral API key is empty.".to_string());
    }

    let client = _state.llm_manager.http_client().clone();

    let result = transcribe_audio_core(
        &client,
        &api_key,
        &audio,
        &mime_type,
        &model_id,
        &context_bias,
        language_override.as_deref(),
    )
    .await
    .map_err(|e| {
        error!(error = %e, "Voxtral transcription failed");
        format!("Transcription failed: {}", e)
    })?;

    info!(
        text_length = result.text.len(),
        model_used = %result.model_used,
        "Voxtral transcription delivered"
    );

    Ok(result)
}
