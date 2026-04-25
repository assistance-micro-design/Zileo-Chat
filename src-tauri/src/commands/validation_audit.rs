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

//! Validation audit log.
//!
//! Backs the new "Settings > Audit Log" page with append-only
//! decision tracking. Honors the user's `AuditConfig` (enable_logging,
//! retention_days) and is resilient to logging failures: a write failure
//! never blocks the underlying validation flow.

use crate::db::DBClient;
use crate::models::{
    AuditBucket, AuditDecision, AuditFilter, AuditStats, DecidedBy, RiskLevel,
    ValidationAuditEntry, ValidationSettings,
};
use crate::AppState;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::sync::Arc;
use tauri::State;
use tracing::{debug, info, instrument, warn};
use uuid::Uuid;

/// Maximum number of "by tool" buckets returned by `get_validation_audit_stats`.
const TOP_TOOLS_LIMIT: usize = 10;

/// Hard upper bound on `list_validation_audit` page size (defense in depth).
const MAX_LIST_LIMIT: u32 = 500;

/// Default page size when the caller omits `limit`.
/// Mirrors the frontend `AUDIT_LOG_PAGE_SIZE` constant in `stores/audit-log.ts`.
const DEFAULT_LIST_LIMIT: u32 = 50;

/// Length cap for `prompt_preview` (UTF-8 char-boundary safe via safe_truncate).
const PROMPT_PREVIEW_MAX_CHARS: usize = 200;

