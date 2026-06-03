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

/// Placeholder returned by [`redact_url_userinfo`] when a URL cannot be parsed.
///
/// Returning a fixed safe string (rather than the original) guarantees an
/// unparseable URL can never leak an embedded `user:pass@` into the logs.
const REDACTED_URL_PLACEHOLDER: &str = "<url redacted>";

/// Returns a copy of `url` with any userinfo (`user:pass@host`) stripped, safe
/// for logging.
///
/// MCP base URLs can carry credentials in the authority
/// (`http://user:pass@192.168.1.5/mcp`). Logging such a URL verbatim would leak
/// the secret (violates "never log secrets"). This helper blanks both the
/// username and password components while preserving scheme/host/port/path.
///
/// # Errors
///
/// Never returns an error: an unparseable or authority-less URL yields
/// [`REDACTED_URL_PLACEHOLDER`] rather than the original string (fail-safe —
/// never echo a possibly-secret raw value).
pub fn redact_url_userinfo(url: &str) -> String {
    let Ok(mut parsed) = url::Url::parse(url) else {
        return REDACTED_URL_PLACEHOLDER.to_string();
    };
    // `set_username("")` / `set_password(None)` return Err for "cannot-be-a-base"
    // URLs (no authority — e.g. `user:pass@not a url` parses as scheme `user:`
    // with the rest as the path). Such a string would round-trip verbatim and
    // could still embed a secret, so fail safe to the placeholder.
    if parsed.set_username("").is_err() || parsed.set_password(None).is_err() {
        return REDACTED_URL_PLACEHOLDER.to_string();
    }
    parsed.to_string()
}

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

    // -------- redact_url_userinfo --------

    #[test]
    fn test_redact_url_strips_user_and_password() {
        let out = redact_url_userinfo("http://user:pass@192.168.1.5/mcp");
        assert!(!out.contains("user"), "username leaked: {out}");
        assert!(!out.contains("pass"), "password leaked: {out}");
        assert!(out.contains("192.168.1.5"), "host lost: {out}");
        assert!(out.contains("/mcp"), "path lost: {out}");
    }

    #[test]
    fn test_redact_url_strips_username_only() {
        let out = redact_url_userinfo("https://token@example.com/mcp");
        assert!(!out.contains("token"), "username leaked: {out}");
        assert!(out.contains("example.com"));
    }

    #[test]
    fn test_redact_url_without_userinfo_is_unchanged() {
        let url = "https://api.example.com:8443/mcp?x=1";
        assert_eq!(redact_url_userinfo(url), url);
    }

    #[test]
    fn test_redact_url_unparseable_returns_safe_placeholder() {
        // A non-URL must never be echoed back verbatim (it could embed creds).
        let out = redact_url_userinfo("user:pass@not a url");
        assert_eq!(out, REDACTED_URL_PLACEHOLDER);
        assert!(!out.contains("pass"));
    }
}
