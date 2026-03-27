use super::*;
use crate::models::streaming::SubAgentOperationType;
use crate::models::{RiskThresholdConfig, SelectiveValidationConfig};

/// Helper to create ValidationSettings with custom mode, thresholds, and selective config.
fn make_settings(
    mode: ValidationMode,
    always_confirm_high: bool,
    auto_approve_low: bool,
    selective_config: SelectiveValidationConfig,
) -> ValidationSettings {
    ValidationSettings {
        mode,
        risk_thresholds: RiskThresholdConfig {
            always_confirm_high,
            auto_approve_low,
        },
        selective_config,
        ..Default::default()
    }
}

/// Auto mode skips validation for low/medium risk
#[test]
fn test_should_require_validation_auto_mode_skips() {
    let settings = make_settings(
        ValidationMode::Auto,
        false,
        false,
        SelectiveValidationConfig::default(),
    );

    assert!(!should_require_validation(
        &settings,
        &ValidationType::SubAgent,
        &RiskLevel::Low
    ));
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Medium
    ));
}

/// Auto mode with always_confirm_high validates high and critical risk
#[test]
fn test_should_require_validation_auto_mode_confirms_high() {
    let settings = make_settings(
        ValidationMode::Auto,
        true,
        false,
        SelectiveValidationConfig::default(),
    );

    assert!(should_require_validation(
        &settings,
        &ValidationType::SubAgent,
        &RiskLevel::High
    ));
    assert!(should_require_validation(
        &settings,
        &ValidationType::Mcp,
        &RiskLevel::Critical
    ));
    // Medium risk is still skipped in auto mode
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Medium
    ));
}

/// Manual mode validates everything except auto-approved low risk
#[test]
fn test_should_require_validation_manual_mode() {
    let settings = make_settings(
        ValidationMode::Manual,
        false,
        true,
        SelectiveValidationConfig::default(),
    );

    // Low risk is auto-approved
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Low
    ));
    // Medium and high require validation
    assert!(should_require_validation(
        &settings,
        &ValidationType::SubAgent,
        &RiskLevel::Medium
    ));
    assert!(should_require_validation(
        &settings,
        &ValidationType::Mcp,
        &RiskLevel::High
    ));
}

/// Selective mode respects per-type configuration
#[test]
fn test_should_require_validation_selective_mode() {
    let settings = make_settings(
        ValidationMode::Selective,
        false,
        false,
        SelectiveValidationConfig {
            sub_agents: true,
            tools: false,
            mcp: true,
            file_ops: false,
            db_ops: false,
        },
    );

    // sub_agents enabled -> validates
    assert!(should_require_validation(
        &settings,
        &ValidationType::SubAgent,
        &RiskLevel::Medium
    ));
    // tools disabled -> skips
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Medium
    ));
    // mcp enabled -> validates
    assert!(should_require_validation(
        &settings,
        &ValidationType::Mcp,
        &RiskLevel::Medium
    ));
    // file_ops disabled -> skips
    assert!(!should_require_validation(
        &settings,
        &ValidationType::FileOp,
        &RiskLevel::High
    ));
}

/// Selective mode with auto_approve_low skips low risk even for enabled types
#[test]
fn test_should_require_validation_selective_auto_approve_low() {
    let settings = make_settings(
        ValidationMode::Selective,
        false,
        true,
        SelectiveValidationConfig {
            sub_agents: true,
            tools: true,
            mcp: true,
            file_ops: true,
            db_ops: true,
        },
    );

    // Low risk auto-approved even though type is enabled
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Low
    ));
    // Medium risk still validates
    assert!(should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Medium
    ));
}

#[test]
fn test_determine_risk_level() {
    assert_eq!(
        ValidationHelper::determine_risk_level(&SubAgentOperationType::Spawn),
        RiskLevel::Medium
    );
    assert_eq!(
        ValidationHelper::determine_risk_level(&SubAgentOperationType::Delegate),
        RiskLevel::Medium
    );
    assert_eq!(
        ValidationHelper::determine_risk_level(&SubAgentOperationType::ParallelBatch),
        RiskLevel::High
    );
}

#[test]
fn test_spawn_details() {
    let details = ValidationHelper::spawn_details(
        "TestAgent",
        "Analyze this code for bugs",
        &["MemoryTool".to_string(), "TodoTool".to_string()],
        &["serena".to_string()],
    );

    assert_eq!(details["sub_agent_name"], "TestAgent");
    assert!(details["prompt_preview"]
        .as_str()
        .unwrap()
        .contains("Analyze"));
    assert_eq!(details["tools"].as_array().unwrap().len(), 2);
}

#[test]
fn test_spawn_details_long_prompt() {
    let long_prompt = "A".repeat(300);
    let details = ValidationHelper::spawn_details("Agent", &long_prompt, &[], &[]);

    let preview = details["prompt_preview"].as_str().unwrap();
    assert!(preview.ends_with("..."));
    assert!(preview.len() <= 203); // 200 + "..."
}

#[test]
fn test_delegate_details() {
    let details =
        ValidationHelper::delegate_details("db_agent", "Database Agent", "Analyze the schema");

    assert_eq!(details["target_agent_id"], "db_agent");
    assert_eq!(details["target_agent_name"], "Database Agent");
}

