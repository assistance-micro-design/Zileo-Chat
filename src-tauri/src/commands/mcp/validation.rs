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

//! MCP command validation functions
//!
//! Input validation for MCP server configurations, IDs, descriptions,
//! arguments, environment variables, and tool names.

use crate::constants::commands as cmd_const;
use crate::models::custom_provider::check_http_warning;
use crate::models::mcp::{
    MCPAuthMetadata, MCPAuthSecret, MCPAuthType, MCPDeploymentMethod, MCPServerConfig,
};
use crate::tools::validation_helper::validate_trimmed_name;
use std::collections::HashMap;

/// Maximum length of a Bearer token (after trim).
pub(crate) const MAX_BEARER_TOKEN_LEN: usize = 4096;
/// Maximum length of an API key value.
pub(crate) const MAX_API_KEY_VALUE_LEN: usize = 1024;
/// Maximum length of a Basic auth username.
pub(crate) const MAX_BASIC_USERNAME_LEN: usize = 256;
/// Maximum length of a Basic auth password.
pub(crate) const MAX_BASIC_PASSWORD_LEN: usize = 1024;
/// Maximum length of an HTTP header name (auth or extra).
pub(crate) const MAX_HEADER_NAME_LEN: usize = 64;
/// Maximum length of an HTTP header value (extra headers).
pub(crate) const MAX_HEADER_VALUE_LEN: usize = 1024;
/// Maximum number of extra headers per server.
pub(crate) const MAX_EXTRA_HEADERS: usize = 20;

/// Validates an MCP server ID.
///
/// Rules:
/// - Cannot be empty
/// - Maximum 64 characters
/// - Only alphanumeric, underscore, and hyphen allowed
pub fn validate_mcp_server_id(id: &str) -> Result<String, String> {
    let trimmed = id.trim();

    if trimmed.is_empty() {
        return Err("Server ID cannot be empty".to_string());
    }

    if trimmed.len() > cmd_const::MAX_MCP_SERVER_NAME_LEN {
        return Err(format!(
            "Server ID exceeds maximum length of {} characters",
            cmd_const::MAX_MCP_SERVER_NAME_LEN
        ));
    }

    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
    {
        return Err(
            "Server ID can only contain alphanumeric characters, underscore, and hyphen"
                .to_string(),
        );
    }

    Ok(trimmed.to_string())
}

/// Delegates to centralized validate_trimmed_name
fn validate_mcp_server_display_name(name: &str) -> Result<String, String> {
    validate_trimmed_name(name, "Server name", cmd_const::MAX_MCP_SERVER_NAME_LEN)
}

/// Validates an MCP server description.
///
/// Rules:
/// - Can be empty (optional field)
/// - Maximum 1024 characters
/// - No control characters
pub fn validate_mcp_description(description: Option<&str>) -> Result<Option<String>, String> {
    match description {
        None => Ok(None),
        Some(desc) => {
            let trimmed = desc.trim();
            if trimmed.is_empty() {
                return Ok(None);
            }

            if trimmed.len() > cmd_const::MAX_MCP_DESCRIPTION_LEN {
                return Err(format!(
                    "Description exceeds maximum length of {} characters",
                    cmd_const::MAX_MCP_DESCRIPTION_LEN
                ));
            }

            if trimmed.chars().any(|c| c.is_control() && c != '\n') {
                return Err("Description cannot contain control characters".to_string());
            }

            Ok(Some(trimmed.to_string()))
        }
    }
}

