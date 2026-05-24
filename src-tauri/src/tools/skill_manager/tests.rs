// Copyright 2025 Assistance Micro Design
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.

//! Integration tests for `SkillManagerTool`, exercising the dispatcher and
//! every operation against an in-memory SurrealDB instance.

use super::SkillManagerTool;
use crate::db::DBClient;
use crate::models::agent::AgentKind;
use crate::test_utils::setup_test_state;
use crate::tools::{Tool, ToolError};
use serde_json::json;
use std::sync::Arc;

fn kanban_tool(db: Arc<DBClient>, agent_id: &str) -> SkillManagerTool {
    SkillManagerTool::new(db, agent_id.to_string(), Some(AgentKind::Kanban))
}

fn standard_tool(db: Arc<DBClient>, agent_id: &str) -> SkillManagerTool {
    SkillManagerTool::new(db, agent_id.to_string(), None)
}

async fn insert_agent(db: &Arc<DBClient>, id: &str, kind: Option<&str>) {
    let kind_clause = match kind {
        Some(k) => format!(", kind: '{}'", k),
        None => String::new(),
    };
    let q = format!(
        "CREATE agent:`{}` CONTENT {{
            name: 'A_{}', lifecycle: 'permanent', system_prompt: 's',
            llm: {{ provider: 'mistral', model: 'm', temperature: 0.7, max_tokens: 4096 }},
            tools: [], mcp_servers: [], skills: [], folders: [],
            max_tool_iterations: 50,
            auto_analyze_reports: false{},
            created_at: time::now(), updated_at: time::now()
        }}",
        id,
        &id[..8],
        kind_clause
    );
    db.execute(&q).await.unwrap();
}

#[test]
fn test_edit_summary_required() {
    assert!(SkillManagerTool::validate_edit_summary("").is_err());
    assert!(SkillManagerTool::validate_edit_summary("ok").is_ok());
}

#[tokio::test]
async fn test_standard_agent_denied_on_list() {
    let (state, _g) = setup_test_state().await;
    let tool = standard_tool(state.db.clone(), "agent-x");
    let err = tool
        .execute(json!({"operation": "list_skills"}))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_standard_agent_denied_on_create() {
    let (state, _g) = setup_test_state().await;
    let agent_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &agent_id, None).await;
    let tool = standard_tool(state.db.clone(), &agent_id);
    let err = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "new-skill",
            "description": "x",
            "content": "# c",
            "target_agent_id": agent_id
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::PermissionDenied(_)));
}

#[tokio::test]
async fn test_kanban_create_for_standard_agent_yields_standard_kind() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;

    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let res = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "for-standard",
            "description": "Pour un agent standard",
            "content": "# Standard skill",
            "category": "custom",
            "target_agent_id": target_id
        }))
        .await
        .unwrap();
    assert_eq!(res["success"], true);
    assert_eq!(res["kind"], "standard");
    assert_eq!(res["target_agent_id"], target_id);

    // Target agent now has the skill in its allowlist.
    let q = format!("SELECT skills FROM agent:`{}`", target_id);
    let rows = state.db.query_json(&q).await.unwrap();
    let skills = rows[0]["skills"].as_array().unwrap();
    assert!(skills.iter().any(|v| v.as_str() == Some("for-standard")));

    // Skill row has kind = NONE (returned as null from query_json).
    let sq = "SELECT kind FROM skill WHERE name = $n";
    let srows = state
        .db
        .query_json_with_params(sq, vec![("n".to_string(), json!("for-standard"))])
        .await
        .unwrap();
    assert!(srows[0]["kind"].is_null());
}

#[tokio::test]
async fn test_kanban_create_for_kanban_target_yields_kanban_kind() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, Some("kanban")).await;

    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let res = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "for-kanban",
            "description": "Pour un agent kanban",
            "content": "# Kanban skill",
            "category": "workflow",
            "target_agent_id": target_id
        }))
        .await
        .unwrap();
    assert_eq!(res["kind"], "kanban");

    let sq = "SELECT kind FROM skill WHERE name = $n";
    let srows = state
        .db
        .query_json_with_params(sq, vec![("n".to_string(), json!("for-kanban"))])
        .await
        .unwrap();
    assert_eq!(srows[0]["kind"], "kanban");
}

#[tokio::test]
async fn test_kanban_create_rejects_missing_target() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let err = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "x", "description": "x", "content": "# x"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

async fn create_skill_for_target(tool: &SkillManagerTool, name: &str, target_id: &str) -> String {
    let res = tool
        .execute(json!({
            "operation": "create_skill",
            "name": name,
            "description": "seed desc",
            "content": "# Seed content",
            "target_agent_id": target_id
        }))
        .await
        .unwrap();
    res["skill_id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn test_create_skill_records_baseline_version() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "baseline-skill", &target_id).await;

    let res = tool
        .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
        .await
        .unwrap();
    let versions = res["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 1);
    assert_eq!(versions[0]["version"], 1);
    assert_eq!(versions[0]["edit_summary"], "Initial version");
}

#[tokio::test]
async fn test_create_skill_rejects_duplicate_name() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    create_skill_for_target(&tool, "dup-name", &target_id).await;

    let err = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "dup-name",
            "description": "again",
            "content": "# again",
            "target_agent_id": target_id
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::ValidationFailed(_)));
}