#[test]
fn test_parallel_details() {
    let tasks = vec![
        ("agent_1".to_string(), "Task 1".to_string()),
        ("agent_2".to_string(), "Task 2".to_string()),
        ("agent_3".to_string(), "Task 3".to_string()),
    ];
    let details = ValidationHelper::parallel_details(&tasks);

    assert_eq!(details["task_count"], 3);
    assert_eq!(details["tasks"].as_array().unwrap().len(), 3);
}

#[test]
fn test_validation_timeout_default() {
    use crate::tools::constants::sub_agent::VALIDATION_TIMEOUT_SECS;
    assert_eq!(VALIDATION_TIMEOUT_SECS, 60);
}

#[test]
fn test_parallel_details_utf8_prompt() {
    // Regression test for panic at line 420
    let tasks = vec![
        ("agent_1".to_string(), "Rechercher sources fiables sur ACTUALITE pour: Mistral AI nouveautes 2025 actualites recentes lancements produits avec accents francais".to_string()),
    ];
    // This should not panic
    let details = ValidationHelper::parallel_details(&tasks);
    assert_eq!(details["task_count"], 1);
    let task = &details["tasks"].as_array().unwrap()[0];
    let preview = task["prompt_preview"].as_str().unwrap();
    assert!(preview.ends_with("..."));
}

// Tests for validate_trimmed_name

#[test]
fn test_validate_trimmed_name_valid() {
    let result = validate_trimmed_name("My Agent", "agent name", 64);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "My Agent");
}

#[test]
fn test_validate_trimmed_name_trims_whitespace() {
    let result = validate_trimmed_name("  My Agent  ", "agent name", 64);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "My Agent");
}

#[test]
fn test_validate_trimmed_name_empty() {
    let result = validate_trimmed_name("", "agent name", 64);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[test]
fn test_validate_trimmed_name_whitespace_only() {
    let result = validate_trimmed_name("   ", "agent name", 64);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("cannot be empty"));
}

#[test]
fn test_validate_trimmed_name_too_long() {
    let long = "a".repeat(65);
    let result = validate_trimmed_name(&long, "agent name", 64);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeds maximum length"));
}

#[test]
fn test_validate_trimmed_name_exact_max() {
    let exact = "a".repeat(64);
    let result = validate_trimmed_name(&exact, "agent name", 64);
    assert!(result.is_ok());
}

#[test]
fn test_validate_trimmed_name_control_chars() {
    let result = validate_trimmed_name("agent\x00name", "agent name", 64);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("control characters"));
}

#[test]
fn test_validate_trimmed_name_allows_newline() {
    let result = validate_trimmed_name("agent\nname", "agent name", 64);
    assert!(result.is_ok());
}

#[test]
fn test_validate_trimmed_name_utf8() {
    let result = validate_trimmed_name("Mon Agent Francais", "agent name", 64);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "Mon Agent Francais");
}

#[test]
fn test_spawn_details_utf8_prompt() {
    // Test spawn_details with UTF-8 text (must be > 200 chars to trigger truncation)
    let prompt = "Analyser le code pour trouver les problemes de securite. Verifier les entrees utilisateur et les acces a la base de donnees. Ceci est un texte long avec des accents francais pour tester la troncature UTF-8. Nous ajoutons encore plus de texte pour depasser la limite de 200 caracteres.";
    assert!(
        prompt.chars().count() > 200,
        "Test prompt must be > 200 chars"
    );
    let details = ValidationHelper::spawn_details(
        "SecurityAgent",
        prompt,
        &["MemoryTool".to_string()],
        &["serena".to_string()],
    );
    let preview = details["prompt_preview"].as_str().unwrap();
    assert!(preview.ends_with("..."), "Preview should end with ellipsis");
}

#[test]
fn test_is_destructive_file_op() {
    assert!(is_destructive_file_op("write"));
    assert!(is_destructive_file_op("replace"));
    assert!(is_destructive_file_op("delete"));
    assert!(is_destructive_file_op("move"));
    assert!(is_destructive_file_op("rename"));
    assert!(!is_destructive_file_op("list"));
    assert!(!is_destructive_file_op("read"));
    assert!(!is_destructive_file_op("create"));
    assert!(!is_destructive_file_op("search_glob"));
    assert!(!is_destructive_file_op("search_content"));
    assert!(!is_destructive_file_op("unknown"));
}

#[test]
fn test_should_require_validation_selective_file_ops() {
    let selective_with_file_ops = SelectiveValidationConfig {
        sub_agents: false,
        tools: false,
        mcp: false,
        file_ops: true,
        db_ops: false,
    };
    let settings = make_settings(
        ValidationMode::Selective,
        false,
        false,
        selective_with_file_ops,
    );
    assert!(should_require_validation(
        &settings,
        &ValidationType::FileOp,
        &RiskLevel::Medium
    ));
    assert!(!should_require_validation(
        &settings,
        &ValidationType::Tool,
        &RiskLevel::Medium
    ));
}

#[test]
fn test_file_op_details() {
    let extra = serde_json::json!({"destination": "/tmp/backup"});
    let details = ValidationHelper::file_op_details("move", "/home/user/file.txt", &extra);

    assert_eq!(details["operation"], "move");
    assert_eq!(details["path"], "/home/user/file.txt");
    assert_eq!(details["details"]["destination"], "/tmp/backup");
}
