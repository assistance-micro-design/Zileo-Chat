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

//! Secure API key storage using OS keychain (keyring) + AES-256-GCM encryption.
//!
//! Provides a secure way to store and retrieve API keys for LLM providers.
//! The keys are stored in the OS keychain (Linux: libsecret, macOS: Keychain, Windows: Credential Manager)
//! and additionally encrypted with AES-256-GCM for defense in depth.

use aes_gcm::{
    aead::{Aead, KeyInit, OsRng},
    Aes256Gcm, Nonce,
};
use keyring::Entry;
use thiserror::Error;

/// Service name for keyring entries
const KEYRING_SERVICE: &str = "zileo-chat";
/// Username prefix for keyring entries
const KEYRING_USER_PREFIX: &str = "api_key_";
/// Key used for storing the encryption master key in keyring
const MASTER_KEY_NAME: &str = "__master_encryption_key__";
/// AES-256 key size in bytes
const AES_KEY_SIZE: usize = 32;
/// AES-GCM nonce size in bytes
const NONCE_SIZE: usize = 12;

/// Errors that can occur during keystore operations
#[derive(Debug, Error)]
pub enum KeyStoreError {
    /// Failed to access OS keychain
    #[error("Keychain access error: {0}")]
    KeychainError(String),

    /// Failed to encrypt/decrypt data
    #[error("Encryption error: {0}")]
    EncryptionError(String),

    /// The requested key was not found
    #[error("API key not found for provider: {0}")]
    NotFound(String),

    /// Invalid data format
    #[error("Invalid data format: {0}")]
    InvalidFormat(String),

    /// Provider name is invalid
    #[error("Invalid provider name: {0}")]
    InvalidProvider(String),
}

/// Secure keystore for API key management.
///
/// Uses a two-layer security approach:
/// 1. OS keychain (keyring) for secure storage
/// 2. AES-256-GCM encryption for additional protection
pub struct KeyStore {
    /// Cached AES cipher initialized with master key
    cipher: Option<Aes256Gcm>,
}

impl KeyStore {
    /// Creates a new KeyStore instance.
    ///
    /// Initializes or retrieves the master encryption key from the keychain.
    pub fn new() -> Result<Self, KeyStoreError> {
        let master_key = Self::get_or_create_master_key()?;
        let cipher = Aes256Gcm::new_from_slice(&master_key).map_err(|e| {
            KeyStoreError::EncryptionError(format!("Failed to create cipher: {}", e))
        })?;

        Ok(Self {
            cipher: Some(cipher),
        })
    }

    /// Creates a KeyStore that only uses keyring without additional encryption.
    ///
    /// Useful for tests that should not depend on AES master-key bootstrap.
    #[cfg(test)]
    pub fn new_without_encryption() -> Self {
        Self { cipher: None }
    }

    /// Stores an API key for a provider.
    ///
    /// The key is encrypted with AES-256-GCM before being stored in the keychain.
    pub fn save(&self, provider: &str, api_key: &str) -> Result<(), KeyStoreError> {
        // Validate provider name
        if provider.is_empty() || provider.contains(char::is_whitespace) {
            return Err(KeyStoreError::InvalidProvider(provider.to_string()));
        }

        let entry = Self::get_entry(provider)?;

        // Encrypt the API key if cipher is available
        let data_to_store = if let Some(ref cipher) = self.cipher {
            let encrypted = self.encrypt(cipher, api_key.as_bytes())?;
            // Store as base64 for safe keychain storage
            base64_encode(&encrypted)
        } else {
            api_key.to_string()
        };

        entry
            .set_password(&data_to_store)
            .map_err(|e| KeyStoreError::KeychainError(format!("Failed to store key: {}", e)))?;

        Ok(())
    }

    /// Retrieves an API key for a provider.
    ///
    /// Returns the decrypted API key if found.
    pub fn get(&self, provider: &str) -> Result<String, KeyStoreError> {
        let entry = Self::get_entry(provider)?;

        let stored_data = entry.get_password().map_err(|e| match e {
            keyring::Error::NoEntry => KeyStoreError::NotFound(provider.to_string()),
            _ => KeyStoreError::KeychainError(format!("Failed to retrieve key: {}", e)),
        })?;

        // Decrypt if cipher is available
        if let Some(ref cipher) = self.cipher {
            let encrypted = base64_decode(&stored_data)
                .map_err(|e| KeyStoreError::InvalidFormat(format!("Invalid base64: {}", e)))?;
            let decrypted = self.decrypt(cipher, &encrypted)?;
            String::from_utf8(decrypted)
                .map_err(|e| KeyStoreError::InvalidFormat(format!("Invalid UTF-8: {}", e)))
        } else {
            Ok(stored_data)
        }
    }

