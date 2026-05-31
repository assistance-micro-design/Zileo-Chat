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

//! SSRF protection for the MCP HTTP transport (R-SEC-1 / R-SEC-3 / R-SEC-7).
//!
//! Two enforcement points, because hyper/reqwest only invokes a custom
//! `dns::Resolve` for a *hostname*:
//! 1. **Pre-connect** ([`screen_request_url`]): classifies a **literal** IP
//!    host parsed via `url::Host` (decimal/octal/hex are normalised by the URL
//!    parser) — `http://169.254.169.254/`, `http://[::1]/`, `http://2130706433/`
//!    never reach the resolver.
//! 2. **At connect** ([`SsrfResolver`]): classifies **every** resolved
//!    `SocketAddr`; the whole resolution is refused if a single address is
//!    forbidden (a malicious DNS cannot hide `169.254.169.254` behind a public
//!    A record). No decision cache => no DNS-rebinding window.
//!
//! The classifier [`classify_ip`] is pure and **decapsulates embedded IPv4**
//! (mapped / compatible / 6to4 / Teredo / NAT64) before re-classifying, so an
//! IPv6 wrapper cannot smuggle a private/metadata v4.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

/// Security classification of an IP address. Only [`IpClass::Global`] is
/// routable to the public internet; everything else is an SSRF risk
/// (loopback is conditionally allowed via the `allow_loopback` flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpClass {
    /// `127.0.0.0/8`, `::1`.
    Loopback,
    /// RFC1918 (`10/8`, `172.16/12`, `192.168/16`), CGNAT `100.64/10`, ULA `fc00::/7`.
    Private,
    /// IPv6 link-local `fe80::/10`.
    LinkLocal,
    /// Cloud metadata / IPv4 link-local `169.254.0.0/16` (entire block).
    Metadata,
    /// Unspecified, broadcast, benchmarking, protocol-assignment, etc.
    Reserved,
    /// Multicast (`224.0.0.0/4`, `ff00::/8`).
    Multicast,
    /// Publicly routable — the only allowed class.
    Global,
}

/// Classifies an IP address for SSRF screening (pure, no I/O).
pub fn classify_ip(ip: IpAddr) -> IpClass {
    match ip {
        IpAddr::V4(v4) => classify_v4(v4),
        IpAddr::V6(v6) => classify_v6(v6),
    }
}

fn classify_v4(ip: Ipv4Addr) -> IpClass {
    let [a, b, c, _d] = ip.octets();
    if ip.is_loopback() {
        return IpClass::Loopback; // 127.0.0.0/8
    }
    if ip.is_unspecified() || a == 0 {
        return IpClass::Reserved; // 0.0.0.0/8
    }
    if a == 169 && b == 254 {
        return IpClass::Metadata; // 169.254.0.0/16 (cloud metadata / link-local)
    }
    if ip.is_private() {
        return IpClass::Private; // 10/8, 172.16/12, 192.168/16
    }
    if a == 100 && (64..=127).contains(&b) {
        return IpClass::Private; // CGNAT 100.64.0.0/10
    }
    if a == 198 && (b == 18 || b == 19) {
        return IpClass::Reserved; // benchmarking 198.18.0.0/15
    }
    if a == 192 && b == 0 && c == 0 {
        return IpClass::Reserved; // IETF protocol assignments 192.0.0.0/24
    }
    if ip.is_broadcast() {
        return IpClass::Reserved; // 255.255.255.255
    }
    if ip.is_multicast() {
        return IpClass::Multicast; // 224.0.0.0/4
    }
    IpClass::Global
}

fn classify_v6(ip: Ipv6Addr) -> IpClass {
    // Decapsulate an embedded IPv4 first, then re-classify it — an IPv6
    // wrapper must not smuggle a private/metadata v4 past the classifier.
    if let Some(v4) = embedded_v4(ip) {
        return classify_v4(v4);
    }
    if ip.is_loopback() {
        return IpClass::Loopback; // ::1
    }
    if ip.is_unspecified() {
        return IpClass::Reserved; // ::
    }
    let seg = ip.segments();
    if (seg[0] & 0xffc0) == 0xfe80 {
        return IpClass::LinkLocal; // fe80::/10
    }
    if (seg[0] & 0xfe00) == 0xfc00 {
        return IpClass::Private; // ULA fc00::/7
    }
    if (seg[0] & 0xff00) == 0xff00 {
        return IpClass::Multicast; // ff00::/8
    }
    IpClass::Global
}

