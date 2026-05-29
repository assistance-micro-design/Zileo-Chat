// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! `ListAgentsTool` — read-only discovery of permanent worker agents.
//!
//! Lets a Kanban-kind agent discover the `target_agent_id` it should pick
//! when composing a card. Excludes Kanban-kind agents themselves (they are
//! composers, not executors).
//!
//! Private tool: not registered, not factory-instantiable. Injected via the
//! `extra_tools` parameter of `tool_loop::execute_with_tools` by
//! `compose_card.rs` and `kanban_analyzer.rs`.

use crate::db::DBClient;
use crate::tools::description_builder::ToolDescriptionBuilder;
use crate::tools::{Tool, ToolDefinition, ToolError, ToolResult};
use async_trait::async_trait;
use serde_json::{json, Value};
use std::sync::{Arc, LazyLock};
use tracing::debug;

/// Hard cap to keep the prompt budget reasonable. The Kanban setup is not
/// expected to host hundreds of worker agents.
const LIST_LIMIT: usize = 100;

static DEFINITION: LazyLock<ToolDefinition> = LazyLock::new(|| {
    ToolDefinition {
    id: "ListAgentsTool".to_string(),
    name: "ListAgents".to_string(),
    summary: "List permanent worker agents available as a target_agent_id (read-only)."
        .to_string(),
    description: ToolDescriptionBuilder::new(
        "List permanent worker agents available as a target_agent_id (read-only).",
    )
    .use_when(&[
        "You are composing a kanban card and need to pick the target agent that will execute it",
        "You want to inspect an agent's name, system_prompt, and authorized folders before delegating",
    ])
    .do_not_use(&[
        "To delegate actual work - this tool is read-only and does not execute anything",
        "To list Kanban-kind composer agents - they are filtered out (composers, not executors)",
    ])
    .operations(&[(
        "list",
        "Takes no parameters. Returns up to 100 agents with id, name, system_prompt, tools, folders and has_file_manager (true when the agent can read/write files via FileManagerTool).",
    )])
    .examples(&[json!({})])
    .build(),
    input_schema: json!({
        "type": "object",
        "properties": {},
        "additionalProperties": false
    }),
    output_schema: json!({
        "type": "object",
        "properties": {
            "success": {"type": "boolean"},
            "count": {"type": "integer"},
            "agents": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "id": {"type": "string"},
                        "name": {"type": "string"},
                        "system_prompt": {"type": "string"},
                        "tools": {"type": "array", "items": {"type": "string"}},
                        "folders": {"type": "array", "items": {"type": "string"}},
                        "has_file_manager": {"type": "boolean"}
                    }
                }
            }
        }
    }),
    requires_confirmation: false,
}
});

/// Read-only tool listing permanent worker agents (excludes Kanban-kind).
pub struct ListAgentsTool {
    db: Arc<DBClient>,
}

impl ListAgentsTool {
    pub fn new(db: Arc<DBClient>) -> Self {
        Self { db }
    }
}

#[async_trait]
impl Tool for ListAgentsTool {
    fn id(&self) -> &str {
        "ListAgentsTool"
    }

    fn definition(&self) -> ToolDefinition {
        DEFINITION.clone()
    }

    async fn execute(&self, input: Value) -> ToolResult<Value> {
        self.validate_input(&input)?;
        // Kanban-kind agents are composers, not executors. `kind` is stored as
        // None / NONE for legacy standard agents, hence the IS NONE branch.
        let query = format!(
            "SELECT meta::id(id) AS id, name, system_prompt, tools, folders \
             FROM agent \
             WHERE kind IS NONE OR kind != 'kanban' \
             ORDER BY name ASC \
             LIMIT {LIST_LIMIT}"
        );
        let mut rows = self
            .db
            .query_json(&query)
            .await
            .map_err(|e| ToolError::DatabaseError(format!("Failed to list agents: {}", e)))?;

        // K8: the description/output advertised `has_file_manager` but the SELECT
        // never produced it. Derive it from `tools` (mirrors
        // delegate_task_execution.rs) so a Kanban composer knows whether a
        // candidate target can read/write files before delegating.
        for row in rows.iter_mut() {
            let has_file_manager = row
                .get("tools")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().any(|t| t.as_str() == Some("FileManagerTool")))
                .unwrap_or(false);
            if let Some(obj) = row.as_object_mut() {
                obj.insert("has_file_manager".to_string(), json!(has_file_manager));
            }
        }

