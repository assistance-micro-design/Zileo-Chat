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
use providers::{
    MistralEmbeddingRequest, MistralEmbeddingResponse, OllamaEmbeddingRequest,
    OllamaEmbeddingResponse,
};

// ---- EmbeddingError Tests ----

#[test]
fn test_embedding_error_display() {
    let err = EmbeddingError::TextTooLong(60000, 50000);
    assert!(err.to_string().contains("60000"));
    assert!(err.to_string().contains("50000"));

    let err = EmbeddingError::NotConfigured("test".to_string());
    assert!(err.to_string().contains("not configured"));

    let err = EmbeddingError::BatchTooLarge(100, 96);
    assert!(err.to_string().contains("100"));
    assert!(err.to_string().contains("96"));
}

// ---- EmbeddingProvider Tests ----

#[test]
fn test_embedding_provider_mistral() {
    let provider = EmbeddingProvider::mistral("test-key");
    assert_eq!(provider.name(), "mistral");
    assert_eq!(provider.model(), MISTRAL_EMBED_MODEL);
    assert_eq!(provider.dimension(), MISTRAL_EMBED_DIMENSION);
}

#[test]
fn test_embedding_provider_ollama() {
    let provider = EmbeddingProvider::ollama();
    assert_eq!(provider.name(), "ollama");
    assert_eq!(provider.model(), DEFAULT_OLLAMA_EMBED_MODEL);
    assert_eq!(provider.dimension(), OLLAMA_NOMIC_DIMENSION);
}

#[test]
fn test_embedding_provider_ollama_mxbai() {
    let provider =
        EmbeddingProvider::ollama_with_config("http://localhost:11434", "mxbai-embed-large");
    assert_eq!(provider.dimension(), OLLAMA_MXBAI_DIMENSION);
}

#[test]
fn test_embedding_provider_ollama_nomic() {
    let provider =
        EmbeddingProvider::ollama_with_config("http://localhost:11434", "nomic-embed-text");
    assert_eq!(provider.dimension(), OLLAMA_NOMIC_DIMENSION);
}

// ---- EmbeddingConfig Tests ----

#[test]
fn test_embedding_config_default() {
    let config = EmbeddingConfig::default();
    assert_eq!(config.provider, "mistral");
    assert_eq!(config.model, MISTRAL_EMBED_MODEL);
    assert_eq!(config.dimension, MISTRAL_EMBED_DIMENSION);
}

#[test]
fn test_embedding_config_ollama_nomic() {
    let config = EmbeddingConfig::ollama_nomic();
    assert_eq!(config.provider, "ollama");
    assert_eq!(config.model, "nomic-embed-text");
    assert_eq!(config.dimension, OLLAMA_NOMIC_DIMENSION);
}

#[test]
fn test_embedding_config_ollama_mxbai() {
    let config = EmbeddingConfig::ollama_mxbai();
    assert_eq!(config.provider, "ollama");
    assert_eq!(config.model, "mxbai-embed-large");
    assert_eq!(config.dimension, OLLAMA_MXBAI_DIMENSION);
}

#[test]
fn test_embedding_config_serialization() {
    let config = EmbeddingConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: EmbeddingConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.provider, config.provider);
    assert_eq!(deserialized.model, config.model);
    assert_eq!(deserialized.dimension, config.dimension);
}

// ---- EmbeddingService Tests ----

#[test]
fn test_embedding_service_new() {
    let service = EmbeddingService::new().expect("test embedding service");
    assert!(!service.is_configured());
}

#[test]
fn test_embedding_service_with_provider() {
    let provider = EmbeddingProvider::mistral("test-key");
    let service = EmbeddingService::with_provider(provider).expect("test embedding service");
    assert!(service.is_configured());
}

#[tokio::test]
async fn test_embedding_service_configure() {
    let service = EmbeddingService::new().expect("test embedding service");
    assert!(!service.is_configured());

    let provider = EmbeddingProvider::mistral("test-key");
    service.configure(provider).await;
    assert!(service.is_configured());

    service.clear().await;
    assert!(!service.is_configured());
}

#[tokio::test]
async fn test_embedding_service_dimension() {
    let provider = EmbeddingProvider::mistral("test-key");
    let service = EmbeddingService::with_provider(provider).expect("test embedding service");
    assert_eq!(service.dimension().await, MISTRAL_EMBED_DIMENSION);
}

