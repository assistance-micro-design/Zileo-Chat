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

use crate::commands::mcp::validation::*;
use crate::constants::commands as cmd_const;
use crate::models::mcp::{
    MCPAuthMetadata, MCPAuthSecret, MCPAuthType, MCPDeploymentMethod, MCPServerConfig,
};
use std::collections::HashMap;

#[test]
fn test_validate_mcp_server_id_valid() {
    assert!(validate_mcp_server_id("serena").is_ok());
    assert!(validate_mcp_server_id("context-7").is_ok());
    assert!(validate_mcp_server_id("my_server").is_ok());
    assert!(validate_mcp_server_id("Server123").is_ok());
}

#[test]
fn test_validate_mcp_server_id_empty() {
    let result = validate_mcp_server_id("");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[test]
fn test_validate_mcp_server_id_too_long() {
    let long_name = "a".repeat(cmd_const::MAX_MCP_SERVER_NAME_LEN + 1);
    let result = validate_mcp_server_id(&long_name);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("maximum length"));
}

#[test]
fn test_validate_mcp_server_id_invalid_chars() {
    let result = validate_mcp_server_id("server with spaces");
    assert!(result.is_err());

    let result = validate_mcp_server_id("server@special");
    assert!(result.is_err());

    let result = validate_mcp_server_id("server.name");
    assert!(result.is_err());
}

#[test]
fn test_validate_mcp_description_valid() {
    assert!(validate_mcp_description(None).unwrap().is_none());
    assert!(validate_mcp_description(Some("")).unwrap().is_none());
    assert_eq!(
        validate_mcp_description(Some("A test server")).unwrap(),
        Some("A test server".to_string())
    );
    assert_eq!(
        validate_mcp_description(Some("Multi\nline")).unwrap(),
        Some("Multi\nline".to_string())
    );
}

#[test]
fn test_validate_mcp_description_too_long() {
    let long_desc = "a".repeat(cmd_const::MAX_MCP_DESCRIPTION_LEN + 1);
    let result = validate_mcp_description(Some(&long_desc));
    assert!(result.is_err());
}

#[test]
fn test_validate_mcp_args_valid() {
    let args = vec!["run".to_string(), "-i".to_string(), "image:tag".to_string()];
    assert!(validate_mcp_args(&args).is_ok());
}

#[test]
fn test_validate_mcp_args_too_many() {
    let args: Vec<String> = (0..cmd_const::MAX_MCP_ARGS_COUNT + 1)
        .map(|i| format!("arg{}", i))
        .collect();
    let result = validate_mcp_args(&args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Too many"));
}

#[test]
fn test_validate_mcp_args_null_char() {
    let args = vec!["arg\0with\0nulls".to_string()];
    let result = validate_mcp_args(&args);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("null character"));
}

#[test]
fn test_validate_mcp_env_valid() {
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "secret".to_string());
    env.insert("DEBUG".to_string(), "true".to_string());
    assert!(validate_mcp_env(&env).is_ok());
}

#[test]
fn test_validate_mcp_env_invalid_name() {
    let mut env = HashMap::new();
    env.insert("INVALID-NAME".to_string(), "value".to_string());
    let result = validate_mcp_env(&env);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("alphanumeric"));
}

#[test]
fn test_validate_mcp_env_null_in_value() {
    let mut env = HashMap::new();
    env.insert("KEY".to_string(), "value\0with\0null".to_string());
    let result = validate_mcp_env(&env);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("null character"));
}

