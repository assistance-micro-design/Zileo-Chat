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

//! Custom provider data model.
//!
//! Stores metadata for user-created OpenAI-compatible providers.
//! API keys are stored separately in SecureKeyStore.

use serde::{Deserialize, Serialize};

/// Custom provider metadata stored in the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomProvider {
    /// URL-safe identifier (e.g., "routerlab", "openrouter")
    pub name: String,
    /// Human-readable display name (e.g., "RouterLab", "OpenRouter")
    pub display_name: String,
    /// API base URL (e.g., "https://api.routerlab.ch/v1")
    pub base_url: String,
    /// Whether this provider is enabled
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Whether to inject Anthropic-style `cache_control` content parts.
    ///
    /// `None` or `Some(true)` preserves the OpenRouter Anthropic behaviour.
    /// `Some(false)` skips the rewrite for strict providers (Fireworks, Groq,
    /// Together, Cerebras) that reject the field with HTTP 400.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_cache_control: Option<bool>,
    /// Whether to inject the OpenRouter-style top-level `reasoning` object.
    ///
    /// `None` or `Some(true)` preserves OpenRouter and RouterLab behaviour
    /// (RouterLab proxies via Anthropic and only honours `reasoning.max_tokens`).
    /// `Some(false)` clears the object for strict providers that reject it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_param: Option<bool>,
    /// Creation timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<String>,
}

fn default_enabled() -> bool {
    true
}

/// Metadata about a provider (builtin or custom) returned to the frontend.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    /// Provider identifier (e.g., "mistral", "ollama", "routerlab")
    pub id: String,
    /// Human-readable display name
    pub display_name: String,
    /// Whether this is a builtin provider (Mistral, Ollama)
    pub is_builtin: bool,
    /// Whether this is a cloud provider (requires API key)
    pub is_cloud: bool,
    /// Whether this provider requires an API key
    pub requires_api_key: bool,
    /// Whether this provider has a configurable base URL
    pub has_base_url: bool,
    /// Current base URL (if applicable)
    pub base_url: Option<String>,
    /// Whether this provider is enabled
    pub enabled: bool,
    /// Whether the provider accepts Anthropic-style `cache_control` content
    /// parts. Omitted for builtins (managed natively) and absent on custom
    /// providers that haven't explicitly opted out (= preserved current
    /// OpenRouter behaviour).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_cache_control: Option<bool>,
    /// Whether the provider accepts the OpenRouter-style top-level
    /// `reasoning` object. Same defaulting semantics as
    /// `supports_cache_control`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub supports_reasoning_param: Option<bool>,
}

/// Response from create/update custom provider commands.
///
/// Wraps `ProviderInfo` with an optional security warning (e.g., HTTP usage).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderResponse {
    /// Provider metadata
    pub provider: ProviderInfo,
    /// Optional security warning (e.g., HTTP without TLS on non-localhost)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// Checks whether a URL uses HTTP without TLS on a non-localhost host.
