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

//! Process-global snapshot of the MCP network connectivity settings.
//!
//! `MCPHttpHandle::connect()` has no database handle, so the user-toggled LAN
//! opt-in is mirrored here as a read-mostly global. The snapshot is `Copy`
//! (a single bool today) and is seeded once at boot from the persisted
//! settings, then updated by the `update_mcp_network_settings` command.
//!
//! Fail-secure: every accessor degrades to the **default** (`{ allow_private_
//! network: false }` = strict) if the lock is poisoned — never `.unwrap()`,
//! never a panic.
//!
//! ## Test-isolation invariant (MANDATORY — read before adding tests)
//!
//! This module owns process-global **mutable** state. `cargo test` runs the
//! tests of a binary on multiple threads in parallel, and the project has **no**
//! `serial_test` dependency (and must not add one).
//! Therefore:
//!
//! - Any test that **mutates** the global (`set_network_settings`) OR that
//!   **reads** `current_network_settings()` indirectly (e.g. by driving
//!   `MCPHttpHandle::connect()`) must be kept **confined to this module** and
//!   must **restore the default** at the end (`set_network_settings(McpNetwork
//!   Settings::default())`), so a parallel test in the same binary never
//!   observes a transient value.
//! - Do **NOT** assert client-variant behaviour *through* `connect()` in a test
//!   elsewhere in the binary without a serialization guard: it would read this
//!   global concurrently with `set_then_get_roundtrips` and flake. Assert the
//!   policy mapping against the **pure** `screen_*` functions instead (as
//!   `mcp::http_handle::tests` does), never against the live global.
//!
//! Pure logic (`McpNetworkSettings`, the `ScreenPolicy`/`screen_*` helpers) is
//! parameterised and never touches this global — those tests are safe in
//! parallel.

use serde::{Deserialize, Serialize};
use std::sync::RwLock;

/// MCP network connectivity settings.
///
/// `allow_private_network`: opt-in to reach MCP HTTP servers on RFC1918 / CGNAT
/// / ULA ranges (the LAN toggle). Disabled by default (secure-by-default).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct McpNetworkSettings {
    /// Whether MCP HTTP connections to private/LAN addresses are allowed.
    pub allow_private_network: bool,
}

/// Process-global snapshot. Initialised to the secure default; seeded at boot.
static MCP_NETWORK_SETTINGS: RwLock<McpNetworkSettings> = RwLock::new(McpNetworkSettings {
    allow_private_network: false,
});

/// Returns the current network settings snapshot.
///
/// Fail-secure: a poisoned lock yields the strict default rather than panicking.
pub fn current_network_settings() -> McpNetworkSettings {
    MCP_NETWORK_SETTINGS.read().map(|g| *g).unwrap_or_default()
}

/// Replaces the network settings snapshot.
///
/// Fail-secure: a poisoned lock is a no-op (the snapshot keeps its prior, more
/// conservative value) rather than panicking.
pub fn set_network_settings(settings: McpNetworkSettings) {
    if let Ok(mut guard) = MCP_NETWORK_SETTINGS.write() {
        *guard = settings;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_strict() {
        assert!(!McpNetworkSettings::default().allow_private_network);
    }

    #[test]
    fn set_then_get_roundtrips() {
        // This test mutates the global, so it must restore the default after
        // itself to avoid leaking state into other tests in the same binary.
        set_network_settings(McpNetworkSettings {
            allow_private_network: true,
        });
        assert!(current_network_settings().allow_private_network);
        set_network_settings(McpNetworkSettings::default());
        assert!(!current_network_settings().allow_private_network);
    }

    #[test]
    fn serde_uses_camel_case() {
        let json = serde_json::to_string(&McpNetworkSettings {
            allow_private_network: true,
        })
        .unwrap();
        assert!(json.contains("allowPrivateNetwork"));
        let parsed: McpNetworkSettings =
            serde_json::from_str(r#"{"allowPrivateNetwork":true}"#).unwrap();
        assert!(parsed.allow_private_network);
    }
}