/// Builder for a new audit entry. Construct, then pass to `write_audit_entry`.
#[derive(Debug, Clone)]
pub struct AuditEntryDraft {
    pub validation_id: String,
    pub tool_name: String,
    pub decision: AuditDecision,
    pub decided_by: DecidedBy,
    pub risk_level: RiskLevel,
    pub workflow_id: Option<String>,
    pub agent_id: Option<String>,
    pub prompt_preview: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

/// Persists an audit entry if logging is enabled in `settings`.
///
/// Failure modes are logged and swallowed: validation flow continues even if
/// the audit table write fails. This is intentional — auditing must not be
/// load-bearing for user-facing operations.
#[instrument(skip(db, settings, draft), fields(validation_id = %draft.validation_id))]
pub async fn write_audit_entry(
    db: &DBClient,
    settings: &ValidationSettings,
    draft: AuditEntryDraft,
) {
    if !settings.audit.enable_logging {
        debug!("Audit logging disabled, skipping write");
        return;
    }

    let id = Uuid::new_v4().to_string();
    let metadata_json = match &draft.metadata {
        Some(v) => serde_json::to_string(v).unwrap_or_else(|_| "{}".to_string()),
        None => "{}".to_string(),
    };

    let prompt_preview = draft
        .prompt_preview
        .as_deref()
        .map(|p| crate::tools::utils::safe_truncate(p, PROMPT_PREVIEW_MAX_CHARS, true));

    // Build the CONTENT object as a JSON Value, then bind it. SCHEMAFULL fields
    // are populated explicitly; `id` is set by the CREATE record-id syntax
    // (passing `id` in CONTENT can confuse SurrealDB's SCHEMAFULL validation).
    let mut content = serde_json::json!({
        "validation_id": draft.validation_id,
        "tool_name": draft.tool_name,
        "decision": draft.decision.to_string(),
        "decided_by": draft.decided_by.to_string(),
        "risk_level": draft.risk_level.to_string(),
        "metadata": metadata_json,
    });
    if let Some(wf) = draft.workflow_id {
        content["workflow_id"] = serde_json::json!(wf);
    }
    if let Some(aid) = draft.agent_id {
        content["agent_id"] = serde_json::json!(aid);
    }
    if let Some(pp) = prompt_preview {
        content["prompt_preview"] = serde_json::json!(pp);
    }

    let content = crate::db::sanitize_for_surrealdb(content);

    let query = format!("CREATE validation_audit:`{}` CONTENT $data RETURN NONE", id);
    if let Err(e) = db
        .execute_with_params(&query, vec![("data".to_string(), content)])
        .await
    {
        warn!(error = %e, "Failed to write validation audit entry");
        return;
    }
    debug!(id = %id, "Validation audit entry written");
}

// =====================================================
// Tauri commands
// =====================================================

/// Pagination + filter envelope for `list_validation_audit`.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAuditParams {
    #[serde(default)]
    pub filter: AuditFilter,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

/// Lists audit entries, newest first. Filters and paginates server-side.
#[tauri::command]
#[instrument(name = "list_validation_audit", skip(state))]
pub async fn list_validation_audit(
    params: ListAuditParams,
    state: State<'_, AppState>,
) -> Result<Vec<ValidationAuditEntry>, String> {
    list_audit_entries(&state.db, &params).await
}

/// Build the SQL `WHERE` clause + bind parameters for an [`AuditFilter`].
///
/// Shared between `list_audit_entries` and `export_validation_audit_csv` so
/// the two paths cannot drift in what they consider a valid filter.
fn build_audit_where_clause(
    filter: &AuditFilter,
) -> Result<(String, Vec<(String, serde_json::Value)>), String> {
    let mut clauses: Vec<&'static str> = Vec::new();
    let mut binds: Vec<(String, serde_json::Value)> = Vec::new();

    if let Some(tool) = filter.tool_name.as_ref() {
        if !tool.trim().is_empty() {
            clauses.push("tool_name = $tool_name");
            binds.push(("tool_name".to_string(), serde_json::json!(tool)));
        }
    }
    if let Some(decision) = filter.decision {
        clauses.push("decision = $decision");
        binds.push((
            "decision".to_string(),
            serde_json::json!(decision.to_string()),
        ));
    }
    if let Some(decided_by) = filter.decided_by {
        clauses.push("decided_by = $decided_by");
        binds.push((
            "decided_by".to_string(),
            serde_json::json!(decided_by.to_string()),
        ));
    }
    if let Some(since) = filter.since.as_ref() {
        let parsed = parse_rfc3339(since, "since")?;
        // ERR_SURREAL_007: cast bound ISO string to datetime for comparison.
        clauses.push("decided_at >= <datetime> $since");
        binds.push(("since".to_string(), serde_json::json!(parsed.to_rfc3339())));
    }
    if let Some(until) = filter.until.as_ref() {
        let parsed = parse_rfc3339(until, "until")?;
        clauses.push("decided_at <= <datetime> $until");
        binds.push(("until".to_string(), serde_json::json!(parsed.to_rfc3339())));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };
    Ok((where_clause, binds))
}

/// Backend-only helper for `list_validation_audit`. Extracted so unit tests
/// can exercise the full query → deserialize path without a Tauri `State`.
pub(crate) async fn list_audit_entries(
    db: &DBClient,
    params: &ListAuditParams,
) -> Result<Vec<ValidationAuditEntry>, String> {
    let limit = params
        .limit
        .map(|l| l.clamp(1, MAX_LIST_LIMIT))
        .unwrap_or(DEFAULT_LIST_LIMIT);
    let offset = params.offset.unwrap_or(0);

    let (where_clause, binds) = build_audit_where_clause(&params.filter)?;

    // limit/offset are always clamped numerics → safe to inline.
    let query = format!(
        "SELECT meta::id(id) AS id, validation_id, tool_name, decision, decided_at, \
         decided_by, risk_level, workflow_id, agent_id, prompt_preview, metadata \
         FROM validation_audit{} ORDER BY decided_at DESC LIMIT {} START {}",
        where_clause, limit, offset
    );

    let rows: Vec<serde_json::Value> = db
        .query_json_with_params(&query, binds)
        .await
        .map_err(|e| format!("Failed to list audit entries: {}", e))?;

    let entries = rows
        .into_iter()
        .map(row_to_entry)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(entries)
}

/// Returns aggregate statistics: total, by-decision counts, top tools.
#[tauri::command]
#[instrument(name = "get_validation_audit_stats", skip(state))]
pub async fn get_validation_audit_stats(state: State<'_, AppState>) -> Result<AuditStats, String> {
    // Total
    let total_rows: Vec<serde_json::Value> = state
        .db
        .query_json("SELECT count() AS c FROM validation_audit GROUP ALL")
        .await
        .map_err(|e| format!("Failed to count audit entries: {}", e))?;
    let total = total_rows
        .first()
        .and_then(|r| r.get("c"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    // By decision
    let by_decision_rows: Vec<serde_json::Value> = state
        .db
        .query_json("SELECT decision, count() AS c FROM validation_audit GROUP BY decision")
        .await
        .map_err(|e| format!("Failed to group audit by decision: {}", e))?;
    let by_decision = by_decision_rows
        .into_iter()
        .filter_map(|r| {
            Some(AuditBucket {
                label: r.get("decision")?.as_str()?.to_string(),
                count: r.get("c")?.as_u64()?,
            })
        })
        .collect();

    // By tool (top N)
    let by_tool_rows: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT tool_name, count() AS c FROM validation_audit \
             GROUP BY tool_name ORDER BY c DESC LIMIT {}",
            TOP_TOOLS_LIMIT
        ))
        .await
        .map_err(|e| format!("Failed to group audit by tool: {}", e))?;
    let by_tool = by_tool_rows
        .into_iter()
        .filter_map(|r| {
            Some(AuditBucket {
                label: r.get("tool_name")?.as_str()?.to_string(),
                count: r.get("c")?.as_u64()?,
            })
        })
        .collect();

    Ok(AuditStats {
        total,
        by_decision,
        by_tool,
    })
}

/// Manually purges entries older than `audit.retention_days`.
///
/// Returns the number of rows deleted.
#[tauri::command]
#[instrument(name = "purge_validation_audit_now", skip(state))]
pub async fn purge_validation_audit_now(state: State<'_, AppState>) -> Result<u64, String> {
    let settings = load_validation_settings(&state.db).await;
    purge_with_retention(&state.db, settings.audit.retention_days).await
}

/// Exports the audit log as CSV (RFC 4180 quoting), honoring the active
/// `AuditFilter` from the frontend.
///
/// Caller should pipe the resulting string to a file save dialog on the frontend.
#[tauri::command]
#[instrument(name = "export_validation_audit_csv", skip(state))]
pub async fn export_validation_audit_csv(
    filter: Option<AuditFilter>,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let filter = filter.unwrap_or_default();
    let (where_clause, binds) = build_audit_where_clause(&filter)?;

    let query = format!(
        "SELECT meta::id(id) AS id, validation_id, tool_name, decision, decided_at, \
         decided_by, risk_level, workflow_id, agent_id, prompt_preview, metadata \
         FROM validation_audit{} ORDER BY decided_at DESC",
        where_clause
    );

    let rows: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(&query, binds)
        .await
        .map_err(|e| format!("Failed to load audit log for export: {}", e))?;

    let mut out = String::new();
    out.push_str("id,validation_id,tool_name,decision,decided_at,decided_by,risk_level,workflow_id,agent_id,prompt_preview,metadata\n");
    for row in rows {
        let cells = [
            row.get("id").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("validation_id")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            row.get("tool_name").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("decision").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("decided_at").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("decided_by").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("risk_level").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("workflow_id")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            row.get("agent_id").and_then(|v| v.as_str()).unwrap_or(""),
            row.get("prompt_preview")
                .and_then(|v| v.as_str())
                .unwrap_or(""),
            row.get("metadata").and_then(|v| v.as_str()).unwrap_or(""),
        ];
        for (i, cell) in cells.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&csv_escape(cell));
        }
        out.push('\n');
    }
    Ok(out)
}

