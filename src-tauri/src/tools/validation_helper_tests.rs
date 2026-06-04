use super::*;
use crate::models::streaming::SubAgentOperationType;
use crate::models::{RiskThresholdConfig, SelectiveValidationConfig, TimeoutBehavior};

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

// =====================================================
// =====================================================

/// Helper: create ValidationSettings with explicit timeout / behavior overrides.
fn make_timeout_settings(
    timeout_seconds: i32,
    timeout_behavior: TimeoutBehavior,
) -> ValidationSettings {
    ValidationSettings {
        timeout_seconds,
        timeout_behavior,
        ..Default::default()
    }
}

/// Helper: build a ValidationHelper backed by an isolated in-memory DB.
async fn make_test_helper() -> (ValidationHelper, tempfile::TempDir) {
    let temp = crate::test_utils::test_tempdir();
    let path = temp.path().join("validation_helper_test_db");
    let db = std::sync::Arc::new(
        crate::db::DBClient::new(path.to_str().unwrap())
            .await
            .expect("create test db"),
    );
    db.initialize_schema().await.expect("init schema");
    (ValidationHelper::new(db, None), temp)
}

/// Insert a pending validation_request row directly so we can let it time out.
async fn insert_pending_validation(db: &crate::db::DBClient, validation_id: &str) {
    let create = ValidationRequestCreate::new(
        format!("wf-{}", validation_id),
        ValidationType::Tool,
        "test op".to_string(),
        serde_json::json!({}),
        RiskLevel::Low,
        ValidationStatus::Pending,
    );
    db.create("validation_request", validation_id, create)
        .await
        .expect("seed validation_request");
}

/// Read the current `status` of a validation_request row.
async fn read_validation_status(db: &crate::db::DBClient, validation_id: &str) -> Option<String> {
    let q = format!("SELECT status FROM validation_request:`{}`", validation_id);
    let rows: Vec<serde_json::Value> = db.query_json(&q).await.ok()?;
    rows.first()
        .and_then(|r| r.get("status"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

#[test]
fn test_clamp_timeout_seconds_clamps_into_range() {
    // <= 0 means "value not set / unparseable" -> documented fallback (60s).
    assert_eq!(clamp_timeout_seconds(0), VALIDATION_TIMEOUT_SECS);
    assert_eq!(clamp_timeout_seconds(-100), VALIDATION_TIMEOUT_SECS);
    // Above 0 but below floor -> floor.
    assert_eq!(clamp_timeout_seconds(4), VALIDATION_TIMEOUT_MIN_SECS);
    // Inside range -> identity
    assert_eq!(clamp_timeout_seconds(60), 60);
    assert_eq!(clamp_timeout_seconds(120), 120);
    // Above ceiling -> ceiling
    assert_eq!(clamp_timeout_seconds(601), VALIDATION_TIMEOUT_MAX_SECS);
    assert_eq!(clamp_timeout_seconds(i32::MAX), VALIDATION_TIMEOUT_MAX_SECS);
}

#[tokio::test]
async fn test_timeout_behavior_reject_marks_rejected() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-reject";
    insert_pending_validation(&helper.db, id).await;

    let outcome = helper
        .wait_for_validation(id, Duration::from_millis(50), TimeoutBehavior::Reject)
        .await
        .expect("reject path returns Ok(Rejected)");

    assert_eq!(outcome.decision, WaitDecision::Rejected);
    assert!(outcome.via_timeout);
    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("rejected")
    );
}

#[tokio::test]
async fn test_timeout_behavior_approve_marks_approved() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-approve";
    insert_pending_validation(&helper.db, id).await;

    let outcome = helper
        .wait_for_validation(id, Duration::from_millis(50), TimeoutBehavior::Approve)
        .await
        .expect("approve path returns Ok(Approved)");

    assert_eq!(outcome.decision, WaitDecision::Approved);
    assert!(outcome.via_timeout);
    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("approved")
    );
}