#[test]
fn test_validate_mcp_env_shell_injection() {
    // Shell injection prevention tests
    let test_cases = vec![
        ("PIPE", "value|cmd", "pipe"),
        ("SEMI", "value;cmd", "semicolon"),
        ("BACKTICK", "value`cmd`", "backtick"),
        ("DOLLAR", "$HOME", "dollar sign"),
        ("PAREN_OPEN", "$(cmd)", "parenthesis"),
        ("PAREN_CLOSE", "$(cmd)", "parenthesis"),
        ("LT", "<file", "less than"),
        ("GT", ">file", "greater than"),
        ("AMP", "cmd&", "ampersand"),
        ("BACKSLASH", "path\\file", "backslash"),
        ("DOUBLE_QUOTE", "\"value\"", "double quote"),
        ("SINGLE_QUOTE", "'value'", "single quote"),
    ];

    for (key, value, _desc) in test_cases {
        let mut env = HashMap::new();
        env.insert(key.to_string(), value.to_string());
        let result = validate_mcp_env(&env);
        assert!(
            result.is_err(),
            "Should reject shell metacharacter in value for key {}",
            key
        );
        assert!(result.unwrap_err().contains("forbidden shell characters"));
    }
}

#[test]
fn test_validate_mcp_env_allows_safe_values() {
    // Ensure normal values still work
    let mut env = HashMap::new();
    env.insert("API_KEY".to_string(), "sk-1234567890abcdef".to_string());
    env.insert("DEBUG".to_string(), "true".to_string());
    env.insert("PORT".to_string(), "3000".to_string());
    env.insert("PATH_SAFE".to_string(), "/usr/local/bin".to_string());
    assert!(validate_mcp_env(&env).is_ok());
}

#[test]
fn test_validate_tool_name_valid() {
    assert!(validate_tool_name("find_symbol").is_ok());
    assert!(validate_tool_name("mcp__serena__find_symbol").is_ok());
    assert!(validate_tool_name("tool-name").is_ok());
    assert!(validate_tool_name("namespace:tool").is_ok());
    assert!(validate_tool_name("path/to/tool").is_ok());
}

#[test]
fn test_validate_tool_name_empty() {
    let result = validate_tool_name("");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[test]
fn test_validate_tool_name_invalid_chars() {
    let result = validate_tool_name("tool with spaces");
    assert!(result.is_err());

    let result = validate_tool_name("tool@special");
    assert!(result.is_err());
}

#[test]
fn test_validate_mcp_server_config() {
    let config = MCPServerConfig {
        id: "test_server".to_string(),
        name: "Test Server".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Docker,
        args: vec!["run".to_string(), "-i".to_string()],
        env: HashMap::new(),
        description: Some("A test server".to_string()),
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };

    let result = validate_mcp_server_config(&config);
    assert!(result.is_ok());
}

#[test]
fn test_validate_mcp_server_config_invalid_id() {
    let config = MCPServerConfig {
        id: "invalid id with spaces".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Docker,
        args: vec![],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };

    let result = validate_mcp_server_config(&config);
    assert!(result.is_err());
}

#[tokio::test]
async fn test_mcp_call_log_write_read_cycle() {
    let (state, _db_guard) = crate::test_utils::setup_test_state().await;

    // Write a call log with dynamic params using the MCPCallLogCreate struct
    let log = crate::models::mcp::MCPCallLogCreate {
        id: uuid::Uuid::new_v4().to_string(),
        workflow_id: Some("test-workflow".to_string()),
        server_name: "test-server".to_string(),
        tool_name: "find_symbol".to_string(),
        params: serde_json::json!({"symbol": "MyClass", "include_body": true}),
        result: serde_json::json!([{"name": "MyClass", "line": 42}]),
        success: true,
        duration_ms: 150,
    };

    // Serialize and insert using the same pattern as manager.rs
    let json_data = serde_json::to_value(&log).expect("Failed to serialize");
    let json_data = crate::db::sanitize_for_surrealdb(json_data);
    let query = format!("CREATE mcp_call_log:`{}` CONTENT $data", log.id);
    state
        .db
        .execute_with_params(&query, vec![("data".to_string(), json_data)])
        .await
        .expect("Failed to create mcp_call_log");

    // Read back and verify params/result are preserved as JSON strings
    let logs: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT meta::id(id) AS id, params, result, server_name FROM mcp_call_log WHERE meta::id(id) = '{}'",
            log.id
        ))
        .await
        .expect("Failed to query mcp_call_log");

    assert_eq!(logs.len(), 1, "Should find exactly one log entry");
    let entry = &logs[0];

    // Params should be a JSON string in DB (new format)
    let params_str = entry.get("params").expect("params missing");
    assert!(params_str.is_string(), "params should be stored as string");
    let params: serde_json::Value =
        serde_json::from_str(params_str.as_str().unwrap()).expect("params should be valid JSON");
    assert_eq!(params["symbol"], "MyClass");
    assert_eq!(params["include_body"], true);

    // Result should also be a JSON string
    let result_str = entry.get("result").expect("result missing");
    assert!(result_str.is_string(), "result should be stored as string");
    let result: serde_json::Value =
        serde_json::from_str(result_str.as_str().unwrap()).expect("result should be valid JSON");
    assert_eq!(result[0]["name"], "MyClass");
    assert_eq!(result[0]["line"], 42);
}

