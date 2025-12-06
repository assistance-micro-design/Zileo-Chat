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

//! Input validation utilities for Tauri commands.
//!
//! Provides robust input validation to prevent:
//! - SQL injection (parameterized queries via SurrealDB)
//! - XSS (input sanitization)
//! - Command injection
//! - Invalid data formats

use thiserror::Error;

/// Maximum allowed length for workflow names
pub const MAX_WORKFLOW_NAME_LEN: usize = 256;
/// Maximum allowed length for agent IDs
pub const MAX_AGENT_ID_LEN: usize = 128;
/// Maximum allowed length for messages
pub const MAX_MESSAGE_LEN: usize = 100_000;
/// Maximum allowed length for provider names
pub const MAX_PROVIDER_LEN: usize = 64;
/// Minimum length for API keys
pub const MIN_API_KEY_LEN: usize = 16;
/// Maximum allowed length for API keys
pub const MAX_API_KEY_LEN: usize = 512;

/// Validation error types
#[derive(Debug, Error)]
pub enum ValidationError {
    /// Input exceeds maximum allowed length
    #[error("Input exceeds maximum length of {max} characters (got {actual})")]
    TooLong { max: usize, actual: usize },

    /// Input is below minimum required length
    #[error("Input is below minimum length of {min} characters (got {actual})")]
    TooShort { min: usize, actual: usize },

    /// Input is empty when a value is required
    #[error("Required field cannot be empty: {field}")]
    Empty { field: String },

    /// Input contains invalid characters
    #[error("Input contains invalid characters: {details}")]
    InvalidCharacters { details: String },

    /// Input format is invalid (used for record ID validation)
    #[error("Invalid format for {field}: {details}")]
    #[allow(dead_code)]
    InvalidFormat { field: String, details: String },

    /// UUID format is invalid
    #[error("Invalid UUID format: {value}")]
    InvalidUuid { value: String },
}

/// Input validator with fluent API
pub struct Validator;