    /// Deletes an API key for a provider.
    pub fn delete(&self, provider: &str) -> Result<(), KeyStoreError> {
        let entry = Self::get_entry(provider)?;

        entry.delete_credential().map_err(|e| match e {
            keyring::Error::NoEntry => KeyStoreError::NotFound(provider.to_string()),
            _ => KeyStoreError::KeychainError(format!("Failed to delete key: {}", e)),
        })?;

        Ok(())
    }

    /// Checks if an API key exists for a provider.
    pub fn exists(&self, provider: &str) -> bool {
        if let Ok(entry) = Self::get_entry(provider) {
            entry.get_password().is_ok()
        } else {
            false
        }
    }

    /// Lists all providers that have stored API keys.
    ///
    /// Note: This is a best-effort operation as keyring does not support
    /// enumeration on all platforms. Returns known provider names that exist.
    pub fn list_providers(&self) -> Vec<String> {
        // Common provider names to check
        const KNOWN_PROVIDERS: &[&str] = &[
            "Mistral",
            "Ollama",
            "OpenAI",
            "Anthropic",
            "Google",
            "Cohere",
            "HuggingFace",
        ];

        KNOWN_PROVIDERS
            .iter()
            .filter(|p| self.exists(p))
            .map(|p| p.to_string())
            .collect()
    }

    /// Gets or creates the master encryption key.
    fn get_or_create_master_key() -> Result<Vec<u8>, KeyStoreError> {
        let entry = Entry::new(KEYRING_SERVICE, MASTER_KEY_NAME).map_err(|e| {
            KeyStoreError::KeychainError(format!("Failed to access keychain: {}", e))
        })?;

        match entry.get_password() {
            Ok(key_b64) => {
                // Decode existing key
                base64_decode(&key_b64).map_err(|e| {
                    KeyStoreError::InvalidFormat(format!("Invalid master key format: {}", e))
                })
            }
            Err(keyring::Error::NoEntry) => {
                // Generate new master key
                use aes_gcm::aead::rand_core::RngCore;
                let mut key = vec![0u8; AES_KEY_SIZE];
                OsRng.fill_bytes(&mut key);

                // Store in keychain
                let key_b64 = base64_encode(&key);
                entry.set_password(&key_b64).map_err(|e| {
                    KeyStoreError::KeychainError(format!("Failed to store master key: {}", e))
                })?;

                Ok(key)
            }
            Err(e) => Err(KeyStoreError::KeychainError(format!(
                "Failed to access master key: {}",
                e
            ))),
        }
    }

    /// Gets a keyring entry for a provider.
    fn get_entry(provider: &str) -> Result<Entry, KeyStoreError> {
        let username = format!("{}{}", KEYRING_USER_PREFIX, provider);
        Entry::new(KEYRING_SERVICE, &username)
            .map_err(|e| KeyStoreError::KeychainError(format!("Failed to create entry: {}", e)))
    }

    /// Encrypts data using AES-256-GCM.
    fn encrypt(&self, cipher: &Aes256Gcm, plaintext: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        use aes_gcm::aead::rand_core::RngCore;

        // Generate random nonce
        let mut nonce_bytes = [0u8; NONCE_SIZE];
        OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        // Encrypt
        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| KeyStoreError::EncryptionError(format!("Encryption failed: {}", e)))?;

        // Prepend nonce to ciphertext
        let mut result = nonce_bytes.to_vec();
        result.extend(ciphertext);
        Ok(result)
    }

    /// Decrypts data using AES-256-GCM.
    fn decrypt(&self, cipher: &Aes256Gcm, data: &[u8]) -> Result<Vec<u8>, KeyStoreError> {
        if data.len() < NONCE_SIZE {
            return Err(KeyStoreError::InvalidFormat(
                "Data too short for decryption".to_string(),
            ));
        }

        // Extract nonce and ciphertext
        let (nonce_bytes, ciphertext) = data.split_at(NONCE_SIZE);
        let nonce = Nonce::from_slice(nonce_bytes);

        // Decrypt
        cipher
            .decrypt(nonce, ciphertext)
            .map_err(|e| KeyStoreError::EncryptionError(format!("Decryption failed: {}", e)))
    }
}

use base64::{engine::general_purpose::STANDARD, Engine as _};

