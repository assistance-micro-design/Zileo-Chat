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

//! Per-server MCP secret storage in the OS keychain (v1.2).
//!
//! Stores `MCPAuthSecret` payloads — JSON-encoded then AES-256-GCM
//! encrypted via the existing [`KeyStore`] (same `zileo-chat` keyring
//! service used for LLM API keys). Each MCP server gets its own entry
//! keyed by `mcp_auth_<server_id>`.
//!
//! Secrets never appear in error messages: `KeyStoreError::NotFound`
//! becomes `Ok(None)` on load, every other error is propagated as
//! [`MCPError::DatabaseError`] (no secret leakage).

use crate::mcp::{MCPError, MCPResult};
use crate::models::mcp::MCPAuthSecret;
use crate::security::keystore::{KeyStore, KeyStoreError};
use tracing::warn;

/// Builds the keystore "provider" name used to address the MCP secret entry.
///
/// The keyring service is shared with LLM API keys (`zileo-chat`); the
/// `KeyStore::save` API treats its first argument as the keychain user.
/// Prefixing with `mcp_auth_` keeps the namespaces disjoint from LLM keys
/// (`api_key_<provider>`).
fn secret_key(server_id: &str) -> String {
    format!("mcp_auth_{}", server_id)
}

/// Persists a [`MCPAuthSecret`] for a given MCP server in the OS keychain.
///
/// The payload is JSON-encoded — `MCPAuthSecret` is a flat struct of three
/// optional strings, sized in the few hundred bytes range — then encrypted
/// by the underlying `KeyStore`. Empty secrets (no token / value /
/// password) are rejected to avoid storing useless rows.
pub fn save_mcp_secret(
    keystore: &KeyStore,
    server_id: &str,
    secret: &MCPAuthSecret,
) -> MCPResult<()> {
    if secret.token.is_none() && secret.value.is_none() && secret.password.is_none() {
        return Err(MCPError::InvalidConfig {
            field: "auth_secret".to_string(),
            reason: "secret payload is empty".to_string(),
        });
    }
    let payload = serde_json::to_string(secret).map_err(|e| MCPError::SerializationError {
        context: "serialize MCP auth secret".to_string(),
        message: e.to_string(),
    })?;
    keystore
        .save(&secret_key(server_id), &payload)
        .map_err(|e| MCPError::DatabaseError {
            context: "save MCP secret in keychain".to_string(),
            message: e.to_string(),
        })?;
    Ok(())
}

/// Loads the [`MCPAuthSecret`] for a given MCP server from the keychain.
///
/// Returns `Ok(None)` when the entry is absent (e.g. server with
/// `auth_type=None`, or freshly imported server whose secret hasn't been
/// re-entered yet). Other failures bubble up as [`MCPError::DatabaseError`]
/// with no secret content.
pub fn load_mcp_secret(keystore: &KeyStore, server_id: &str) -> MCPResult<Option<MCPAuthSecret>> {
    match keystore.get(&secret_key(server_id)) {
        Ok(payload) => {
            let secret: MCPAuthSecret =
                serde_json::from_str(&payload).map_err(|e| MCPError::SerializationError {
                    context: "deserialize MCP auth secret".to_string(),
                    message: e.to_string(),
                })?;
            Ok(Some(secret))
        }
        Err(KeyStoreError::NotFound(_)) => Ok(None),
        Err(e) => Err(MCPError::DatabaseError {
            context: "load MCP secret from keychain".to_string(),
            message: e.to_string(),
        }),
    }
}

/// Deletes the keychain entry for a given MCP server (best-effort).
///
/// Missing entries are not errors — we treat them as already-deleted.
/// Other failures are surfaced via a `warn!` log so callers can decide
/// whether to fail the surrounding operation.
pub fn delete_mcp_secret(keystore: &KeyStore, server_id: &str) -> MCPResult<()> {
    match keystore.delete(&secret_key(server_id)) {
        Ok(()) => Ok(()),
        Err(KeyStoreError::NotFound(_)) => Ok(()),
        Err(e) => {
            warn!(
                server_id = %server_id,
                error = %e,
                "Failed to delete MCP secret from keychain (best-effort)"
            );
            Err(MCPError::DatabaseError {
                context: "delete MCP secret from keychain".to_string(),
                message: e.to_string(),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unencrypted_keystore() -> KeyStore {
        // The unencrypted variant talks to the OS keyring directly without
        // the AES master key. It's the path used in CI Linux without a
        // libsecret daemon available — the `keyring` crate falls back to
        // the mock backend on platforms without an active credential store.
        KeyStore::new_without_encryption()
    }

    fn unique_id() -> String {
        // A timestamp + random suffix avoids collisions across parallel tests.
        format!(
            "test_{}_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            uuid::Uuid::new_v4()
        )
    }

    #[test]
    fn test_secret_key_is_namespaced() {
        // The naming scheme is part of the contract: it must keep MCP
        // secrets disjoint from LLM API key entries (`api_key_<provider>`).
        assert_eq!(secret_key("abc"), "mcp_auth_abc");
        assert_eq!(secret_key("with-dash"), "mcp_auth_with-dash");
    }

    #[test]
    fn test_save_rejects_empty_secret() {
        let store = unencrypted_keystore();
        let empty = MCPAuthSecret::default();
        let err = save_mcp_secret(&store, &unique_id(), &empty).unwrap_err();
        match err {
            MCPError::InvalidConfig { reason, .. } => assert!(reason.contains("empty")),
            other => panic!("expected InvalidConfig, got {:?}", other),
        }
    }

    #[test]
    #[ignore = "Requires keyring access; flaky in headless CI"]
    fn test_save_load_delete_round_trip() {
        let store = KeyStore::new().expect("keystore with master key");
        let id = unique_id();

        // Initially absent
        assert!(load_mcp_secret(&store, &id).unwrap().is_none());

        let secret = MCPAuthSecret {
            token: Some("sk-test-token".to_string()),
            ..Default::default()
        };
        save_mcp_secret(&store, &id, &secret).unwrap();

        let loaded = load_mcp_secret(&store, &id).unwrap().expect("present");
        assert_eq!(loaded.token.as_deref(), Some("sk-test-token"));

        delete_mcp_secret(&store, &id).unwrap();
        assert!(load_mcp_secret(&store, &id).unwrap().is_none());

        // Idempotent delete
        delete_mcp_secret(&store, &id).unwrap();
    }
}
