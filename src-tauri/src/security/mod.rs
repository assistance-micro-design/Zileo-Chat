// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

//! Security module for Zileo-Chat-3.
//!
//! Provides:
//! - Input validation utilities for sanitizing and validating user input
//! - Secure API key storage using OS keychain (keyring) + AES-256 encryption
//! - Security-related error types

pub mod keystore;
pub mod validation;

pub use keystore::{KeyStore, KeyStoreError};
pub use validation::Validator;
// ValidationError is used in tests
#[allow(unused_imports)]
pub use validation::ValidationError;