// =====================================================
// Background cleanup
// =====================================================

/// Spawn a background task that purges expired audit entries every 24h.
///
/// Lazy cleanup pattern (PAT_TRASH_001) — runs once at startup after a small
/// delay, then on a 24h tick. `retention_days <= 0` is treated as "no cleanup".
pub fn spawn_audit_cleanup_task(db: Arc<DBClient>) -> tokio::task::JoinHandle<()> {
    use std::time::Duration;
    let interval = Duration::from_secs(24 * 60 * 60);
    // Initial delay so cleanup doesn't fight other startup work.
    let initial_delay = Duration::from_secs(5 * 60);

    tokio::spawn(async move {
        tokio::time::sleep(initial_delay).await;
        loop {
            let settings = load_validation_settings(&db).await;
            match purge_with_retention(&db, settings.audit.retention_days).await {
                Ok(n) if n > 0 => info!(deleted = n, "Audit cleanup deleted expired entries"),
                Ok(_) => debug!("Audit cleanup ran, no expired entries"),
                Err(e) => warn!(error = %e, "Audit cleanup failed"),
            }
            tokio::time::sleep(interval).await;
        }
    })
}

// =====================================================
// Internal helpers
// =====================================================

/// Loads `ValidationSettings` from the DB, returning defaults on miss.
async fn load_validation_settings(db: &DBClient) -> ValidationSettings {
    let query = "SELECT config FROM settings:`settings:validation`";
    let rows: Vec<serde_json::Value> = match db.query_json(query).await {
        Ok(r) => r,
        Err(_) => return ValidationSettings::default(),
    };
    rows.first()
        .and_then(|r| r.get("config"))
        .and_then(|c| serde_json::from_value::<ValidationSettings>(c.clone()).ok())
        .unwrap_or_default()
}

