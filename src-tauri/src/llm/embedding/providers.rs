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

//! Provider-specific embedding implementations (Mistral, Ollama).

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, warn};

use super::{EmbeddingError, MISTRAL_EMBEDDING_URL};

/// Mistral embedding API request
#[derive(Debug, Serialize)]
pub(super) struct MistralEmbeddingRequest<'a> {
    pub(super) model: &'a str,
    pub(super) input: Vec<&'a str>,
    pub(super) encoding_format: &'a str,
}

/// Mistral embedding API response
#[derive(Debug, Deserialize)]
pub(super) struct MistralEmbeddingResponse {
    #[allow(dead_code)]
    pub id: String,
    #[allow(dead_code)]
    pub object: String,
    pub data: Vec<MistralEmbeddingData>,
    #[allow(dead_code)]
    pub model: String,
    #[allow(dead_code)]
    pub usage: MistralUsage,
}

/// Mistral embedding data item
#[derive(Debug, Deserialize)]
pub(super) struct MistralEmbeddingData {
    #[allow(dead_code)]
    pub object: String,
    pub index: usize,
    pub embedding: Vec<f32>,
}

/// Mistral API usage statistics
#[derive(Debug, Deserialize)]
pub(super) struct MistralUsage {
    #[allow(dead_code)]
    pub prompt_tokens: usize,
    #[allow(dead_code)]
    pub total_tokens: usize,
}

/// Ollama embedding API request
#[derive(Debug, Serialize)]
pub(super) struct OllamaEmbeddingRequest<'a> {
    pub(super) model: &'a str,
    pub(super) prompt: &'a str,
}

/// Ollama embedding API response
#[derive(Debug, Deserialize)]
pub(super) struct OllamaEmbeddingResponse {
    pub embedding: Vec<f32>,
}

/// Embeds text using Mistral API.
pub async fn embed_mistral(
    client: &Client,
    dimension: &Arc<RwLock<usize>>,
    text: &str,
    api_key: &str,
    model: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    let request = MistralEmbeddingRequest {
        model,
        input: vec![text],
        encoding_format: "float",
    };

    debug!(model = model, "Sending Mistral embedding request");

    let response = client
        .post(MISTRAL_EMBEDDING_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(EmbeddingError::RequestFailed(format!(
            "Mistral API returned {}: {}",
            status, body
        )));
    }

    let result: MistralEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

    let embedding = result
        .data
        .into_iter()
        .next()
        .map(|d| d.embedding)
        .ok_or_else(|| EmbeddingError::InvalidResponse("No embedding in response".to_string()))?;

    let expected_dim = *dimension.read().await;
    if embedding.len() != expected_dim {
        warn!(
            expected = expected_dim,
            actual = embedding.len(),
            "Embedding dimension mismatch"
        );
    }

    debug!(
        dimension = embedding.len(),
        "Mistral embedding generated successfully"
    );

    Ok(embedding)
}

/// Embeds batch using Mistral API (native batch support).
pub async fn embed_batch_mistral(
    client: &Client,
    texts: &[&str],
    api_key: &str,
    model: &str,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    let request = MistralEmbeddingRequest {
        model,
        input: texts.to_vec(),
        encoding_format: "float",
    };

    debug!(
        model = model,
        batch_size = texts.len(),
        "Sending Mistral batch embedding request"
    );

    let response = client
        .post(MISTRAL_EMBEDDING_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(EmbeddingError::RequestFailed(format!(
            "Mistral API returned {}: {}",
            status, body
        )));
    }

    let result: MistralEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

    let mut embeddings: Vec<_> = result.data.into_iter().collect();
    embeddings.sort_by_key(|d| d.index);

    let embeddings: Vec<Vec<f32>> = embeddings.into_iter().map(|d| d.embedding).collect();

    debug!(
        count = embeddings.len(),
        "Mistral batch embedding generated successfully"
    );

    Ok(embeddings)
}

/// Embeds text using Ollama API.
pub async fn embed_ollama(
    client: &Client,
    text: &str,
    base_url: &str,
    model: &str,
) -> Result<Vec<f32>, EmbeddingError> {
    let url = format!("{}/api/embeddings", base_url);
    let request = OllamaEmbeddingRequest {
        model,
        prompt: text,
    };

    debug!(model = model, url = %url, "Sending Ollama embedding request");

    let response = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&request)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                EmbeddingError::ConnectionError(format!(
                    "Cannot connect to Ollama server at {}. Is Ollama running?",
                    base_url
                ))
            } else {
                EmbeddingError::from(e)
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if body.contains("not found") || body.contains("does not exist") {
            return Err(EmbeddingError::ModelNotAvailable(format!(
                "Model '{}' not found. Try: ollama pull {}",
                model, model
            )));
        }

        return Err(EmbeddingError::RequestFailed(format!(
            "Ollama API returned {}: {}",
            status, body
        )));
    }

    let result: OllamaEmbeddingResponse = response
        .json()
        .await
        .map_err(|e| EmbeddingError::InvalidResponse(e.to_string()))?;

    debug!(
        dimension = result.embedding.len(),
        "Ollama embedding generated successfully"
    );

    Ok(result.embedding)
}

/// Embeds batch using Ollama API (sequential, no native batch support).
pub async fn embed_batch_ollama(
    client: &Client,
    texts: &[&str],
    base_url: &str,
    model: &str,
) -> Result<Vec<Vec<f32>>, EmbeddingError> {
    debug!(
        batch_size = texts.len(),
        "Processing Ollama batch sequentially"
    );

    let mut embeddings = Vec::with_capacity(texts.len());
    for text in texts {
        let embedding = embed_ollama(client, text, base_url, model).await?;
        embeddings.push(embedding);
    }

    debug!(count = embeddings.len(), "Ollama batch embedding completed");

    Ok(embeddings)
}
