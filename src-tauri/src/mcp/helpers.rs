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

//! MCP parsing helpers for database deserialization
//!
//! These helpers are used to parse SurrealDB JSON values into typed MCP structures.

use crate::mcp::protocol::{
    MCPContent, MCPResourceDefinition, MCPToolCallResponse, MCPToolDefinition,
};
use crate::models::mcp::{MCPAuthMetadata, MCPAuthType, MCPDeploymentMethod, MCPResource, MCPTool};
use std::collections::HashMap;

/// Parses a deployment method string into the enum variant.
///
/// # Arguments
/// * `value` - JSON value that should contain the deployment method as a string
///
/// # Returns
/// * `Some(MCPDeploymentMethod)` if parsing succeeds
/// * `None` if the value is not a string or contains an unknown method
///
/// # Example
/// ```rust,ignore
/// let value = serde_json::json!("docker");
/// assert_eq!(parse_deployment_method(&value), Some(MCPDeploymentMethod::Docker));
/// ```
pub fn parse_deployment_method(value: Option<&serde_json::Value>) -> Option<MCPDeploymentMethod> {
    value.and_then(|v| v.as_str()).and_then(|s| match s {
        "docker" => Some(MCPDeploymentMethod::Docker),
        "npx" => Some(MCPDeploymentMethod::Npx),
        "uvx" => Some(MCPDeploymentMethod::Uvx),
        "http" => Some(MCPDeploymentMethod::Http),
        _ => None,
    })
}

/// Parses an env field from JSON string format to HashMap.
///
/// The env field is stored as a JSON string in SurrealDB to bypass SCHEMAFULL filtering.
/// This helper deserializes it back to a HashMap.
///
/// # Arguments
/// * `value` - Optional JSON value containing the env string
///
/// # Returns
/// * HashMap with key-value pairs, or empty HashMap if parsing fails
///
/// # Example
/// ```rust,ignore
/// let value = serde_json::json!("{\"API_KEY\":\"secret\"}");
/// let env = parse_env_json(Some(&value));
/// assert_eq!(env.get("API_KEY"), Some(&"secret".to_string()));
/// ```
pub fn parse_env_json(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<HashMap<String, String>>(s).ok())
        .unwrap_or_default()
}

/// Parses the persisted auth_type string into [`MCPAuthType`] (v1.2).
///
/// Returns `None` for missing values, `Some(MCPAuthType::None)` for an
/// explicit `"none"`, and the matching variant otherwise. Unknown values
/// degrade gracefully to `None` to avoid breaking the rest of the load.
pub fn parse_auth_type(value: Option<&serde_json::Value>) -> Option<MCPAuthType> {
    value.and_then(|v| v.as_str()).and_then(|s| match s {
        "none" => Some(MCPAuthType::None),
        "bearer" => Some(MCPAuthType::Bearer),
        "apikey" => Some(MCPAuthType::Apikey),
        "basic" => Some(MCPAuthType::Basic),
        _ => None,
    })
}

/// Parses the auth_metadata JSON string into a typed [`MCPAuthMetadata`] (v1.2).
///
/// The DB stores it as a JSON string (ERR_SURREAL_001) so we deserialize twice:
/// first to extract the inner string, then to typed metadata. Returns `None`
/// when every field is `None` (symmetric with [`parse_extra_headers_json`])
/// so callers don't have to distinguish "absent" from "all-fields-None".
pub fn parse_auth_metadata_json(value: Option<&serde_json::Value>) -> Option<MCPAuthMetadata> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<MCPAuthMetadata>(s).ok())
        .filter(|m| m.header_name.is_some() || m.username.is_some())
}

/// Parses the extra_headers JSON string into a HashMap (v1.2).
///
/// Returns `None` (not `Some(empty)`) when absent, so callers can preserve
/// the "no extra headers" state distinctly from "empty record".
pub fn parse_extra_headers_json(
    value: Option<&serde_json::Value>,
) -> Option<HashMap<String, String>> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str::<HashMap<String, String>>(s).ok())
        .filter(|h| !h.is_empty())
}

/// Converts a protocol tool definition to the model type.
///
/// Shared between `MCPServerHandle` and `MCPHttpHandle` to avoid duplication.
pub fn convert_tool_definition(def: MCPToolDefinition) -> MCPTool {
    MCPTool {
        name: def.name,
        description: def.description,
        input_schema: def.input_schema,
    }
}

/// Converts a protocol resource definition to the model type.
///
/// Shared between `MCPServerHandle` and `MCPHttpHandle` to avoid duplication.
pub fn convert_resource_definition(def: MCPResourceDefinition) -> MCPResource {
    MCPResource {
        uri: def.uri,
        name: def.name,
        description: def.description,
        mime_type: def.mime_type,
    }
}

