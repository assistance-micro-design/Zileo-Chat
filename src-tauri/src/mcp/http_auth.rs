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

//! HTTP authentication header builder for MCP servers (v1.2).
//!
//! Pure function — no IO, no side effects. Given an auth type, optional
//! metadata, optional secret, and an extra-headers map, produces the
//! corresponding `reqwest::header::HeaderMap` plus a list of warnings (e.g.
//! when an extra header conflicts with the main `Authorization`).
//!
//! Secrets never appear in error messages or logs from this module: any
//! per-byte issue surfaces via [`MCPError::InvalidConfig`] with a generic
//! reason string.

use crate::mcp::MCPError;
use crate::models::mcp::{MCPAuthMetadata, MCPAuthSecret, MCPAuthType};
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, AUTHORIZATION};
use std::collections::HashMap;

/// Builds the HTTP authentication headers for a given MCP server.
///
/// The function is pure: it takes the auth model and produces a
/// `(HeaderMap, warnings)` tuple. Warnings are non-fatal and meant to be
/// surfaced to logs / UI (e.g. "extraHeaders.Authorization conflicts with
/// the main bearer auth").
///
/// # Rules
/// - `auth_type == None` (or absent) → no `Authorization` header is generated;
///   only `extra_headers` are added.
/// - `Bearer` → `Authorization: Bearer <secret.token>`.
/// - `Apikey` → `<metadata.header_name | "X-API-Key">: <secret.value>`.
/// - `Basic`  → `Authorization: Basic base64(metadata.username:secret.password)`.
/// - `extra_headers` are added LAST. If a key would overwrite the main auth
///   header (`Authorization` for Bearer/Basic, the configured header name
///   for Apikey), the main auth wins and a warning is recorded.
///
/// # Errors
/// Returns [`MCPError::InvalidConfig`] when:
/// - the secret is missing for an auth type that needs one,
/// - a header name / value cannot be parsed by `reqwest` (non-ASCII, etc.).
pub fn build_auth_headers(
    auth_type: MCPAuthType,
    metadata: &MCPAuthMetadata,
    secret: Option<&MCPAuthSecret>,
    extra_headers: &HashMap<String, String>,
) -> Result<(HeaderMap, Vec<String>), MCPError> {
    let mut headers = HeaderMap::new();
    let mut warnings: Vec<String> = Vec::new();

    // --- Main auth header --------------------------------------------------
    // `auth_header_name` is the lowercase canonical form of the header that
    // the auth code installed (so we can detect conflicts when iterating
    // extra_headers below).
    let auth_header_name: Option<String> = match auth_type {
        MCPAuthType::None => None,
        MCPAuthType::Bearer => {
            let token = secret
                .and_then(|s| s.token.as_deref())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .ok_or_else(|| MCPError::InvalidConfig {
                    field: "auth_secret.token".to_string(),
                    reason: "missing bearer token".to_string(),
                })?;

            let value = HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|_| {
                MCPError::InvalidConfig {
                    field: "auth_secret.token".to_string(),
                    reason: "invalid bearer token (non-ASCII characters)".to_string(),
                }
            })?;
            headers.insert(AUTHORIZATION, value);
            Some("authorization".to_string())
        }
        MCPAuthType::Apikey => {
            let header_name_raw = metadata
                .header_name
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("X-API-Key");

            let header_name = HeaderName::from_bytes(header_name_raw.as_bytes()).map_err(|_| {
                MCPError::InvalidConfig {
                    field: "auth_metadata.header_name".to_string(),
                    reason: format!(
                        "invalid api-key header name '{}': must be ASCII",
                        header_name_raw
                    ),
                }
            })?;

            let value_str =
                secret
                    .and_then(|s| s.value.as_deref())
                    .ok_or_else(|| MCPError::InvalidConfig {
                        field: "auth_secret.value".to_string(),
                        reason: "missing api-key value".to_string(),
                    })?;
            let value = HeaderValue::from_str(value_str).map_err(|_| MCPError::InvalidConfig {
                field: "auth_secret.value".to_string(),
                reason: "invalid api-key value (non-ASCII characters)".to_string(),
            })?;
            let header_lower = header_name.as_str().to_ascii_lowercase();
            headers.insert(header_name, value);
            Some(header_lower)
        }
        MCPAuthType::Basic => {
            let username = metadata
                .username
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| MCPError::InvalidConfig {
                    field: "auth_metadata.username".to_string(),
                    reason: "missing basic auth username".to_string(),
                })?;
            let password = secret.and_then(|s| s.password.as_deref()).ok_or_else(|| {
                MCPError::InvalidConfig {
                    field: "auth_secret.password".to_string(),
                    reason: "missing basic auth password".to_string(),
                }
            })?;
            if username.contains(':') {
                return Err(MCPError::InvalidConfig {
                    field: "auth_metadata.username".to_string(),
                    reason: "basic auth username cannot contain ':'".to_string(),
                });
            }
            let encoded = B64.encode(format!("{}:{}", username, password));
            let value = HeaderValue::from_str(&format!("Basic {}", encoded)).map_err(|_| {
                MCPError::InvalidConfig {
                    field: "auth_secret.password".to_string(),
                    reason: "invalid basic auth credentials (non-ASCII)".to_string(),
                }
            })?;
            headers.insert(AUTHORIZATION, value);
            Some("authorization".to_string())
        }
    };

    // --- Extra headers -----------------------------------------------------
    for (raw_name, raw_value) in extra_headers {
        let name_lower = raw_name.to_ascii_lowercase();
        if let Some(ref auth_name) = auth_header_name {
            if &name_lower == auth_name {
                warnings.push(format!(
                    "Ignored extra header '{}' (would override the main authentication header); main auth wins",
                    raw_name
                ));
                continue;
            }
        }

        let header_name =
            HeaderName::from_bytes(raw_name.as_bytes()).map_err(|_| MCPError::InvalidConfig {
                field: "extra_headers".to_string(),
                reason: format!("invalid extra header name '{}': must be ASCII", raw_name),
            })?;
        let header_value =
            HeaderValue::from_str(raw_value).map_err(|_| MCPError::InvalidConfig {
                field: "extra_headers".to_string(),
                reason: format!("invalid extra header '{}' value (non-ASCII)", raw_name),
            })?;
        headers.insert(header_name, header_value);
    }

    Ok((headers, warnings))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_metadata() -> MCPAuthMetadata {
        MCPAuthMetadata::default()
    }

    fn empty_extra() -> HashMap<String, String> {
        HashMap::new()
    }

    // --- auth_type=None ---------------------------------------------------

    #[test]
    fn test_none_yields_empty_headers_and_no_warnings() {
        let (headers, warnings) =
            build_auth_headers(MCPAuthType::None, &empty_metadata(), None, &empty_extra()).unwrap();
        assert!(headers.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_none_still_adds_extra_headers() {
        let mut extra = HashMap::new();
        extra.insert("X-Tenant".to_string(), "42".to_string());
        let (headers, warnings) =
            build_auth_headers(MCPAuthType::None, &empty_metadata(), None, &extra).unwrap();
        assert_eq!(headers.get("X-Tenant").unwrap(), "42");
        assert!(headers.get(AUTHORIZATION).is_none());
        assert!(warnings.is_empty());
    }

    // --- Bearer -----------------------------------------------------------

    #[test]
    fn test_bearer_sets_authorization_header() {
        let secret = MCPAuthSecret {
            token: Some("sk-secret-1234567890".to_string()),
            ..Default::default()
        };
        let (headers, _) = build_auth_headers(
            MCPAuthType::Bearer,
            &empty_metadata(),
            Some(&secret),
            &empty_extra(),
        )
        .unwrap();
        assert_eq!(
            headers.get(AUTHORIZATION).unwrap(),
            "Bearer sk-secret-1234567890"
        );
    }

    #[test]
    fn test_bearer_trims_token_whitespace() {
        let secret = MCPAuthSecret {
            token: Some("  sk-token  ".to_string()),
            ..Default::default()
        };
        let (headers, _) = build_auth_headers(
            MCPAuthType::Bearer,
            &empty_metadata(),
            Some(&secret),
            &empty_extra(),
        )
        .unwrap();
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer sk-token");
    }

    #[test]
    fn test_bearer_missing_token_errors() {
        let err = build_auth_headers(MCPAuthType::Bearer, &empty_metadata(), None, &empty_extra())
            .unwrap_err();
        match err {
            MCPError::InvalidConfig { reason, .. } => assert!(reason.contains("missing bearer")),
            other => panic!("expected InvalidConfig, got {:?}", other),
        }
    }

    #[test]
    fn test_bearer_extra_authorization_is_ignored_with_warning() {
        let secret = MCPAuthSecret {
            token: Some("sk-token".to_string()),
            ..Default::default()
        };
        let mut extra = HashMap::new();
        extra.insert("Authorization".to_string(), "Bearer leaked".to_string());
        let (headers, warnings) = build_auth_headers(
            MCPAuthType::Bearer,
            &empty_metadata(),
            Some(&secret),
            &extra,
        )
        .unwrap();
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer sk-token");
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("main auth wins"));
    }

    // --- Apikey -----------------------------------------------------------

    #[test]
    fn test_apikey_default_header_name() {
        let secret = MCPAuthSecret {
            value: Some("abc123".to_string()),
            ..Default::default()
        };
        let (headers, _) = build_auth_headers(
            MCPAuthType::Apikey,
            &empty_metadata(),
            Some(&secret),
            &empty_extra(),
        )
        .unwrap();
        assert_eq!(headers.get("X-API-Key").unwrap(), "abc123");
    }

    #[test]
    fn test_apikey_custom_header_name() {
        let metadata = MCPAuthMetadata {
            header_name: Some("X-Custom-Key".to_string()),
            username: None,
        };
        let secret = MCPAuthSecret {
            value: Some("abc123".to_string()),
            ..Default::default()
        };
        let (headers, _) = build_auth_headers(
            MCPAuthType::Apikey,
            &metadata,
            Some(&secret),
            &empty_extra(),
        )
        .unwrap();
        assert_eq!(headers.get("X-Custom-Key").unwrap(), "abc123");
    }

    #[test]
    fn test_apikey_missing_value_errors() {
        let err = build_auth_headers(MCPAuthType::Apikey, &empty_metadata(), None, &empty_extra())
            .unwrap_err();
        match err {
            MCPError::InvalidConfig { reason, .. } => assert!(reason.contains("missing api-key")),
            other => panic!("expected InvalidConfig, got {:?}", other),
        }
    }

    #[test]
    fn test_apikey_extra_same_header_is_ignored_with_warning() {
        let metadata = MCPAuthMetadata {
            header_name: Some("X-API-Key".to_string()),
            username: None,
        };
        let secret = MCPAuthSecret {
            value: Some("abc123".to_string()),
            ..Default::default()
        };
        let mut extra = HashMap::new();
        // Same key but different case: must still conflict (HTTP header names
        // are case-insensitive).
        extra.insert("x-api-key".to_string(), "leaked".to_string());
        let (headers, warnings) =
            build_auth_headers(MCPAuthType::Apikey, &metadata, Some(&secret), &extra).unwrap();
        assert_eq!(headers.get("X-API-Key").unwrap(), "abc123");
        assert_eq!(warnings.len(), 1);
    }

    // --- Basic ------------------------------------------------------------

    #[test]
    fn test_basic_encodes_credentials() {
        let metadata = MCPAuthMetadata {
            username: Some("alice".to_string()),
            header_name: None,
        };
        let secret = MCPAuthSecret {
            password: Some("secret".to_string()),
            ..Default::default()
        };
        let (headers, _) =
            build_auth_headers(MCPAuthType::Basic, &metadata, Some(&secret), &empty_extra())
                .unwrap();
        // "alice:secret" -> base64 = "YWxpY2U6c2VjcmV0"
        assert_eq!(
            headers.get(AUTHORIZATION).unwrap(),
            "Basic YWxpY2U6c2VjcmV0"
        );
    }

    #[test]
    fn test_basic_missing_username_errors() {
        let secret = MCPAuthSecret {
            password: Some("p".to_string()),
            ..Default::default()
        };
        let err = build_auth_headers(
            MCPAuthType::Basic,
            &empty_metadata(),
            Some(&secret),
            &empty_extra(),
        )
        .unwrap_err();
        assert!(matches!(err, MCPError::InvalidConfig { .. }));
    }

    #[test]
    fn test_basic_username_with_colon_rejected() {
        let metadata = MCPAuthMetadata {
            username: Some("ali:ce".to_string()),
            header_name: None,
        };
        let secret = MCPAuthSecret {
            password: Some("p".to_string()),
            ..Default::default()
        };
        let err = build_auth_headers(MCPAuthType::Basic, &metadata, Some(&secret), &empty_extra())
            .unwrap_err();
        match err {
            MCPError::InvalidConfig { reason, .. } => assert!(reason.contains(':')),
            other => panic!("expected InvalidConfig, got {:?}", other),
        }
    }

    // --- Extra headers ---------------------------------------------------

    #[test]
    fn test_extra_headers_added_after_auth() {
        let secret = MCPAuthSecret {
            token: Some("sk-token".to_string()),
            ..Default::default()
        };
        let mut extra = HashMap::new();
        extra.insert("X-Tenant-ID".to_string(), "42".to_string());
        extra.insert("X-Trace".to_string(), "abc123".to_string());
        let (headers, warnings) = build_auth_headers(
            MCPAuthType::Bearer,
            &empty_metadata(),
            Some(&secret),
            &extra,
        )
        .unwrap();
        assert_eq!(headers.get(AUTHORIZATION).unwrap(), "Bearer sk-token");
        assert_eq!(headers.get("X-Tenant-ID").unwrap(), "42");
        assert_eq!(headers.get("X-Trace").unwrap(), "abc123");
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_extra_headers_invalid_name_errors() {
        let mut extra = HashMap::new();
        extra.insert("éclair".to_string(), "value".to_string());
        let err =
            build_auth_headers(MCPAuthType::None, &empty_metadata(), None, &extra).unwrap_err();
        assert!(matches!(err, MCPError::InvalidConfig { .. }));
    }
}
