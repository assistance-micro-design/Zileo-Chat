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

//! Speech-to-text (Voxtral) settings and result types.
//!
//! Settings are persisted as a single JSON blob in `settings:stt`. Mirrors
//! the `settings:validation` / `settings:embedding_config` pattern — no
//! per-field DEFINE FIELD, no migration needed when new fields are added.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Default Voxtral transcribe-specialized model. Confirmed alive via
/// `GET /v1/models` on 2026-05-20 — the doc page lists a different ID that
/// the live API rejects.
pub const DEFAULT_VOXTRAL_MODEL: &str = "voxtral-mini-transcribe-2507";

/// Maximum allowed `model_id` length (regex-validated upstream).
pub const MAX_MODEL_ID_LEN: usize = 128;

/// Maximum number of context-bias hints persisted alongside the settings.
pub const MAX_CONTEXT_BIAS_ENTRIES: usize = 100;

/// Maximum length per context-bias entry (after trim).
pub const MAX_CONTEXT_BIAS_ENTRY_LEN: usize = 50;

/// Languages the UI surfaces. `None` (`null` in JSON) = auto-detect.
pub const SUPPORTED_LANGUAGES: &[&str] = &["fr", "en", "es", "de", "it", "pt", "nl", "hi", "ar"];

/// Persisted STT (speech-to-text) settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct STTSettings {
    /// Whether the floating microphone FAB is shown.
    pub enabled: bool,
    /// Voxtral model ID. Free-form string (validated against `voxtral`
    /// substring + character regex on both sides of the IPC).
    pub model_id: String,
    /// Optional context-bias hints sent with every transcription request.
    pub context_bias: Vec<String>,
    /// Language override (`None` = auto-detect, otherwise BCP-47 short code).
    pub language: Option<String>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

impl Default for STTSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            model_id: DEFAULT_VOXTRAL_MODEL.to_string(),
            context_bias: Vec::new(),
            language: None,
            updated_at: Utc::now(),
        }
    }
}

/// Partial update payload — only provided fields are applied.
///
/// `language` is wrapped in a double `Option` so the frontend can distinguish
/// "leave as-is" (field absent) from "clear to auto-detect" (field present
/// with `null`). The custom deserializer `deserialize_explicit_option` is
/// defined just below in this module (single call-site).
#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSTTSettingsRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_bias: Option<Vec<String>>,
    /// Outer `Option` = "field present in payload?"
    /// Inner `Option` = "value (null clears the override, Some sets it)"
    #[serde(default, deserialize_with = "deserialize_explicit_option")]
    pub language: Option<Option<String>>,
}

/// Result returned to the frontend after a successful transcription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptionResult {
    pub text: String,
    pub language: Option<String>,
    pub model_used: String,
}

/// Tri-state deserializer for an explicit `Option<Option<T>>` field.
///
/// Treats absence as `None`, JSON `null` as `Some(None)`, and a value as
/// `Some(Some(value))`. Mirrors the helper documented in MEMORY for
/// `reasoning_effort` (PAT_SERDE_TRI_STATE).
fn deserialize_explicit_option<'de, D, T>(deserializer: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}

/// Returns `Ok(())` when `model_id` is a syntactically valid Voxtral model
/// reference. The substring check is intentionally permissive (case-
/// insensitive) so newly released `voxtral-*` IDs do not require a code
/// bump — only the `realtime` variant is rejected because the batch
/// transcription endpoint cannot serve it (confirmed by Mistral API).
pub fn validate_voxtral_model_id(model_id: &str) -> Result<(), String> {
    let trimmed = model_id.trim();
    if trimmed.is_empty() {
        return Err("model_id is required".to_string());
    }
    if trimmed.len() > MAX_MODEL_ID_LEN {
        return Err(format!("model_id exceeds {} characters", MAX_MODEL_ID_LEN));
    }
    if !trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err("model_id contains invalid characters".to_string());
    }
    let lower = trimmed.to_lowercase();
    if !lower.contains("voxtral") {
        return Err("model_id must reference a Voxtral model".to_string());
    }
    if lower.contains("realtime") {
        return Err(
            "Realtime Voxtral models cannot be used via the batch endpoint; \
             try voxtral-mini-transcribe-2507"
                .to_string(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_default_model_passes() {
        assert!(validate_voxtral_model_id(DEFAULT_VOXTRAL_MODEL).is_ok());
    }

    #[test]
    fn other_voxtral_variant_passes() {
        assert!(validate_voxtral_model_id("Voxtral-Small-2507").is_ok());
    }

    #[test]
    fn empty_is_rejected() {
        assert!(validate_voxtral_model_id("").is_err());
        assert!(validate_voxtral_model_id("   ").is_err());
    }

    #[test]
    fn too_long_is_rejected() {
        let long = "a".repeat(MAX_MODEL_ID_LEN + 1);
        assert!(validate_voxtral_model_id(&long).is_err());
    }

    #[test]
    fn invalid_chars_are_rejected() {
        assert!(validate_voxtral_model_id("voxtral mini").is_err());
        assert!(validate_voxtral_model_id("voxtral/mini").is_err());
        assert!(validate_voxtral_model_id("voxtral;DROP TABLE").is_err());
    }

    #[test]
    fn non_voxtral_is_rejected() {
        assert!(validate_voxtral_model_id("whisper-large-v3").is_err());
        assert!(validate_voxtral_model_id("mistral-large").is_err());
    }

    #[test]
    fn realtime_variant_is_rejected() {
        assert!(validate_voxtral_model_id("voxtral-mini-realtime-2602").is_err());
        assert!(validate_voxtral_model_id("voxtral-mini-transcribe-realtime-2602").is_err());
    }

    #[test]
    fn default_settings_have_disabled_with_default_model() {
        let s = STTSettings::default();
        assert!(!s.enabled);
        assert_eq!(s.model_id, DEFAULT_VOXTRAL_MODEL);
        assert!(s.context_bias.is_empty());
        assert!(s.language.is_none());
    }

    #[test]
    fn update_request_language_tri_state() {
        // Field absent -> None
        let req: UpdateSTTSettingsRequest = serde_json::from_str("{}").unwrap();
        assert!(req.language.is_none());

        // Field present with null -> Some(None) (clear override)
        let req: UpdateSTTSettingsRequest = serde_json::from_str(r#"{"language": null}"#).unwrap();
        assert_eq!(req.language, Some(None));

        // Field present with value -> Some(Some(value))
        let req: UpdateSTTSettingsRequest = serde_json::from_str(r#"{"language": "fr"}"#).unwrap();
        assert_eq!(req.language, Some(Some("fr".to_string())));
    }
}
