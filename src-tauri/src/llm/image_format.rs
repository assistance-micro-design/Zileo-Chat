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

//! Per-provider multimodal image content shape.
//!
//! All providers expose the same outer envelope (`{type: "image_url", ...}`)
//! but diverge on the `image_url` payload:
//!
//! | Provider          | `image_url` shape                |
//! |-------------------|----------------------------------|
//! | OpenAI            | Object `{url, detail?}`          |
//! | OpenRouter/Custom | Object `{url}`                   |
//! | Mistral           | **String** (URL or data URL)     |
//! | Ollama `/v1/`     | **String** (data URL only)       |
//!
//! `load_conversation_history` always emits the DEFAULT OpenAI object form,
//! and per-provider adapters call [`normalize_messages_for_mistral`] or
//! [`normalize_messages_for_ollama`] right before the request body is built.
//! Both normalizers are idempotent — running them twice is a no-op.

use crate::models::MessageAttachment;
use serde_json::{json, Value};

/// Builds a `content[]` part for an image, in the default OpenAI object shape.
/// `image_url` is `{url: "data:<mime>;base64,<payload>"}`.
pub fn build_image_content_part_openai(attachment: &MessageAttachment) -> Value {
    let data_url = format!(
        "data:{};base64,{}",
        attachment.mime_type, attachment.data_base64
    );
    json!({
        "type": "image_url",
        "image_url": { "url": data_url }
    })
}

/// Re-shapes any `image_url` parts inside `messages[*].content[]` arrays so
/// `image_url` is a bare string (Mistral / Ollama format). No-op on messages
/// whose `content` is a plain string or whose parts already match the target
/// shape. Idempotent.
pub fn normalize_messages_for_string_image_url(messages: &mut [Value]) {
    for msg in messages.iter_mut() {
        let Some(content) = msg.get_mut("content") else {
            continue;
        };
        let Some(parts) = content.as_array_mut() else {
            continue;
        };
        for part in parts.iter_mut() {
            let Some(part_obj) = part.as_object_mut() else {
                continue;
            };
            let is_image = part_obj
                .get("type")
                .and_then(|v| v.as_str())
                .is_some_and(|s| s == "image_url");
            if !is_image {
                continue;
            }
            if let Some(image_url) = part_obj.get("image_url") {
                if let Some(obj) = image_url.as_object() {
                    if let Some(url) = obj.get("url").and_then(|v| v.as_str()) {
                        let url_owned = url.to_string();
                        part_obj.insert("image_url".to_string(), Value::String(url_owned));
                    }
                }
            }
        }
    }
}

/// Alias used at Mistral call sites — semantic name.
pub fn normalize_messages_for_mistral(messages: &mut [Value]) {
    normalize_messages_for_string_image_url(messages);
}