///
/// Returns a warning message if the URL uses plain HTTP to a remote host,
/// which exposes API keys and data in transit.
///
/// # Arguments
/// * `url` - The base URL to check
///
/// # Returns
/// `Some(warning)` if the URL is insecure, `None` otherwise
pub fn check_http_warning(url: &str) -> Option<String> {
    let lower = url.to_lowercase();

    // HTTPS is always fine
    if lower.starts_with("https://") {
        return None;
    }

    // Not HTTP at all - no warning (could be a relative URL or other scheme)
    if !lower.starts_with("http://") {
        return None;
    }

    // Extract host portion after "http://"
    let after_scheme = &lower[7..];
    let host_port = after_scheme.split('/').next().unwrap_or(after_scheme);

    // Extract host, handling IPv6 bracket notation (e.g., [::1]:8080)
    let host = if host_port.starts_with('[') {
        // IPv6: extract content up to and including the closing bracket
        host_port
            .split(']')
            .next()
            .map(|s| format!("{}]", s))
            .unwrap_or_default()
    } else {
        // IPv4 or hostname: take everything before the port
        host_port.split(':').next().unwrap_or(host_port).to_string()
    };

    // Localhost variants are safe (local development, Ollama, etc.)
    if host == "localhost" || host == "127.0.0.1" || host == "[::1]" {
        return None;
    }

    Some(format!(
        "Warning: Using HTTP instead of HTTPS for '{}'. \
         API keys and data will be sent unencrypted. \
         Consider using HTTPS for production endpoints.",
        url
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_https_urls_no_warning() {
        assert!(check_http_warning("https://api.openai.com/v1").is_none());
        assert!(check_http_warning("https://api.routerlab.ch/v1").is_none());
        assert!(check_http_warning("HTTPS://API.EXAMPLE.COM").is_none());
    }

    #[test]
    fn test_http_localhost_no_warning() {
        assert!(check_http_warning("http://localhost:11434").is_none());
        assert!(check_http_warning("http://localhost:8080/v1").is_none());
        assert!(check_http_warning("http://localhost").is_none());
        assert!(check_http_warning("http://LOCALHOST:3000").is_none());
    }

    #[test]
    fn test_http_127_0_0_1_no_warning() {
        assert!(check_http_warning("http://127.0.0.1:11434").is_none());
        assert!(check_http_warning("http://127.0.0.1:8080/v1").is_none());
        assert!(check_http_warning("http://127.0.0.1").is_none());
    }

    #[test]
    fn test_http_ipv6_loopback_no_warning() {
        assert!(check_http_warning("http://[::1]:8080").is_none());
        assert!(check_http_warning("http://[::1]:11434/v1").is_none());
        assert!(check_http_warning("http://[::1]").is_none());
    }

    #[test]
    fn test_http_remote_returns_warning() {
        let warning = check_http_warning("http://api.example.com/v1");
        assert!(warning.is_some());
        let msg = warning.unwrap();
        assert!(msg.contains("HTTP instead of HTTPS"));
        assert!(msg.contains("http://api.example.com/v1"));
    }

    #[test]
    fn test_http_remote_ip_returns_warning() {
        let warning = check_http_warning("http://192.168.1.100:8080/v1");
        assert!(warning.is_some());
        let msg = warning.unwrap();
        assert!(msg.contains("HTTP instead of HTTPS"));
    }

    #[test]
    fn test_http_remote_various_hosts() {
        assert!(check_http_warning("http://10.0.0.1:8080").is_some());
        assert!(check_http_warning("http://my-server.local:3000/v1").is_some());
        assert!(check_http_warning("http://api.openai.com/v1").is_some());
    }

    #[test]
    fn test_non_http_schemes_no_warning() {
        assert!(check_http_warning("ftp://example.com").is_none());
        assert!(check_http_warning("ws://example.com").is_none());
        assert!(check_http_warning("").is_none());
    }

    #[test]
    fn custom_provider_deserializes_supports_flags_when_present() {
        let json = r#"{
            "name": "fireworks",
            "display_name": "Fireworks",
            "base_url": "https://api.fireworks.ai/inference/v1",
            "enabled": true,
            "supports_cache_control": false,
            "supports_reasoning_param": false
        }"#;
        let cp: CustomProvider = serde_json::from_str(json).expect("parses");
        assert_eq!(cp.supports_cache_control, Some(false));
        assert_eq!(cp.supports_reasoning_param, Some(false));
    }

    #[test]
    fn custom_provider_deserializes_legacy_rows_without_supports_flags() {
        // Existing rows in the database predate the new fields. They must
        // continue to deserialize cleanly with the flags absent (= None),
        // which the wire logic treats as the OpenRouter-preserving default.
        let json = r#"{
            "name": "openrouter",
            "display_name": "OpenRouter",
            "base_url": "https://openrouter.ai/api/v1",
            "enabled": true
        }"#;
        let cp: CustomProvider = serde_json::from_str(json).expect("parses");
        assert_eq!(cp.supports_cache_control, None);
        assert_eq!(cp.supports_reasoning_param, None);
    }

    #[test]
    fn provider_info_serializes_supports_flags_in_camel_case() {
        let info = ProviderInfo {
            id: "fireworks".to_string(),
            display_name: "Fireworks".to_string(),
            is_builtin: false,
            is_cloud: true,
            requires_api_key: true,
            has_base_url: true,
            base_url: Some("https://api.fireworks.ai/inference/v1".to_string()),
            enabled: true,
            supports_cache_control: Some(false),
            supports_reasoning_param: Some(false),
        };
        let json = serde_json::to_value(&info).expect("serialises");
        // serde rename_all = "camelCase" must apply to the new fields.
        assert_eq!(json["supportsCacheControl"], serde_json::Value::Bool(false));
        assert_eq!(
            json["supportsReasoningParam"],
            serde_json::Value::Bool(false)
        );
    }

    #[test]
    fn provider_info_omits_supports_flags_when_none() {
        let info = ProviderInfo {
            id: "openrouter".to_string(),
            display_name: "OpenRouter".to_string(),
            is_builtin: false,
            is_cloud: true,
            requires_api_key: true,
            has_base_url: true,
            base_url: Some("https://openrouter.ai/api/v1".to_string()),
            enabled: true,
            supports_cache_control: None,
            supports_reasoning_param: None,
        };
        let json = serde_json::to_value(&info).expect("serialises");
        assert!(
            json.get("supportsCacheControl").is_none(),
            "None must be skipped to keep the TS contract `field?: boolean`"
        );
        assert!(json.get("supportsReasoningParam").is_none());
    }
}