#[tokio::test]
async fn test_timeout_behavior_skip_returns_skipped_without_decision() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-skip";
    insert_pending_validation(&helper.db, id).await;

    let outcome = helper
        .wait_for_validation(id, Duration::from_millis(50), TimeoutBehavior::Skip)
        .await
        .expect("skip path returns Ok(Skipped)");

    assert_eq!(outcome.decision, WaitDecision::Skipped);
    assert!(outcome.via_timeout);
    // Skip must NOT mutate the row -> still pending
    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("pending")
    );
}

#[tokio::test]
async fn test_create_and_wait_applies_user_timeout_seconds() {
    // Persist user settings with a tiny timeout (5s clamp floor) and Skip behavior,
    // then verify that create_and_wait_validation honors them rather than the
    // hardcoded 60s default. We use Skip so the call returns quickly without
    // requiring an external approver.
    let (helper, _tmp) = make_test_helper().await;

    // Persist a ValidationSettings with timeout=5s + Skip via the same UPSERT
    // path used by update_validation_settings (CONTENT JSON).
    let settings = make_timeout_settings(5, TimeoutBehavior::Skip);
    let json = serde_json::to_string(&settings).expect("serialize settings");
    let upsert = format!(
        "UPSERT settings:`settings:validation` CONTENT {{ id: 'settings:validation', config: {} }}",
        json
    );
    helper.db.execute(&upsert).await.expect("seed settings");

    // Verify load_validation_settings actually returns our overrides.
    let loaded = helper.load_validation_settings().await;
    assert_eq!(loaded.timeout_seconds, 5);
    assert_eq!(loaded.timeout_behavior, TimeoutBehavior::Skip);

    // create_and_wait_validation should clamp 5 -> 5 (floor) and return Ok(())
    // via the Skip path *without* waiting the full 60s default.
    let id = "vh-cw-skip";
    let started = std::time::Instant::now();
    let res = helper
        .create_and_wait_validation(
            id,
            "wf-cw",
            ValidationType::Tool,
            "user-timeout test",
            serde_json::json!({}),
            RiskLevel::Low,
            false, // attended run
        )
        .await;
    let elapsed = started.elapsed();

    assert!(res.is_ok(), "Skip behavior must surface as Ok(())");
    // Must complete within ~user timeout (5s) + small slack, NOT the 60s default.
    assert!(
        elapsed < Duration::from_secs(15),
        "Should respect user timeout of 5s, took {:?}",
        elapsed
    );
}

#[tokio::test]
async fn test_falls_back_to_default_when_settings_unavailable() {
    // No settings row in DB -> load_validation_settings() returns ValidationSettings::default(),
    // which has timeout_seconds = 60 and timeout_behavior = Reject.
    let (helper, _tmp) = make_test_helper().await;

    let loaded = helper.load_validation_settings().await;
    assert_eq!(loaded.timeout_seconds, 60);
    assert_eq!(loaded.timeout_behavior, TimeoutBehavior::Reject);
    // Sanity: clamp(60) is identity.
    assert_eq!(clamp_timeout_seconds(loaded.timeout_seconds), 60);
}