/// Deletes audit entries older than `retention_days`. Returns rows deleted.
///
/// `retention_days <= 0` short-circuits with `Ok(0)` (cleanup disabled).
pub(crate) async fn purge_with_retention(
    db: &DBClient,
    retention_days: i32,
) -> Result<u64, String> {
    if retention_days <= 0 {
        debug!("retention_days <= 0, skipping cleanup");
        return Ok(0);
    }
    let threshold = Utc::now() - chrono::Duration::days(retention_days as i64);

    // Count first so we can return how many rows we removed.
    // ERR_SURREAL_007: cast `$threshold` (ISO string) to datetime for comparison.
    let count_rows: Vec<serde_json::Value> = db
        .query_json_with_params(
            "SELECT count() AS c FROM validation_audit WHERE decided_at < <datetime> $threshold GROUP ALL",
            vec![("threshold".to_string(), serde_json::json!(threshold.to_rfc3339()))],
        )
        .await
        .map_err(|e| format!("Failed to count expired audit entries: {}", e))?;
    let to_delete = count_rows
        .first()
        .and_then(|r| r.get("c"))
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    db.execute_with_params(
        "DELETE FROM validation_audit WHERE decided_at < <datetime> $threshold",
        vec![(
            "threshold".to_string(),
            serde_json::json!(threshold.to_rfc3339()),
        )],
    )
    .await
    .map_err(|e| format!("Failed to delete expired audit entries: {}", e))?;

    Ok(to_delete)
}

/// Parse RFC3339 timestamp + return descriptive error.
fn parse_rfc3339(input: &str, field: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(input)
        .map(|d| d.with_timezone(&Utc))
        .map_err(|e| format!("Invalid {} (expected RFC3339): {}", field, e))
}

/// RFC 4180 CSV cell quoting: wraps in quotes if needed, escapes inner quotes.
fn csv_escape(cell: &str) -> String {
    let needs_quotes = cell.contains(',') || cell.contains('"') || cell.contains('\n');
    if !needs_quotes {
        return cell.to_string();
    }
    let escaped = cell.replace('"', "\"\"");
    format!("\"{}\"", escaped)
}

/// Convert a JSON row from `validation_audit` into a typed entry.
///
/// Two adjustments are applied to make the snake_case DB row match
/// `ValidationAuditEntry`'s camelCase serde contract (ERR_SERDE_001):
/// 1. snake_case column keys are renamed to their camelCase counterparts;
/// 2. `metadata` is stored as a JSON string in DB (per ERR_SURREAL_001) — it
///    is parsed back into a real `Value` so the frontend gets structured data.
fn row_to_entry(row: serde_json::Value) -> Result<ValidationAuditEntry, String> {
    let mut row_owned = row;
    let obj = row_owned
        .as_object_mut()
        .ok_or_else(|| "Audit row is not a JSON object".to_string())?;

    // ERR_SERDE_001: ValidationAuditEntry uses #[serde(rename_all = "camelCase")]
    // for the frontend wire format, but SurrealDB returns columns in snake_case.
    // Without this remap, `serde_json::from_value` fails on every row.
    const KEY_RENAMES: &[(&str, &str)] = &[
        ("validation_id", "validationId"),
        ("tool_name", "toolName"),
        ("decided_at", "decidedAt"),
        ("decided_by", "decidedBy"),
        ("risk_level", "riskLevel"),
        ("workflow_id", "workflowId"),
        ("agent_id", "agentId"),
        ("prompt_preview", "promptPreview"),
    ];
    for (snake, camel) in KEY_RENAMES {
        if let Some(value) = obj.remove(*snake) {
            obj.insert((*camel).to_string(), value);
        }
    }

    // Parse the JSON-string metadata back into a real Value.
    let metadata = obj
        .get("metadata")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty() && *s != "{}")
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok());
    obj.insert(
        "metadata".to_string(),
        metadata.unwrap_or(serde_json::Value::Null),
    );

    serde_json::from_value::<ValidationAuditEntry>(row_owned)
        .map_err(|e| format!("Invalid audit row: {}", e))
}

#[cfg(test)]
#[path = "validation_audit_tests.rs"]
mod tests;
