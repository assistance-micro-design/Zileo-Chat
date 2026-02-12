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
}
