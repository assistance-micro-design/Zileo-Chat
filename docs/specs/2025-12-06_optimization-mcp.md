# Plan d'Optimisation - MCP (Model Context Protocol)

## Metadata
- **Date**: 2025-12-06
- **Domaine**: mcp
- **Stack**: Rust 1.91.1 + Tauri 2.9.4 + Tokio 1.48.0 + MCP Protocol 2025-06-18
- **Impact estime**: Performance / Fiabilite / Observabilite

## Resume Executif

Le systeme MCP de Zileo-Chat-3 comprend **5,567 lignes** de code bien structure avec une architecture en couches (Manager -> Client -> Handle) supportant deux transports (stdio, HTTP/SSE). L'analyse revele des opportunites d'optimisation dans le caching, le pooling de connexions, et l'observabilite. Aucune vulnerabilite critique, mais des gains de performance et de fiabilite significatifs sont realisables avec des changements a faible risque.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src-tauri/src/mcp/manager.rs` | 911 | Haute | HashMap keyed par NAME, O(n) lookups par ID, get_saved_configs 120 lignes |
| `src-tauri/src/mcp/server_handle.rs` | 813 | Haute | Blocking I/O reads (BufReader::read_line) |
| `src-tauri/src/mcp/http_handle.rs` | 702 | Haute | Drop incomplet, pas de connection pooling |
| `src-tauri/src/mcp/protocol.rs` | 650 | Moyenne | JSON-RPC 2.0 conforme |
| `src-tauri/src/mcp/client.rs` | 508 | Moyenne | set_auto_reconnect() non implemente |
| `src-tauri/src/mcp/error.rs` | 312 | Basse | Error types complets |
| `src-tauri/src/mcp/mod.rs` | 104 | Basse | Documentation architecture |

### Patterns Identifies

- **Pattern 1**: Architecture en couches (Manager -> Client -> Handle) - Fichiers: tous MCP
- **Pattern 2**: RwLock avec scopes corrects - Fichier: `manager.rs`
- **Pattern 3**: Drop impl pour cleanup processus - Fichiers: `server_handle.rs`, `http_handle.rs`
- **Pattern 4**: Dual transport abstraction (stdio/HTTP) - Fichiers: `client.rs`, handles

### Problemes Detectes

1. **O(n) Lookups**: `stop_server()`, `get_server()`, `restart_server()` font O(n) scan car HashMap keyed par NAME mais operations acceptent ID (`manager.rs:249,292,814`)

2. **Duplicate Lock Checks**: Verification unicite serveur faite 2x (`manager.rs:171-176` et `manager.rs:199-206`)

3. **Parsing Manuel Complexe**: `get_saved_configs()` fait 120 lignes de parsing string->enum (`manager.rs:660-780`)

4. **HTTP Drop Incomplet**: Connection non fermee proprement (`http_handle.rs:624-634`)

5. **Pas de Caching**: Tool discovery refait a chaque appel sans cache

6. **Pas de Circuit Breaker**: Erreurs cascadent si serveur down

### Metriques Actuelles

- **Couverture tests**: ~60% (mostly happy paths)
- **Latency tracking**: duration_ms basique seulement
- **Connection reuse**: Aucune (nouvelle connexion par requete HTTP)

## Best Practices (2024-2025)

### Sources Consultees