#[tokio::test]
async fn test_update_skill_rejects_kind_field() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "kind-reject", &target_id).await;

    let err = tool
        .execute(json!({
            "operation": "update_skill",
            "skill_id": skill_id,
            "kind": "kanban",
            "edit_summary": "trying to change kind"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[tokio::test]
async fn test_update_skill_rejects_target_agent_id_field() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "target-reject", &target_id).await;

    let err = tool
        .execute(json!({
            "operation": "update_skill",
            "skill_id": skill_id,
            "target_agent_id": target_id,
            "edit_summary": "trying to retarget"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::InvalidInput(_)));
}

#[tokio::test]
async fn test_update_skill_rename_cascades_to_agents() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    let other_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    insert_agent(&state.db, &other_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "old-name", &target_id).await;
    // Manually grant the same skill to a second agent so the cascade has
    // more than one agent to update.
    tool.grant_skill_name_raw(&other_id, "old-name")
        .await
        .unwrap();

    tool.execute(json!({
        "operation": "update_skill",
        "skill_id": skill_id,
        "name": "new-name",
        "edit_summary": "Renamed"
    }))
    .await
    .unwrap();

    for aid in [&target_id, &other_id] {
        let q = format!("SELECT skills FROM agent:`{}`", aid);
        let rows = state.db.query_json(&q).await.unwrap();
        let skills = rows[0]["skills"].as_array().unwrap();
        assert!(
            skills.iter().any(|v| v.as_str() == Some("new-name")),
            "agent {} missing renamed skill",
            aid
        );
        assert!(
            skills.iter().all(|v| v.as_str() != Some("old-name")),
            "agent {} still has old name",
            aid
        );
    }
}

#[tokio::test]
async fn test_update_skill_rejects_rename_collision() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    create_skill_for_target(&tool, "skill-a", &target_id).await;
    let id_b = create_skill_for_target(&tool, "skill-b", &target_id).await;

    let err = tool
        .execute(json!({
            "operation": "update_skill",
            "skill_id": id_b,
            "name": "skill-a",
            "edit_summary": "Collide on purpose"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::ValidationFailed(_)));
}

#[tokio::test]
async fn test_list_skill_versions_orders_desc() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "versioned", &target_id).await;

    tool.execute(json!({
        "operation": "update_skill",
        "skill_id": skill_id,
        "content": "# Updated once",
        "edit_summary": "Second cut"
    }))
    .await
    .unwrap();

    let res = tool
        .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
        .await
        .unwrap();
    let versions = res["versions"].as_array().unwrap();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["version"], 2);
    assert_eq!(versions[1]["version"], 1);
}

#[tokio::test]
async fn test_restore_skill_version_rolls_back_content() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let skill_id = create_skill_for_target(&tool, "rollback-me", &target_id).await;

    // v2 changes the content.
    tool.execute(json!({
        "operation": "update_skill",
        "skill_id": skill_id,
        "content": "# Mutated content",
        "edit_summary": "Worse version"
    }))
    .await
    .unwrap();

    // Identify v1's version_id (the baseline).
    let versions = tool
        .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
        .await
        .unwrap();
    let v1 = versions["versions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|v| v["version"] == 1)
        .unwrap()
        .clone();
    let v1_id = v1["id"].as_str().unwrap().to_string();

    tool.execute(json!({
        "operation": "restore_skill_version",
        "skill_id": skill_id,
        "version_id": v1_id
    }))
    .await
    .unwrap();

    // Skill content now matches the baseline; history grew to 3 rows.
    let read = tool
        .execute(json!({"operation": "read_skill", "name": "rollback-me"}))
        .await
        .unwrap();
    assert_eq!(read["skill"]["content"], "# Seed content");

    let versions = tool
        .execute(json!({"operation": "list_skill_versions", "skill_id": skill_id}))
        .await
        .unwrap();
    assert_eq!(versions["versions"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn test_revoke_skill_from_agent_removes_name() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    create_skill_for_target(&tool, "revoke-me", &target_id).await;

    // Sanity: target has the skill granted by create_skill.
    let q = format!("SELECT skills FROM agent:`{}`", target_id);
    let rows = state.db.query_json(&q).await.unwrap();
    assert!(rows[0]["skills"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v.as_str() == Some("revoke-me")));

    tool.execute(json!({
        "operation": "revoke_skill_from_agent",
        "target_agent_id": target_id,
        "skill_name": "revoke-me"
    }))
    .await
    .unwrap();

    let rows = state.db.query_json(&q).await.unwrap();
    assert!(rows[0]["skills"]
        .as_array()
        .unwrap()
        .iter()
        .all(|v| v.as_str() != Some("revoke-me")));
}

#[tokio::test]
async fn test_revoke_skill_from_agent_is_idempotent() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);

    // Skill was never granted; revoke should still succeed.
    let res = tool
        .execute(json!({
            "operation": "revoke_skill_from_agent",
            "target_agent_id": target_id,
            "skill_name": "never-granted"
        }))
        .await
        .unwrap();
    assert_eq!(res["success"], true);
}