/// Validates MCP server command arguments.
///
/// Rules:
/// - Maximum 50 arguments
/// - Each argument maximum 512 characters
/// - No null characters
pub fn validate_mcp_args(args: &[String]) -> Result<Vec<String>, String> {
    if args.len() > cmd_const::MAX_MCP_ARGS_COUNT {
        return Err(format!(
            "Too many arguments (max {})",
            cmd_const::MAX_MCP_ARGS_COUNT
        ));
    }

    let validated: Vec<String> = args
        .iter()
        .enumerate()
        .map(|(i, arg)| {
            if arg.len() > cmd_const::MAX_MCP_ARG_LEN {
                return Err(format!(
                    "Argument {} exceeds maximum length of {} characters",
                    i,
                    cmd_const::MAX_MCP_ARG_LEN
                ));
            }
            // Basic shell metacharacter protection (not comprehensive, defense in depth)
            if arg.contains('\0') {
                return Err(format!("Argument {} contains null character", i));
            }
            Ok(arg.clone())
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(validated)
}

/// Validates MCP server environment variables.
///
/// Rules:
/// - Maximum 50 variables
/// - Names: alphanumeric + underscore, max 128 chars
/// - Values: max 4096 chars, no null characters
pub fn validate_mcp_env(
    env: &std::collections::HashMap<String, String>,
) -> Result<std::collections::HashMap<String, String>, String> {
    if env.len() > cmd_const::MAX_MCP_ENV_COUNT {
        return Err(format!(
            "Too many environment variables (max {})",
            cmd_const::MAX_MCP_ENV_COUNT
        ));
    }

    let validated: std::collections::HashMap<String, String> = env
        .iter()
        .map(|(name, value)| {
            // Validate name
            if name.is_empty() {
                return Err("Environment variable name cannot be empty".to_string());
            }
            if name.len() > cmd_const::MAX_MCP_ENV_NAME_LEN {
                return Err(format!(
                    "Environment variable name '{}' exceeds maximum length of {} characters",
                    name, cmd_const::MAX_MCP_ENV_NAME_LEN
                ));
            }
            if !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                return Err(format!(
                    "Environment variable name '{}' can only contain alphanumeric characters and underscore",
                    name
                ));
            }

            // Validate value
            if value.len() > cmd_const::MAX_MCP_ENV_VALUE_LEN {
                return Err(format!(
                    "Environment variable '{}' value exceeds maximum length of {} characters",
                    name, cmd_const::MAX_MCP_ENV_VALUE_LEN
                ));
            }
            if value.contains('\0') {
                return Err(format!(
                    "Environment variable '{}' value contains null character",
                    name
                ));
            }

            // Shell injection prevention: reject shell metacharacters
            const FORBIDDEN_SHELL_CHARS: &[char] =
                &['|', ';', '`', '$', '(', ')', '<', '>', '&', '\\', '"', '\''];
            if value.chars().any(|c| FORBIDDEN_SHELL_CHARS.contains(&c)) {
                return Err(format!(
                    "Environment variable '{}' value contains forbidden shell characters",
                    name
                ));
            }

            Ok((name.clone(), value.clone()))
        })
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;

    Ok(validated)
}

/// Validates an MCP server configuration.
pub fn validate_mcp_server_config(config: &MCPServerConfig) -> Result<MCPServerConfig, String> {
    let validated_id = validate_mcp_server_id(&config.id)?;
    let validated_name = validate_mcp_server_display_name(&config.name)?;
    let validated_description = validate_mcp_description(config.description.as_deref())?;
    let validated_args = validate_mcp_args(&config.args)?;
    let validated_env = validate_mcp_env(&config.env)?;

    Ok(MCPServerConfig {
        id: validated_id,
        name: validated_name,
        enabled: config.enabled,
        command: config.command.clone(),
        args: validated_args,
        env: validated_env,
        description: validated_description,
        // Auth fields are validated separately by `validate_mcp_auth` (Phase 2)
        // before persistence; here we propagate them unchanged.
        auth_type: config.auth_type,
        auth_metadata: config.auth_metadata.clone(),
        extra_headers: config.extra_headers.clone(),
    })
}

/// Validates a tool name.
pub fn validate_tool_name(name: &str) -> Result<String, String> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        return Err("Tool name cannot be empty".to_string());
    }

    if trimmed.len() > cmd_const::MAX_TOOL_NAME_LEN {
        return Err(format!(
            "Tool name exceeds maximum length of {} characters",
            cmd_const::MAX_TOOL_NAME_LEN
        ));
    }

    // Tool names can contain alphanumeric, underscore, hyphen, and some special chars
    if !trimmed
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == ':' || c == '/')
    {
        return Err(
            "Tool name can only contain alphanumeric characters, underscore, hyphen, colon, and slash"
                .to_string(),
        );
    }

    Ok(trimmed.to_string())
}

