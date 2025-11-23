# Spécifications Multi-Provider LLM

> Guide de référence pour architecture agnostique multi-provider
> Focus: particularités paramétriques et formats spécifiques
> **Providers Phase 1** : Mistral + Ollama

## Compatibilité API

### OpenAI-Compatible
**Providers**: GPT, Mistral, DeepSeek, xAI Grok, Ollama, OpenRouter
**Format**: Endpoints `/chat/completions` standardisés
**Avantage**: Migration simplifiée, SDKs interchangeables

### Format Propriétaire
**Claude**: API Anthropic spécifique avec structure événementielle distincte
**Gemini**: Format Google avec `?alt=sse` pour streaming

## Paramètres d'Inférence

| Provider | Temperature | Top_P | Top_K | Contrainte |
|----------|------------|-------|-------|------------|
| Claude | 0.0-1.0 (défaut: 1.0) | 0.0-1.0 | ✗ | **Exclusif**: temp OU top_p |
| GPT | 0.0-2.0 | 0.0-1.0 | ✗ | Combinable |
| Gemini | 0.0-2.0 | 0.0-1.0 | 1-40+ | Combinable |
| Mistral | 0.0-0.7 recommandé | ✓ | ✗ | Combinable |
| DeepSeek | 0.0-2.0 | ✓ | ✓ | OpenAI-compatible |
| xAI Grok | 0.0-2.0 | ✓ | ✗ | OpenAI-compatible |
| Ollama | ✓ | ✓ | ✓ | Dépend modèle |

### Recommandations Usage
- **Analytique/Déterministe**: temp ≈ 0.0-0.3
- **Créatif/Génératif**: temp ≈ 0.7-1.0
- **Claude**: Modifier uniquement `temperature`, laisser autres par défaut

## Fenêtres de Contexte

| Provider | Contexte Max | Modèle | Notes |
|----------|--------------|--------|-------|
| Claude | 200K | Opus/Sonnet 4 | 1M tokens (versions futures) |
| GPT | 1M | GPT-4.1 | 128K pour GPT-4 |
| Gemini | 2M | Gemini 3.0 Pro | Pricing différencié </>200K |
| Mistral | 128K | Large/Medium | 256K pour Codestral |
| DeepSeek | 64K-128K | V3 | Architecture MoE efficiente |
| xAI Grok | 128K | Grok 4 | Recherche temps réel |
| Ollama | 2K-128K | Varie | Configurable `num_ctx` |

**Règle tokens**: `prompt_tokens + max_tokens ≤ context_length`

## Streaming SSE

### Structure Claude
```
event: message_start → content_block_start → content_block_delta
→ content_block_stop → message_delta → message_stop
```
- **Events nommés**: Chaque type avec `event:` SSE
- **Tokens usage**: Distribués incrémentalement

### Structure GPT/OpenAI-Compatible
```
Chunks avec delta objects → [DONE]
```
- **Complétion**: Signal `[DONE]` final
- **Tokens usage**: Dernier chunk (avec `stream_options: {include_usage: true}`)

### Structure Gemini
```
streamGenerateContent?alt=sse → candidates[] → responseId
```
- **Chunks larges**: Taille supérieure autres providers
- **WebSocket**: Live API pour bidirectionnel temps réel

### Mistral
```
data-only SSE → data: [DONE]
```
- **Compatible OpenAI**: Format similaire GPT

## Calcul de Tokens

| Provider | Input Field | Output Field | Vitesse Calcul |
|----------|-------------|--------------|----------------|
| Claude | `input_tokens` | `output_tokens` | Incrémental stream |
| GPT | `prompt_tokens` | `completion_tokens` | Dernier chunk |
| Gemini | `promptTokenCount` | `candidatesTokenCount` | Par chunk |
| Mistral | Comptés séparément | Facturés séparément | Standard |
| DeepSeek | OpenAI-compatible | OpenAI-compatible | Standard |
| Ollama | `prompt_eval_count` | `eval_count` | Token/s: `eval_count / eval_duration * 10^9` |

### Token Counting Tools
- **Tokuin** (Rust): Comparaison cross-provider, support rôles, minification markdown
- **Provider natif**: Chaque SDK expose tokenizer spécifique

## Capacités Spécifiques

### Reasoning
| Provider | Mode | Activation |
|----------|------|------------|
| Claude | Extended Thinking | Paramètre dédié |
| GPT | Reasoning models | GPT-o1/o3 |
| xAI Grok | Reasoning Effort | `grok-3-mini` uniquement, contrôle niveau |
| DeepSeek | MoE Architecture | 671B params, 37B actifs |

### Features Uniques
**Claude**: Tool use natif robuste, IDE integrations
**GPT**: Multimodal (vision, audio temps réel 320ms)
**Gemini**: Multimodal natif, vidéo, Google services
**xAI Grok**: Live Search (web/X temps réel), modes: off/auto/on
**Mistral**: `safe_prompt` injection optionnelle, structured JSON
**Ollama**: Exécution locale, privacy-first, quantized models
**OpenRouter**: Routing intelligent, parameter transformation auto

## Paramètres Provider-Spécifiques

### Claude
- **Contrainte majeure**: `temperature` XOR `top_p` (jamais les deux)
- `max_tokens`: Requis explicitement

### Gemini
- `top_k`: Support natif (1-40+)
- Pricing contextuel: x2 si prompt >200K tokens

### Mistral
- `safe_prompt`: Bool, injection sécurité pré-conversation
- `frequency_penalty`: Pénalise répétitions fréquence-based

### xAI Grok
- `search`: off | auto | on (recherche temps réel)
- `reasoning_effort`: Contrôle profondeur raisonnement

