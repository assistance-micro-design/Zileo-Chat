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

use super::*;
use crate::db::DBClient;
use crate::models::{AuditConfig, ValidationSettings};
use std::sync::Arc;

async fn make_db() -> (Arc<DBClient>, tempfile::TempDir) {
    let temp = crate::test_utils::test_tempdir();
    let path = temp.path().join("audit_test_db");
    let db = Arc::new(DBClient::new(path.to_str().unwrap()).await.expect("db"));
    db.initialize_schema().await.expect("schema");
    (db, temp)
}

fn settings(enabled: bool, retention_days: i32) -> ValidationSettings {
    ValidationSettings {
        audit: AuditConfig {
            enable_logging: enabled,
            retention_days,
        },
        ..Default::default()
    }
}

fn draft(validation_id: &str, decision: AuditDecision, decided_by: DecidedBy) -> AuditEntryDraft {
    AuditEntryDraft {
        validation_id: validation_id.to_string(),
        tool_name: "test_tool".to_string(),
        decision,
        decided_by,
        risk_level: RiskLevel::Medium,
        workflow_id: Some("wf-test".to_string()),
        agent_id: Some("agent-test".to_string()),
        prompt_preview: Some("hello world".to_string()),
        metadata: Some(serde_json::json!({"reason": "ok"})),
    }
}

async fn count_audit(db: &DBClient) -> u64 {
    let rows: Vec<serde_json::Value> = db
        .query_json("SELECT count() AS c FROM validation_audit GROUP ALL")
        .await
        .unwrap_or_default();
    rows.first()
        .and_then(|r| r.get("c"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0)
}

#[tokio::test]
async fn audit_entry_created_on_approve() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-approve", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    assert_eq!(count_audit(&db).await, 1);
}

#[tokio::test]
async fn audit_entry_created_on_reject() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-reject", AuditDecision::Rejected, DecidedBy::User),
    )
    .await;
    assert_eq!(count_audit(&db).await, 1);
}

#[tokio::test]
async fn audit_entry_created_on_timeout() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-to", AuditDecision::Timeout, DecidedBy::Timeout),
    )
    .await;
    assert_eq!(count_audit(&db).await, 1);
}

#[tokio::test]
async fn audit_disabled_when_logging_off() {
    let (db, _t) = make_db().await;
    let s = settings(false, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-off", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    assert_eq!(count_audit(&db).await, 0, "Disabled logging must not write");
}

#[tokio::test]
async fn cleanup_purges_old_entries() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);

    // Insert one fresh row + manually insert one stale row with decided_at in the far past.
    write_audit_entry(
        &db,
        &s,
        draft("v-fresh", AuditDecision::Approved, DecidedBy::User),
    )
    .await;

    // Seed a stale row: CREATE first (decided_at gets default now), then
    // UPDATE with `<datetime>` cast (ERR_SURREAL_007: datetime fields reject
    // ISO strings via CONTENT but accept them via cast).
    let stale_iso = (chrono::Utc::now() - chrono::Duration::days(60)).to_rfc3339();
    let id = uuid::Uuid::new_v4().to_string();
    let content = serde_json::json!({
        "validation_id": "v-stale",
        "tool_name": "tt",
        "decision": "approved",
        "decided_by": "user",
        "risk_level": "low",
        "metadata": "{}",
    });
    db.execute_with_params(
        &format!("CREATE validation_audit:`{}` CONTENT $data RETURN NONE", id),
        vec![("data".to_string(), content)],
    )
    .await
    .expect("seed stale row");
    db.execute_with_params(
        &format!(
            "UPDATE validation_audit:`{}` SET decided_at = <datetime> $iso",
            id
        ),
        vec![("iso".to_string(), serde_json::json!(stale_iso))],
    )
    .await
    .expect("backdate stale row");

    assert_eq!(count_audit(&db).await, 2);
    let deleted = purge_with_retention(&db, 30).await.unwrap();
    assert_eq!(deleted, 1, "Should delete exactly the stale row");
    assert_eq!(count_audit(&db).await, 1);
}

#[tokio::test]
async fn cleanup_respects_retention_zero() {
    // retention_days = 0 -> cleanup is a no-op.
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-keep", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    assert_eq!(count_audit(&db).await, 1);

    let deleted = purge_with_retention(&db, 0).await.unwrap();
    assert_eq!(deleted, 0);
    assert_eq!(count_audit(&db).await, 1, "retention=0 must be a no-op");
}

#[tokio::test]
async fn list_filter_by_decision_only_returns_matching() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-a", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    write_audit_entry(
        &db,
        &s,
        draft("v-b", AuditDecision::Rejected, DecidedBy::User),
    )
    .await;
    write_audit_entry(
        &db,
        &s,
        draft("v-c", AuditDecision::Approved, DecidedBy::User),
    )
    .await;

    // Direct call to underlying SQL path (mirrors what list_validation_audit does).
    // ORDER BY requires the field to be SELECTed (ERR_SURREAL_005).
    let rows: Vec<serde_json::Value> = db
        .query_json_with_params(
            "SELECT meta::id(id) AS id, decision, decided_at FROM validation_audit \
             WHERE decision = $decision ORDER BY decided_at DESC",
            vec![("decision".to_string(), serde_json::json!("approved"))],
        )
        .await
        .expect("query");
    assert_eq!(rows.len(), 2);
}

