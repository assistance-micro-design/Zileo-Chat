# Plan d'Optimisation - LLM

## Metadata
- **Date**: 2025-12-08
- **Domaine**: llm
- **Stack**: Rust 1.91.1 + Rig.rs 0.24.0 + Tauri 2.9.3
- **Impact estime**: Performance / Maintenabilite / Robustesse

## Resume Executif

Ce plan d'optimisation cible le module LLM (~3,700 lignes) pour eliminer les duplications de code, ameliorer les performances HTTP et renforcer la robustesse des appels API. Les quick wins (OPT-LLM-1 a OPT-LLM-3) offrent des gains immediats de maintenabilite, tandis que les optimisations strategiques (OPT-LLM-4 a OPT-LLM-6) preparent le systeme pour la production.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src-tauri/src/llm/mistral.rs` | 839 | Haute | custom_complete() 170 LOC, reasoning models |
| `src-tauri/src/llm/ollama.rs` | 681 | Moyenne | complete_with_thinking() 85 LOC |
| `src-tauri/src/llm/manager.rs` | 520 | Moyenne | Provider matching repete 3x |
| `src-tauri/src/llm/provider.rs` | 243 | Faible | Trait bien defini |
| `src-tauri/src/llm/tool_adapter.rs` | 287 | Moyenne | Adapters par provider |
| `src-tauri/src/llm/adapters/` | ~400 | Moyenne | Tests integration |
| `src-tauri/src/llm/pricing.rs` | ~120 | Faible | Calcul couts |
| `src-tauri/src/llm/embedding.rs` | ~200 | Faible | Service embeddings |

**Total module LLM**: ~3,700 lignes

### Patterns Identifies

- **LLMProvider trait**: Interface claire pour providers (complete, complete_stream)
- **ProviderManager**: Orchestration multi-provider avec Arc/RwLock
- **Tool Adapters**: MistralToolAdapter, OllamaToolAdapter pour JSON function calling
- **Error Handling**: LLMError enum avec 9 variantes (thiserror)

### Duplications Trouvees

1. **estimate_tokens()** - 3 implementations identiques:
   - `mistral.rs:618-623`
   - `ollama.rs:240-247`
   - `ollama.rs:443-447`

2. **Streaming simulation** - Code identique:
   - `mistral.rs:679-705`
   - `ollama.rs:513-535`

3. **Provider matching** - Logique repetee:
   - `manager.rs:203-226` (complete)
   - `manager.rs:239-250` (complete_with_provider)
   - `manager.rs:291-310` (complete_with_tools)

### Metriques Actuelles

- **Tests**: ~55 tests, ~70% coverage
- **Arc/RwLock**: 91 occurrences (thread safety OK)
- **Async functions**: 26+ dans le module
- **HTTP Clients**: Nouveau `reqwest::Client::new()` par request

## Best Practices (2024-2025)