/// Re-shapes multipart messages for Ollama's native `/api/chat` endpoint.
///
/// `/api/chat` does NOT accept `content` arrays. Instead, each message has:
/// - `content`: a plain string (the text)
/// - `images`: an array of base64 strings (no `data:` prefix)
///
/// This function rewrites every user message whose `content` is an array:
/// - concatenates `{type:"text", text:...}` parts into a single string,
/// - extracts `{type:"image_url", image_url: <string OR {url: string}>}` parts,
///   strips the `data:<mime>;base64,` prefix if present, and appends the raw
///   base64 to a sibling `images: []` array.
///
/// Idempotent: messages already in native shape are left untouched.
pub fn normalize_messages_for_ollama_native_api(messages: &mut [Value]) {
    for msg in messages.iter_mut() {
        let Some(content) = msg.get("content") else {
            continue;
        };
        let Some(parts) = content.as_array() else {
            continue;
        };

        let mut text_pieces: Vec<String> = Vec::new();
        let mut images: Vec<String> = Vec::new();
        for part in parts {
            let Some(part_obj) = part.as_object() else {
                continue;
            };
            let ty = part_obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match ty {
                "text" => {
                    if let Some(t) = part_obj.get("text").and_then(|v| v.as_str()) {
                        text_pieces.push(t.to_string());
                    }
                }
                "image_url" => {
                    let url_str = match part_obj.get("image_url") {
                        Some(Value::String(s)) => s.clone(),
                        Some(Value::Object(obj)) => obj
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                        _ => continue,
                    };
                    // Strip optional `data:<mime>;base64,` prefix — Ollama wants raw.
                    let raw = match url_str.find(";base64,") {
                        Some(idx) => url_str[idx + ";base64,".len()..].to_string(),
                        None => url_str,
                    };
                    if !raw.is_empty() {
                        images.push(raw);
                    }
                }
                _ => {}
            }
        }

        if let Some(obj) = msg.as_object_mut() {
            obj.insert("content".to_string(), Value::String(text_pieces.join("\n")));
            if !images.is_empty() {
                let arr: Vec<Value> = images.into_iter().map(Value::String).collect();
                obj.insert("images".to_string(), Value::Array(arr));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_attachment() -> MessageAttachment {
        MessageAttachment {
            kind: "image".into(),
            mime_type: "image/png".into(),
            data_base64: "AAAA".into(),
            name: None,
            size_bytes: None,
        }
    }

    #[test]
    fn build_image_content_part_openai_wraps_data_url_in_image_url_object() {
        let part = build_image_content_part_openai(&sample_attachment());
        assert_eq!(part["type"], "image_url");
        assert!(part["image_url"].is_object());
        assert_eq!(
            part["image_url"]["url"].as_str().unwrap(),
            "data:image/png;base64,AAAA"
        );
    }

    #[test]
    fn normalize_for_mistral_flattens_image_url_object_to_string() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "look"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,ABC"}}
            ]
        })];
        normalize_messages_for_mistral(&mut messages);
        let parts = messages[0]["content"].as_array().unwrap();
        assert_eq!(parts[1]["image_url"], "data:image/png;base64,ABC");
    }

    #[test]
    fn normalize_for_mistral_is_idempotent_when_image_url_is_already_string() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "image_url", "image_url": "data:image/png;base64,ABC"}
            ]
        })];
        let before = messages.clone();
        normalize_messages_for_mistral(&mut messages);
        normalize_messages_for_mistral(&mut messages);
        assert_eq!(messages, before, "normalize must be idempotent");
    }

    #[test]
    fn normalize_for_mistral_leaves_message_untouched_when_content_is_plain_string() {
        let mut messages = vec![json!({
            "role": "user",
            "content": "plain text only"
        })];
        let before = messages.clone();
        normalize_messages_for_mistral(&mut messages);
        assert_eq!(messages, before);
    }

    #[test]
    fn normalize_for_mistral_leaves_message_untouched_when_content_has_only_text_parts() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "hello"}
            ]
        })];
        let before = messages.clone();
        normalize_messages_for_mistral(&mut messages);
        assert_eq!(messages, before);
    }

    #[test]
    fn normalize_for_ollama_native_extracts_images_and_collapses_text_into_string_content() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "text", "text": "look"},
                {"type": "image_url", "image_url": {"url": "data:image/png;base64,RAW1"}},
                {"type": "image_url", "image_url": "data:image/jpeg;base64,RAW2"}
            ]
        })];
        normalize_messages_for_ollama_native_api(&mut messages);
        assert_eq!(messages[0]["content"], "look");
        let images = messages[0]["images"].as_array().unwrap();
        assert_eq!(images[0], "RAW1");
        assert_eq!(images[1], "RAW2");
    }

    #[test]
    fn normalize_for_ollama_native_strips_data_url_prefix_from_image_payload() {
        let mut messages = vec![json!({
            "role": "user",
            "content": [
                {"type": "image_url", "image_url": "data:image/png;base64,XYZ"}
            ]
        })];
        normalize_messages_for_ollama_native_api(&mut messages);
        assert_eq!(messages[0]["images"][0], "XYZ");
        assert_eq!(messages[0]["content"], "");
    }

    #[test]
    fn normalize_for_ollama_native_is_idempotent_when_content_is_already_a_string() {
        let mut messages = vec![json!({
            "role": "user",
            "content": "already a string"
        })];
        let before = messages.clone();
        normalize_messages_for_ollama_native_api(&mut messages);
        assert_eq!(messages, before);
    }
}