#[test]
fn csv_escape_quotes_and_commas() {
    assert_eq!(csv_escape("hello"), "hello");
    assert_eq!(csv_escape("a,b"), "\"a,b\"");
    assert_eq!(csv_escape("she said \"hi\""), "\"she said \"\"hi\"\"\"");
    assert_eq!(csv_escape("line\nbreak"), "\"line\nbreak\"");
}

#[tokio::test]
async fn list_filter_by_decided_by_only_returns_matching() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-user", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    write_audit_entry(
        &db,
        &s,
        draft("v-timeout", AuditDecision::Approved, DecidedBy::Timeout),
    )
    .await;
    write_audit_entry(
        &db,
        &s,
        draft("v-user-2", AuditDecision::Rejected, DecidedBy::User),
    )
    .await;

    let params = ListAuditParams {
        filter: AuditFilter {
            decided_by: Some(DecidedBy::User),
            ..Default::default()
        },
        ..Default::default()
    };
    let entries = list_audit_entries(&db, &params).await.expect("list");
    assert_eq!(entries.len(), 2);
    assert!(entries.iter().all(|e| e.decided_by == DecidedBy::User));
}

#[tokio::test]
async fn list_filter_by_since_until_window_excludes_outside_rows() {
    let (db, _t) = make_db().await;
    let s = settings(true, 90);

    // Insert one row whose decided_at falls inside the window, one in the past
    // (before `since`) and one in the future (after `until`).
    write_audit_entry(
        &db,
        &s,
        draft("v-now", AuditDecision::Approved, DecidedBy::User),
    )
    .await;

    let backdate = |id: &str, when: chrono::DateTime<chrono::Utc>| {
        let id = id.to_string();
        let iso = when.to_rfc3339();
        let db = db.clone();
        async move {
            let row_id = uuid::Uuid::new_v4().to_string();
            let content = serde_json::json!({
                "validation_id": id,
                "tool_name": "tt",
                "decision": "approved",
                "decided_by": "user",
                "risk_level": "low",
                "metadata": "{}",
            });
            db.execute_with_params(
                &format!(
                    "CREATE validation_audit:`{}` CONTENT $data RETURN NONE",
                    row_id
                ),
                vec![("data".to_string(), content)],
            )
            .await
            .expect("seed row");
            db.execute_with_params(
                &format!(
                    "UPDATE validation_audit:`{}` SET decided_at = <datetime> $iso",
                    row_id
                ),
                vec![("iso".to_string(), serde_json::json!(iso))],
            )
            .await
            .expect("backdate row");
        }
    };

    let now = chrono::Utc::now();
    backdate("v-old", now - chrono::Duration::days(30)).await;
    backdate("v-future", now + chrono::Duration::days(30)).await;

    let params = ListAuditParams {
        filter: AuditFilter {
            since: Some((now - chrono::Duration::hours(1)).to_rfc3339()),
            until: Some((now + chrono::Duration::hours(1)).to_rfc3339()),
            ..Default::default()
        },
        ..Default::default()
    };
    let entries = list_audit_entries(&db, &params).await.expect("list");
    assert_eq!(
        entries.len(),
        1,
        "since/until window must exclude rows outside it"
    );
    assert_eq!(entries[0].validation_id, "v-now");
}

#[tokio::test]
async fn list_filter_invalid_since_returns_descriptive_error() {
    let (db, _t) = make_db().await;
    let params = ListAuditParams {
        filter: AuditFilter {
            since: Some("not-a-date".to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    let err = list_audit_entries(&db, &params).await.unwrap_err();
    assert!(err.contains("since"), "error must name the offending field");
}

/// Regression: ERR_SERDE_001 — `ValidationAuditEntry` carries
/// `#[serde(rename_all = "camelCase")]` because it is sent to the frontend in
/// camelCase, but rows from the DB come back in snake_case (column names).
/// Without explicit remapping, `serde_json::from_value` fails on every row and
/// the previous `filter_map(.ok())` silently swallowed the error → list page
/// stayed empty even when stats showed entries existed.
#[tokio::test]
async fn list_audit_entries_returns_inserted_row_end_to_end() {
    let (db, _t) = make_db().await;
    let s = settings(true, 30);
    write_audit_entry(
        &db,
        &s,
        draft("v-end2end", AuditDecision::Approved, DecidedBy::User),
    )
    .await;
    assert_eq!(count_audit(&db).await, 1);

    let params = ListAuditParams::default();
    let entries = list_audit_entries(&db, &params)
        .await
        .expect("list_audit_entries must succeed");

    assert_eq!(
        entries.len(),
        1,
        "stats and list must agree: 1 row in DB → 1 entry returned"
    );
    let entry = &entries[0];
    assert_eq!(entry.validation_id, "v-end2end");
    assert_eq!(entry.tool_name, "test_tool");
    assert_eq!(entry.decision, AuditDecision::Approved);
    assert_eq!(entry.decided_by, DecidedBy::User);
    assert_eq!(entry.risk_level, RiskLevel::Medium);
    assert_eq!(entry.workflow_id.as_deref(), Some("wf-test"));
    assert_eq!(entry.agent_id.as_deref(), Some("agent-test"));
    assert_eq!(entry.prompt_preview.as_deref(), Some("hello world"));
    assert_eq!(
        entry.metadata.as_ref().and_then(|m| m.get("reason")),
        Some(&serde_json::json!("ok"))
    );
}