/// Extracts an embedded IPv4 address from an IPv6 wrapper (mapped, compatible,
/// 6to4, Teredo, NAT64), or `None` for a native IPv6 address.
fn embedded_v4(ip: Ipv6Addr) -> Option<Ipv4Addr> {
    let seg = ip.segments();
    let v4 = |hi: u16, lo: u16| {
        Ipv4Addr::new(
            (hi >> 8) as u8,
            (hi & 0xff) as u8,
            (lo >> 8) as u8,
            (lo & 0xff) as u8,
        )
    };

    // IPv4-mapped ::ffff:a.b.c.d
    if seg[0..5] == [0, 0, 0, 0, 0] && seg[5] == 0xffff {
        return Some(v4(seg[6], seg[7]));
    }
    // 6to4 2002:AABB:CCDD::/16 -> AABB.CCDD
    if seg[0] == 0x2002 {
        return Some(v4(seg[1], seg[2]));
    }
    // Teredo 2001:0000::/32 -> client v4 in the last 32 bits, obfuscated (XOR 0xffff)
    if seg[0] == 0x2001 && seg[1] == 0x0000 {
        return Some(v4(seg[6] ^ 0xffff, seg[7] ^ 0xffff));
    }
    // NAT64 well-known 64:ff9b::/96 and 64:ff9b:1::/48 -> last 32 bits
    if seg[0] == 0x0064 && seg[1] == 0xff9b {
        return Some(v4(seg[6], seg[7]));
    }
    // IPv4-compatible ::a.b.c.d (deprecated), excluding :: and ::1.
    if seg[0..6] == [0, 0, 0, 0, 0, 0] && !(seg[6] == 0 && seg[7] <= 1) {
        return Some(v4(seg[6], seg[7]));
    }
    None
}

/// Returns whether an address of the given class may be connected to.
///
/// `allow_loopback` is `true` for manual/runtime origins (a local MCP server
/// is legitimate) and `false` for imports.
fn ip_allowed(class: IpClass, allow_loopback: bool) -> bool {
    match class {
        IpClass::Global => true,
        IpClass::Loopback => allow_loopback,
        _ => false,
    }
}

/// Pre-connect screen of a request URL. Rejects unsupported schemes and
/// forbidden **literal** IP hosts; domains pass through (the resolver screens
/// them at connect time). `allow_loopback`: `true` = manual/runtime, `false` =
/// import.
pub fn screen_request_url(url: &str, allow_loopback: bool) -> Result<(), String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL '{url}': {e}"))?;
    match parsed.scheme() {
        "http" | "https" => {}
        other => {
            return Err(format!(
                "unsupported URL scheme '{other}': only http:// and https:// are allowed"
            ))
        }
    }
    match parsed.host() {
        Some(url::Host::Ipv4(v4)) => enforce_ip(IpAddr::V4(v4), allow_loopback, url),
        Some(url::Host::Ipv6(v6)) => enforce_ip(IpAddr::V6(v6), allow_loopback, url),
        Some(url::Host::Domain(d)) => {
            // A domain is normally screened by the resolver at connect, but the
            // well-known loopback name `localhost` (RFC 6761) must be blocked up
            // front under the import policy — it resolves to loopback, which the
            // shared runtime resolver allows.
            if !allow_loopback && is_loopback_domain(d) {
                Err(format!(
                    "refused loopback host '{d}' in URL '{url}' (import policy)"
                ))
            } else {
                Ok(())
            }
        }
        None => Err(format!("URL '{url}' has no host")),
    }
}

/// Returns true for the reserved loopback domain name `localhost` and its
/// subdomains (RFC 6761 §6.3), case-insensitively and ignoring a trailing dot.
fn is_loopback_domain(domain: &str) -> bool {
    let d = domain.trim_end_matches('.').to_ascii_lowercase();
    d == "localhost" || d.ends_with(".localhost")
}