### Sources Consultees
- [Rig.rs Documentation](https://rig.rs/) - Framework LLM Rust
- [Rust LLM Streaming Bridge](https://raymondclanan.com/blog/rust-llm-streaming-bridge/) - Streaming patterns
- [Fast LiteLLM](https://github.com/neul-labs/fast-litellm) - Performance patterns
- [Tokio Streams Guide](https://tokio.rs/tokio/tutorial/streams) - Async streaming
- [mistralai-client-rs](https://github.com/ivangabriele/mistralai-client-rs) - Mistral integration

### Patterns Recommandes

1. **Unified Provider API**: Rig.rs fournit abstraction multi-provider (deja implemente)
2. **First Token Latency**: Objectif < 1 seconde pour premiere reponse
3. **Exponential Backoff**: Retry avec delais croissants pour resilience
4. **Connection Pooling**: Reutiliser reqwest::Client (eviter handshake HTTPS)
5. **Circuit Breaker**: Proteger contre cascade failures (implemente pour MCP)

### Anti-Patterns a Eviter

1. **Nouveau HTTP Client par request**: Overhead handshake HTTPS repete
2. **Streaming simule sans real streaming**: UX degradee (delais artificiels)
3. **Unbounded buffers**: Risque explosion memoire
4. **Ignorer context limits**: Depasser window sans gestion

## Contraintes du Projet

### Rate Limits API (CRITIQUE)

Les providers LLM imposent des limites d'appels API. Pour eviter les erreurs `429 Rate Limit Exceeded`, un delai minimum entre appels est obligatoire.

| Provider | Tier Free | Tier Paid | Delai Recommande |
|----------|-----------|-----------|------------------|
| **Mistral** | 1 req/s, 500K tokens/min | 5 req/s, 2M tokens/min | **1000ms** |
| **Ollama** | Local, pas de limite API | - | **1000ms** (securite) |

**Sources**:
- [Mistral Rate Limits](https://docs.mistral.ai/deployment/ai-studio/tier)
- [Mistral Help Center](https://help.mistral.ai/en/articles/424390-how-do-api-rate-limits-work-and-how-do-i-increase-them)

**Implementation requise** (voir OPT-LLM-8):
```rust
/// Delai minimum entre appels API (1 seconde)
pub const MIN_DELAY_BETWEEN_CALLS_MS: u64 = 1000;
```

### Decisions Existantes a Respecter

- **Type Sync**: TypeScript et Rust types synchronises (CLAUDE.md)
- **API Keys**: Tauri secure storage + AES-256 (Phase 0)
- **Query Limits**: LIMIT obligatoire sur toutes queries
- **Error Handling**: Result<T, String> pour IPC
- **Nullability**: `?` pour Option avec skip_serializing_if, `| null` sinon
- **Rate Limiting**: 1 requete/seconde minimum entre appels LLM

### Dependencies Actuelles

| Crate | Version | Statut |
|-------|---------|--------|
| rig-core | 0.24.0 | 0.26.0 disponible (breaking changes) |
| tokio | 1.48.0 | Optimal (LTS) |
| reqwest | 0.12.24 | A jour |
| futures | 0.3.31 | A jour |
| surrealdb | 2.4.0 | A jour |

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-LLM-1: Extraire estimate_tokens() vers llm/utils.rs
- **Fichiers**:
  - `src-tauri/src/llm/utils.rs` (nouveau)
  - `src-tauri/src/llm/mistral.rs:618-623`
  - `src-tauri/src/llm/ollama.rs:240-247, 443-447`
  - `src-tauri/src/llm/mod.rs`
- **Changement**: Creer fonction utilitaire unique pour estimation tokens
- **Code propose**:
```rust
// src-tauri/src/llm/utils.rs
/// Estimates token count using word-based approximation.
/// French/English text averages ~1.3-1.5 tokens per word.
pub fn estimate_tokens(text: &str) -> usize {
    let word_count = text.split_whitespace().count();
    let estimate = ((word_count as f64) * 1.5).ceil() as usize;
    estimate.max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 1);
    }

    #[test]
    fn test_estimate_tokens_single_word() {
        assert_eq!(estimate_tokens("hello"), 2); // 1 * 1.5 = 1.5 -> 2
    }

    #[test]
    fn test_estimate_tokens_sentence() {
        // "This is a test" = 4 words * 1.5 = 6
        assert_eq!(estimate_tokens("This is a test"), 6);
    }
}
```
- **Benefice**: Maintenance (1 endroit), DRY principle
- **Risque regression**: Faible (meme logique)
- **Validation**: `cargo test llm::utils`
- **Effort**: 1h

#### OPT-LLM-2: Centraliser reqwest::Client dans ProviderManager
- **Fichiers**:
  - `src-tauri/src/llm/manager.rs:47-59`
  - `src-tauri/src/llm/mistral.rs:349`
  - `src-tauri/src/llm/ollama.rs:211, 327`
- **Changement**: Creer reqwest::Client une fois au demarrage, partager via Arc
- **Code propose**:
```rust
// Dans ProviderManager
pub struct ProviderManager {
    mistral: Arc<MistralProvider>,
    ollama: Arc<OllamaProvider>,
    config: Arc<RwLock<ProviderConfig>>,
    http_client: reqwest::Client, // NOUVEAU
}

impl ProviderManager {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .pool_max_idle_per_host(5)
            .build()
            .expect("Failed to create HTTP client");
        // ...
    }

    pub fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
}
```
- **Benefice**: Performance (connection pooling, evite handshake HTTPS)
- **Risque regression**: Faible (API identique)
- **Validation**: Tests existants + test_llm_completion manuel
- **Effort**: 2-3h

#### OPT-LLM-3: Dedupliquer streaming simulation
- **Fichiers**:
  - `src-tauri/src/llm/utils.rs` (ajouter)
  - `src-tauri/src/llm/mistral.rs:679-705`
  - `src-tauri/src/llm/ollama.rs:513-535`
- **Changement**: Extraire logique streaming simulee en fonction utilitaire
- **Code propose**:
```rust
// src-tauri/src/llm/utils.rs
use tokio::sync::mpsc;
use std::time::Duration;

/// Simulates streaming by chunking a complete response.
/// Used when provider doesn't support native streaming.
pub async fn simulate_streaming(
    content: String,
    chunk_size: usize,
) -> mpsc::Receiver<Result<String, LLMError>> {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move {
        for chunk in content.as_bytes().chunks(chunk_size) {
            let chunk_str = String::from_utf8_lossy(chunk).to_string();
            if tx.send(Ok(chunk_str)).await.is_err() {
                tracing::warn!("Streaming receiver dropped");
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    });

    rx
}
```
- **Benefice**: Maintenance, preparation pour real streaming
- **Risque regression**: Faible
- **Validation**: Test manuel streaming
- **Effort**: 1-2h

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-LLM-4: Retry mechanism avec exponential backoff
- **Fichiers**:
  - `src-tauri/src/llm/retry.rs` (nouveau)
  - `src-tauri/src/llm/manager.rs`
  - `src-tauri/src/llm/mod.rs`
- **Changement**: Ajouter wrapper retry pour appels LLM avec backoff exponentiel
- **Code propose**:
```rust
// src-tauri/src/llm/retry.rs
use std::time::Duration;
use tokio::time::sleep;

pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay_ms: 1000,
            max_delay_ms: 30000,
        }
    }
}

pub async fn with_retry<F, T, E, Fut>(
    mut operation: F,
    config: &RetryConfig,
) -> Result<T, E>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    let mut attempt = 0;
    loop {
        match operation().await {
            Ok(result) => return Ok(result),
            Err(e) if attempt >= config.max_retries => {
                tracing::error!(
                    attempt = attempt,
                    error = %e,
                    "Max retries exceeded"
                );
                return Err(e);
            }
            Err(e) => {
                attempt += 1;
                let delay = std::cmp::min(
                    config.initial_delay_ms * 2_u64.pow(attempt - 1),
                    config.max_delay_ms,
                );
                tracing::warn!(
                    attempt = attempt,
                    delay_ms = delay,
                    error = %e,
                    "Retrying after error"
                );
                sleep(Duration::from_millis(delay)).await;
            }
        }
    }
}
```
- **Phases**:
  1. Creer module retry.rs avec RetryConfig
  2. Integrer dans ProviderManager.complete()
  3. Ajouter tests unitaires
- **Prerequis**: Aucun
- **Risque regression**: Faible (nouvelle fonctionnalite additive)
- **Tests requis**:
  - test_retry_success_first_attempt()
  - test_retry_success_after_failures()
  - test_retry_max_exceeded()
- **Effort**: 3-4h

#### OPT-LLM-5: Token collection dans StreamChunk
- **Fichiers**:
  - `src-tauri/src/commands/streaming.rs`
  - `src-tauri/src/models/streaming.rs`
  - `src/types/streaming.ts`
- **Changement**: Ajouter champs tokens dans StreamChunk pour tracking
- **Code propose**:
```rust
// Etendre StreamChunk
#[derive(Debug, Clone, Serialize)]
pub struct StreamChunk {
    pub workflow_id: String,
    pub chunk_type: ChunkType,
    pub content: String,
    pub timestamp: DateTime<Utc>,
    // NOUVEAU
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_delta: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens_total: Option<usize>,
}
```
- **Phases**:
  1. Etendre struct Rust
  2. Mettre a jour type TypeScript
  3. Collecter tokens dans workflow execution
- **Prerequis**: Aucun
- **Risque regression**: Faible (champs optionnels)
- **Tests requis**: test_stream_chunk_with_tokens()
- **Effort**: 3h

#### OPT-LLM-6: Circuit breaker pour LLM providers
- **Fichiers**:
  - `src-tauri/src/llm/circuit_breaker.rs` (nouveau, adapter de MCP)
  - `src-tauri/src/llm/manager.rs`
- **Changement**: Reutiliser pattern circuit breaker de MCP pour LLM providers
- **Phases**:
  1. Extraire CircuitBreaker de mcp/manager.rs vers module partage
  2. Integrer dans ProviderManager
  3. Ajouter config par provider (failure_threshold, cooldown)
- **Prerequis**: OPT-LLM-4 (retry mechanism)
- **Risque regression**: Moyen (nouveau comportement)
- **Tests requis**: Adapter tests MCP circuit breaker
- **Effort**: 4h

### Quick Wins Prioritaires (Impact haut, Effort faible)

#### OPT-LLM-8: Rate Limiter pour appels API (CRITIQUE)
- **Fichiers**:
  - `src-tauri/src/llm/rate_limiter.rs` (nouveau)
  - `src-tauri/src/llm/manager.rs`
  - `src-tauri/src/llm/mod.rs`
- **Changement**: Ajouter delai minimum de 1 seconde entre chaque appel API LLM
- **Code propose**:
```rust
// src-tauri/src/llm/rate_limiter.rs
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tokio::time::sleep;

/// Delai minimum entre appels API (1 seconde)
/// Compatible avec Mistral Free Tier (1 req/s) et securise pour Ollama
pub const MIN_DELAY_BETWEEN_CALLS_MS: u64 = 1000;

/// Rate limiter pour respecter les limites API des providers LLM
pub struct RateLimiter {
    last_call: Arc<Mutex<Option<Instant>>>,
    min_delay: Duration,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self {
            last_call: Arc::new(Mutex::new(None)),
            min_delay: Duration::from_millis(MIN_DELAY_BETWEEN_CALLS_MS),
        }
    }

    /// Attend si necessaire avant d'autoriser un nouvel appel API
    pub async fn wait_if_needed(&self) {
        let mut last = self.last_call.lock().await;

        if let Some(last_time) = *last {
            let elapsed = last_time.elapsed();
            if elapsed < self.min_delay {
                let wait_time = self.min_delay - elapsed;
                tracing::debug!(
                    wait_ms = wait_time.as_millis(),
                    "Rate limiting: waiting before next API call"
                );
                sleep(wait_time).await;
            }
        }

        *last = Some(Instant::now());
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limiter_first_call_no_wait() {
        let limiter = RateLimiter::new();
        let start = Instant::now();
        limiter.wait_if_needed().await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_rate_limiter_second_call_waits() {
        let limiter = RateLimiter::new();
        limiter.wait_if_needed().await;
        let start = Instant::now();
        limiter.wait_if_needed().await;
        // Devrait attendre ~1 seconde
        assert!(start.elapsed() >= Duration::from_millis(900));
    }
}
```
- **Integration dans ProviderManager**:
```rust
// src-tauri/src/llm/manager.rs
pub struct ProviderManager {
    // ... existing fields ...
    rate_limiter: RateLimiter, // NOUVEAU
}

impl ProviderManager {
    pub async fn complete(&self, ...) -> Result<LLMResponse, LLMError> {
        // Attendre si necessaire (rate limiting)
        self.rate_limiter.wait_if_needed().await;

        // ... existing code ...
    }
}
```
- **Benefice**: Evite erreurs 429, compatible tous tiers Mistral/Ollama
- **Risque regression**: Faible (delai additionnel transparent)
- **Validation**: Tests unitaires + test manuel avec appels consecutifs
- **Effort**: 1-2h
- **Priorite**: **P0 (CRITIQUE)** - A implementer avant toute mise en production

### Nice to Have (Impact faible, Effort faible)

#### OPT-LLM-7: Consolider HTTP error handling
- **Fichiers**:
  - `src-tauri/src/llm/utils.rs` (ajouter)
  - `src-tauri/src/llm/mistral.rs:356-380`
  - `src-tauri/src/llm/ollama.rs:340-350`
- **Changement**: Creer fonction generique pour parser erreurs HTTP
- **Benefice**: Maintenance, messages d'erreur consistants
- **Effort**: 2h

### Differe (Impact variable, Effort eleve)

| Optimisation | Raison du report | Prerequis |
|--------------|------------------|-----------|
| Upgrade rig-core 0.26.0 | Breaking changes, tester en staging | Tests complets |
| Real streaming | Investigation rig-core capabilities | OPT-LLM-3 |
| Context window manager | Post-v1, conversations longues | Metrics usage |
| Fallback automatique | Complexe, necessite circuit breaker | OPT-LLM-6 |
| Intelligent suggestions | Non prioritaire v1 | Analytics usage |

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| rig-core | 0.24.0 | 0.26.0 | Oui - Tester en staging |

### Nouvelles Dependencies

Aucune nouvelle dependance requise pour ce plan.

## Verification Non-Regression

### Tests Existants
- [x] `cargo test --lib` - 55 tests module LLM
- [x] `npm run test` - Tests frontend
- [x] Provider tests (mistral, ollama, manager)
- [x] Adapter tests (tool responses)

### Tests a Ajouter
- [ ] `test_estimate_tokens()` - utils.rs
- [ ] `test_simulate_streaming()` - utils.rs
- [ ] `test_retry_*()` - retry.rs (3 tests)
- [ ] `test_stream_chunk_with_tokens()` - streaming.rs
- [ ] `test_circuit_breaker_llm()` - circuit_breaker.rs

### Benchmarks

```bash
# Avant optimisation OPT-LLM-2 (HTTP client)
# Mesurer temps de completion sur 10 requests consecutives

# Apres optimisation
# Comparer temps total (attendu: -30% sur handshake)
```

## Estimation

| Optimisation | Effort | Impact | Priorite | Status |
|--------------|--------|--------|----------|--------|
| **OPT-LLM-8** | **1-2h** | **Critique** | **P0** | **DONE** |
| OPT-LLM-1 | 1h | Haut | P1 | DONE |
| OPT-LLM-2 | 2-3h | Haut | P1 | DONE |
| OPT-LLM-3 | 1-2h | Moyen | P1 | DONE |
| OPT-LLM-4 | 3-4h | Haut | P2 | DONE |
| OPT-LLM-5 | 3h | Moyen | P2 | DONE |
| OPT-LLM-6 | 4h | Moyen | P2 | DONE |
| OPT-LLM-7 | 2h | Faible | P3 | Differe |

**Total Critique (P0)**: 1-2h - Rate Limiter - COMPLETE
**Total Quick Wins (P1)**: 4-6h - COMPLETE
**Total Strategic (P2)**: 10-11h - COMPLETE
**Total Nice to Have (P3)**: 2h - Differe (OPT-LLM-7)

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression streaming | Faible | Eleve | Tests manuels e2e |
| Performance HTTP degradee | Faible | Moyen | Benchmarks avant/apres |
| Circuit breaker trop agressif | Moyen | Moyen | Config tunable |
| Breaking changes rig-core | Eleve | Eleve | Tester en staging |

## Prochaines Etapes

1. [x] Valider ce plan avec l'utilisateur
2. [x] **Executer OPT-LLM-8 (P0 CRITIQUE - Rate Limiter 1 req/s)** - commit c6bcc80
3. [x] Executer OPT-LLM-1 (quick win - estimate_tokens) - commit ad0551a
4. [x] Executer OPT-LLM-2 (quick win - HTTP client) - commit ea6541f
5. [x] Executer OPT-LLM-3 (quick win - streaming dedup) - commit 3f2fcb9
6. [x] Executer OPT-LLM-4 (retry mechanism) - commit 3a2eb0a
7. [x] Executer OPT-LLM-5 (token tracking) - commit 81a2225
8. [x] Executer OPT-LLM-6 (circuit breaker) - commit 1cc4dc2
9. [ ] Mesurer impact performance
10. [ ] Executer OPT-LLM-7 (HTTP error handling consolidation) - differe

## References

### Code Analyse
- `src-tauri/src/llm/` - Module LLM complet
- `src-tauri/src/commands/llm.rs` - Commandes Tauri
- `src-tauri/src/commands/streaming.rs` - Streaming events
- `src-tauri/Cargo.toml` - Dependencies

### Documentation Consultee
- `CLAUDE.md` - Contraintes projet
- `docs/ARCHITECTURE_DECISIONS.md` - Decisions LLM
- `docs/TECH_STACK.md` - Versions stack
- `docs/API_REFERENCE.md` - Commandes LLM

### Sources Externes
- https://rig.rs/ - Rig.rs documentation
- https://raymondclanan.com/blog/rust-llm-streaming-bridge/ - Streaming patterns
- https://github.com/neul-labs/fast-litellm - Performance patterns
- https://tokio.rs/tokio/tutorial/streams - Tokio streaming
- https://github.com/ivangabriele/mistralai-client-rs - Mistral integration