/// Extracts text content from an MCP tool call response.
///
/// Handles all content types:
/// - `Text`: extracts the text directly
/// - `Resource`: extracts the resource text (if available)
/// - `Image`: skipped (cannot be converted to text)
///
/// Shared between `MCPServerHandle` and `MCPHttpHandle` to avoid duplication.
pub fn extract_text_content(response: &MCPToolCallResponse) -> String {
    response
        .content
        .iter()
        .filter_map(|c| match c {
            MCPContent::Text { text } => Some(text.as_str()),
            MCPContent::Resource { resource } => resource.text.as_deref(),
            MCPContent::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_deployment_method_docker() {
        assert_eq!(
            parse_deployment_method(Some(&json!("docker"))),
            Some(MCPDeploymentMethod::Docker)
        );
    }

    #[test]
    fn test_parse_deployment_method_npx() {
        assert_eq!(
            parse_deployment_method(Some(&json!("npx"))),
            Some(MCPDeploymentMethod::Npx)
        );
    }

    #[test]
    fn test_parse_deployment_method_uvx() {
        assert_eq!(
            parse_deployment_method(Some(&json!("uvx"))),
            Some(MCPDeploymentMethod::Uvx)
        );
    }

    #[test]
    fn test_parse_deployment_method_http() {
        assert_eq!(
            parse_deployment_method(Some(&json!("http"))),
            Some(MCPDeploymentMethod::Http)
        );
    }

    #[test]
    fn test_parse_deployment_method_unknown() {
        assert_eq!(parse_deployment_method(Some(&json!("ftp"))), None);
    }

    #[test]
    fn test_parse_deployment_method_none() {
        assert_eq!(parse_deployment_method(None), None);
    }

    #[test]
    fn test_parse_deployment_method_not_string() {
        assert_eq!(parse_deployment_method(Some(&json!(123))), None);
    }

    #[test]
    fn test_parse_env_json_valid() {
        let value = json!("{\"KEY\":\"value\",\"OTHER\":\"test\"}");
        let env = parse_env_json(Some(&value));
        assert_eq!(env.get("KEY"), Some(&"value".to_string()));
        assert_eq!(env.get("OTHER"), Some(&"test".to_string()));
    }

    #[test]
    fn test_parse_env_json_empty_string() {
        let value = json!("{}");
        let env = parse_env_json(Some(&value));
        assert!(env.is_empty());
    }

    #[test]
    fn test_parse_env_json_none() {
        let env = parse_env_json(None);
        assert!(env.is_empty());
    }

    #[test]
    fn test_parse_env_json_invalid() {
        let value = json!("not valid json");
        let env = parse_env_json(Some(&value));
        assert!(env.is_empty());
    }

    #[test]
    fn test_parse_auth_type_known_values() {
        assert_eq!(
            parse_auth_type(Some(&json!("none"))),
            Some(MCPAuthType::None)
        );
        assert_eq!(
            parse_auth_type(Some(&json!("bearer"))),
            Some(MCPAuthType::Bearer)
        );
        assert_eq!(
            parse_auth_type(Some(&json!("apikey"))),
            Some(MCPAuthType::Apikey)
        );
        assert_eq!(
            parse_auth_type(Some(&json!("basic"))),
            Some(MCPAuthType::Basic)
        );
    }

    #[test]
    fn test_parse_auth_type_unknown_or_missing() {
        assert_eq!(parse_auth_type(Some(&json!("oauth"))), None);
        assert_eq!(parse_auth_type(Some(&json!(42))), None);
        assert_eq!(parse_auth_type(None), None);
    }

    #[test]
    fn test_parse_auth_metadata_json_round_trip() {
        let stored = json!("{\"headerName\":\"X-API-Key\",\"username\":\"alice\"}");
        let parsed = parse_auth_metadata_json(Some(&stored)).expect("parsed metadata");
        assert_eq!(parsed.header_name.as_deref(), Some("X-API-Key"));
        assert_eq!(parsed.username.as_deref(), Some("alice"));
    }

    #[test]
    fn test_parse_auth_metadata_json_invalid_or_missing() {
        assert!(parse_auth_metadata_json(None).is_none());
        assert!(parse_auth_metadata_json(Some(&json!("not json"))).is_none());
        // An all-None metadata record is treated as absent so callers don't
        // need to distinguish "no row" from "row with both fields cleared".
        assert!(parse_auth_metadata_json(Some(&json!("{}"))).is_none());
    }

    #[test]
    fn test_parse_auth_metadata_json_partial_fields_kept() {
        // Only one field set is still meaningful (e.g. apikey with header
        // name but no username) and must round-trip.
        let stored = json!("{\"username\":\"alice\"}");
        let parsed = parse_auth_metadata_json(Some(&stored)).expect("partial metadata kept");
        assert!(parsed.header_name.is_none());
        assert_eq!(parsed.username.as_deref(), Some("alice"));
    }

    #[test]
    fn test_parse_extra_headers_json_round_trip() {
        let stored = json!("{\"X-Tenant\":\"42\",\"X-Source\":\"zileo\"}");
        let parsed = parse_extra_headers_json(Some(&stored)).expect("parsed headers");
        assert_eq!(parsed.get("X-Tenant").map(String::as_str), Some("42"));
        assert_eq!(parsed.get("X-Source").map(String::as_str), Some("zileo"));
    }

    #[test]
    fn test_parse_extra_headers_json_empty_or_missing() {
        assert!(parse_extra_headers_json(None).is_none());
        // An empty object is treated as no extra headers (returns None)
        assert!(parse_extra_headers_json(Some(&json!("{}"))).is_none());
        assert!(parse_extra_headers_json(Some(&json!("not json"))).is_none());
    }
}
