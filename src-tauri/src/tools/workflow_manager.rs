// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! WorkflowManagerTool — list/rename workflows + read final state + folder ops.
//!
//! Read-only for `read_workflow` and `list_workflow_errors`. The other
//! operations mirror what a user can do via the sidebar.

use crate::db::DBClient;
use crate::security::{serialize_for_query, validate_uuid_field, Validator};
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::{debug, info};

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "WorkflowManagerTool".to_string(),
    name: "WorkflowManager".to_string(),
    summary: "List/rename workflows, organize folders, read final state and errors".to_string(),
    description: ToolDescriptionBuilder::new(
        "Manages workflows: list, rename, folders, plus reading the final state of a workflow.",
    )
    .use_when(&[
        "You need to know which workflows exist or organize them into folders",
        "You want to read the demand + final report of a finished workflow",
        "You want to inspect the errors of a workflow to learn from them",
    ])
    .do_not_use(&[
        "Starting or cancelling a workflow (use DelegateTaskTool / SpawnAgentTool)",
        "Deleting a workflow or folder (not exposed — the user does it)",
    ])
    .operations(&[
        (
            "list_workflows",
            "List workflows; optional `folder_id` and `limit`",
        ),
        (
            "rename_workflow",
            "Rename by `workflow_id`, requires `new_name`",
        ),
        ("list_workflow_folders", "List all folders"),
        (
            "create_workflow_folder",
            "Create with `name` (+ optional `color`); rejects duplicates",
        ),
        (
            "move_workflow_to_folder",
            "Move workflow to folder. Pass `folder_id: null` (or omit) to uncategorize.",
        ),
        (
            "read_workflow",
            "Read workflow metadata + first-user message (demand) + last-assistant message (report) + status + target_agent_id + folder (folder_id + resolved folder_name, null when uncategorized) + completed_at + cumulative tokens/cost. Pass `include_messages: true` to also return the last `messages_limit` user/assistant turns (default 20, max 50, chronological ASC) so you can inspect intermediate exchanges. Useful in Kanban analyze mode to grade a worker's report.",
        ),
        (
            "list_workflow_errors",
            "Up to 50 failed tool executions ordered by sequence ASC. Each entry carries tool_name, server_name, error_message, iteration, agent_id, duration_ms — enough to tell orchestrator-level failures apart from sub-agent ones.",
        ),
        (
            "list_workflow_sub_agents",
            "Up to 200 sub-agent executions ordered by created_at ASC. Each entry carries sub_agent_id, sub_agent_name, parent_agent_id, status, task_description, duration_ms, cost_usd, tokens_input/output, created_at/completed_at, error_message. Lets a Kanban orchestrator identify which permanent agents actually participated in a workflow (successes included, unlike list_workflow_errors which only surfaces failures).",
        ),
    ])
    .examples(&[
        json!({"operation": "list_workflows", "limit": 20}),
        json!({"operation": "read_workflow", "workflow_id": "<uuid>"}),
        json!({"operation": "list_workflow_errors", "workflow_id": "<uuid>"}),
        json!({"operation": "list_workflow_sub_agents", "workflow_id": "<uuid>"}),
        json!({"operation": "create_workflow_folder", "name": "Reports", "color": "#10b981"}),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {"type": "string", "description": "One of: list_workflows, rename_workflow, list_workflow_folders, create_workflow_folder, move_workflow_to_folder, read_workflow, list_workflow_errors, list_workflow_sub_agents."},
            "workflow_id": {"type": "string", "description": "UUID of the target workflow (required by rename/move/read/list_workflow_errors)."},
            "folder_id": {"type": ["string", "null"], "description": "UUID of a workflow folder. For move_workflow_to_folder, pass null (or omit) to uncategorize. For list_workflows, acts as a filter."},
            "limit": {"type": "integer", "minimum": 1, "maximum": 200, "description": "Cap for list_workflows (default 50)."},
            "new_name": {"type": "string", "description": "New workflow name (rename_workflow)."},
            "name": {"type": "string", "description": "Folder name (create_workflow_folder)."},
            "color": {"type": "string", "description": "Folder color as #rrggbb (create_workflow_folder, optional)."},
            "include_messages": {"type": "boolean", "description": "read_workflow only: when true, also returns the last `messages_limit` user/assistant turns chronologically. Useful in Kanban analyze mode to inspect intermediate exchanges."},
            "messages_limit": {"type": "integer", "minimum": 1, "maximum": 50, "description": "read_workflow only: cap for the `messages` array when include_messages=true (default 20)."},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
}
});