#[test]
fn test_deserialize_mcp_call_log_from_string_params() {
    // New format: params and result stored as JSON strings
    let json = serde_json::json!({
        "id": "test-id",
        "workflow_id": "wf-1",
        "server_name": "test-server",
        "tool_name": "find_symbol",
        "params": "{\"symbol\": \"MyClass\"}",
        "result": "[{\"name\": \"MyClass\"}]",
        "success": true,
        "duration_ms": 100,
        "timestamp": "2026-01-01T00:00:00Z"
    });

    let log: crate::models::mcp::MCPCallLog =
        serde_json::from_value(json).expect("Should deserialize MCPCallLog with string params");
    assert_eq!(log.params["symbol"], "MyClass");
    assert_eq!(log.result[0]["name"], "MyClass");
}

#[test]
fn test_deserialize_mcp_call_log_from_legacy_object_params() {
    // Legacy format: params and result stored as objects (backward compat)
    let json = serde_json::json!({
        "id": "test-id",
        "server_name": "test-server",
        "tool_name": "find_symbol",
        "params": {"symbol": "MyClass"},
        "result": [{"name": "MyClass"}],
        "success": true,
        "duration_ms": 100,
        "timestamp": "2026-01-01T00:00:00Z"
    });

    let log: crate::models::mcp::MCPCallLog = serde_json::from_value(json)
        .expect("Should deserialize MCPCallLog with legacy object params");
    assert_eq!(log.params["symbol"], "MyClass");
    assert_eq!(log.result[0]["name"], "MyClass");
}

#[test]
fn test_check_mcp_http_warning_docker_no_warning() {
    let config = MCPServerConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Docker,
        args: vec!["run".to_string(), "-i".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_none());
}

#[test]
fn test_check_mcp_http_warning_npx_no_warning() {
    let config = MCPServerConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Npx,
        args: vec!["-y".to_string(), "@test/mcp".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_none());
}

#[test]
fn test_check_mcp_http_warning_https_no_warning() {
    let config = MCPServerConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Http,
        args: vec!["https://api.example.com/mcp".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_none());
}

#[test]
fn test_check_mcp_http_warning_localhost_no_warning() {
    let config = MCPServerConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Http,
        args: vec!["http://localhost:3000/mcp".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_none());
}