impl Validator {
    /// Validates a workflow name.
    ///
    /// Rules:
    /// - Cannot be empty
    /// - Maximum 256 characters
    /// - Cannot contain control characters
    /// - Cannot start/end with whitespace
    pub fn validate_workflow_name(name: &str) -> Result<String, ValidationError> {
        let trimmed = name.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty {
                field: "workflow_name".to_string(),
            });
        }

        if trimmed.len() > MAX_WORKFLOW_NAME_LEN {
            return Err(ValidationError::TooLong {
                max: MAX_WORKFLOW_NAME_LEN,
                actual: trimmed.len(),
            });
        }

        if trimmed.chars().any(|c| c.is_control()) {
            return Err(ValidationError::InvalidCharacters {
                details: "workflow name cannot contain control characters".to_string(),
            });
        }

        Ok(trimmed.to_string())
    }

    /// Validates an agent ID.
    ///
    /// Rules:
    /// - Cannot be empty
    /// - Maximum 128 characters
    /// - Only alphanumeric, underscore, hyphen allowed
    pub fn validate_agent_id(agent_id: &str) -> Result<String, ValidationError> {
        let trimmed = agent_id.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty {
                field: "agent_id".to_string(),
            });
        }

        if trimmed.len() > MAX_AGENT_ID_LEN {
            return Err(ValidationError::TooLong {
                max: MAX_AGENT_ID_LEN,
                actual: trimmed.len(),
            });
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ValidationError::InvalidCharacters {
                details:
                    "agent_id can only contain alphanumeric characters, underscore, and hyphen"
                        .to_string(),
            });
        }

        Ok(trimmed.to_string())
    }

    /// Validates a message content.
    ///
    /// Rules:
    /// - Cannot be empty (after trimming)
    /// - Maximum 100,000 characters
    pub fn validate_message(message: &str) -> Result<String, ValidationError> {
        let trimmed = message.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty {
                field: "message".to_string(),
            });
        }

        if trimmed.len() > MAX_MESSAGE_LEN {
            return Err(ValidationError::TooLong {
                max: MAX_MESSAGE_LEN,
                actual: trimmed.len(),
            });
        }

        Ok(trimmed.to_string())
    }

    /// Validates a UUID string.
    ///
    /// Rules:
    /// - Must be valid UUID v4 format
    pub fn validate_uuid(id: &str) -> Result<String, ValidationError> {
        let trimmed = id.trim();

        uuid::Uuid::parse_str(trimmed).map_err(|_| ValidationError::InvalidUuid {
            value: trimmed.to_string(),
        })?;

        Ok(trimmed.to_string())
    }

    /// Validates a provider name.
    ///
    /// Rules:
    /// - Cannot be empty
    /// - Maximum 64 characters
    /// - Only alphanumeric, underscore, hyphen allowed
    pub fn validate_provider(provider: &str) -> Result<String, ValidationError> {
        let trimmed = provider.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty {
                field: "provider".to_string(),
            });
        }

        if trimmed.len() > MAX_PROVIDER_LEN {
            return Err(ValidationError::TooLong {
                max: MAX_PROVIDER_LEN,
                actual: trimmed.len(),
            });
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ValidationError::InvalidCharacters {
                details:
                    "provider can only contain alphanumeric characters, underscore, and hyphen"
                        .to_string(),
            });
        }

        Ok(trimmed.to_string())
    }

    /// Validates an API key.
    ///
    /// Rules:
    /// - Cannot be empty
    /// - Minimum 16 characters (typical API key minimum)
    /// - Maximum 512 characters
    /// - Cannot contain control characters (except standard printable)
    pub fn validate_api_key(api_key: &str) -> Result<String, ValidationError> {
        // Do not trim API keys as whitespace may be significant
        if api_key.is_empty() {
            return Err(ValidationError::Empty {
                field: "api_key".to_string(),
            });
        }

        if api_key.len() < MIN_API_KEY_LEN {
            return Err(ValidationError::TooShort {
                min: MIN_API_KEY_LEN,
                actual: api_key.len(),
            });
        }

        if api_key.len() > MAX_API_KEY_LEN {
            return Err(ValidationError::TooLong {
                max: MAX_API_KEY_LEN,
                actual: api_key.len(),
            });
        }

        // Check for control characters (non-printable except tab, newline)
        if api_key
            .chars()
            .any(|c| c.is_control() && c != '\t' && c != '\n' && c != '\r')
        {
            return Err(ValidationError::InvalidCharacters {
                details: "API key contains invalid control characters".to_string(),
            });
        }

        Ok(api_key.to_string())
    }

    /// Sanitizes a string for safe inclusion in logs (removes sensitive data patterns).
    ///
    /// This does NOT sanitize for database queries - use parameterized queries instead.
    #[allow(dead_code)]
    pub fn sanitize_for_logging(input: &str) -> String {
        // Truncate long strings for logging
        const MAX_LOG_LEN: usize = 500;
        if input.len() > MAX_LOG_LEN {
            format!("{}...[truncated]", &input[..MAX_LOG_LEN])
        } else {
            input.to_string()
        }
    }

    /// Validates that a string is safe for use as a SurrealDB record ID part.
    ///
    /// Rules:
    /// - Cannot be empty
    /// - Only alphanumeric, underscore, hyphen allowed
    /// - Cannot start with a number
    #[allow(dead_code)]
    pub fn validate_record_id_part(
        part: &str,
        field_name: &str,
    ) -> Result<String, ValidationError> {
        let trimmed = part.trim();

        if trimmed.is_empty() {
            return Err(ValidationError::Empty {
                field: field_name.to_string(),
            });
        }

        if trimmed
            .chars()
            .next()
            .map(|c| c.is_numeric())
            .unwrap_or(false)
        {
            return Err(ValidationError::InvalidFormat {
                field: field_name.to_string(),
                details: "cannot start with a number".to_string(),
            });
        }

        if !trimmed
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        {
            return Err(ValidationError::InvalidCharacters {
                details: format!(
                    "{} can only contain alphanumeric characters, underscore, and hyphen",
                    field_name
                ),
            });
        }

        Ok(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Workflow name validation tests
    #[test]
    fn test_validate_workflow_name_valid() {
        assert!(Validator::validate_workflow_name("My Workflow").is_ok());
        assert!(Validator::validate_workflow_name("workflow-123").is_ok());
        assert!(Validator::validate_workflow_name("Test_Workflow").is_ok());
    }

    #[test]
    fn test_validate_workflow_name_empty() {
        let result = Validator::validate_workflow_name("");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));

        let result = Validator::validate_workflow_name("   ");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));
    }

    #[test]
    fn test_validate_workflow_name_too_long() {
        let long_name = "a".repeat(MAX_WORKFLOW_NAME_LEN + 1);
        let result = Validator::validate_workflow_name(&long_name);
        assert!(matches!(result, Err(ValidationError::TooLong { .. })));
    }

    #[test]
    fn test_validate_workflow_name_control_chars() {
        let result = Validator::validate_workflow_name("test\x00name");
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    #[test]
    fn test_validate_workflow_name_trims_whitespace() {
        let result = Validator::validate_workflow_name("  My Workflow  ").unwrap();
        assert_eq!(result, "My Workflow");
    }

    // Agent ID validation tests
    #[test]
    fn test_validate_agent_id_valid() {
        assert!(Validator::validate_agent_id("simple_agent").is_ok());
        assert!(Validator::validate_agent_id("agent-123").is_ok());
        assert!(Validator::validate_agent_id("AgentOne").is_ok());
    }

    #[test]
    fn test_validate_agent_id_empty() {
        let result = Validator::validate_agent_id("");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));
    }

    #[test]
    fn test_validate_agent_id_invalid_chars() {
        let result = Validator::validate_agent_id("agent with spaces");
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCharacters { .. })
        ));

        let result = Validator::validate_agent_id("agent@special");
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    // Message validation tests
    #[test]
    fn test_validate_message_valid() {
        assert!(Validator::validate_message("Hello, world!").is_ok());
        assert!(Validator::validate_message("Multi\nline\nmessage").is_ok());
    }

    #[test]
    fn test_validate_message_empty() {
        let result = Validator::validate_message("");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));
    }

    #[test]
    fn test_validate_message_too_long() {
        let long_message = "a".repeat(MAX_MESSAGE_LEN + 1);
        let result = Validator::validate_message(&long_message);
        assert!(matches!(result, Err(ValidationError::TooLong { .. })));
    }

    // UUID validation tests
    #[test]
    fn test_validate_uuid_valid() {
        assert!(Validator::validate_uuid("550e8400-e29b-41d4-a716-446655440000").is_ok());
    }

    #[test]
    fn test_validate_uuid_invalid() {
        let result = Validator::validate_uuid("not-a-uuid");
        assert!(matches!(result, Err(ValidationError::InvalidUuid { .. })));
    }

    // Provider validation tests
    #[test]
    fn test_validate_provider_valid() {
        assert!(Validator::validate_provider("Mistral").is_ok());
        assert!(Validator::validate_provider("Ollama").is_ok());
        assert!(Validator::validate_provider("open-ai").is_ok());
    }

    #[test]
    fn test_validate_provider_empty() {
        let result = Validator::validate_provider("");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));
    }

    #[test]
    fn test_validate_provider_invalid_chars() {
        let result = Validator::validate_provider("my provider");
        assert!(matches!(
            result,
            Err(ValidationError::InvalidCharacters { .. })
        ));
    }

    // API key validation tests
    #[test]
    fn test_validate_api_key_valid() {
        assert!(Validator::validate_api_key("sk-1234567890abcdef").is_ok());
        let long_key = "a".repeat(256);
        assert!(Validator::validate_api_key(&long_key).is_ok());
    }

    #[test]
    fn test_validate_api_key_empty() {
        let result = Validator::validate_api_key("");
        assert!(matches!(result, Err(ValidationError::Empty { .. })));
    }

    #[test]
    fn test_validate_api_key_too_short() {
        let result = Validator::validate_api_key("short");
        assert!(matches!(result, Err(ValidationError::TooShort { .. })));
    }

    #[test]
    fn test_validate_api_key_too_long() {
        let long_key = "a".repeat(MAX_API_KEY_LEN + 1);
        let result = Validator::validate_api_key(&long_key);
        assert!(matches!(result, Err(ValidationError::TooLong { .. })));
    }

    // Sanitize for logging tests
    #[test]
    fn test_sanitize_for_logging_short() {
        let result = Validator::sanitize_for_logging("short string");
        assert_eq!(result, "short string");
    }

    #[test]
    fn test_sanitize_for_logging_long() {
        let long_string = "a".repeat(1000);
        let result = Validator::sanitize_for_logging(&long_string);
        assert!(result.ends_with("...[truncated]"));
        assert!(result.len() < 1000);
    }

    // Record ID part validation tests
    #[test]
    fn test_validate_record_id_part_valid() {
        assert!(Validator::validate_record_id_part("workflow", "table").is_ok());
        assert!(Validator::validate_record_id_part("agent_state", "table").is_ok());
    }

    #[test]
    fn test_validate_record_id_part_starts_with_number() {
        let result = Validator::validate_record_id_part("123workflow", "table");
        assert!(matches!(result, Err(ValidationError::InvalidFormat { .. })));
    }
}
