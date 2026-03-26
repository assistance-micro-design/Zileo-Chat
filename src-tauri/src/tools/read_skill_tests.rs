use super::*;

async fn create_test_db() -> Arc<DBClient> {
    let temp = tempfile::tempdir().unwrap();
    let db = DBClient::new(temp.path().join("test").to_str().unwrap())
        .await
        .unwrap();
    db.initialize_schema().await.unwrap();
    Arc::new(db)
}

#[tokio::test]
async fn test_definition() {
    let db = create_test_db().await;
    let tool = ReadSkillTool::new(db, vec!["skill1".to_string()]);
    let def = tool.definition();
    assert_eq!(def.id, "ReadSkillTool");
    assert_eq!(def.name, "ReadSkill");
    assert!(!def.requires_confirmation);
}

#[tokio::test]
async fn test_validate_list_operation() {
    let db = create_test_db().await;
    let tool = ReadSkillTool::new(db, vec![]);
    assert!(tool.validate_input(&json!({"operation": "list"})).is_ok());
}

#[tokio::test]
async fn test_validate_read_missing_name() {
    let db = create_test_db().await;
    let tool = ReadSkillTool::new(db, vec![]);
    assert!(tool.validate_input(&json!({"operation": "read"})).is_err());
    assert!(tool.validate_input(&json!({})).is_err());
}

#[tokio::test]
async fn test_validate_read_with_name() {
    let db = create_test_db().await;
    let tool = ReadSkillTool::new(db, vec![]);
    assert!(tool.validate_input(&json!({"name": "my-skill"})).is_ok());
}

#[tokio::test]
async fn test_validate_invalid_operation() {
    let db = create_test_db().await;
    let tool = ReadSkillTool::new(db, vec![]);
    assert!(tool
        .validate_input(&json!({"operation": "delete"}))
        .is_err());
}

#[tokio::test]
async fn test_list_empty_skills() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap(),
    );
    db.initialize_schema().await.unwrap();

    let tool = ReadSkillTool::new(db, vec![]);
    let result = tool.execute(json!({"operation": "list"})).await.unwrap();
    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["skills"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_read_unassigned_skill() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap(),
    );
    db.initialize_schema().await.unwrap();

    let tool = ReadSkillTool::new(db, vec!["allowed-skill".to_string()]);
    let result = tool.execute(json!({"name": "forbidden-skill"})).await;
    assert!(result.is_err());
    match result {
        Err(ToolError::PermissionDenied(_)) => {}
        other => panic!("Expected PermissionDenied, got {:?}", other),
    }
}

#[tokio::test]
async fn test_read_assigned_but_missing_skill() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap(),
    );
    db.initialize_schema().await.unwrap();

    let tool = ReadSkillTool::new(db, vec!["nonexistent-skill".to_string()]);
    let result = tool.execute(json!({"name": "nonexistent-skill"})).await;
    assert!(result.is_err());
    match result {
        Err(ToolError::NotFound(_)) => {}
        other => panic!("Expected NotFound, got {:?}", other),
    }
}

#[tokio::test]
async fn test_read_existing_skill() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap(),
    );
    db.initialize_schema().await.unwrap();

    // Seed a skill
    let id = uuid::Uuid::new_v4().to_string();
    db.execute_with_params(
        &format!(
            r#"CREATE skill:`{}` CONTENT {{
                name: $name,
                description: $description,
                category: $category,
                content: $content,
                enabled: true,
                created_at: time::now(),
                updated_at: time::now()
            }}"#,
            id
        ),
        vec![
            ("name".to_string(), json!("test-skill")),
            ("description".to_string(), json!("A test skill")),
            ("category".to_string(), json!("coding")),
            (
                "content".to_string(),
                json!("# Test Skill\n\nFollow these rules."),
            ),
        ],
    )
    .await
    .unwrap();

    let tool = ReadSkillTool::new(db, vec!["test-skill".to_string()]);
    let result = tool.execute(json!({"name": "test-skill"})).await.unwrap();

    assert!(result["success"].as_bool().unwrap());
    assert_eq!(result["name"], "test-skill");
    assert_eq!(result["content"], "# Test Skill\n\nFollow these rules.");
}

#[tokio::test]
async fn test_read_disabled_skill() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(
        DBClient::new(temp.path().join("test").to_str().unwrap())
            .await
            .unwrap(),
    );
    db.initialize_schema().await.unwrap();

    // Seed a disabled skill
    let id = uuid::Uuid::new_v4().to_string();
    db.execute_with_params(
        &format!(
            r#"CREATE skill:`{}` CONTENT {{
                name: $name,
                description: $description,
                category: $category,
                content: $content,
                enabled: false,
                created_at: time::now(),
                updated_at: time::now()
            }}"#,
            id
        ),
        vec![
            ("name".to_string(), json!("disabled-skill")),
            ("description".to_string(), json!("Disabled")),
            ("category".to_string(), json!("custom")),
            ("content".to_string(), json!("Content")),
        ],
    )
    .await
    .unwrap();

    let tool = ReadSkillTool::new(db, vec!["disabled-skill".to_string()]);
    let result = tool.execute(json!({"name": "disabled-skill"})).await;
    assert!(result.is_err());
    match result {
        Err(ToolError::NotFound(_)) => {}
        other => panic!("Expected NotFound for disabled skill, got {:?}", other),
    }
}