#[tokio::test]
async fn test_embedding_service_dimension_ollama() {
    let provider = EmbeddingProvider::ollama();
    let service = EmbeddingService::with_provider(provider).expect("test embedding service");
    assert_eq!(service.dimension().await, OLLAMA_NOMIC_DIMENSION);
}

// ---- Validation Tests ----

#[test]
fn test_validate_text_empty() {
    let service = EmbeddingService::new().expect("test embedding service");
    let result = service.validate_text("");
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::InvalidResponse(_) => {}
        _ => panic!("Expected InvalidResponse error"),
    }
}

#[test]
fn test_validate_text_too_long() {
    let service = EmbeddingService::new().expect("test embedding service");
    let long_text = "x".repeat(MAX_EMBEDDING_TEXT_LENGTH + 1);
    let result = service.validate_text(&long_text);
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::TextTooLong(len, max) => {
            assert_eq!(len, MAX_EMBEDDING_TEXT_LENGTH + 1);
            assert_eq!(max, MAX_EMBEDDING_TEXT_LENGTH);
        }
        _ => panic!("Expected TextTooLong error"),
    }
}

#[test]
fn test_validate_text_valid() {
    let service = EmbeddingService::new().expect("test embedding service");
    let result = service.validate_text("Hello, world!");
    assert!(result.is_ok());
}

// ---- Error Handling Tests ----

#[tokio::test]
async fn test_embed_not_configured() {
    let service = EmbeddingService::new().expect("test embedding service");
    let result = service.embed("test").await;
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::NotConfigured(_) => {}
        _ => panic!("Expected NotConfigured error"),
    }
}

#[tokio::test]
async fn test_embed_batch_not_configured() {
    let service = EmbeddingService::new().expect("test embedding service");
    let result = service.embed_batch(&["test1", "test2"]).await;
    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::NotConfigured(_) => {}
        _ => panic!("Expected NotConfigured error"),
    }
}

#[tokio::test]
async fn test_embed_batch_empty() {
    let service = EmbeddingService::new().expect("test embedding service");
    let result = service.embed_batch(&[]).await;
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[tokio::test]
async fn test_embed_batch_too_large() {
    let provider = EmbeddingProvider::mistral("test-key");
    let service = EmbeddingService::with_provider(provider).expect("test embedding service");

    let texts: Vec<&str> = (0..MAX_BATCH_SIZE + 1).map(|_| "test").collect();
    let result = service.embed_batch(&texts).await;

    assert!(result.is_err());
    match result.unwrap_err() {
        EmbeddingError::BatchTooLarge(size, max) => {
            assert_eq!(size, MAX_BATCH_SIZE + 1);
            assert_eq!(max, MAX_BATCH_SIZE);
        }
        _ => panic!("Expected BatchTooLarge error"),
    }
}

// ---- Request/Response Serialization Tests ----

#[test]
fn test_mistral_request_serialization() {
    let request = MistralEmbeddingRequest {
        model: "mistral-embed",
        input: vec!["Hello", "World"],
        encoding_format: "float",
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("mistral-embed"));
    assert!(json.contains("Hello"));
    assert!(json.contains("World"));
    assert!(json.contains("float"));
}

#[test]
fn test_mistral_response_deserialization() {
    let json = r#"{
        "id": "emb-123",
        "object": "list",
        "data": [
            {
                "object": "embedding",
                "index": 0,
                "embedding": [0.1, 0.2, 0.3]
            }
        ],
        "model": "mistral-embed",
        "usage": {
            "prompt_tokens": 10,
            "total_tokens": 10
        }
    }"#;

    let response: MistralEmbeddingResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.id, "emb-123");
    assert_eq!(response.data.len(), 1);
    assert_eq!(response.data[0].embedding, vec![0.1, 0.2, 0.3]);
}

#[test]
fn test_ollama_request_serialization() {
    let request = OllamaEmbeddingRequest {
        model: "nomic-embed-text",
        prompt: "Hello, world!",
    };
    let json = serde_json::to_string(&request).unwrap();
    assert!(json.contains("nomic-embed-text"));
    assert!(json.contains("Hello, world!"));
}

#[test]
fn test_ollama_response_deserialization() {
    let json = r#"{
        "embedding": [0.1, 0.2, 0.3, 0.4, 0.5]
    }"#;

    let response: OllamaEmbeddingResponse = serde_json::from_str(json).unwrap();
    assert_eq!(response.embedding.len(), 5);
    assert_eq!(response.embedding[0], 0.1);
}