/// A validation-required operation in a DETACHED run must be refused
/// IMMEDIATELY (no modal, no poll, no row), within milliseconds — the boot
/// catch-up loop must never block. A `validation_request` row is NOT created.
#[tokio::test]
async fn test_create_and_wait_short_circuits_when_detached() {
    let (helper, _tmp) = make_test_helper().await;

    let id = "vh-detached-short-circuit";
    let started = std::time::Instant::now();
    let res = helper
        .create_and_wait_validation(
            id,
            "wf-detached",
            ValidationType::ManagerWrite,
            "detached write that would need validation",
            serde_json::json!({"tool_id": "PromptManagerTool", "operation": "update_prompt"}),
            RiskLevel::High,
            true, // detached: no human in the loop
        )
        .await;
    let elapsed = started.elapsed();

    assert!(
        matches!(res, Err(ToolError::PermissionDenied(_))),
        "a detached validation-required op must be refused, got {res:?}"
    );
    // Must NOT block on the poll/timeout — refusal is immediate.
    assert!(
        elapsed < Duration::from_secs(2),
        "detached refusal must be immediate (no poll), took {elapsed:?}"
    );
    // No validation_request row is created in the detached path.
    assert!(
        read_validation_status(&helper.db, id).await.is_none(),
        "detached court-circuit must NOT create a validation_request row"
    );
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

// =====================================================
// Security-policy refusal audit
// =====================================================

/// Reads the raw audit rows (selected fields) for assertions.
async fn read_audit_rows(db: &crate::db::DBClient) -> Vec<serde_json::Value> {
    db.query_json(
        "SELECT decision, decided_by, risk_level, tool_name, workflow_id, metadata \
         FROM validation_audit",
    )
    .await
    .unwrap_or_default()
}

#[tokio::test]
async fn record_security_refusal_persists_blocked_policy_entry() {
    let (helper, _tmp) = make_test_helper().await;
    helper
        .record_security_refusal(
            "mcp__files__delete",
            Some("files-srv"),
            "not armed for this agent in a detached run",
            false,
            "wf-detached-1",
        )
        .await;

    let rows = read_audit_rows(&helper.db).await;
    assert_eq!(rows.len(), 1, "one refusal must persist one audit row");
    let r = &rows[0];
    assert_eq!(r["decision"], "blocked");
    assert_eq!(r["decided_by"], "policy");
    assert_eq!(r["risk_level"], "high");
    assert_eq!(r["tool_name"], "mcp__files__delete");
    assert_eq!(r["workflow_id"], "wf-detached-1");
}

#[tokio::test]
async fn record_security_refusal_writes_even_when_audit_logging_disabled() {
    let (helper, _tmp) = make_test_helper().await;
    // Seed validation settings with audit logging OFF. A security refusal must
    // be traceable UNCONDITIONALLY: if record_security_refusal honored
    // enable_logging, an attacker-relevant refusal would be silently dropped.
    let disabled = crate::models::ValidationSettings {
        audit: crate::models::AuditConfig {
            enable_logging: false,
            retention_days: 30,
        },
        ..Default::default()
    };
    let config = serde_json::to_string(&disabled).expect("serialize settings");
    let seed = format!(
        "UPSERT settings:`settings:validation` CONTENT {{ id: 'settings:validation', config: {} }}",
        config
    );
    helper
        .db
        .execute(&seed)
        .await
        .expect("seed disabled settings");
    // Sanity: the helper would indeed read logging as disabled.
    assert!(!helper.load_validation_settings().await.audit.enable_logging);

    helper
        .record_security_refusal("mcp__x__y", Some("x"), "unarmed", true, "wf-2")
        .await;

    let rows = read_audit_rows(&helper.db).await;
    assert_eq!(
        rows.len(),
        1,
        "refusal must be logged even when audit logging is disabled (unconditional)"
    );
}

#[tokio::test]
async fn record_security_refusal_dedups_identical_refusals_within_a_run() {
    let (helper, _tmp) = make_test_helper().await;
    // Anti-flood (critique): at most ONE audit row per (server_id, tool_name)
    // per run. A detached run that hammers the same blocked tool across many
    // iterations must not inflate the audit table — the first refusal of a
    // distinct (server, tool) is recorded, identical ones are skipped.
    for _ in 0..5 {
        helper
            .record_security_refusal("mcp__a__b", Some("a"), "unarmed", false, "wf-3")
            .await;
    }

    let rows = read_audit_rows(&helper.db).await;
    assert_eq!(
        rows.len(),
        1,
        "identical (server, tool) refusals in the same run collapse to one row"
    );
}

#[tokio::test]
async fn record_security_refusal_audits_distinct_tools_separately() {
    let (helper, _tmp) = make_test_helper().await;
    // Distinct (server, tool) pairs each keep their own security signal. Writing
    // two rows also proves the fresh per-refusal uuid avoids the UNIQUE
    // collision on validation_id (a constant id would fail the second CREATE).
    helper
        .record_security_refusal("mcp__a__b", Some("a"), "unarmed", false, "wf-3")
        .await;
    helper
        .record_security_refusal("mcp__a__c", Some("a"), "unarmed", false, "wf-3")
        .await;
    helper
        .record_security_refusal("mcp__z__b", Some("z"), "unarmed", false, "wf-3")
        .await;

    let rows = read_audit_rows(&helper.db).await;
    assert_eq!(
        rows.len(),
        3,
        "distinct (server, tool) pairs each produce one row"
    );
}

#[tokio::test]
async fn record_security_refusal_dedup_is_run_scoped_a_new_run_reaudits() {
    // The dedup set lives on the per-run ValidationHelper. A SECOND run (new
    // helper, same DB) re-records the same (server, tool): the signal is not
    // suppressed across runs, only within one run.
    let (helper1, _tmp) = make_test_helper().await;
    let helper2 = ValidationHelper::new(helper1.db.clone(), None);

    helper1
        .record_security_refusal("mcp__a__b", Some("a"), "unarmed", false, "wf-3")
        .await;
    helper2
        .record_security_refusal("mcp__a__b", Some("a"), "unarmed", false, "wf-3")
        .await;

    let rows = read_audit_rows(&helper1.db).await;
    assert_eq!(
        rows.len(),
        2,
        "each run re-audits the same blocked tool once"
    );
}

#[tokio::test]
async fn record_security_refusal_metadata_carries_reason_server_and_delegated() {
    let (helper, _tmp) = make_test_helper().await;
    helper
        .record_security_refusal(
            "mcp__srv__tool",
            Some("srv-id"),
            "not armed for this agent in a detached run",
            true,
            "wf-4",
        )
        .await;

    let rows = read_audit_rows(&helper.db).await;
    assert_eq!(rows.len(), 1);
    // metadata is stored as a JSON string.
    let meta_str = rows[0]["metadata"]
        .as_str()
        .expect("metadata is a JSON string");
    let meta: serde_json::Value = serde_json::from_str(meta_str).expect("metadata parses");
    assert_eq!(meta["reason"], "not armed for this agent in a detached run");
    assert_eq!(meta["server_id"], "srv-id");
    assert_eq!(meta["delegated"], true);
}

// =====================================================
// GAP-1 — nominal path: the user decides BEFORE the timeout window.
// Characterization tests locking the happy path (no test covered it before):
// a user-set status must surface as a USER decision (via_timeout = false) and
// the stored row must be left exactly as the user set it.
// =====================================================

#[tokio::test]
async fn test_user_approves_before_timeout_returns_user_decision() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-user-approve-intime";
    insert_pending_validation(&helper.db, id).await;
    // User approves while the request is still pending.
    helper
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'approved'",
            id
        ))
        .await
        .expect("user approve");

    // Generous timeout: the poll must see 'approved' immediately and return it
    // as a USER decision, never reaching the timeout behavior.
    let outcome = helper
        .wait_for_validation(id, Duration::from_secs(30), TimeoutBehavior::Reject)
        .await
        .expect("wait returns Ok");

    assert_eq!(outcome.decision, WaitDecision::Approved);
    assert!(
        !outcome.via_timeout,
        "a user decision must not be flagged via_timeout"
    );
    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("approved")
    );
}

