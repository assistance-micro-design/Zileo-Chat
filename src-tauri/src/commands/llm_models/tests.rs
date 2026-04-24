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
use crate::constants::commands as cmd_const;

#[test]
fn test_validate_model_id_valid() {
    assert!(validate_model_id("550e8400-e29b-41d4-a716-446655440000").is_ok());
    assert!(validate_model_id("valid-id").is_ok());
    assert!(validate_model_id("mistral-large-latest").is_ok());
    assert!(validate_model_id("a").is_ok());
    assert!(validate_model_id("my_model.v2").is_ok());
    assert!(validate_model_id(&"a".repeat(128)).is_ok());
}

#[test]
fn test_validate_model_id_invalid() {
    assert!(validate_model_id("").is_err());
    assert!(validate_model_id("   ").is_err());
    assert!(validate_model_id("\t\n").is_err());
    assert!(validate_model_id(&"a".repeat(129)).is_err());
    assert!(validate_model_id(&"x".repeat(200)).is_err());
    assert!(validate_model_id("id'; DROP TABLE --").is_err());
    assert!(validate_model_id("id`; DELETE llm_model").is_err());
    assert!(validate_model_id("id with spaces").is_err());
    assert!(validate_model_id("id{injection}").is_err());
    assert!(validate_model_id("id\x00null").is_err());
}

#[test]
fn test_validate_provider_string_valid() {
    assert!(validate_provider_string("mistral").is_ok());
    assert!(validate_provider_string("ollama").is_ok());
    assert!(validate_provider_string("MISTRAL").is_ok());
    assert!(validate_provider_string("OLLAMA").is_ok());
    assert!(validate_provider_string("Mistral").is_ok());
    assert!(validate_provider_string("Ollama").is_ok());
    assert!(validate_provider_string("MiStRaL").is_ok());
}

#[test]
fn test_validate_provider_string_returns_correct_type() {
    let mistral = validate_provider_string("mistral").unwrap();
    assert_eq!(mistral, ProviderType::Mistral);

    let ollama = validate_provider_string("OLLAMA").unwrap();
    assert_eq!(ollama, ProviderType::Ollama);
}

#[test]
fn test_validate_provider_string_invalid() {
    assert!(validate_provider_string("").is_err());
}

#[test]
fn test_validate_provider_string_custom_providers() {
    let custom = validate_provider_string("routerlab").unwrap();
    assert_eq!(custom, ProviderType::Custom("routerlab".to_string()));

    let custom2 = validate_provider_string("openai").unwrap();
    assert_eq!(custom2, ProviderType::Custom("openai".to_string()));
}

#[test]
fn test_validate_provider_string_error_message() {
    let err = validate_provider_string("").unwrap_err();
    assert!(err.contains("Invalid provider"));
}

#[test]
fn test_max_model_id_len_constant() {
    assert_eq!(cmd_const::MAX_MODEL_ID_LEN, 128);
}

mod injection_tests {
    use crate::test_utils::setup_test_state;

    #[tokio::test]
    async fn test_model_name_with_apostrophe() {
        let (state, _db_guard) = setup_test_state().await;
        let model_id = uuid::Uuid::new_v4().to_string();

        let insert_query = format!("CREATE llm_model:`{}` CONTENT $data", model_id);
        let data = serde_json::json!({
            "id": model_id,
            "provider": "mistral",
            "name": "L'assistant intelligent",
            "api_name": "test-apostrophe-model",
            "context_window": 32000,
            "max_output_tokens": 4096,
            "temperature_default": 0.7,
            "is_builtin": false,
            "is_reasoning": false,
            "input_price_per_mtok": 0.0,
            "output_price_per_mtok": 0.0,
            "cache_read_price_per_mtok": 0.0,
            "cache_write_price_per_mtok": 0.0,
        });

        state
            .db
            .execute_with_params(
                &format!(
                    "{} ; UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
                    insert_query, model_id
                ),
                vec![("data".to_string(), data)],
            )
            .await
            .expect("Failed to create model with apostrophe in name");

        let query = format!(
            "SELECT meta::id(id) AS id, name FROM llm_model:`{}`",
            model_id
        );
        let results: Vec<serde_json::Value> = state
            .db
            .query_json(&query)
            .await
            .expect("Failed to query model");

        assert_eq!(results.len(), 1);
        assert_eq!(
            results[0].get("name").and_then(|v| v.as_str()),
            Some("L'assistant intelligent")
        );
    }

    #[tokio::test]
    async fn test_model_search_injection_safe() {
        let (state, _db_guard) = setup_test_state().await;

        let legit_id = uuid::Uuid::new_v4().to_string();
        let seed_data = serde_json::json!({
            "id": legit_id,
            "provider": "mistral",
            "name": "Legitimate Model",
            "api_name": "legit-model",
            "context_window": 32000,
            "max_output_tokens": 4096,
            "temperature_default": 0.7,
            "is_builtin": false,
            "is_reasoning": false,
            "input_price_per_mtok": 0.0,
            "output_price_per_mtok": 0.0,
            "cache_read_price_per_mtok": 0.0,
            "cache_write_price_per_mtok": 0.0,
        });
        state
            .db
            .execute_with_params(
                &format!(
                    "CREATE llm_model:`{}` CONTENT $data ; \
                     UPDATE llm_model:`{}` SET created_at = time::now(), updated_at = time::now()",
                    legit_id, legit_id
                ),
                vec![("data".to_string(), seed_data)],
            )
            .await
            .expect("Failed to seed legitimate model");

        let injection_string = "' OR 1=1; DELETE FROM llm_model; --";
        let search_query = "SELECT meta::id(id) AS id, name FROM llm_model \
            WHERE api_name = $api_name AND provider = $provider";

        let results: Vec<serde_json::Value> = state
            .db
            .query_json_with_params(
                search_query,
                vec![
                    ("api_name".to_string(), serde_json::json!(injection_string)),
                    ("provider".to_string(), serde_json::json!("mistral")),
                ],
            )
            .await
            .expect("Parameterized query should not fail");

        assert!(
            results.is_empty(),
            "Injection string should not match any model"
        );

        let verify_query = format!(
            "SELECT meta::id(id) AS id, name FROM llm_model:`{}`",
            legit_id
        );
        let verify_results: Vec<serde_json::Value> = state
            .db
            .query_json(&verify_query)
            .await
            .expect("Failed to verify model still exists");

        assert_eq!(
            verify_results.len(),
            1,
            "Legitimate model should still exist after injection attempt"
        );
        assert_eq!(
            verify_results[0].get("name").and_then(|v| v.as_str()),
            Some("Legitimate Model")
        );
    }
}