fn enforce_ip(ip: IpAddr, allow_loopback: bool, url: &str) -> Result<(), String> {
    let class = classify_ip(ip);
    if ip_allowed(class, allow_loopback) {
        Ok(())
    } else {
        Err(format!(
            "refused SSRF target {ip} (classified {class:?}) in URL '{url}'"
        ))
    }
}

/// Screens the addresses a hostname resolved to. The **entire** resolution is
/// refused if a single address is forbidden, so a hostile DNS cannot pair a
/// public A record with `169.254.169.254`. `allow_loopback`: `true` =
/// manual/runtime, `false` = import.
pub fn screen_resolved_addrs(addrs: &[SocketAddr], allow_loopback: bool) -> Result<(), String> {
    for sa in addrs {
        let class = classify_ip(sa.ip());
        if !ip_allowed(class, allow_loopback) {
            return Err(format!(
                "refused SSRF: resolved address {} is {:?}",
                sa.ip(),
                class
            ));
        }
    }
    Ok(())
}

/// R-SEC-7: screens credentials sent over plaintext HTTP.
///
/// Returns `Err` when authentication would be sent over `http://` to a
/// non-loopback host (credentials leak), `Ok(Some(warning))` for an
/// acceptable-but-noteworthy case, and `Ok(None)` when there is nothing to
/// report (https, or plain http without auth on loopback).
pub fn screen_http_auth(url: &str, has_auth: bool) -> Result<Option<String>, String> {
    let parsed = url::Url::parse(url).map_err(|e| format!("invalid URL '{url}': {e}"))?;
    if parsed.scheme() != "http" {
        // https (or anything non-http) carries no plaintext-credential risk here.
        return Ok(None);
    }
    let is_loopback = match parsed.host() {
        Some(url::Host::Ipv4(v4)) => classify_ip(IpAddr::V4(v4)) == IpClass::Loopback,
        Some(url::Host::Ipv6(v6)) => classify_ip(IpAddr::V6(v6)) == IpClass::Loopback,
        Some(url::Host::Domain(d)) => d.eq_ignore_ascii_case("localhost"),
        None => false,
    };
    match (has_auth, is_loopback) {
        (true, false) => Err(format!(
            "refusing to send credentials over plaintext HTTP to a non-loopback host: '{url}' — use https://"
        )),
        (true, true) => Ok(Some(format!(
            "MCP server '{url}' uses HTTP (not HTTPS) with authentication on loopback; credentials stay on this host"
        ))),
        (false, false) => Ok(Some(format!(
            "MCP server '{url}' uses HTTP instead of HTTPS"
        ))),
        (false, true) => Ok(None),
    }
}

/// A `reqwest::dns::Resolve` that classifies every resolved address and refuses
/// the resolution if any address is forbidden. Re-classifies on every connect
/// (no decision cache) to close the DNS-rebinding window.
#[derive(Debug, Clone)]
pub struct SsrfResolver {
    allow_loopback: bool,
}

impl SsrfResolver {
    /// Creates a resolver. `allow_loopback`: `true` = manual/runtime,
    /// `false` = import.
    pub fn new(allow_loopback: bool) -> Self {
        Self { allow_loopback }
    }
}

impl reqwest::dns::Resolve for SsrfResolver {
    fn resolve(&self, name: reqwest::dns::Name) -> reqwest::dns::Resolving {
        let host = name.as_str().to_owned();
        let allow_loopback = self.allow_loopback;
        Box::pin(async move {
            let resolved: Vec<SocketAddr> = tokio::net::lookup_host((host.as_str(), 0u16))
                .await
                .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?
                .collect();
            screen_resolved_addrs(&resolved, allow_loopback)
                .map_err(|m| -> Box<dyn std::error::Error + Send + Sync> { m.into() })?;
            let iter: reqwest::dns::Addrs = Box::new(resolved.into_iter());
            Ok(iter)
        })
    }
}

/// Maximum number of redirect hops followed by the MCP HTTP client.
const MAX_REDIRECT_HOPS: usize = 3;