const MAX_ERRORS_PER_LIST: usize = 50;
const MAX_SUB_AGENTS_PER_LIST: usize = 200;
const DEFAULT_LIST_LIMIT: i64 = 50;
const DEFAULT_MESSAGES_LIMIT: i64 = 20;
const MAX_MESSAGES_LIMIT: i64 = 50;

pub struct WorkflowManagerTool {
    db: Arc<DBClient>,
}

impl WorkflowManagerTool {
    pub fn new(db: Arc<DBClient>) -> Self {
        Self { db }
    }

    async fn list_workflows(
        &self,
        folder_id: Option<&str>,
        limit: Option<i64>,
    ) -> ToolResult<Value> {
        let limit = limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, 200);
        // Always exclude `hidden_from_list` workflows (the confined per-card
        // Kanban review chats): an agent holding this tool in analyze mode must
        // never be able to enumerate the chats of OTHER cards (Sec-I1).
        // `(hidden_from_list ?? false)` coalesces legacy rows predating the
        // field (ERR_SURREAL_011: DEFAULT does not backfill).
        let (where_clause, params): (String, Vec<(String, Value)>) = match folder_id {
            Some(id) if !id.trim().is_empty() => {
                let v = validate_uuid_field(id, "folder_id").map_err(ToolError::InvalidInput)?;
                (
                    "WHERE folder_id = $fid AND (hidden_from_list ?? false) = false".to_string(),
                    vec![("fid".to_string(), json!(v))],
                )
            }
            _ => (
                "WHERE (hidden_from_list ?? false) = false".to_string(),
                Vec::new(),
            ),
        };
        let q = format!(
            "SELECT meta::id(id) AS id, name, status, agent_id, folder_id, pinned, \
             created_at, updated_at FROM workflow {} ORDER BY updated_at DESC LIMIT {}",
            where_clause, limit
        );
        let rows = self
            .db
            .query_json_with_params(&q, params)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_workflows: {}", e)))?;
        Ok(json!({"success": true, "workflows": rows}))
    }

    async fn rename_workflow(&self, workflow_id: &str, new_name: &str) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;
        // Refuse hidden workflows: an agent must not tamper with the confined
        // per-card Kanban review chat of another card (Sec-I1). A hidden (or
        // missing) row degrades to NotFound; only then do we run the UPDATE.
        let guard_q = format!(
            "SELECT meta::id(id) AS id FROM workflow \
             WHERE meta::id(id) = '{}' AND (hidden_from_list ?? false) = false",
            wid
        );
        let visible = self
            .db
            .query_json(&guard_q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("rename_workflow guard: {}", e)))?;
        if visible.is_empty() {
            return Err(ToolError::NotFound(format!("Workflow {}", wid)));
        }
        let name = Validator::validate_workflow_name(new_name)
            .map_err(|e| ToolError::ValidationFailed(format!("{:?}", e)))?;
        let name_json = serialize_for_query(&name, "name").map_err(ToolError::ExecutionFailed)?;
        let q = format!(
            "UPDATE workflow:`{}` SET name = {}, updated_at = time::now()",
            wid, name_json
        );
        self.db
            .execute(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("rename_workflow: {}", e)))?;
        info!(workflow_id = %wid, "Workflow renamed by agent");
        Ok(json!({"success": true, "workflow_id": wid}))
    }

    async fn list_workflow_folders(&self) -> ToolResult<Value> {
        let q = "SELECT meta::id(id) AS id, name, color, sort_order, created_at, updated_at \
                 FROM workflow_folder ORDER BY sort_order ASC, name ASC";
        let rows = self
            .db
            .query_json(q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_workflow_folders: {}", e)))?;
        Ok(json!({"success": true, "folders": rows}))
    }

    async fn create_workflow_folder(&self, name: &str, color: Option<&str>) -> ToolResult<Value> {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.len() > 64 {
            return Err(ToolError::ValidationFailed(
                "folder name must be 1..=64 chars".to_string(),
            ));
        }
        // Reject duplicates (PAT requested in spec).
        let dup_q = "SELECT meta::id(id) AS id FROM workflow_folder WHERE name = $n";
        let dup = self
            .db
            .query_json_with_params(dup_q, vec![("n".to_string(), json!(trimmed))])
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        if !dup.is_empty() {
            return Err(ToolError::ValidationFailed(format!(
                "A folder named '{}' already exists",
                trimmed
            )));
        }
        let color = color
            .map(|c| c.trim())
            .filter(|c| !c.is_empty())
            .unwrap_or("#3b82f6");
        let id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE workflow_folder:`{}` CONTENT {{
                name: $name, color: $color, sort_order: 0,
                created_at: time::now(), updated_at: time::now()
            }}",
            id
        );
        self.db
            .execute_with_params(
                &q,
                vec![
                    ("name".to_string(), json!(trimmed)),
                    ("color".to_string(), json!(color)),
                ],
            )
            .await
            .map_err(|e| ToolError::DatabaseError(format!("create_workflow_folder: {}", e)))?;
        Ok(json!({"success": true, "folder_id": id, "name": trimmed}))
    }

    async fn move_workflow_to_folder(
        &self,
        workflow_id: &str,
        folder_id: Option<&str>,
    ) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;
        // Refuse hidden workflows (the confined per-card Kanban review chat):
        // moving another card's chat into a folder is cross-card tampering
        // (Sec-I1 / K9). Mirrors rename_workflow's guard.
        let guard_q = format!(
            "SELECT meta::id(id) AS id FROM workflow \
             WHERE meta::id(id) = '{}' AND (hidden_from_list ?? false) = false",
            wid
        );
        let visible = self.db.query_json(&guard_q).await.map_err(|e| {
            ToolError::DatabaseError(format!("move_workflow_to_folder guard: {}", e))
        })?;
        if visible.is_empty() {
            return Err(ToolError::NotFound(format!("Workflow {}", wid)));
        }
        let set_clause = match folder_id {
            Some(fid) if !fid.trim().is_empty() => {
                let v = validate_uuid_field(fid, "folder_id").map_err(ToolError::InvalidInput)?;
                format!("folder_id = '{}'", v)
            }
            _ => "folder_id = NONE".to_string(),
        };
        let q = format!(
            "UPDATE workflow:`{}` SET {}, updated_at = time::now()",
            wid, set_clause
        );
        self.db
            .execute(&q)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("move_workflow_to_folder: {}", e)))?;
        Ok(json!({"success": true, "workflow_id": wid}))
    }

    async fn read_workflow(
        &self,
        workflow_id: &str,
        include_messages: bool,
        messages_limit: Option<i64>,
    ) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;

        // Core workflow row + execution metadata so the analyzer can see how
        // long, how expensive and when the workflow finished.
        //
        // Excludes `hidden_from_list` workflows so the tool can never read the
        // confined per-card Kanban review chat of any card (Sec-I1). A hidden
        // row matches no record and degrades to the NotFound path below. The
        // worker workflow (not hidden) stays readable for analyze grading.
        let wq = format!(
            "SELECT meta::id(id) AS id, name, status, agent_id, folder_id, completed_at, \
             total_tokens_input, total_tokens_output, total_cost_usd \
             FROM workflow WHERE meta::id(id) = '{}' AND (hidden_from_list ?? false) = false",
            wid
        );
        let wrows = self
            .db
            .query_json(&wq)
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let wf = wrows
            .into_iter()
            .next()
            .ok_or_else(|| ToolError::NotFound(format!("Workflow {}", wid)))?;

        // Resolve the workflow's folder so the caller sees where it lives
        // without a second `list_workflow_folders` cross-reference. `folder_id`
        // is a plain UUID string (NONE when uncategorized); resolve its name
        // best-effort (a dangling id degrades to a null name, not an error).
        let folder_id = wf
            .get("folder_id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(String::from);
        let folder_name = match folder_id.as_deref() {
            Some(fid) => {
                let fq = "SELECT name FROM workflow_folder WHERE meta::id(id) = $fid LIMIT 1";
                self.db
                    .query_json_with_params(fq, vec![("fid".to_string(), json!(fid))])
                    .await
                    .ok()
                    .and_then(|rows| rows.into_iter().next())
                    .and_then(|r| r["name"].as_str().map(String::from))
            }
            None => None,
        };

        // First user message = the demand.
        let demand_q = "SELECT content, timestamp FROM message \
                        WHERE workflow_id = $wid AND role = 'user' \
                        ORDER BY timestamp ASC LIMIT 1";
        let demand_rows = self
            .db
            .query_json_with_params(demand_q, vec![("wid".to_string(), json!(wid.clone()))])
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let demand = demand_rows
            .into_iter()
            .next()
            .and_then(|r| r["content"].as_str().map(String::from))
            .unwrap_or_default();

        // Last assistant message = the report.
        let report_q = "SELECT content, timestamp FROM message \
                        WHERE workflow_id = $wid AND role = 'assistant' \
                        ORDER BY timestamp DESC LIMIT 1";
        let report_rows = self
            .db
            .query_json_with_params(report_q, vec![("wid".to_string(), json!(wid.clone()))])
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let report = report_rows
            .into_iter()
            .next()
            .and_then(|r| r["content"].as_str().map(String::from));

        // Optional: return the last N messages (user+assistant interleaved,
        // chronological ASC) so the analyzer can inspect intermediate turns
        // — `demand` and `report` only cover the extremities.
        let messages = if include_messages {
            let limit = messages_limit
                .unwrap_or(DEFAULT_MESSAGES_LIMIT)
                .clamp(1, MAX_MESSAGES_LIMIT);
            // Pull the last `limit` rows by DESC then re-sort ASC client-side
            // to keep both "newest-bounded" and "chronological-display".
            let mq = format!(
                "SELECT role, content, timestamp FROM message \
                 WHERE workflow_id = $wid AND role IN ['user', 'assistant'] \
                 ORDER BY timestamp DESC LIMIT {}",
                limit
            );
            let mut rows = self
                .db
                .query_json_with_params(&mq, vec![("wid".to_string(), json!(wid.clone()))])
                .await
                .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
            // Query returned DESC (newest first); reverse to ASC for display.
            rows.reverse();
            Value::Array(rows)
        } else {
            Value::Null
        };

        Ok(json!({
            "success": true,
            "workflow_id": wid,
            "demand": demand,
            "report": report,
            "status": wf["status"],
            "target_agent_id": wf["agent_id"],
            "folder_id": folder_id,
            "folder_name": folder_name,
            "completed_at": wf["completed_at"],
            "total_tokens_input": wf["total_tokens_input"],
            "total_tokens_output": wf["total_tokens_output"],
            "total_cost_usd": wf["total_cost_usd"],
            "messages": messages,
        }))
    }

    async fn list_workflow_sub_agents(&self, workflow_id: &str) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;
        let q = format!(
            "SELECT meta::id(id) AS id, sub_agent_id, sub_agent_name, parent_agent_id, \
             task_description, status, duration_ms, cost_usd, tokens_input, tokens_output, \
             cached_tokens, cache_write_tokens, thinking_tokens, \
             error_message, created_at, completed_at \
             FROM sub_agent_execution WHERE workflow_id = $wid \
             ORDER BY created_at ASC LIMIT {}",
            MAX_SUB_AGENTS_PER_LIST
        );
        let rows = self
            .db
            .query_json_with_params(&q, vec![("wid".to_string(), json!(wid))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_workflow_sub_agents: {}", e)))?;
        Ok(json!({"success": true, "sub_agents": rows}))
    }

    async fn list_workflow_errors(&self, workflow_id: &str) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;
        let q = format!(
            "SELECT meta::id(id) AS id, tool_name, server_name, error_message, \
             iteration, agent_id, duration_ms, sequence, created_at \
             FROM tool_execution WHERE workflow_id = $wid AND success = false \
             ORDER BY sequence ASC LIMIT {}",
            MAX_ERRORS_PER_LIST
        );
        let rows = self
            .db
            .query_json_with_params(&q, vec![("wid".to_string(), json!(wid))])
            .await
            .map_err(|e| ToolError::DatabaseError(format!("list_workflow_errors: {}", e)))?;
        Ok(json!({"success": true, "errors": rows}))
    }
}