        debug!(count = rows.len(), "ListAgentsTool returning agents");
        Ok(json!({
            "success": true,
            "count": rows.len(),
            "agents": rows,
        }))
    }

    fn validate_input(&self, _input: &Value) -> ToolResult<()> {
        // No params required.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::setup_test_state;

    /// Inserts a minimal SCHEMAFULL-valid agent row of the requested kind.
    async fn seed_agent(db: &Arc<DBClient>, id: &str, name: &str, kind: Option<&str>) {
        let kind_clause = match kind {
            Some(k) => format!("'{}'", k),
            None => "NONE".to_string(),
        };
        let query = format!(
            "CREATE agent:`{id}` SET \
                id = '{id}', \
                name = '{name}', \
                lifecycle = 'permanent', \
                llm = {{ provider: 'mistral', model: 'mistral-medium', \
                         temperature: 0.7, max_tokens: 4000 }}, \
                tools = [], \
                mcp_servers = [], \
                system_prompt = 'Test agent.', \
                max_tool_iterations = 50, \
                reasoning_effort = NONE, \
                kind = {kind_clause}, \
                created_at = time::now(), \
                updated_at = time::now()"
        );
        db.execute(&query).await.unwrap();
    }

    #[tokio::test]
    async fn lists_non_kanban_agents_only() {
        let (state, _g) = setup_test_state().await;
        let worker_id = uuid::Uuid::new_v4().to_string();
        let kanban_id = uuid::Uuid::new_v4().to_string();
        let legacy_id = uuid::Uuid::new_v4().to_string();
        seed_agent(&state.db, &worker_id, "worker", None).await;
        seed_agent(&state.db, &kanban_id, "kanbanish", Some("kanban")).await;
        seed_agent(&state.db, &legacy_id, "legacy", None).await;

        let tool = ListAgentsTool::new(state.db.clone());
        let out = tool.execute(json!({})).await.unwrap();
        assert_eq!(out["success"], json!(true));
        let agents = out["agents"].as_array().expect("agents array");
        let ids: Vec<&str> = agents.iter().filter_map(|a| a["id"].as_str()).collect();
        assert!(ids.contains(&worker_id.as_str()));
        assert!(ids.contains(&legacy_id.as_str()));
        assert!(!ids.contains(&kanban_id.as_str()));
    }

    /// K8: `has_file_manager` is derived from each agent's `tools` and included
    /// in the output (it was advertised by the description but never produced).
    #[tokio::test]
    async fn computes_has_file_manager_from_tools() {
        let (state, _g) = setup_test_state().await;
        let with_fm = uuid::Uuid::new_v4().to_string();
        let without_fm = uuid::Uuid::new_v4().to_string();

        let seed_with_tools = |id: &str, name: &str, tools: &str| {
            format!(
                "CREATE agent:`{id}` SET id = '{id}', name = '{name}', lifecycle = 'permanent', \
                 llm = {{ provider: 'mistral', model: 'mistral-medium', temperature: 0.7, max_tokens: 4000 }}, \
                 tools = {tools}, mcp_servers = [], system_prompt = 'sp', max_tool_iterations = 50, \
                 reasoning_effort = NONE, kind = NONE, created_at = time::now(), updated_at = time::now()"
            )
        };
        state
            .db
            .execute(&seed_with_tools(
                &with_fm,
                "fm",
                "['FileManagerTool', 'MemoryTool']",
            ))
            .await
            .unwrap();
        state
            .db
            .execute(&seed_with_tools(&without_fm, "nofm", "['MemoryTool']"))
            .await
            .unwrap();

        let tool = ListAgentsTool::new(state.db.clone());
        let out = tool.execute(json!({})).await.unwrap();
        let agents = out["agents"].as_array().expect("agents array");
        let find = |id: &str| {
            agents
                .iter()
                .find(|a| a["id"].as_str() == Some(id))
                .unwrap()
        };
        assert_eq!(
            find(&with_fm)["has_file_manager"],
            json!(true),
            "agent with FileManagerTool must report has_file_manager=true"
        );
        assert_eq!(
            find(&without_fm)["has_file_manager"],
            json!(false),
            "agent without FileManagerTool must report has_file_manager=false"
        );
    }

    #[tokio::test]
    async fn returns_empty_when_no_agents() {
        let (state, _g) = setup_test_state().await;
        let tool = ListAgentsTool::new(state.db.clone());
        let out = tool.execute(json!({})).await.unwrap();
        assert_eq!(out["count"], json!(0));
        assert!(out["agents"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn ignores_input_payload() {
        // Tool takes no params; any input is accepted.
        let (state, _g) = setup_test_state().await;
        let tool = ListAgentsTool::new(state.db.clone());
        tool.execute(json!({"junk": "ignored"})).await.unwrap();
    }
}