/// Checks if an MCP server config uses HTTP on a non-localhost host.
///
/// Only applies to HTTP deployment method where args[0] is the base URL.
/// Returns `Some(warning)` if insecure, `None` otherwise.
pub fn check_mcp_http_warning(config: &MCPServerConfig) -> Option<String> {
    if config.command != MCPDeploymentMethod::Http {
        return None;
    }
    config.args.first().and_then(|url| check_http_warning(url))
}

/// Returns true when `name` is a valid HTTP header name for our purposes
/// (1..=64 chars, only `[A-Za-z0-9_-]`).
fn is_valid_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_HEADER_NAME_LEN
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Validates a Bearer token (v1.2).
///
/// Rules: 1..=`MAX_BEARER_TOKEN_LEN` chars after trim, no `\r` / `\n`.
/// The bearer token is auto-trimmed by callers; this function only checks
/// the post-trim invariants.
pub fn validate_bearer_token(token: &str) -> Result<(), String> {
    if token.is_empty() {
        return Err("Bearer token cannot be empty".to_string());
    }
    if token.len() > MAX_BEARER_TOKEN_LEN {
        return Err(format!(
            "Bearer token exceeds maximum length of {} characters",
            MAX_BEARER_TOKEN_LEN
        ));
    }
    if token.contains('\r') || token.contains('\n') {
        return Err("Bearer token cannot contain newline characters".to_string());
    }
    Ok(())
}

/// Validates an API-Key header value (v1.2).
///
/// Rules: 1..=`MAX_API_KEY_VALUE_LEN` chars, no `\r` / `\n`.
pub fn validate_api_key_value(value: &str) -> Result<(), String> {
    if value.is_empty() {
        return Err("API key value cannot be empty".to_string());
    }
    if value.len() > MAX_API_KEY_VALUE_LEN {
        return Err(format!(
            "API key value exceeds maximum length of {} characters",
            MAX_API_KEY_VALUE_LEN
        ));
    }
    if value.contains('\r') || value.contains('\n') {
        return Err("API key value cannot contain newline characters".to_string());
    }
    Ok(())
}

/// Validates the API-Key header name (v1.2).
///
/// Defaults to `X-API-Key` when caller passes `None`. Rejects names that
/// are not within 1..=`MAX_HEADER_NAME_LEN` chars or contain characters
/// outside `^[A-Za-z0-9_-]+$`.
pub fn validate_apikey_header_name(name: Option<&str>) -> Result<String, String> {
    let header = name
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("X-API-Key");
    if !is_valid_header_name(header) {
        return Err(format!(
            "Invalid API key header name '{}': must match [A-Za-z0-9_-] (1..={} chars)",
            header, MAX_HEADER_NAME_LEN
        ));
    }
    Ok(header.to_string())
}

/// Validates Basic auth credentials (v1.2).
///
/// Rules:
/// - username: 1..=`MAX_BASIC_USERNAME_LEN` chars, no `\r` / `\n`, no `:`
///   (the colon would break the `user:password` colon-separator).
/// - password: 1..=`MAX_BASIC_PASSWORD_LEN` chars, no `\r` / `\n`.
pub fn validate_basic_auth(username: &str, password: &str) -> Result<(), String> {
    if username.is_empty() {
        return Err("Basic auth username cannot be empty".to_string());
    }
    if username.len() > MAX_BASIC_USERNAME_LEN {
        return Err(format!(
            "Basic auth username exceeds maximum length of {} characters",
            MAX_BASIC_USERNAME_LEN
        ));
    }
    if username.contains('\r') || username.contains('\n') {
        return Err("Basic auth username cannot contain newline characters".to_string());
    }
    if username.contains(':') {
        return Err(
            "Basic auth username cannot contain ':' (would break credential encoding)".to_string(),
        );
    }
    if password.is_empty() {
        return Err("Basic auth password cannot be empty".to_string());
    }
    if password.len() > MAX_BASIC_PASSWORD_LEN {
        return Err(format!(
            "Basic auth password exceeds maximum length of {} characters",
            MAX_BASIC_PASSWORD_LEN
        ));
    }
    if password.contains('\r') || password.contains('\n') {
        return Err("Basic auth password cannot contain newline characters".to_string());
    }
    Ok(())
}