#[tokio::test]
async fn test_user_rejects_before_timeout_returns_user_decision() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-user-reject-intime";
    insert_pending_validation(&helper.db, id).await;
    helper
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'rejected'",
            id
        ))
        .await
        .expect("user reject");

    let outcome = helper
        .wait_for_validation(id, Duration::from_secs(30), TimeoutBehavior::Approve)
        .await
        .expect("wait returns Ok");

    assert_eq!(outcome.decision, WaitDecision::Rejected);
    assert!(
        !outcome.via_timeout,
        "a user decision must not be flagged via_timeout"
    );
    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("rejected")
    );
}

// =====================================================
// GAP-2 — race / fail-open: the timeout behavior must NOT clobber a decision
// the user already recorded in the final poll window. First-writer-wins.
// These reproduce the clobber on the CURRENT code (unconditional UPDATE).
// =====================================================

#[tokio::test]
async fn test_timeout_reject_does_not_clobber_user_approval() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-race-user-approved";
    insert_pending_validation(&helper.db, id).await;
    // The user approved in the last poll window (status already 'approved').
    helper
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'approved'",
            id
        ))
        .await
        .expect("user approve");

    // Timeout fires with Reject behavior. It MUST observe the existing decision
    // and leave it intact rather than overwriting 'approved' -> 'rejected'.
    let outcome = helper
        .apply_timeout_behavior(id, Duration::from_millis(1), &TimeoutBehavior::Reject)
        .await;

    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("approved"),
        "timeout Reject must not clobber the user's approval"
    );
    assert_eq!(
        outcome.decision,
        WaitDecision::Approved,
        "outcome must reflect the surviving user decision"
    );
    assert!(
        !outcome.via_timeout,
        "a surviving user decision must not be audited as a timeout decision"
    );
}