/// Outcome of [`classify_redirect`].
#[derive(Debug, PartialEq, Eq)]
pub enum RedirectDecision {
    /// The redirect is safe to follow.
    Follow,
    /// The redirect is refused, with a human-readable reason.
    Refuse(String),
}

/// Pure decision for a redirect hop (R-SEC-3), independent of reqwest's
/// `Attempt` so it can be unit-tested.
///
/// Refuses, in order: too many hops; an `https -> http` downgrade; a
/// **cross-host** redirect (the target host differs from the previous host —
/// closes both the cross-host SSRF case and the custom-auth-header leak, since
/// the shared client cannot strip per-request `X-API-Key`/extra headers); and a
/// **literal-IP** target that is not [`IpClass::Global`] (loopback included —
/// a remote server must never redirect us to a local/private/metadata IP).
/// Domain targets are followed and re-screened by [`SsrfResolver`] at the next
/// connect.
pub fn classify_redirect(target: &url::Url, prev: &url::Url, hops: usize) -> RedirectDecision {
    if hops >= MAX_REDIRECT_HOPS {
        return RedirectDecision::Refuse(format!("too many redirects (max {MAX_REDIRECT_HOPS})"));
    }
    if prev.scheme() == "https" && target.scheme() == "http" {
        return RedirectDecision::Refuse(
            "refused redirect downgrade from https to http".to_string(),
        );
    }
    // Cross-host redirect (keyed on the HOST, so a same-host scheme/port change
    // is still allowed): refused. Closes the cross-host SSRF case and the
    // custom-auth-header leak — the shared client cannot strip a per-request
    // `X-API-Key`/extra header on a cross-origin hop.
    if target.host_str() != prev.host_str() {
        return RedirectDecision::Refuse(format!(
            "refused cross-host redirect ({} -> {})",
            prev.host_str().unwrap_or("?"),
            target.host_str().unwrap_or("?")
        ));
    }
    // Literal-IP target: hyper never invokes the DNS resolver for a literal IP,
    // so it must be classified here. On a redirect target even loopback is
    // refused — a remote server must never bounce us to a local/private IP.
    match target.host() {
        Some(url::Host::Ipv4(v4)) if classify_ip(IpAddr::V4(v4)) != IpClass::Global => {
            RedirectDecision::Refuse(format!("refused redirect to non-global literal IP {v4}"))
        }
        Some(url::Host::Ipv6(v6)) if classify_ip(IpAddr::V6(v6)) != IpClass::Global => {
            RedirectDecision::Refuse(format!("refused redirect to non-global literal IP {v6}"))
        }
        _ => RedirectDecision::Follow,
    }
}