/// Validates the `extra_headers` map (v1.2).
///
/// Rules:
/// - At most `MAX_EXTRA_HEADERS` entries.
/// - Each key matches `is_valid_header_name`.
/// - Each value is 1..=`MAX_HEADER_VALUE_LEN` chars and free of `\r` / `\n`.
/// - When `auth_type_set` is true, an `Authorization` extra header is rejected
///   (would conflict with the main auth header).
pub fn validate_extra_headers(
    headers: &HashMap<String, String>,
    auth_type_set: bool,
) -> Result<(), String> {
    if headers.len() > MAX_EXTRA_HEADERS {
        return Err(format!(
            "Too many extra HTTP headers (max {})",
            MAX_EXTRA_HEADERS
        ));
    }
    for (name, value) in headers {
        if !is_valid_header_name(name) {
            return Err(format!(
                "Invalid extra header name '{}': must match [A-Za-z0-9_-] (1..={} chars)",
                name, MAX_HEADER_NAME_LEN
            ));
        }
        if auth_type_set && name.eq_ignore_ascii_case("authorization") {
            return Err(
                "extraHeaders.Authorization conflicts with the main authentication; remove one"
                    .to_string(),
            );
        }
        if value.is_empty() {
            return Err(format!(
                "Extra header '{}' has empty value (drop it instead)",
                name
            ));
        }
        if value.len() > MAX_HEADER_VALUE_LEN {
            return Err(format!(
                "Extra header '{}' value exceeds maximum length of {} characters",
                name, MAX_HEADER_VALUE_LEN
            ));
        }
        if value.contains('\r') || value.contains('\n') {
            return Err(format!(
                "Extra header '{}' value cannot contain newline characters",
                name
            ));
        }
    }
    Ok(())
}

/// Validates an MCP HTTP auth configuration (v1.2).
///
/// Pure function: takes the auth type, optional metadata, and optional
/// secret, and verifies that the combination is internally consistent
/// (lengths, charset, presence rules). Used by the create/update commands
/// before persisting to DB / keychain.
///
/// `secret_required` controls whether a missing secret is an error. On
/// create-with-non-`None` auth the secret IS required. On update without
/// secret rotation, callers pass `false` so they can preserve the previous
/// keychain value.
pub fn validate_mcp_auth(
    auth_type: Option<MCPAuthType>,
    metadata: Option<&MCPAuthMetadata>,
    secret: Option<&MCPAuthSecret>,
    secret_required: bool,
) -> Result<(), String> {
    match auth_type.unwrap_or(MCPAuthType::None) {
        MCPAuthType::None => Ok(()),
        MCPAuthType::Bearer => {
            if let Some(s) = secret.and_then(|s| s.token.as_deref()) {
                validate_bearer_token(s.trim())
            } else if secret_required {
                Err("Bearer auth requires a token (auth_secret.token missing)".to_string())
            } else {
                Ok(())
            }
        }
        MCPAuthType::Apikey => {
            // Header name (defaults to X-API-Key when absent) must be valid.
            validate_apikey_header_name(metadata.and_then(|m| m.header_name.as_deref()))?;
            if let Some(v) = secret.and_then(|s| s.value.as_deref()) {
                validate_api_key_value(v)
            } else if secret_required {
                Err("Apikey auth requires a value (auth_secret.value missing)".to_string())
            } else {
                Ok(())
            }
        }
        MCPAuthType::Basic => {
            let username = metadata.and_then(|m| m.username.as_deref()).unwrap_or("");
            if username.is_empty() {
                return Err("Basic auth requires a username (auth_metadata.username)".to_string());
            }
            if let Some(p) = secret.and_then(|s| s.password.as_deref()) {
                validate_basic_auth(username, p)
            } else if secret_required {
                Err("Basic auth requires a password (auth_secret.password missing)".to_string())
            } else {
                // No password rotation: still validate the username invariants.
                if username.len() > MAX_BASIC_USERNAME_LEN
                    || username.contains('\r')
                    || username.contains('\n')
                    || username.contains(':')
                {
                    return Err(
                        "Basic auth username invalid (length, newlines, or ':' forbidden)"
                            .to_string(),
                    );
                }
                Ok(())
            }
        }
    }
}