### Ollama
- `num_predict`: Max tokens (-1=infini, -2=remplir contexte)
- `num_ctx`: Taille fenêtre contexte (défaut: 2048)

### OpenRouter
- `require_parameters`: true → Exclusion providers incompatibles
- `provider_routing`: Stratégie sélection automatique
- Transformation automatique paramètres non-OpenAI

## Architecture Abstraction

### Rig.rs Multi-Provider
**Interface unifiée**: API cohérente tous providers
**Providers supportés**: Anthropic, OpenAI, Google, extensible
**Switch**: Configuration uniquement, logique inchangée

### MCP Layer
**Standardisation**: Protocol unifié indépendant LLM backend
**Tools System**: Exposition outils custom cross-provider
**Resources**: Gestion contexte standardisée
**Sampling**: Délégation génération unifiée

### Transport
- **Stdio**: Inter-process local (Tauri)
- **HTTP/SSE**: APIs cloud, streaming
- **WebSocket**: Bidirectionnel temps réel (Gemini Live)

## Gestion Différences

### Pattern Factory
```rust
match provider {
    Provider::Claude => enforce_temp_or_top_p(),
    Provider::Gemini => apply_context_pricing(),
    Provider::Grok => enable_live_search(),
    Provider::Ollama => set_local_context_window(),
    _ => default_openai_compatible()
}
```

### Normalisation Tokens
```rust
fn normalize_token_count(response: ProviderResponse) -> TokenUsage {
    match response.provider {
        Claude => TokenUsage {
            input: response.input_tokens,
            output: response.output_tokens
        },
        GPT => TokenUsage {
            input: response.prompt_tokens,
            output: response.completion_tokens
        },
        Ollama => TokenUsage {
            input: response.prompt_eval_count,
            output: response.eval_count
        },
        // ...
    }
}
```

### Validation Paramètres
```rust
impl ValidateParams for ClaudeProvider {
    fn validate(&self, params: &InferenceParams) -> Result<()> {
        if params.temperature.is_some() && params.top_p.is_some() {
            Err("Claude: exclusif temperature ou top_p")
        }
        // ...
    }
}
```

## Configuration Unifiée

### Structure
```rust
struct ProviderConfig {
    api_key: SecretString,
    model: String,
    context_window: usize,

    // Paramètres normalisés
    default_temperature: f32,
    supports_top_p_with_temp: bool,
    supports_top_k: bool,

    // Spécifiques
    custom_params: HashMap<String, Value>,

    // Fallback
    fallback_providers: Vec<Provider>,
}
```

### Exemple Multi-Provider
```rust
let configs = vec![
    ProviderConfig::claude("sonnet-4", 200_000)
        .temp_only()
        .tools_native(),

    ProviderConfig::gpt("gpt-4.1", 1_000_000)
        .multimodal()
        .voice_realtime(),

    ProviderConfig::gemini("2.5-pro", 2_000_000)
        .context_pricing_aware()
        .video_native(),

    ProviderConfig::ollama("llama3", 128_000)
        .local_execution()
        .privacy_first(),
];

let router = LLMRouter::new(configs).with_fallback_cascade();
```

## Pricing (par 1M tokens)

| Provider | Input | Output | Notes |
|----------|-------|--------|-------|
| Claude 4 | $3-15 | $15-75 | Opus premium |
| GPT-4.1 | $2 | $8 | -26% vs GPT-4o |
| Gemini 2.5 | $1.25-2.50 | $5-10 | Context-aware |
| Mistral Large | ~$2-4 | ~$6-12 | Compétitif EU |
| DeepSeek V3 | $0.27 | $1.10 | MoE efficient |
| Grok 4 Fast | $0.20 | $0.50 | Low-cost leader |
| Ollama | $0 | $0 | Local, pas API |

## Monitoring Multi-Provider

### Métriques Essentielles
- **Latence par provider**: P50, P95, P99
- **Token usage**: Input/output séparés
- **Taux erreur**: Par provider et endpoint
- **Coût**: Tracking temps réel avec budgets
- **Throughput**: Tokens/sec par provider

### Observabilité
```rust
struct ProviderMetrics {
    latency_ms: Histogram,
    tokens_consumed: Counter,
    error_rate: Gauge,
    cost_usd: Counter,
    active_streams: Gauge,
}
```

## Roadmap Implémentation

### Phase 1: Foundation (Actuelle)
- MCP Rust SDK officiel
- Rig.rs avec 2 providers : **Mistral + Ollama**
- Abstraction paramètres de base
- Validation provider-spécifique
- Configuration via UI Settings (pas .env)

### Phase 2: Expansion (Future)
- +4 providers optionnels (Claude, GPT, Gemini, Grok)
- Routing automatique avec fallback
- Token counting normalisé
- Streaming unifié tous providers

### Phase 3: Optimisation
- Caching intelligent cross-provider
- Cost optimization routing
- Performance monitoring complet
- Provider health checks

### Phase 4: Production
- A/B testing providers
- Auto-scaling par charge
- Anomaly detection
- Compliance & audit trail

## Références

**SDKs**:
- Rig.rs: rig.rs
- MCP Rust: github.com/modelcontextprotocol/rust-sdk
- Tokuin: github.com/nooscraft/tokuin

**APIs**:
- Claude: docs.anthropic.com
- GPT: platform.openai.com
- Gemini: ai.google.dev
- Mistral: docs.mistral.ai
- xAI: docs.x.ai
- DeepSeek: api-docs.deepseek.com
- Ollama: ollama.readthedocs.io
- OpenRouter: openrouter.ai/docs
