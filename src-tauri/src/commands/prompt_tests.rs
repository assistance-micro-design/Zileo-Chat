use crate::test_utils::{seed_test_prompt, seed_test_prompt_with_category, setup_test_state};

#[tokio::test]
async fn test_search_prompts_with_valid_query() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    let results: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            r#"SELECT meta::id(id) AS id, name FROM prompt
               WHERE string::lowercase(name) CONTAINS $search"#,
            vec![("search".to_string(), serde_json::json!("test"))],
        )
        .await
        .unwrap();
    assert!(!results.is_empty(), "Should find prompts matching 'test'");
}

#[tokio::test]
async fn test_search_prompts_injection_safe() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    // Attempt SQL injection via search parameter
    let results: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            r#"SELECT meta::id(id) AS id, name FROM prompt
               WHERE string::lowercase(name) CONTAINS $search"#,
            vec![(
                "search".to_string(),
                serde_json::json!("'; DELETE prompt WHERE '1'='1"),
            )],
        )
        .await
        .unwrap();
    assert!(results.is_empty(), "Injection string should match nothing");
}

#[tokio::test]
async fn test_search_prompts_injection_preserves_data() {
    let state = setup_test_state().await;
    seed_test_prompt(&state.db).await;
    // Attempt injection
    let _ = state
        .db
        .query_json_with_params(
            r#"SELECT meta::id(id) AS id FROM prompt
               WHERE string::lowercase(name) CONTAINS $search"#,
            vec![(
                "search".to_string(),
                serde_json::json!("'; DELETE prompt WHERE '1'='1"),
            )],
        )
        .await;
    // Verify data is still intact
    let all: Vec<serde_json::Value> = state
        .db
        .query_json("SELECT meta::id(id) AS id FROM prompt")
        .await
        .unwrap();
    assert!(!all.is_empty(), "Data should not be deleted by injection");
}

#[tokio::test]
async fn test_search_prompts_with_category() {
    let state = setup_test_state().await;
    seed_test_prompt_with_category(&state.db, "coding").await;
    let results: Vec<serde_json::Value> = state
        .db
        .query_json_with_params(
            "SELECT meta::id(id) AS id FROM prompt WHERE category = $category",
            vec![("category".to_string(), serde_json::json!("coding"))],
        )
        .await
        .unwrap();
    assert!(
        !results.is_empty(),
        "Should find prompts with category 'coding'"
    );
}

#[tokio::test]
async fn test_create_prompt_with_bind_params() {
    let state = setup_test_state().await;
    let id = uuid::Uuid::new_v4().to_string();
    let query = format!(
        r#"CREATE prompt:`{}` CONTENT {{
            name: $name,
            description: $description,
            category: $category,
            content: $content,
            variables: $variables,
            created_at: time::now(),
            updated_at: time::now()
        }}"#,
        id
    );
    state
        .db
        .execute_with_params(
            &query,
            vec![
                ("name".to_string(), serde_json::json!("Test's Prompt")),
                (
                    "description".to_string(),
                    serde_json::json!("A prompt with 'quotes' and \"doubles\""),
                ),
                ("category".to_string(), serde_json::json!("general")),
                ("content".to_string(), serde_json::json!("Hello {{name}}")),
                ("variables".to_string(), serde_json::json!([])),
            ],
        )
        .await
        .unwrap();

    // Verify the prompt was created with correct data
    let results: Vec<serde_json::Value> = state
        .db
        .query_json(&format!(
            "SELECT meta::id(id) AS id, name FROM prompt:`{}`",
            id
        ))
        .await
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0]["name"], "Test's Prompt");
}