#[tokio::test]
async fn test_timeout_approve_does_not_clobber_user_rejection() {
    let (helper, _tmp) = make_test_helper().await;
    let id = "vh-race-user-rejected";
    insert_pending_validation(&helper.db, id).await;
    // The user rejected in the last poll window (status already 'rejected').
    helper
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'rejected'",
            id
        ))
        .await
        .expect("user reject");

    // Timeout fires with Approve behavior. It MUST NOT flip a security rejection
    // into an approval.
    let outcome = helper
        .apply_timeout_behavior(id, Duration::from_millis(1), &TimeoutBehavior::Approve)
        .await;

    assert_eq!(
        read_validation_status(&helper.db, id).await.as_deref(),
        Some("rejected"),
        "timeout Approve must not clobber the user's rejection"
    );
    assert_eq!(
        outcome.decision,
        WaitDecision::Rejected,
        "outcome must reflect the surviving user decision"
    );
    assert!(
        !outcome.via_timeout,
        "a surviving user decision must not be audited as a timeout decision"
    );
}

// =====================================================
// GAP-2 — user-command side guard. `resolve_validation_if_pending` is the shared
// first-writer-wins primitive used by both apply_timeout_behavior AND the user
// approve/reject commands. This locks the inverse direction (a late user command
// must not clobber a decision the timeout path already recorded).
// =====================================================

#[tokio::test]
async fn test_resolve_if_pending_first_writer_wins() {
    let (helper, _tmp) = make_test_helper().await;

    // A still-pending row -> the resolver wins and the status is applied.
    let id_win = "vh-resolve-pending";
    insert_pending_validation(&helper.db, id_win).await;
    let won = resolve_validation_if_pending(&helper.db, id_win, "approved", None, vec![])
        .await
        .expect("resolve ok");
    assert!(won.won, "resolving a pending row must win");
    assert_eq!(won.effective, "approved");
    assert_eq!(
        read_validation_status(&helper.db, id_win).await.as_deref(),
        Some("approved")
    );

    // An already-resolved row -> a late competing resolve must NOT clobber it.
    // Models a late user approve_validation arriving after a timeout auto-reject.
    let id_late = "vh-resolve-already";
    insert_pending_validation(&helper.db, id_late).await;
    helper
        .db
        .execute(&format!(
            "UPDATE validation_request:`{}` SET status = 'rejected'",
            id_late
        ))
        .await
        .expect("competing path wins first");
    let late = resolve_validation_if_pending(&helper.db, id_late, "approved", None, vec![])
        .await
        .expect("resolve ok");
    assert!(!late.won, "a late resolve on a decided row must not win");
    assert_eq!(
        late.effective, "rejected",
        "effective status must reflect the first writer"
    );
    assert_eq!(
        read_validation_status(&helper.db, id_late).await.as_deref(),
        Some("rejected"),
        "the first writer's decision must survive"
    );
}