#[tokio::test]
async fn test_revoke_skill_rejects_unknown_agent() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    let unknown = uuid::Uuid::new_v4().to_string();
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let err = tool
        .execute(json!({
            "operation": "revoke_skill_from_agent",
            "target_agent_id": unknown,
            "skill_name": "anything"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}

#[tokio::test]
async fn test_standard_agent_denied_on_new_ops() {
    let (state, _g) = setup_test_state().await;
    let agent_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &agent_id, None).await;
    let tool = standard_tool(state.db.clone(), &agent_id);
    for op in [
        json!({"operation": "list_skill_versions", "skill_id": uuid::Uuid::new_v4().to_string()}),
        json!({
            "operation": "restore_skill_version",
            "skill_id": uuid::Uuid::new_v4().to_string(),
            "version_id": uuid::Uuid::new_v4().to_string()
        }),
        json!({
            "operation": "revoke_skill_from_agent",
            "target_agent_id": agent_id,
            "skill_name": "x"
        }),
    ] {
        let err = tool.execute(op).await.unwrap_err();
        assert!(matches!(err, ToolError::PermissionDenied(_)));
    }
}

#[tokio::test]
async fn test_kanban_create_rejects_unknown_target() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    let unknown_id = uuid::Uuid::new_v4().to_string();
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    let err = tool
        .execute(json!({
            "operation": "create_skill",
            "name": "x", "description": "x", "content": "# x",
            "target_agent_id": unknown_id
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}

#[tokio::test]
async fn test_grant_existing_skill_same_kind() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let other_kanban_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &other_kanban_id, Some("kanban")).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);

    // Create a kanban skill (kind derived from the kanban target).
    create_skill_for_target(&tool, "shared-kanban-skill", &kanban_id).await;

    // Grant it to another kanban agent → allowed.
    let res = tool
        .execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": other_kanban_id,
            "skill_name": "shared-kanban-skill"
        }))
        .await
        .unwrap();
    assert_eq!(res["success"], true);

    let q = format!("SELECT skills FROM agent:`{}`", other_kanban_id);
    let rows = state.db.query_json(&q).await.unwrap();
    let skills = rows[0]["skills"].as_array().unwrap();
    assert!(skills
        .iter()
        .any(|v| v.as_str() == Some("shared-kanban-skill")));
}

#[tokio::test]
async fn test_grant_rejects_cross_kind() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let standard_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &standard_id, None).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);

    // Skill is kanban-kind (created for a kanban target).
    create_skill_for_target(&tool, "kanban-only-skill", &kanban_id).await;

    // Granting it to a standard agent must be rejected.
    let err = tool
        .execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": standard_id,
            "skill_name": "kanban-only-skill"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::ValidationFailed(_)));
    assert!(err.to_string().contains("strict separation"));
}

#[tokio::test]
async fn test_grant_unknown_skill_not_found() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, Some("kanban")).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);

    let err = tool
        .execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": target_id,
            "skill_name": "ghost-skill"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}

#[tokio::test]
async fn test_grant_unknown_agent_not_found() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    create_skill_for_target(&tool, "real-skill", &kanban_id).await;

    let unknown_agent = uuid::Uuid::new_v4().to_string();
    let err = tool
        .execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": unknown_agent,
            "skill_name": "real-skill"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}

#[tokio::test]
async fn test_grant_is_idempotent() {
    let (state, _g) = setup_test_state().await;
    let kanban_id = uuid::Uuid::new_v4().to_string();
    let target_id = uuid::Uuid::new_v4().to_string();
    insert_agent(&state.db, &kanban_id, Some("kanban")).await;
    insert_agent(&state.db, &target_id, Some("kanban")).await;
    let tool = kanban_tool(state.db.clone(), &kanban_id);
    create_skill_for_target(&tool, "idem-skill", &kanban_id).await;

    for _ in 0..2 {
        tool.execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": target_id,
            "skill_name": "idem-skill"
        }))
        .await
        .unwrap();
    }

    // The skill appears exactly once (array::union dedupes).
    let q = format!("SELECT skills FROM agent:`{}`", target_id);
    let rows = state.db.query_json(&q).await.unwrap();
    let count = rows[0]["skills"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v.as_str() == Some("idem-skill"))
        .count();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn test_standard_agent_denied_on_grant() {
    let (state, _g) = setup_test_state().await;
    let tool = standard_tool(state.db.clone(), "agent-x");
    let err = tool
        .execute(json!({
            "operation": "grant_skill_to_agent",
            "target_agent_id": uuid::Uuid::new_v4().to_string(),
            "skill_name": "whatever"
        }))
        .await
        .unwrap_err();
    assert!(matches!(err, ToolError::PermissionDenied(_)));
}