#[async_trait]
impl Tool for WorkflowManagerTool {
    fn id(&self) -> &str {
        "WorkflowManagerTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        let op = input["operation"].as_str().unwrap_or("");
        debug!(operation = %op, "WorkflowManagerTool execute");
        match op {
            "list_workflows" => {
                self.list_workflows(input["folder_id"].as_str(), input["limit"].as_i64())
                    .await
            }
            "rename_workflow" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                let n = input["new_name"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("new_name required".to_string()))?;
                self.rename_workflow(wid, n).await
            }
            "list_workflow_folders" => self.list_workflow_folders().await,
            "create_workflow_folder" => {
                let n = input["name"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("name required".to_string()))?;
                self.create_workflow_folder(n, input["color"].as_str())
                    .await
            }
            "move_workflow_to_folder" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                let fid = if input["folder_id"].is_null() {
                    None
                } else {
                    input["folder_id"].as_str()
                };
                self.move_workflow_to_folder(wid, fid).await
            }
            "read_workflow" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                let include_messages = input["include_messages"].as_bool().unwrap_or(false);
                let messages_limit = input["messages_limit"].as_i64();
                self.read_workflow(wid, include_messages, messages_limit)
                    .await
            }
            "list_workflow_errors" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                self.list_workflow_errors(wid).await
            }
            "list_workflow_sub_agents" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                self.list_workflow_sub_agents(wid).await
            }
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                other
            ))),
        }
    }

    fn validate_input(&self, input: &Value) -> ToolResult<()> {
        let op = input["operation"]
            .as_str()
            .ok_or_else(|| ToolError::InvalidInput("operation required".to_string()))?;
        match op {
            "list_workflows"
            | "rename_workflow"
            | "list_workflow_folders"
            | "create_workflow_folder"
            | "move_workflow_to_folder"
            | "read_workflow"
            | "list_workflow_errors"
            | "list_workflow_sub_agents" => Ok(()),
            other => Err(ToolError::InvalidInput(format!(
                "Unknown operation: {}",
                other
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    #[tokio::test]
    async fn test_create_folder_rejects_duplicate() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        tool.create_workflow_folder("Reports", Some("#10b981"))
            .await
            .unwrap();
        let err = tool
            .create_workflow_folder("Reports", None)
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::ValidationFailed(_)));
    }

    #[tokio::test]
    async fn test_list_workflows_empty_returns_ok() {
        // The full read/error tests require seeding many SCHEMAFULL fields;
        // the smoke check below confirms the wrapper assembles a valid query.
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let res = tool.list_workflows(None, Some(10)).await.unwrap();
        assert_eq!(res["success"], true);
        assert!(res["workflows"].is_array());
    }

    #[tokio::test]
    async fn test_list_workflow_errors_empty_returns_ok() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let res = tool.list_workflow_errors(&wid).await.unwrap();
        assert_eq!(res["success"], true);
        assert!(res["errors"].as_array().unwrap().is_empty());
    }

    async fn seed_workflow(db: &Arc<DBClient>, wid: &str, agent_id: &str) {
        let q = format!(
            "CREATE workflow:`{wid}` SET \
                id = '{wid}', name = 'test', agent_id = '{agent_id}', status = 'completed', \
                completed_at = time::now(), \
                total_tokens_input = 100, total_tokens_output = 50, total_cost_usd = 0.0042, \
                total_cached_tokens = 0, total_cache_write_tokens = 0, \
                sub_agent_tokens_input = 0, sub_agent_tokens_output = 0, \
                current_context_tokens = 0, pinned = false, \
                created_at = time::now(), updated_at = time::now()"
        );
        db.execute(&q).await.unwrap();
    }

    async fn seed_message(db: &Arc<DBClient>, wid: &str, role: &str, content: &str) {
        let mid = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE message:`{mid}` SET \
                id = '{mid}', workflow_id = '{wid}', role = '{role}', \
                content = $c, tokens = 0, \
                created_at = time::now(), updated_at = time::now()"
        );
        db.execute_with_params(&q, vec![("c".to_string(), json!(content))])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn read_workflow_returns_metadata_and_extremities() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow(&state.db, &wid, &aid).await;
        seed_message(&state.db, &wid, "user", "the demand").await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        seed_message(&state.db, &wid, "assistant", "the report").await;

        let res = tool.read_workflow(&wid, false, None).await.unwrap();
        assert_eq!(res["success"], true);
        assert_eq!(res["demand"], "the demand");
        assert_eq!(res["report"], "the report");
        assert_eq!(res["target_agent_id"], aid);
        assert_eq!(res["total_tokens_input"], 100);
        assert_eq!(res["total_cost_usd"], 0.0042);
        assert!(res["completed_at"].is_string());
        assert!(res["messages"].is_null());
        // Uncategorized workflow: folder fields degrade to null.
        assert!(res["folder_id"].is_null());
        assert!(res["folder_name"].is_null());
    }

    #[tokio::test]
    async fn read_workflow_resolves_folder() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        // Create a folder, then place a workflow in it.
        let folder = tool
            .create_workflow_folder("Reports", Some("#10b981"))
            .await
            .unwrap();
        let fid = folder["folder_id"].as_str().unwrap().to_string();
        let wid = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow(&state.db, &wid, &aid).await;
        tool.move_workflow_to_folder(&wid, Some(&fid))
            .await
            .unwrap();

        let res = tool.read_workflow(&wid, false, None).await.unwrap();
        // read_workflow now surfaces the folder directly (no second
        // list_workflow_folders cross-reference needed).
        assert_eq!(res["folder_id"], fid);
        assert_eq!(res["folder_name"], "Reports");
    }

    #[tokio::test]
    async fn read_workflow_with_include_messages_returns_chronological_list() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow(&state.db, &wid, &aid).await;
        seed_message(&state.db, &wid, "user", "first user").await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        seed_message(&state.db, &wid, "assistant", "first assistant").await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        seed_message(&state.db, &wid, "user", "second user").await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        seed_message(&state.db, &wid, "assistant", "second assistant").await;

        let res = tool.read_workflow(&wid, true, Some(10)).await.unwrap();
        let msgs = res["messages"].as_array().expect("messages array");
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[0]["content"], "first user");
        assert_eq!(msgs[1]["content"], "first assistant");
        assert_eq!(msgs[2]["content"], "second user");
        assert_eq!(msgs[3]["content"], "second assistant");
    }

    #[tokio::test]
    async fn read_workflow_messages_limit_clamps_to_last_n() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow(&state.db, &wid, &aid).await;
        for i in 0..5 {
            seed_message(&state.db, &wid, "user", &format!("user-{i}")).await;
            // SurrealDB `time::now()` has millisecond resolution at best;
            // 25ms keeps the rows strictly ordered for the ORDER BY check.
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
        let res = tool.read_workflow(&wid, true, Some(2)).await.unwrap();
        let msgs = res["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 2);
        // limit=2 keeps the LAST two chronologically (user-3, user-4).
        assert_eq!(msgs[0]["content"], "user-3");
        assert_eq!(msgs[1]["content"], "user-4");
    }

    async fn seed_sub_agent_execution(
        db: &Arc<DBClient>,
        workflow_id: &str,
        parent_agent_id: &str,
        sub_agent_id: &str,
        sub_agent_name: &str,
        status: &str,
    ) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let q = format!(
            "CREATE sub_agent_execution:`{id}` SET \
                id = '{id}', workflow_id = '{workflow_id}', \
                parent_agent_id = '{parent_agent_id}', \
                sub_agent_id = '{sub_agent_id}', \
                sub_agent_name = $name, \
                task_description = 'seed task', \
                status = '{status}', \
                duration_ms = 1234, tokens_input = 10, tokens_output = 20, \
                cost_usd = 0.001, \
                created_at = time::now(), completed_at = time::now()"
        );
        db.execute_with_params(&q, vec![("name".to_string(), json!(sub_agent_name))])
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn list_workflow_sub_agents_empty_returns_ok() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let res = tool.list_workflow_sub_agents(&wid).await.unwrap();
        assert_eq!(res["success"], true);
        assert!(res["sub_agents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn list_workflow_sub_agents_returns_chronological_list() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let parent = uuid::Uuid::new_v4().to_string();
        let worker_a = uuid::Uuid::new_v4().to_string();
        let worker_b = uuid::Uuid::new_v4().to_string();

        seed_sub_agent_execution(&state.db, &wid, &parent, &worker_a, "Worker A", "completed")
            .await;
        // SurrealDB time::now() resolution is ms; sleep keeps ordering stable.
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        seed_sub_agent_execution(&state.db, &wid, &parent, &worker_b, "Worker B", "error").await;
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        // Same worker invoked twice — both executions must surface.
        seed_sub_agent_execution(&state.db, &wid, &parent, &worker_a, "Worker A", "completed")
            .await;

        let res = tool.list_workflow_sub_agents(&wid).await.unwrap();
        let rows = res["sub_agents"].as_array().unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["sub_agent_name"], "Worker A");
        assert_eq!(rows[0]["status"], "completed");
        assert_eq!(rows[1]["sub_agent_name"], "Worker B");
        assert_eq!(rows[1]["status"], "error");
        assert_eq!(rows[2]["sub_agent_name"], "Worker A");
        assert_eq!(rows[0]["parent_agent_id"], parent);
        assert_eq!(rows[0]["duration_ms"], 1234);
        assert_eq!(rows[0]["tokens_input"], 10);
    }

    #[tokio::test]
    async fn list_workflow_sub_agents_isolates_by_workflow() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid_a = uuid::Uuid::new_v4().to_string();
        let wid_b = uuid::Uuid::new_v4().to_string();
        let parent = uuid::Uuid::new_v4().to_string();
        let worker = uuid::Uuid::new_v4().to_string();

        seed_sub_agent_execution(&state.db, &wid_a, &parent, &worker, "A", "completed").await;
        seed_sub_agent_execution(&state.db, &wid_b, &parent, &worker, "B", "completed").await;

        let res = tool.list_workflow_sub_agents(&wid_a).await.unwrap();
        let rows = res["sub_agents"].as_array().unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["sub_agent_name"], "A");
    }

    #[tokio::test]
    async fn list_workflow_sub_agents_rejects_invalid_uuid() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let err = tool
            .list_workflow_sub_agents("not-a-uuid")
            .await
            .unwrap_err();
        assert!(matches!(err, ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn read_workflow_unknown_returns_not_found() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let wid = uuid::Uuid::new_v4().to_string();
        let err = tool.read_workflow(&wid, false, None).await.unwrap_err();
        assert!(matches!(err, ToolError::NotFound(_)));
    }

    /// Seeds a workflow with an explicit `hidden_from_list` flag. Mirrors
    /// `seed_workflow` but lets the caller mark the row as a confined Kanban
    /// review chat.
    async fn seed_workflow_hidden(db: &Arc<DBClient>, wid: &str, agent_id: &str, hidden: bool) {
        let q = format!(
            "CREATE workflow:`{wid}` SET \
                id = '{wid}', name = 'hidden chat', agent_id = '{agent_id}', status = 'idle', \
                hidden_from_list = {hidden}, \
                total_tokens_input = 0, total_tokens_output = 0, total_cost_usd = 0.0, \
                total_cached_tokens = 0, total_cache_write_tokens = 0, \
                sub_agent_tokens_input = 0, sub_agent_tokens_output = 0, \
                current_context_tokens = 0, pinned = false, \
                created_at = time::now(), updated_at = time::now()"
        );
        db.execute(&q).await.unwrap();
    }

    /// Sec-I1: list_workflows must NEVER surface a `hidden_from_list` workflow
    /// (the per-card Kanban review chat). Otherwise an agent holding this tool
    /// in analyze mode could enumerate the confined chats of OTHER cards.
    #[tokio::test]
    async fn list_workflows_excludes_hidden() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let visible = uuid::Uuid::new_v4().to_string();
        let hidden = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow(&state.db, &visible, &aid).await;
        seed_workflow_hidden(&state.db, &hidden, &aid, true).await;

        let res = tool.list_workflows(None, Some(50)).await.unwrap();
        let rows = res["workflows"].as_array().unwrap();
        let ids: Vec<&str> = rows.iter().filter_map(|r| r["id"].as_str()).collect();
        assert!(
            ids.contains(&visible.as_str()),
            "visible workflow must list"
        );
        assert!(
            !ids.contains(&hidden.as_str()),
            "hidden review-chat workflow must NEVER appear in list_workflows"
        );
    }

    /// Sec-I1: read_workflow must refuse a hidden workflow — reading another
    /// card's confined chat would leak the conversation. The worker workflow
    /// (not hidden) stays readable for analyze-mode grading.
    #[tokio::test]
    async fn read_workflow_rejects_hidden() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let hidden = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow_hidden(&state.db, &hidden, &aid, true).await;

        let err = tool.read_workflow(&hidden, false, None).await.unwrap_err();
        assert!(
            matches!(err, ToolError::NotFound(_)),
            "hidden workflow must read as NotFound, got {err:?}"
        );
    }

    /// K9: move_workflow_to_folder must refuse a hidden workflow — moving
    /// another card's confined chat into a folder is cross-card tampering.
    #[tokio::test]
    async fn move_workflow_to_folder_rejects_hidden() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let hidden = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow_hidden(&state.db, &hidden, &aid, true).await;

        let err = tool
            .move_workflow_to_folder(&hidden, None)
            .await
            .unwrap_err();
        assert!(
            matches!(err, ToolError::NotFound(_)),
            "hidden workflow move must fail with NotFound, got {err:?}"
        );
    }

    /// Sec-I1: rename_workflow must refuse a hidden workflow so an agent cannot
    /// tamper with another card's confined chat.
    #[tokio::test]
    async fn rename_workflow_rejects_hidden() {
        let (state, _g) = setup_test_state().await;
        let tool = WorkflowManagerTool::new(state.db.clone());
        let hidden = uuid::Uuid::new_v4().to_string();
        let aid = uuid::Uuid::new_v4().to_string();
        seed_workflow_hidden(&state.db, &hidden, &aid, true).await;

        let err = tool.rename_workflow(&hidden, "new name").await.unwrap_err();
        assert!(
            matches!(err, ToolError::NotFound(_)),
            "hidden workflow rename must fail with NotFound, got {err:?}"
        );

        // The hidden row must remain untouched.
        let rows = state
            .db
            .query_json(&format!("SELECT name FROM workflow:`{}`", hidden))
            .await
            .unwrap();
        assert_eq!(rows[0]["name"], "hidden chat", "name must not change");
    }
}
