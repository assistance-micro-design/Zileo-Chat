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
use crate::models::mcp::{MCPDeploymentMethod, MCPServerConfig};
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
    };
    assert!(check_mcp_http_warning(&config).is_none());
}