#[test]
fn test_check_mcp_http_warning_remote_http_returns_warning() {
    let config = MCPServerConfig {
        id: "remote-mcp".to_string(),
        name: "Remote MCP".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Http,
        args: vec!["http://api.example.com/mcp".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    let warning = check_mcp_http_warning(&config);
    assert!(warning.is_some());
    let msg = warning.unwrap();
    assert!(msg.contains("HTTP instead of HTTPS"));
    assert!(msg.contains("http://api.example.com/mcp"));
}

#[test]
fn test_check_mcp_http_warning_remote_ip_returns_warning() {
    let config = MCPServerConfig {
        id: "remote-ip".to_string(),
        name: "Remote IP".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Http,
        args: vec!["http://192.168.1.100:8080/mcp".to_string()],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_some());
}

#[test]
fn test_check_mcp_http_warning_empty_args_no_warning() {
    let config = MCPServerConfig {
        id: "test".to_string(),
        name: "Test".to_string(),
        enabled: true,
        command: MCPDeploymentMethod::Http,
        args: vec![],
        env: HashMap::new(),
        description: None,
        auth_type: None,
        auth_metadata: None,
        extra_headers: None,
    };
    assert!(check_mcp_http_warning(&config).is_none());
}

// ============================================================================
// Phase 2 — HTTP authentication validation (v1.2)
// ============================================================================

#[test]
fn test_validate_bearer_token_ok() {
    assert!(validate_bearer_token("sk-1234567890abcdef").is_ok());
    assert!(validate_bearer_token(&"a".repeat(4096)).is_ok());
}

#[test]
fn test_validate_bearer_token_rejects_empty() {
    let err = validate_bearer_token("").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn test_validate_bearer_token_rejects_too_long() {
    let too_long = "a".repeat(4097);
    let err = validate_bearer_token(&too_long).unwrap_err();
    assert!(err.contains("maximum length"));
}

#[test]
fn test_validate_bearer_token_rejects_newlines() {
    assert!(validate_bearer_token("sk-abc\nInjection").is_err());
    assert!(validate_bearer_token("sk-abc\rInjection").is_err());
    assert!(validate_bearer_token("sk-abc\r\nInjection").is_err());
}

#[test]
fn test_validate_apikey_header_name_default() {
    let resolved = validate_apikey_header_name(None).unwrap();
    assert_eq!(resolved, "X-API-Key");
}

#[test]
fn test_validate_apikey_header_name_explicit_ok() {
    let resolved = validate_apikey_header_name(Some("X-Custom-Key")).unwrap();
    assert_eq!(resolved, "X-Custom-Key");
}

#[test]
fn test_validate_apikey_header_name_rejects_invalid_chars() {
    assert!(validate_apikey_header_name(Some("X Custom Key")).is_err());
    assert!(validate_apikey_header_name(Some("X-Custom Key:Bad")).is_err());
    assert!(validate_apikey_header_name(Some("éclair")).is_err());
}

#[test]
fn test_validate_apikey_header_name_rejects_too_long() {
    let too_long = "A".repeat(65);
    assert!(validate_apikey_header_name(Some(&too_long)).is_err());
}

#[test]
fn test_validate_api_key_value_ok() {
    assert!(validate_api_key_value("abc123").is_ok());
}

#[test]
fn test_validate_api_key_value_rejects_newlines() {
    assert!(validate_api_key_value("abc\nbad").is_err());
    assert!(validate_api_key_value("abc\rbad").is_err());
}

#[test]
fn test_validate_basic_auth_ok() {
    assert!(validate_basic_auth("alice", "p@ss").is_ok());
}

#[test]
fn test_validate_basic_auth_rejects_colon_in_username() {
    let err = validate_basic_auth("ali:ce", "p@ss").unwrap_err();
    assert!(err.contains(':'));
}

#[test]
fn test_validate_basic_auth_rejects_empty() {
    assert!(validate_basic_auth("", "p@ss").is_err());
    assert!(validate_basic_auth("alice", "").is_err());
}

#[test]
fn test_validate_basic_auth_rejects_newlines() {
    assert!(validate_basic_auth("alice\n", "p@ss").is_err());
    assert!(validate_basic_auth("alice", "p@\nss").is_err());
}

#[test]
fn test_validate_extra_headers_ok() {
    let mut headers = HashMap::new();
    headers.insert("X-Tenant-ID".to_string(), "42".to_string());
    headers.insert("X-Trace".to_string(), "abc123".to_string());
    assert!(validate_extra_headers(&headers, false).is_ok());
}

#[test]
fn test_validate_extra_headers_rejects_too_many() {
    let mut headers = HashMap::new();
    for i in 0..21 {
        headers.insert(format!("X-Header-{}", i), "v".to_string());
    }
    let err = validate_extra_headers(&headers, false).unwrap_err();
    assert!(err.contains("Too many"));
}

#[test]
fn test_validate_extra_headers_rejects_authorization_when_auth_set() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer xxx".to_string());
    assert!(validate_extra_headers(&headers, true).is_err());

    // Lower / mixed case must also be rejected (HTTP headers are case-insensitive)
    let mut headers2 = HashMap::new();
    headers2.insert("authorization".to_string(), "Bearer xxx".to_string());
    assert!(validate_extra_headers(&headers2, true).is_err());
}

#[test]
fn test_validate_extra_headers_allows_authorization_when_no_auth() {
    let mut headers = HashMap::new();
    headers.insert("Authorization".to_string(), "Bearer xxx".to_string());
    assert!(validate_extra_headers(&headers, false).is_ok());
}

#[test]
fn test_validate_extra_headers_rejects_invalid_name() {
    let mut headers = HashMap::new();
    headers.insert("X Tenant".to_string(), "42".to_string());
    assert!(validate_extra_headers(&headers, false).is_err());
}

#[test]
fn test_validate_extra_headers_rejects_newline_in_value() {
    let mut headers = HashMap::new();
    headers.insert("X-Tenant".to_string(), "42\nInjection".to_string());
    assert!(validate_extra_headers(&headers, false).is_err());
}

#[test]
fn test_validate_extra_headers_rejects_empty_value() {
    let mut headers = HashMap::new();
    headers.insert("X-Tenant".to_string(), "".to_string());
    assert!(validate_extra_headers(&headers, false).is_err());
}

#[test]
fn test_validate_mcp_auth_none_always_ok() {
    assert!(validate_mcp_auth(None, None, None, true).is_ok());
    assert!(validate_mcp_auth(Some(MCPAuthType::None), None, None, true).is_ok());
}

#[test]
fn test_validate_mcp_auth_bearer_requires_secret() {
    let err = validate_mcp_auth(Some(MCPAuthType::Bearer), None, None, true).unwrap_err();
    assert!(err.contains("Bearer"));

    let secret = MCPAuthSecret {
        token: Some("sk-abc".to_string()),
        ..Default::default()
    };
    assert!(validate_mcp_auth(Some(MCPAuthType::Bearer), None, Some(&secret), true).is_ok());
}

#[test]
fn test_validate_mcp_auth_bearer_secret_optional_on_update() {
    // secret_required=false simulates an "update without rotating the secret"
    assert!(validate_mcp_auth(Some(MCPAuthType::Bearer), None, None, false).is_ok());
}

#[test]
fn test_validate_mcp_auth_apikey_default_header_name() {
    let secret = MCPAuthSecret {
        value: Some("abc".to_string()),
        ..Default::default()
    };
    // No header name -> defaults to X-API-Key, accepted
    assert!(validate_mcp_auth(Some(MCPAuthType::Apikey), None, Some(&secret), true).is_ok());
}

#[test]
fn test_validate_mcp_auth_apikey_invalid_header_name() {
    let metadata = MCPAuthMetadata {
        header_name: Some("Bad Header".to_string()),
        username: None,
    };
    let secret = MCPAuthSecret {
        value: Some("abc".to_string()),
        ..Default::default()
    };
    assert!(validate_mcp_auth(
        Some(MCPAuthType::Apikey),
        Some(&metadata),
        Some(&secret),
        true
    )
    .is_err());
}

#[test]
fn test_validate_mcp_auth_basic_requires_username_and_password() {
    // Missing username
    let secret = MCPAuthSecret {
        password: Some("p@ss".to_string()),
        ..Default::default()
    };
    assert!(validate_mcp_auth(Some(MCPAuthType::Basic), None, Some(&secret), true).is_err());

    // Missing password (with secret_required=true)
    let metadata = MCPAuthMetadata {
        username: Some("alice".to_string()),
        header_name: None,
    };
    assert!(validate_mcp_auth(Some(MCPAuthType::Basic), Some(&metadata), None, true).is_err());

    // Both present -> OK
    assert!(validate_mcp_auth(
        Some(MCPAuthType::Basic),
        Some(&metadata),
        Some(&secret),
        true
    )
    .is_ok());
}
