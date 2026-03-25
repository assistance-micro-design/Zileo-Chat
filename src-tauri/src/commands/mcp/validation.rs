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
use crate::models::mcp::{MCPDeploymentMethod, MCPServerConfig};
use crate::tools::validation_helper::validate_trimmed_name;

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
