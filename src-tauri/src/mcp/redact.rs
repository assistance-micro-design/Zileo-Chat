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

//! Header redaction for safe logging (v1.2).
//!
//! When the HTTP transport logs a `HeaderMap`, we must NEVER include the
//! values of authentication-bearing headers. This module produces a list
//! of `"name: ***"` strings safe for `tracing::debug!` / `info!`.

use reqwest::header::HeaderMap;

/// Header names that always have their value masked.
/// All comparisons are case-insensitive.
const ALWAYS_MASKED: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "set-cookie",
];

/// Threshold above which any header value is masked, even when the header
/// name is not in [`ALWAYS_MASKED`]. Anything longer than 6 chars is treated
/// as "potentially secret" (extra headers may carry tokens too).
const VALUE_LENGTH_MASK_THRESHOLD: usize = 6;

/// Returns a redacted representation of the given headers, safe for logs.
///
/// Each entry is formatted as `"<name>: ***"` for masked headers, or
/// `"<name>: <value>"` for short, non-sensitive ones. The header NAME is
/// always preserved (so that operators can still see *which* headers were
/// set), but the VALUE is masked whenever it could plausibly carry a secret.
///
/// Output is sorted alphabetically by header name for stable test
/// assertions and predictable log output.
pub fn redact_headers(headers: &HeaderMap) -> Vec<String> {
    let mut entries: Vec<(String, String)> = headers
        .iter()
        .map(|(name, value)| {
            let name_str = name.as_str().to_string();
            let display_value = if is_always_masked(&name_str) {
                "***".to_string()
            } else {
                match value.to_str() {
                    Ok(v) if v.len() <= VALUE_LENGTH_MASK_THRESHOLD => v.to_string(),
                    _ => "***".to_string(),
                }
            };
            (name_str, display_value)
        })
        .collect();
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    entries
        .into_iter()
        .map(|(name, value)| format!("{}: {}", name, value))
        .collect()
}

fn is_always_masked(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    ALWAYS_MASKED.iter().any(|m| *m == lower)
        // Custom auth-style headers (X-API-Key, X-Auth-Token, X-Token, ...)
        // — masked even when not exhaustively listed.
        || lower.starts_with("x-api-")
        || lower.contains("api-key")
        || lower.contains("token")
        || lower.contains("secret")
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderName, HeaderValue, AUTHORIZATION};

    fn header_map(entries: &[(&str, &str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (name, value) in entries {
            let name = HeaderName::from_bytes(name.as_bytes()).expect("valid header name");
            let value = HeaderValue::from_str(value).expect("valid header value");
            map.insert(name, value);
        }
        map
    }

    #[test]
    fn test_authorization_value_is_always_masked() {
        let mut map = HeaderMap::new();
        map.insert(
            AUTHORIZATION,
            HeaderValue::from_static("Bearer sk-secret-token-1234"),
        );
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["authorization: ***"]);
    }

    #[test]
    fn test_x_api_key_value_is_masked() {
        let map = header_map(&[("X-API-Key", "abcdef1234")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["x-api-key: ***"]);
    }

    #[test]
    fn test_long_value_is_masked_even_when_name_is_innocent() {
        let map = header_map(&[("X-Tenant-ID", "looooooooong-tenant")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["x-tenant-id: ***"]);
    }

    #[test]
    fn test_short_value_is_kept_when_name_is_innocent() {
        let map = header_map(&[("X-Tenant", "42")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["x-tenant: 42"]);
    }

    #[test]
    fn test_token_in_name_is_masked() {
        let map = header_map(&[("X-Auth-Token", "abc")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["x-auth-token: ***"]);
    }

    #[test]
    fn test_secret_in_name_is_masked_short_value() {
        let map = header_map(&[("X-Client-Secret", "sec")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["x-client-secret: ***"]);
    }

    #[test]
    fn test_proxy_authorization_is_masked() {
        let map = header_map(&[("Proxy-Authorization", "Basic xxx")]);
        let redacted = redact_headers(&map);
        assert_eq!(redacted, vec!["proxy-authorization: ***"]);
    }

    #[test]
    fn test_output_is_alphabetically_sorted() {
        let map = header_map(&[("X-Trace", "12"), ("Authorization", "Bearer xxx")]);
        let redacted = redact_headers(&map);
        // After lower-casing the names: 'authorization' < 'x-trace'
        assert_eq!(redacted[0], "authorization: ***");
        assert_eq!(redacted[1], "x-trace: 12");
    }

    #[test]
    fn test_empty_map() {
        let redacted = redact_headers(&HeaderMap::new());
        assert!(redacted.is_empty());
    }

    #[test]
    fn test_name_preserved_value_masked() {
        // The NAME of the header must always be visible — only the VALUE is
        // hidden. This test reinforces that contract.
        let map = header_map(&[("Authorization", "Bearer s")]);
        let redacted = redact_headers(&map);
        assert!(redacted[0].starts_with("authorization:"));
        assert!(redacted[0].ends_with("***"));
    }
}
