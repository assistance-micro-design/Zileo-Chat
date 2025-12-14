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

use crate::models::mcp::MCPDeploymentMethod;
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
}
