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

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| ToolDefinition {
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
            "Move workflow to folder (`folder_id` null = uncategorized)",
        ),
        (
            "read_workflow",
            "Read demand + report + status + target_agent_id",
        ),
        (
            "list_workflow_errors",
            "Up to 20 failed tool executions, ordered by sequence ASC",
        ),
    ])
    .examples(&[
        json!({"operation": "list_workflows", "limit": 20}),
        json!({"operation": "read_workflow", "workflow_id": "<uuid>"}),
        json!({"operation": "list_workflow_errors", "workflow_id": "<uuid>"}),
        json!({"operation": "create_workflow_folder", "name": "Reports", "color": "#10b981"}),
    ])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {
            "operation": {"type": "string"},
            "workflow_id": {"type": "string"},
            "folder_id": {"type": ["string", "null"]},
            "limit": {"type": "integer", "minimum": 1, "maximum": 200},
            "new_name": {"type": "string"},
            "name": {"type": "string"},
            "color": {"type": "string"},
        },
        "required": ["operation"],
    }),
    output_schema: json!({"type": "object"}),
    requires_confirmation: false,
});

const MAX_ERRORS_PER_LIST: usize = 20;
const DEFAULT_LIST_LIMIT: i64 = 50;

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
        let (where_clause, params): (String, Vec<(String, Value)>) = match folder_id {
            Some(id) if !id.trim().is_empty() => {
                let v = validate_uuid_field(id, "folder_id").map_err(ToolError::InvalidInput)?;
                (
                    "WHERE folder_id = $fid".to_string(),
                    vec![("fid".to_string(), json!(v))],
                )
            }
            _ => (String::new(), Vec::new()),
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

    async fn read_workflow(&self, workflow_id: &str) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;

        // Core workflow row.
        let wq = format!(
            "SELECT meta::id(id) AS id, name, status, agent_id FROM workflow:`{}`",
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

        // First user message = the demand.
        let demand_q = "SELECT content FROM message WHERE workflow_id = $wid AND role = 'user' \
                        ORDER BY created_at ASC LIMIT 1";
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
        let report_q =
            "SELECT content FROM message WHERE workflow_id = $wid AND role = 'assistant' \
                        ORDER BY created_at DESC LIMIT 1";
        let report_rows = self
            .db
            .query_json_with_params(report_q, vec![("wid".to_string(), json!(wid.clone()))])
            .await
            .map_err(|e| ToolError::DatabaseError(e.to_string()))?;
        let report = report_rows
            .into_iter()
            .next()
            .and_then(|r| r["content"].as_str().map(String::from));

        Ok(json!({
            "success": true,
            "workflow_id": wid,
            "demand": demand,
            "report": report,
            "status": wf["status"],
            "target_agent_id": wf["agent_id"],
        }))
    }

    async fn list_workflow_errors(&self, workflow_id: &str) -> ToolResult<Value> {
        let wid =
            validate_uuid_field(workflow_id, "workflow_id").map_err(ToolError::InvalidInput)?;
        let q = format!(
            "SELECT meta::id(id) AS id, tool_name, error_message, sequence, created_at \
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
                self.read_workflow(wid).await
            }
            "list_workflow_errors" => {
                let wid = input["workflow_id"]
                    .as_str()
                    .ok_or_else(|| ToolError::InvalidInput("workflow_id required".to_string()))?;
                self.list_workflow_errors(wid).await
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
            | "list_workflow_errors" => Ok(()),
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
}