/// Redirect policy for the MCP HTTP client (R-SEC-3). Delegates each hop to the
/// pure [`classify_redirect`]: caps hops, blocks `https -> http` downgrade,
/// refuses cross-host redirects, and screens literal-IP targets (the DNS
/// resolver is never invoked for a literal-IP redirect target, so it must be
/// classified here). Domain targets are re-screened by [`SsrfResolver`] at the
/// next connect.
pub fn mcp_redirect_policy() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let hops = attempt.previous().len();
        let prev = match attempt.previous().last() {
            Some(p) => p.clone(),
            None => return attempt.follow(),
        };
        match classify_redirect(attempt.url(), &prev, hops) {
            RedirectDecision::Follow => attempt.follow(),
            RedirectDecision::Refuse(reason) => attempt.error(reason),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v4(s: &str) -> IpAddr {
        IpAddr::V4(s.parse::<Ipv4Addr>().unwrap())
    }
    fn v6(s: &str) -> IpAddr {
        IpAddr::V6(s.parse::<Ipv6Addr>().unwrap())
    }

    // ---------- classify_ip: IPv4 ----------

    #[test]
    fn classify_v4_non_global() {
        assert_eq!(classify_ip(v4("127.0.0.1")), IpClass::Loopback);
        assert_eq!(classify_ip(v4("169.254.169.254")), IpClass::Metadata);
        assert_eq!(classify_ip(v4("10.0.0.1")), IpClass::Private);
        assert_eq!(classify_ip(v4("172.16.0.1")), IpClass::Private);
        assert_eq!(classify_ip(v4("192.168.1.1")), IpClass::Private);
        assert_eq!(classify_ip(v4("100.64.0.1")), IpClass::Private); // CGNAT
        assert_eq!(classify_ip(v4("0.0.0.0")), IpClass::Reserved);
        assert_eq!(classify_ip(v4("198.18.0.1")), IpClass::Reserved); // benchmarking
        assert_eq!(classify_ip(v4("192.0.0.1")), IpClass::Reserved); // 192.0.0.0/24
        assert_eq!(classify_ip(v4("255.255.255.255")), IpClass::Reserved); // broadcast
        assert_eq!(classify_ip(v4("224.0.0.1")), IpClass::Multicast);
    }

    #[test]
    fn classify_v4_global() {
        assert_eq!(classify_ip(v4("93.184.216.34")), IpClass::Global);
        assert_eq!(classify_ip(v4("8.8.8.8")), IpClass::Global);
    }

    // ---------- classify_ip: IPv6 + decapsulation ----------

    #[test]
    fn classify_v6_non_global() {
        assert_eq!(classify_ip(v6("::1")), IpClass::Loopback);
        assert_eq!(classify_ip(v6("::")), IpClass::Reserved);
        assert_eq!(classify_ip(v6("fc00::1")), IpClass::Private); // ULA
        assert_eq!(classify_ip(v6("fe80::1")), IpClass::LinkLocal);
        assert_eq!(classify_ip(v6("ff02::1")), IpClass::Multicast);
    }

    #[test]
    fn classify_v6_embedded_v4_is_decapsulated() {
        // IPv4-mapped ::ffff:169.254.169.254
        assert_eq!(classify_ip(v6("::ffff:169.254.169.254")), IpClass::Metadata);
        // IPv4-compatible ::169.254.169.254 (== ::a9fe:a9fe)
        assert_eq!(classify_ip(v6("::a9fe:a9fe")), IpClass::Metadata);
        // 6to4 2002:0a00:0001:: -> 10.0.0.1
        assert_eq!(classify_ip(v6("2002:0a00:0001::")), IpClass::Private);
        // NAT64 64:ff9b::169.254.169.254
        assert_eq!(classify_ip(v6("64:ff9b::a9fe:a9fe")), IpClass::Metadata);
    }

    #[test]
    fn classify_v6_global() {
        assert_eq!(classify_ip(v6("2606:4700:4700::1111")), IpClass::Global);
    }

    // ---------- ip_allowed policy ----------

    #[test]
    fn loopback_allowed_only_under_allow_policy() {
        assert!(ip_allowed(IpClass::Loopback, true));
        assert!(!ip_allowed(IpClass::Loopback, false));
        assert!(ip_allowed(IpClass::Global, false));
        assert!(!ip_allowed(IpClass::Metadata, true));
        assert!(!ip_allowed(IpClass::Private, true));
    }

    // ---------- pre-connect literal screening ----------

    #[test]
    fn screen_url_literal_metadata_refused_any_policy() {
        assert!(screen_request_url("http://169.254.169.254/", true).is_err());
        assert!(screen_request_url("http://169.254.169.254/", false).is_err());
    }

    #[test]
    fn screen_url_literal_loopback_depends_on_policy() {
        // [::1], decimal and octal/hex forms all normalise to loopback.
        for u in [
            "http://[::1]/",
            "http://2130706433/",
            "http://0177.0.0.1/",
            "http://0x7f.0.0.1/",
        ] {
            assert!(
                screen_request_url(u, true).is_ok(),
                "{u} should be allowed under Allow"
            );
            assert!(
                screen_request_url(u, false).is_err(),
                "{u} should be blocked under Block (import)"
            );
        }
    }

    #[test]
    fn screen_url_domain_passes_to_resolver() {
        assert!(screen_request_url("https://example.com/mcp", false).is_ok());
    }

    #[test]
    fn screen_url_localhost_domain_blocked_on_import() {
        // The `localhost` name resolves to loopback -> blocked under import
        // (allow_loopback=false) but allowed under runtime/manual (true).
        assert!(screen_request_url("http://localhost:8080/", false).is_err());
        assert!(screen_request_url("http://localhost:8080/", true).is_ok());
        assert!(screen_request_url("http://app.localhost/", false).is_err());
    }

    #[test]
    fn screen_url_rejects_non_http_scheme() {
        assert!(screen_request_url("file:///etc/passwd", true).is_err());
    }

    // ---------- resolver decision (no network) ----------

    #[test]
    fn resolved_addrs_reject_whole_set_if_any_forbidden() {
        let public: SocketAddr = "93.184.216.34:443".parse().unwrap();
        let meta: SocketAddr = "169.254.169.254:80".parse().unwrap();
        assert!(screen_resolved_addrs(&[public], true).is_ok());
        // [public, metadata] -> entire resolution refused.
        assert!(screen_resolved_addrs(&[public, meta], true).is_err());
    }

    #[test]
    fn resolved_addrs_loopback_policy() {
        let lo: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        assert!(screen_resolved_addrs(&[lo], true).is_ok());
        assert!(screen_resolved_addrs(&[lo], false).is_err());
    }

    // ---------- R-SEC-7 ----------

    #[test]
    fn http_auth_remote_blocked() {
        assert!(screen_http_auth("http://api.example.com/mcp", true).is_err());
        // decimal-encoded public IP (8.8.8.8) is decoded and treated as remote.
        assert!(screen_http_auth("http://134744072/", true).is_err());
    }

    #[test]
    fn http_auth_loopback_warns_not_blocked() {
        // localhost + auth over http -> warning, allowed.
        let r = screen_http_auth("http://localhost:8080/mcp", true);
        assert!(matches!(r, Ok(Some(_))), "got {r:?}");
        // 2130706433 == 127.0.0.1 (loopback) -> warning, NOT blocked.
        let r2 = screen_http_auth("http://2130706433/", true);
        assert!(matches!(r2, Ok(Some(_))), "got {r2:?}");
    }

    #[test]
    fn http_auth_https_ok() {
        assert_eq!(
            screen_http_auth("https://api.example.com/mcp", true).unwrap(),
            None
        );
    }

    #[test]
    fn http_no_auth_remote_warns() {
        assert!(matches!(
            screen_http_auth("http://api.example.com/mcp", false),
            Ok(Some(_))
        ));
    }

    // ---------- redirect policy decision (R-SEC-3 hardening) ----------

    fn u(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    #[test]
    fn redirect_too_many_hops_refused() {
        assert!(matches!(
            classify_redirect(&u("https://h.com/b"), &u("https://h.com/a"), 3),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_downgrade_https_to_http_refused() {
        assert!(matches!(
            classify_redirect(&u("http://h.com/"), &u("https://h.com/"), 1),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_cross_host_domain_refused() {
        assert!(matches!(
            classify_redirect(&u("https://other.com/"), &u("https://evil.com/"), 1),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_to_literal_metadata_refused() {
        // remote server redirecting to the cloud metadata IP.
        assert!(matches!(
            classify_redirect(
                &u("http://169.254.169.254/"),
                &u("https://api.example.com/mcp"),
                1
            ),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_same_host_literal_private_refused() {
        // same literal-IP host + path change -> the IP-classification branch refuses.
        assert!(matches!(
            classify_redirect(&u("https://10.0.0.1/b"), &u("https://10.0.0.1/a"), 1),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_same_host_literal_loopback_refused() {
        // on a redirect target, loopback is blocked too.
        assert!(matches!(
            classify_redirect(&u("https://[::1]/b"), &u("https://[::1]/a"), 1),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_same_host_literal_decimal_loopback_refused() {
        // http://2130706433/ == 127.0.0.1
        assert!(matches!(
            classify_redirect(&u("http://2130706433/b"), &u("http://2130706433/a"), 1),
            RedirectDecision::Refuse(_)
        ));
    }

    #[test]
    fn redirect_same_host_upgrade_followed() {
        assert_eq!(
            classify_redirect(&u("https://h.com/"), &u("http://h.com/"), 1),
            RedirectDecision::Follow
        );
    }

    #[test]
    fn redirect_same_host_path_change_followed() {
        assert_eq!(
            classify_redirect(&u("https://h.com/b"), &u("https://h.com/a"), 1),
            RedirectDecision::Follow
        );
    }
}