- [MCP Specification 2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18) - Novembre 2024
- [7 MCP Server Best Practices - MarkTechPost](https://www.marktechpost.com/2025/07/23/7-mcp-server-best-practices-for-scalable-ai-integrations-in-2025/) - Juillet 2025
- [Code Execution with MCP - Anthropic](https://www.anthropic.com/engineering/code-execution-with-mcp) - 2025
- [MCP Security Risks - Red Hat](https://www.redhat.com/en/blog/model-context-protocol-mcp-understanding-security-risks-and-controls) - 2025
- [MCP Server Lifecycle - EmergentMind](https://www.emergentmind.com/topics/mcp-server-lifecycle) - 2025

### Patterns Recommandes

1. **Progressive Tool Loading**: Presenter tools comme APIs explorable on-demand au lieu de charger tous upfront (reduit token costs)

2. **Circuit Breaker**: 3 failures -> cooldown 60s, evite cascade errors sur serveurs instables

3. **Logging Verbeux**: Reduit MTTR de 40% selon etudes

4. **Tool Definition Caching**: Cache tools avec TTL, invalider sur error

5. **Connection Pooling**: Reutiliser connexions HTTP/TLS pour performance

6. **Health Checks Periodiques**: Detection proactive pannes (interval 5min recommande)

### Anti-Patterns a Eviter

1. **Over-tooling**: Un endpoint API = un outil MCP (augmente complexite, reduit adoption)

2. **Upfront Loading All Tools**: Tous outils charges dans context = latence + couts tokens

3. **Silent Failures**: Defaults silencieux au lieu d'erreurs explicites

4. **No Validation Schema**: Inputs non valides avant envoi au serveur MCP

## Contraintes du Projet

- **Decision 1**: Server identification par NAME (pas ID) - Source: `docs/ARCHITECTURE_DECISIONS.md` Q11
  - HashMap<ServerName, MCPClient> est le pattern etabli
  - Agents referent servers par nom dans leur config

- **Decision 2**: Configuration statique v1 - Source: `docs/ARCHITECTURE_DECISIONS.md` Q18
  - Hot-reload prevu v2 si demande forte
  - Restart app requis apres modification config MCP

- **Decision 3**: User-controlled deployment - Source: `docs/ARCHITECTURE_DECISIONS.md` Q11
  - Pas de serveurs MCP bundled
  - Utilisateur choisit docker/npx/uvx/http

- **Decision 4**: 3-level error recovery - Source: `docs/ARCHITECTURE_DECISIONS.md` Q19
  - Niveau 1: Retry automatique (3x exponential backoff)
  - Niveau 2: Fallback (skip tool, continuer workflow)
  - Niveau 3: User decision (pause, choix retry/skip/abort)

## Plan d'Optimisation

### Quick Wins (Impact haut, Effort faible)

#### OPT-1: Tool Discovery Caching

- **Fichiers**: `src-tauri/src/mcp/manager.rs`
- **Changement**: Ajouter cache `HashMap<ServerName, (Vec<MCPTool>, Instant)>` avec TTL 1h
- **Benefice**: Reduit appels reseau, latence amelioree (cold ~200ms -> cached <50ms)
- **Risque regression**: Faible - Pattern additive, ne modifie pas logic existante
- **Validation**:
  - Test cache hit/miss
  - Test TTL expiry
  - Test invalidation on tool call error

```rust
// Proposition d'implementation
struct ToolCache {
    cache: HashMap<String, (Vec<MCPTool>, Instant)>,
    ttl: Duration,
}

impl ToolCache {
    fn get(&self, server: &str) -> Option<&Vec<MCPTool>> {
        self.cache.get(server)
            .filter(|(_, cached_at)| cached_at.elapsed() < self.ttl)
            .map(|(tools, _)| tools)
    }

    fn invalidate(&mut self, server: &str) {
        self.cache.remove(server);
    }
}
```

#### OPT-2: Latency Metrics (p50/p95/p99)

- **Fichiers**: `src-tauri/src/mcp/manager.rs`, `src-tauri/src/db/` (mcp_call_log queries)
- **Changement**: Stocker duration_ms deja fait, ajouter query percentiles
- **Benefice**: Observabilite production, identification bottlenecks
- **Risque regression**: Tres faible - Lecture seule
- **Validation**: Query retourne percentiles corrects sur donnees de test

```sql
-- Query SurrealDB pour percentiles
SELECT
    server_name,
    math::percentile(duration_ms, 0.50) AS p50,
    math::percentile(duration_ms, 0.95) AS p95,
    math::percentile(duration_ms, 0.99) AS p99,
    count() AS total_calls
FROM mcp_call_log
WHERE created_at > time::now() - 1h
GROUP BY server_name;
```

#### OPT-3: HTTP Connection Pooling

- **Fichiers**: `src-tauri/src/mcp/http_handle.rs`, `src-tauri/src/mcp/manager.rs`
- **Changement**: Partager single `reqwest::Client` entre tous HTTP handles
- **Benefice**: Reutilise connexions TCP/TLS, performance amelioree
- **Risque regression**: Faible - Configuration reqwest::Client
- **Validation**: Verifier connection reuse dans logs reqwest (enable debug)

```rust
// Dans MCPManager
pub struct MCPManager {
    clients: RwLock<HashMap<String, MCPClient>>,
    http_client: reqwest::Client,  // Shared client
    // ...
}

impl MCPManager {
    pub fn new() -> Self {
        let http_client = reqwest::Client::builder()
            .pool_max_idle_per_host(5)
            .pool_idle_timeout(Duration::from_secs(90))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");
        // ...
    }
}
```

#### OPT-4: Remove Duplicate Lock Checks

- **Fichiers**: `src-tauri/src/mcp/manager.rs:169-206`
- **Changement**: Supprimer verification unicite redondante dans `spawn_server_internal()`
- **Benefice**: Code plus propre, une acquisition lock en moins
- **Risque regression**: Tres faible - Meme comportement fonctionnel
- **Validation**: Tests manager existants passent

```rust
// AVANT (manager.rs:199-206)
pub async fn spawn_server_internal(&self, config: MCPServerConfig) -> MCPResult<()> {
    // Cette verification est deja faite dans spawn_server()
    let clients = self.clients.read().await;
    if clients.contains_key(&config.name) {
        return Err(MCPError::ServerAlreadyExists { name: config.name });
    }
    drop(clients);
    // ...
}

// APRES - Supprimer le check redondant
pub async fn spawn_server_internal(&self, config: MCPServerConfig) -> MCPResult<()> {
    // Caller (spawn_server) a deja verifie l'unicite
    // Proceder directement a la creation
    // ...
}
```

#### OPT-5: HTTP Handle Cleanup Fix

- **Fichiers**: `src-tauri/src/mcp/http_handle.rs:624-634`
- **Changement**: Spawn async cleanup task dans Drop
- **Benefice**: Evite connection leaks sur drop sans disconnect()
- **Risque regression**: Faible - Amelioration cleanup
- **Validation**: Test que connexions sont fermees apres drop

```rust
// AVANT
impl Drop for MCPHttpHandle {
    fn drop(&mut self) {
        if self.connected {
            // Note: Cannot do async cleanup in drop
            debug!("HTTP handle dropped without disconnect");
        }
    }
}

// APRES
impl Drop for MCPHttpHandle {
    fn drop(&mut self) {
        if self.connected {
            // Spawn blocking cleanup task
            let client = self.client.clone();
            let base_url = self.base_url.clone();
            tokio::task::spawn(async move {
                // Attempt graceful disconnect
                let _ = client
                    .post(format!("{}/disconnect", base_url))
                    .send()
                    .await;
            });
        }
    }
}
```

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-6: Circuit Breaker

- **Fichiers**: `src-tauri/src/mcp/manager.rs`, nouveau `src-tauri/src/mcp/circuit_breaker.rs`
- **Changement**: State machine (closed -> open -> half-open) avec seuils configurables
- **Phases**:
  1. Design state machine et config
  2. Implementation CircuitBreaker struct
  3. Integration dans MCPManager.call_tool()
  4. Configuration UI (optionnel)
- **Prerequis**: Aucun
- **Risque regression**: Moyen - Nouvelle logique dans path critique
- **Tests requis**:
  - State transitions (closed->open after N failures)
  - Half-open recovery
  - Cooldown timing
  - False positive prevention

```rust
pub struct CircuitBreaker {
    state: CircuitState,
    failure_count: u32,
    failure_threshold: u32,     // Default: 3
    cooldown: Duration,          // Default: 60s
    last_failure: Option<Instant>,
}

pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Rejecting requests
    HalfOpen,    // Testing recovery
}

impl CircuitBreaker {
    pub fn allow_request(&mut self) -> bool {
        match self.state {
            CircuitState::Closed => true,
            CircuitState::Open => {
                if self.last_failure.map(|t| t.elapsed() > self.cooldown).unwrap_or(true) {
                    self.state = CircuitState::HalfOpen;
                    true
                } else {
                    false
                }
            }
            CircuitState::HalfOpen => true,  // Allow one request to test
        }
    }

    pub fn record_success(&mut self) {
        self.failure_count = 0;
        self.state = CircuitState::Closed;
    }

    pub fn record_failure(&mut self) {
        self.failure_count += 1;
        self.last_failure = Some(Instant::now());
        if self.failure_count >= self.failure_threshold {
            self.state = CircuitState::Open;
        }
    }
}
```

#### OPT-7: ID Lookup Table

- **Fichiers**: `src-tauri/src/mcp/manager.rs`
- **Changement**: Ajouter `HashMap<ID, ServerName>` pour O(1) lookups
- **Benefice**: `stop_server()`, `get_server()`, `restart_server()` passent de O(n) a O(1)
- **Prerequis**: Aucun
- **Risque regression**: Moyen - Synchronisation deux HashMaps requise
- **Tests requis**:
  - Lookup consistency (ID trouve correct NAME)
  - Synchronization (insert/remove atomique)
  - Edge cases (duplicate IDs)

```rust
pub struct MCPManager {
    clients: RwLock<HashMap<String, MCPClient>>,  // Keyed by NAME
    id_to_name: RwLock<HashMap<String, String>>,  // ID -> NAME lookup
    // ...
}

impl MCPManager {
    // Dans spawn_server_internal, apres insertion:
    {
        let mut id_lookup = self.id_to_name.write().await;
        id_lookup.insert(config.id.clone(), config.name.clone());
    }

    // stop_server devient O(1):
    pub async fn stop_server(&self, id: &str) -> MCPResult<()> {
        let name = {
            let id_lookup = self.id_to_name.read().await;
            id_lookup.get(id).cloned()
                .ok_or_else(|| MCPError::ServerNotFound { name: id.to_string() })?
        };
        // Maintenant utiliser name pour remove du HashMap principal
    }
}
```

#### OPT-8: Health Checks Periodic

- **Fichiers**: `src-tauri/src/mcp/manager.rs`, nouveau background task
- **Changement**: Task async verifiant status serveurs toutes les 5min
- **Phases**:
  1. Definir health check protocol (ping/pong ou list_tools)
  2. Implementer background task avec tokio::spawn
  3. Update status dans MCPClient
  4. Emit events Tauri pour UI
- **Prerequis**: Aucun
- **Risque regression**: Faible - Pattern additive
- **Tests requis**:
  - Task scheduling (interval correct)
  - Status update propagation
  - Graceful shutdown

```rust
impl MCPManager {
    pub fn start_health_checks(self: Arc<Self>, interval: Duration) {
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                self.check_all_servers_health().await;
            }
        });
    }

    async fn check_all_servers_health(&self) {
        let clients = self.clients.read().await;
        for (name, client) in clients.iter() {
            match client.list_tools().await {
                Ok(_) => {
                    // Server healthy
                    debug!("Health check passed: {}", name);
                }
                Err(e) => {
                    // Server unhealthy
                    warn!("Health check failed for {}: {}", name, e);
                    // Update status, emit event
                }
            }
        }
    }
}
```

### Nice to Have (Impact faible, Effort faible)

#### OPT-9: Extract get_saved_configs() Helpers

- **Fichiers**: `src-tauri/src/mcp/manager.rs:660-780`
- **Changement**: Creer trait `FromSurrealRow` et helper `parse_enum()`
- **Benefice**: Maintenabilite amelioree, code plus lisible
- **Risque regression**: Faible
- **Tests requis**: Tests deserialization existants

```rust
// Helper pour parsing enum
fn parse_deployment_method(value: &serde_json::Value) -> Option<MCPDeploymentMethod> {
    value.as_str().and_then(|s| match s {
        "docker" => Some(MCPDeploymentMethod::Docker),
        "npx" => Some(MCPDeploymentMethod::Npx),
        "uvx" => Some(MCPDeploymentMethod::Uvx),
        "http" => Some(MCPDeploymentMethod::Http),
        _ => None,
    })
}

// Helper pour env JSON
fn parse_env_json(value: Option<&serde_json::Value>) -> HashMap<String, String> {
    value
        .and_then(|v| v.as_str())
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default()
}
```

#### OPT-10: Structured Error Types

- **Fichiers**: `src-tauri/src/mcp/error.rs`, call sites
- **Changement**: Ajouter enum `MCPErrorCategory` dans responses
- **Benefice**: Filtering/categorization logs facilite
- **Risque regression**: Faible
- **Tests requis**: Tests error formatting

```rust
#[derive(Debug, Clone, Serialize)]
pub enum MCPErrorCategory {
    Connection,     // Network, timeout
    Protocol,       // JSON-RPC, MCP spec
    ServerInternal, // Server-side errors
    Configuration,  // Invalid config
    ResourceNotFound,
}

impl MCPError {
    pub fn category(&self) -> MCPErrorCategory {
        match self {
            MCPError::ConnectionFailed { .. } => MCPErrorCategory::Connection,
            MCPError::Timeout { .. } => MCPErrorCategory::Connection,
            MCPError::InvalidResponse { .. } => MCPErrorCategory::Protocol,
            MCPError::ServerNotFound { .. } => MCPErrorCategory::ResourceNotFound,
            // ...
        }
    }
}
```

### Differe (Impact faible, Effort eleve)

| Optimisation | Justification |
|--------------|---------------|
| **Async I/O (tokio::process)** | Effort HIGH, risque regression moyen. Blocking I/O actuel acceptable avec spawn_blocking wrapper. |
| **Auto-reconnect Implementation** | Effort HIGH, use case rare (connexions locales stables). Documenter comme limitation v1. |
| **Extract Long Functions** | Refactoring pur sans impact fonctionnel. Faire au fil des autres optimisations. |

## Dependencies

### Mises a Jour Recommandees

| Package/Crate | Actuel | Recommande | Breaking Changes |
|---------------|--------|------------|------------------|
| rig-core | 0.24.0 | 0.26.0+ | Possible - Evaluer changelog |
| tokio | 1.48.0 | 1.48.0 | Aucun - Version optimale |
| reqwest | 0.12 | 0.12 | Aucun - Version actuelle |
| serde | 1.0.228 | 1.0.228 | Aucun - Stable |

### Nouvelles Dependencies (si justifie)

*Aucune nouvelle dependance requise pour ces optimisations.*

## Verification Non-Regression

### Tests Existants

- [x] `cargo test --lib` (src-tauri) - ~35 tests MCP couvrent le domaine
- [x] `npm run test` (frontend) - Tests UI MCP settings

### Tests a Ajouter

- [ ] Test OPT-1: Cache tool discovery (hit, miss, TTL, invalidation)
- [ ] Test OPT-2: Query percentiles retourne valeurs correctes
- [ ] Test OPT-3: HTTP connection reuse (mock reqwest)
- [ ] Test OPT-6: Circuit breaker state transitions
- [ ] Test OPT-7: ID lookup consistency
- [ ] Test OPT-8: Health check scheduling

### Benchmarks (si applicable)

```bash
# Avant optimisation
# Tool discovery latency (cold)
time curl -X POST localhost:PORT/mcp/servers/Serena/tools

# Apres optimisation (OPT-1 cache)
# Second call should be <50ms
time curl -X POST localhost:PORT/mcp/servers/Serena/tools
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-1 (Tool Caching) | 1h | Haut | P1 |
| OPT-2 (Metrics) | 0.5h | Haut | P1 |
| OPT-3 (HTTP Pool) | 1h | Moyen | P1 |
| OPT-4 (Dup Locks) | 0.25h | Faible | P1 |
| OPT-5 (HTTP Drop) | 0.5h | Moyen | P1 |
| OPT-6 (Circuit Breaker) | 4h | Haut | P2 |
| OPT-7 (ID Lookup) | 2h | Haut | P2 |
| OPT-8 (Health Checks) | 2h | Haut | P2 |
| OPT-9 (Extract Helpers) | 2h | Moyen | P3 |
| OPT-10 (Error Types) | 1.5h | Moyen | P3 |

**Total estime**: ~14.75h
- Phase 1 (Quick Wins P1): 3.25h
- Phase 2 (Strategic P2): 8h
- Phase 3 (Nice to Have P3): 3.5h

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Cache stale (OPT-1) | Moyenne | Moyen | TTL court (1h) + invalidation on error |
| Pool exhaustion (OPT-3) | Faible | Moyen | Configure pool limits (5 per host) |
| Circuit breaker false positive (OPT-6) | Moyenne | Moyen | Seuils configurables, half-open recovery |
| ID/Name desync (OPT-7) | Faible | Haute | Operations atomiques, tests sync |
| Health check overhead (OPT-8) | Faible | Faible | Interval configurable (default 5min) |

## Prochaines Etapes

1. [ ] Valider ce plan avec l'utilisateur
2. [ ] Executer OPT-4 (quick win, 15min)
3. [ ] Executer OPT-2 (quick win, 30min)
4. [ ] Executer OPT-1 (quick win, 1h)
5. [ ] Executer OPT-3 (quick win, 1h)
6. [ ] Executer OPT-5 (quick win, 30min)
7. [ ] Mesurer impact des quick wins
8. [ ] Planifier OPT-6, OPT-7, OPT-8 (strategic)

## References

### Code Analyse
- `src-tauri/src/mcp/manager.rs` (911 lines)
- `src-tauri/src/mcp/server_handle.rs` (813 lines)
- `src-tauri/src/mcp/http_handle.rs` (702 lines)
- `src-tauri/src/mcp/protocol.rs` (650 lines)
- `src-tauri/src/mcp/client.rs` (508 lines)
- `src-tauri/src/mcp/error.rs` (312 lines)

### Documentation Consultee
- `docs/ARCHITECTURE_DECISIONS.md`
- `docs/MCP_CONFIGURATION_GUIDE.md`
- `docs/API_REFERENCE.md`
- `CLAUDE.md`

### Sources Externes
- [MCP Specification 2025-06-18](https://modelcontextprotocol.io/specification/2025-06-18)
- [MCP Best Practices - MarkTechPost](https://www.marktechpost.com/2025/07/23/7-mcp-server-best-practices-for-scalable-ai-integrations-in-2025/)
- [Code Execution with MCP - Anthropic](https://www.anthropic.com/engineering/code-execution-with-mcp)
- [MCP Security Risks - Red Hat](https://www.redhat.com/en/blog/model-context-protocol-mcp-understanding-security-risks-and-controls)
- [Tokio 1.48.0 Release Notes](https://github.com/tokio-rs/tokio/releases/tag/tokio-1.48.0)