fn base64_encode(data: &[u8]) -> String {
    STANDARD.encode(data)
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    STANDARD
        .decode(s.trim())
        .map_err(|e| format!("Invalid base64: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_roundtrip() {
        let data = b"Hello, World!";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(data.to_vec(), decoded);
    }

    #[test]
    fn test_base64_empty() {
        let data = b"";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(data.to_vec(), decoded);
    }

    #[test]
    fn test_base64_various_lengths() {
        for len in 0..50 {
            let data: Vec<u8> = (0..len).map(|i| i as u8).collect();
            let encoded = base64_encode(&data);
            let decoded = base64_decode(&encoded).unwrap();
            assert_eq!(data, decoded, "Failed for length {}", len);
        }
    }

    #[test]
    fn test_keystore_without_encryption() {
        let store = KeyStore::new_without_encryption();
        assert!(store.cipher.is_none());
    }

    #[test]
    fn test_invalid_provider() {
        let store = KeyStore::new_without_encryption();

        // Empty provider
        let result = store.save("", "test-key-1234567");
        assert!(matches!(result, Err(KeyStoreError::InvalidProvider(_))));

        // Provider with whitespace
        let result = store.save("my provider", "test-key-1234567");
        assert!(matches!(result, Err(KeyStoreError::InvalidProvider(_))));
    }

    /// Builds a cipher from a fixed key so the AES-GCM round-trip can be tested
    /// without bootstrapping the OS keyring. `encrypt`/`decrypt` take the cipher
    /// as a parameter, so a `new_without_encryption()` store (cipher: None) is a
    /// fine receiver for the method calls.
    fn fixed_cipher() -> Aes256Gcm {
        let key = [0x42u8; AES_KEY_SIZE];
        Aes256Gcm::new_from_slice(&key).expect("32-byte key is valid")
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let store = KeyStore::new_without_encryption();
        let cipher = fixed_cipher();
        let plaintext = b"sk-super-secret-api-key-1234567890";

        let encrypted = store.encrypt(&cipher, plaintext).unwrap();

        // Nonce (12 bytes) is prepended, and GCM appends a 16-byte tag, so the
        // output is strictly longer than the plaintext and never equals it.
        assert!(encrypted.len() > NONCE_SIZE + plaintext.len());
        assert_ne!(encrypted.as_slice(), plaintext.as_slice());

        let decrypted = store.decrypt(&cipher, &encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_encrypt_uses_fresh_nonce_each_time() {
        let store = KeyStore::new_without_encryption();
        let cipher = fixed_cipher();
        let plaintext = b"identical plaintext";

        let a = store.encrypt(&cipher, plaintext).unwrap();
        let b = store.encrypt(&cipher, plaintext).unwrap();

        // A random per-message nonce means two encryptions of the same input
        // must differ (no nonce reuse), yet both decrypt back correctly.
        assert_ne!(a, b);
        assert_eq!(store.decrypt(&cipher, &a).unwrap(), plaintext);
        assert_eq!(store.decrypt(&cipher, &b).unwrap(), plaintext);
    }

    #[test]
    fn test_decrypt_rejects_too_short_buffer() {
        let store = KeyStore::new_without_encryption();
        let cipher = fixed_cipher();

        // Fewer bytes than the nonce: cannot even split off a nonce.
        let result = store.decrypt(&cipher, &[0u8; NONCE_SIZE - 1]);
        assert!(matches!(result, Err(KeyStoreError::InvalidFormat(_))));
    }

    #[test]
    fn test_decrypt_rejects_tampered_ciphertext() {
        let store = KeyStore::new_without_encryption();
        let cipher = fixed_cipher();
        let plaintext = b"authenticated payload";

        let mut encrypted = store.encrypt(&cipher, plaintext).unwrap();
        // Flip the last byte (inside the GCM tag): authentication must fail.
        let last = encrypted.len() - 1;
        encrypted[last] ^= 0xFF;

        let result = store.decrypt(&cipher, &encrypted);
        assert!(matches!(result, Err(KeyStoreError::EncryptionError(_))));
    }

    // Note: Full integration tests require actual keychain access
    // which may not be available in CI environments.
    // Run manually with: cargo test -- --ignored

    #[test]
    #[ignore = "Requires keychain access"]
    fn test_keystore_full_cycle() {
        let store = KeyStore::new().expect("Failed to create keystore");

        // Save
        store.save("TestProvider", "test-api-key-12345").unwrap();

        // Verify exists
        assert!(store.exists("TestProvider"));

        // Get
        let retrieved = store.get("TestProvider").unwrap();
        assert_eq!(retrieved, "test-api-key-12345");

        // Delete
        store.delete("TestProvider").unwrap();

        // Verify deleted
        assert!(!store.exists("TestProvider"));
    }
}
