# Plan d'Optimisation - Sub-Agent Tools

## Metadata
- **Date**: 2025-12-09
- **Domaine**: tools/Sub-Agent Tools
- **Stack**: Rust 1.91.1 + Tauri 2.9.3 + tokio 1.48.0 + futures 0.3.31 + async-trait 0.1.89
- **Impact estime**: Performance + Maintenabilite + Robustesse

## Resume Executif

Ce plan vise a ameliorer les Sub-Agent Tools (SpawnAgentTool, DelegateTaskTool, ParallelTasksTool) sans ajouter de nouvelles fonctionnalites. Les optimisations ciblent:

1. **Inactivity Timeout avec Heartbeat** (P2): Detecte les vrais hangs sans couper les executions longues legitimes - le timeout se reset a chaque activite (token LLM, tool call, reponse MCP)
2. **Reduction duplication** (~25%): Unifier event emission et update_execution_record
3. **Centralisation constantes**: Magic numbers dans constants.rs
4. **Patterns de resilience**: Circuit breaker, CancellationToken

Gains attendus: robustesse accrue (detection hangs intelligente), maintenabilite amelioree, meilleure observabilite.

## Etat Actuel

### Analyse du Code

| Fichier | Lignes | Complexite | Points d'attention |
|---------|--------|------------|-------------------|
| `src-tauri/src/tools/spawn_agent.rs` | 854 | CC~18 (Elevee) | 20 etapes orchestrees dans spawn() |
| `src-tauri/src/tools/delegate_task.rs` | 797 | CC~16 (Elevee) | Similar a spawn, ~25% duplication |
| `src-tauri/src/tools/parallel_tasks.rs` | 880 | CC~20 (Tres elevee) | execute_batch() trop complexe |
| `src-tauri/src/tools/sub_agent_executor.rs` | 542 | CC~2-3/methode | Bien structure, couche d'abstraction |
| `src-tauri/src/tools/validation_helper.rs` | 516 | CC~5 | Magic numbers non centralises |
| `src-tauri/src/tools/registry.rs` | 252 | CC~3 | sub_agent_tools() bien isole |

**Total**: 3,841 lignes de code pour le systeme Sub-Agent Tools.

### Patterns Identifies

- **Permission Check**: 3 copies identiques (spawn:197, delegate:186, parallel:176)
- **Limit Check**: 3 copies identiques (spawn:200, delegate:195, parallel:185)
- **Event Emission**: 3 implementations (executor:380-390, parallel:148-158, delegate:162-172)
- **Execution Record Update**: 2 implementations (executor:328-373, parallel:469-507)
- **Validation MCP Servers**: 3 copies quasi-identiques (spawn:248-261, delegate:246-257, parallel:221-238)

### Metriques Actuelles

```
Tests unitaires: 34 (8+9+9+8)
Coverage estimee: ~60% (fonctions principales)
Tests integration: 0
Magic numbers: 4 non centralises (200, 500, 60, 100)
```

## Best Practices (2024-2025)

### Sources Consultees
- [Tokio Timeout Patterns](https://www.slingacademy.com/article/handling-timeouts-in-rust-with-async-and-tokio-timers/)
- [Tokio CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
- [Galileo Multi-Agent Failure Recovery](https://galileo.ai/blog/multi-agent-ai-system-failure-recovery)
- [Anthropic Building Effective Agents](https://www.anthropic.com/research/building-effective-agents)
- [Databricks Agent Design Patterns](https://docs.databricks.com/aws/en/generative-ai/guide/agent-system-design-patterns)
- [OneSignal Rust Concurrency Patterns](https://onesignal.com/blog/rust-concurrency-patterns/)
- [developerlife.com Cancellation Safety](https://developerlife.com/2024/07/10/rust-async-cancellation-safety-tokio/)

### Patterns Recommandes

1. **Inactivity Timeout avec Heartbeat**: Timeout qui se reset a chaque activite (token, tool call) - permet executions longues actives, detecte vrais hangs
2. **CancellationToken**: Pour shutdown graceful coordonne entre tasks
3. **Circuit Breaker**: Isoler les echecs repetitifs (pattern deja applique pour MCP)
4. **JoinSet vs join_all**: `tokio::task::JoinSet` offre meilleur controle par tache
5. **Centralisation Constantes**: Eviter magic numbers disperses

### Anti-Patterns a Eviter

1. **Timeout fixe sur operations longues**: Coupe les executions legitimes (ex: analyse complexe)
2. **Duplication de validation**: Augmente surface de bugs
3. **Magic numbers**: Difficile a maintenir et modifier
4. **CC > 15**: Fonctions trop complexes, difficiles a tester

## Contraintes du Projet

**Decisions existantes a respecter** (Source: `docs/MULTI_AGENT_ARCHITECTURE.md`, `CLAUDE.md`):

| Contrainte | Description | Raison |
|------------|-------------|--------|
| MAX_SUB_AGENTS = 3 | Limite stricte par workflow | Controle orchestration, evite recursion |
| is_primary_agent | Seul l'agent principal peut spawner | Hierarchie uniforme, single-level |
| "Prompt In, Report Out" | Pas de contexte partage | Isolation, simplicite debugging |
| SubAgentStatus enum | State machine pour lifecycle | Transitions explicites, auditabilite |
| Markdown reports | Format de communication | Human-readable, machine-parsable |

**Ces contraintes sont NON NEGOCIABLES.**

## Plan d'Optimisation

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-SA-1: Inactivity Timeout avec Heartbeat

- **Fichiers**:
  - `src-tauri/src/tools/sub_agent_executor.rs` - Ajouter monitoring d'activite
  - `src-tauri/src/agents/core/orchestrator.rs` - Ajouter callback on_activity
  - `src-tauri/src/agents/llm_agent.rs` - Propager callback aux tool calls
- **Changement**: Timeout intelligent qui se reset a chaque activite (token recu, tool call)
- **Benefice**: Detecte vrais hangs SANS couper les executions longues legitimes
- **Risque regression**: Moyen (modification du pipeline d'execution)
- **Validation**: Tests avec execution longue active + test avec silence complet

**Principe du Heartbeat**:
```
Execution demarre
    |
    v
[Activite detectee?] --OUI--> Reset compteur inactivite (300s)
    |                              |
    NON                            v
    |                         Continue execution
    v                              |
[Inactivite > 300s?] --OUI--> ABORT (vraiment bloque)
    |
    NON
    v
Continue monitoring...
```

**Ce qui compte comme "activite"**:
| Activite | Reset timeout? | Raison |
|----------|----------------|--------|
| Token recu du LLM | Oui | LLM repond, pas bloque |
| Tool call demarre | Oui | Agent travaille |
| Tool call termine | Oui | Progres reel |
| MCP server repond | Oui | Communication active |
| **Silence complet** | Non | Potentiellement bloque |

**Implementation**:

```rust
// constants.rs - Nouvelles constantes
pub mod sub_agent {
    pub const MAX_SUB_AGENTS: usize = 3;
    pub const INACTIVITY_TIMEOUT_SECS: u64 = 300;  // 5 min sans activite
    pub const ACTIVITY_CHECK_INTERVAL_SECS: u64 = 30;  // Verification toutes les 30s
}

// sub_agent_executor.rs
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::Instant;

pub struct ActivityMonitor {
    last_activity: Arc<RwLock<Instant>>,
}

impl ActivityMonitor {
    pub fn new() -> Self {
        Self {
            last_activity: Arc::new(RwLock::new(Instant::now())),
        }
    }

    pub fn record_activity(&self) {
        if let Ok(mut last) = self.last_activity.try_write() {
            *last = Instant::now();
        }
    }

    pub fn seconds_since_last_activity(&self) -> u64 {
        self.last_activity
            .try_read()
            .map(|last| last.elapsed().as_secs())
            .unwrap_or(0)
    }
}

impl SubAgentExecutor {
    pub async fn execute_with_heartbeat_timeout(
        &self,
        agent_id: &str,
        task: Task,
    ) -> ExecutionResult {
        let monitor = Arc::new(ActivityMonitor::new());
        let monitor_clone = monitor.clone();

        // Callback pour signaler activite
        let on_activity = move || {
            monitor_clone.record_activity();
        };

        // Future d'execution avec monitoring
        let execution = self.orchestrator.execute_with_mcp_monitored(
            agent_id,
            task,
            self.mcp_manager.clone(),
            Box::new(on_activity),
        );

        tokio::pin!(execution);

        // Boucle de monitoring
        loop {
            tokio::select! {
                result = &mut execution => {
                    // Execution terminee normalement
                    return self.process_execution_result(result);
                }
                _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
                    let inactive_secs = monitor.seconds_since_last_activity();
                    if inactive_secs > INACTIVITY_TIMEOUT_SECS {
                        warn!(
                            agent_id = %agent_id,
                            inactive_secs = %inactive_secs,
                            "Sub-agent execution timed out due to inactivity"
                        );
                        return ExecutionResult {
                            success: false,
                            report: String::new(),
                            metrics: SubAgentMetrics::default(),
                            error_message: Some(format!(
                                "Sub-agent inactive for {} seconds (threshold: {}s). \
                                 Execution aborted to prevent hang.",
                                inactive_secs,
                                INACTIVITY_TIMEOUT_SECS
                            )),
                        };
                    }
                    // Optionnel: emettre heartbeat event pour UI
                    debug!(
                        agent_id = %agent_id,
                        last_activity_secs_ago = %inactive_secs,
                        "Sub-agent still active"
                    );
                }
            }
        }
    }
}
```

**Modifications requises dans orchestrator.rs**:
```rust
// Nouvelle signature avec callback
pub async fn execute_with_mcp_monitored(
    &self,
    agent_id: &str,
    task: Task,
    mcp_manager: Option<Arc<MCPManager>>,
    on_activity: Box<dyn Fn() + Send + Sync>,
) -> anyhow::Result<Report> {
    // Appeler on_activity() a chaque:
    // - Token recu du stream LLM
    // - Avant/apres chaque tool call
    // - Reception reponse MCP
}
```

- **Phases**:
  1. Creer `ActivityMonitor` struct (0.5h)
  2. Ajouter `on_activity` callback a `execute_with_mcp()` (1.5h)
  3. Propager callback dans LLMAgent pour tokens et tools (1h)
  4. Implementer boucle monitoring dans SubAgentExecutor (0.5h)
  5. Tests (0.5h)
- **Effort total**: 4h (deplace de P1 a P2)
- **Prerequis**: Aucun
- **Tests requis**:
  - Test execution longue avec activite continue -> OK
  - Test silence complet > 300s -> timeout
  - Test activite intermittente -> OK

### Quick Wins (Impact haut, Effort faible)

#### OPT-SA-2: Mise a jour once_cell 1.19 -> 1.20

- **Fichiers**: `src-tauri/Cargo.toml`
- **Changement**: `once_cell = "1.20"` (actuellement "1.19")
- **Benefice**: Performance amelioree, MSRV reduit
- **Risque regression**: Quasi-nul (100% compatible)
- **Validation**: `cargo build --release`

#### OPT-SA-3: Centraliser Magic Numbers

- **Fichiers**: `src-tauri/src/tools/constants.rs`
- **Changement**: Ajouter constantes pour Sub-Agent Tools

```rust
pub mod sub_agent {
    pub const MAX_SUB_AGENTS: usize = 3;

    // NEW: Ajouter ces constantes
    pub const RESULT_SUMMARY_MAX_CHARS: usize = 200;
    pub const TASK_DESC_TRUNCATE_CHARS: usize = 100;
    pub const EXECUTION_TIMEOUT_SECS: u64 = 300;
    pub const VALIDATION_POLL_MS: u64 = 500;
    pub const VALIDATION_TIMEOUT_SECS: u64 = 60;
}
```

- **Fichiers a mettre a jour**:
  - `spawn_agent.rs:334,448` - Utiliser `RESULT_SUMMARY_MAX_CHARS`, `TASK_DESC_TRUNCATE_CHARS`
  - `validation_helper.rs:50,53` - Utiliser `VALIDATION_TIMEOUT_SECS`, `VALIDATION_POLL_MS`
  - `sub_agent_executor.rs` - Utiliser `EXECUTION_TIMEOUT_SECS`
- **Benefice**: Maintenabilite, modification centralisee
- **Risque regression**: Quasi-nul
- **Validation**: `cargo test`

### Optimisations Efficaces (Impact moyen, Effort faible)

#### OPT-SA-4: Unifier Event Emission dans SubAgentExecutor

- **Fichiers**:
  - `src-tauri/src/tools/sub_agent_executor.rs` (deja a `emit_event()`)
  - `src-tauri/src/tools/parallel_tasks.rs:148-158` (supprimer)
  - `src-tauri/src/tools/delegate_task.rs:162-172` (supprimer)
- **Changement**: ParallelTasksTool et DelegateTaskTool utilisent `SubAgentExecutor::emit_event()` au lieu de leurs propres implementations
- **Benefice**: Reduction duplication, comportement uniforme
- **Risque regression**: Faible
- **Validation**: Tests d'emission existants

#### OPT-SA-5: Unifier update_execution_record()

- **Fichiers**:
  - `src-tauri/src/tools/sub_agent_executor.rs:328-373` (garder)
  - `src-tauri/src/tools/parallel_tasks.rs:469-507` (supprimer, utiliser executor)
- **Changement**: ParallelTasksTool appelle `executor.update_execution_record()` au lieu de sa propre methode
- **Benefice**: Single source of truth, moins de bugs potentiels
- **Risque regression**: Faible
- **Validation**: Tests de persistence

#### OPT-SA-6: Migrer ParallelTasksTool vers JoinSet

- **Fichiers**: `src-tauri/src/tools/parallel_tasks.rs:325-326`
- **Changement**: Remplacer `orchestrator.execute_parallel()` par `tokio::task::JoinSet`

```rust
// Avant
let results = self.orchestrator.execute_parallel(orchestrator_tasks).await;

// Apres
use tokio::task::JoinSet;

let mut join_set = JoinSet::new();
for (task, agent_id) in orchestrator_tasks {
    let orch = self.orchestrator.clone();
    let mcp = self.mcp_manager.clone();
    join_set.spawn(async move {
        orch.execute_with_mcp(&agent_id, task, mcp).await
    });
}
let results = join_set.join_all().await;
```

- **Benefice**: Meilleur controle par tache, cancellation granulaire
- **Risque regression**: Faible (pattern equivalent)
- **Validation**: Tests paralleles existants

### Optimisations Strategiques (Impact haut, Effort eleve)

#### OPT-SA-7: CancellationToken pour Shutdown Graceful

- **Fichiers**:
  - `src-tauri/src/tools/sub_agent_executor.rs` - Ajouter champ `cancellation_token`
  - `src-tauri/src/commands/streaming.rs` - Implementer `cancel_workflow_streaming`
- **Changement**:

```rust
// sub_agent_executor.rs
use tokio_util::sync::CancellationToken;

pub struct SubAgentExecutor {
    // ... existing fields
    cancellation_token: CancellationToken,
}

impl SubAgentExecutor {
    pub async fn execute_with_metrics(&self, ...) -> ExecutionResult {
        tokio::select! {
            result = self.orchestrator.execute_with_mcp(...) => {
                // Process result
            }
            _ = self.cancellation_token.cancelled() => {
                ExecutionResult {
                    success: false,
                    report: String::new(),
                    metrics: SubAgentMetrics::default(),
                    error_message: Some("Execution cancelled".to_string()),
                }
            }
        }
    }

    pub fn cancel(&self) {
        self.cancellation_token.cancel();
    }
}
```

- **Phases**:
  1. Ajouter CancellationToken a SubAgentExecutor
  2. Modifier execute_with_metrics pour supporter cancellation
  3. Implementer cancel_workflow_streaming dans streaming.rs
  4. Propager cancellation aux sub-agents actifs
- **Prerequis**: OPT-SA-1 (timeouts)
- **Risque regression**: Moyen (nouveau pattern)
- **Tests requis**:
  - Test cancellation mid-execution
  - Test cancellation token propagation
  - Test cleanup apres cancellation

#### OPT-SA-8: Circuit Breaker pour Sub-Agent Execution

- **Fichiers**:
  - Nouveau: `src-tauri/src/tools/sub_agent_circuit_breaker.rs`
  - Modifier: `src-tauri/src/tools/sub_agent_executor.rs`
- **Changement**: Appliquer pattern circuit breaker (deja utilise pour MCP)

```rust
// sub_agent_circuit_breaker.rs
pub struct SubAgentCircuitBreaker {
    failure_count: AtomicUsize,
    state: RwLock<CircuitState>,
    last_failure: RwLock<Option<Instant>>,
}

pub enum CircuitState {
    Closed,      // Normal operation
    Open,        // Reject requests (cooldown)
    HalfOpen,    // Testing recovery
}

impl SubAgentCircuitBreaker {
    const FAILURE_THRESHOLD: usize = 3;
    const COOLDOWN_SECS: u64 = 60;

    pub fn check(&self) -> Result<(), ToolError> {
        // Similar to MCP circuit breaker
    }

    pub fn record_success(&self) { ... }
    pub fn record_failure(&self) { ... }
}
```

- **Phases**:
  1. Creer module circuit breaker
  2. Integrer dans SubAgentExecutor
  3. Ajouter metriques d'etat
  4. Tests de transitions d'etat
- **Prerequis**: None
- **Risque regression**: Moyen
- **Tests requis**:
  - Test 3 echecs -> circuit open
  - Test cooldown expiration -> half-open
  - Test recovery -> closed

#### OPT-SA-9: Reduire Complexite Cyclomatique de execute_batch()

- **Fichiers**: `src-tauri/src/tools/parallel_tasks.rs:170-466`
- **Changement**: Extraire sous-fonctions

```rust
// Avant: 297 lignes, CC~20

// Apres: ~150 lignes dans execute_batch(), CC~8
impl ParallelTasksTool {
    async fn execute_batch(&self, ...) -> ToolResult<Value> {
        self.validate_batch_permissions()?;
        self.validate_tasks(&tasks)?;
        let prepared = self.prepare_execution_records(&tasks).await?;
        let results = self.execute_parallel(prepared).await;
        self.process_results(results).await
    }

    fn validate_batch_permissions(&self) -> ToolResult<()> { ... }
    fn validate_tasks(&self, tasks: &[ParallelTaskSpec]) -> ToolResult<()> { ... }
    async fn prepare_execution_records(&self, ...) -> ToolResult<Vec<PreparedTask>> { ... }
    async fn execute_parallel(&self, ...) -> Vec<TaskResult> { ... }
    async fn process_results(&self, ...) -> ToolResult<Value> { ... }
}
```

- **Phases**:
  1. Extraire validation (lignes 176-238)
  2. Extraire preparation records (lignes 266-322)
  3. Extraire result processing (lignes 330-445)
  4. Simplifier execute_batch()
- **Prerequis**: Tests complets existants
- **Risque regression**: Moyen (refactoring majeur)
- **Tests requis**: Tous tests existants doivent passer

### Nice to Have (Impact faible, Effort faible)

#### OPT-SA-10: Retry avec Exponential Backoff

- **Fichiers**: `src-tauri/src/tools/sub_agent_executor.rs`
- **Changement**: Ajouter retry logic sur echecs temporaires

```rust
const MAX_RETRIES: u32 = 2;
const INITIAL_DELAY_MS: u64 = 500;

async fn execute_with_retry(&self, ...) -> ExecutionResult {
    for attempt in 0..=MAX_RETRIES {
        match self.execute_with_metrics_inner(...).await {
            Ok(result) if result.success => return result,
            Ok(result) if is_retryable_error(&result.error_message) => {
                let delay = INITIAL_DELAY_MS * 2_u64.pow(attempt);
                tokio::time::sleep(Duration::from_millis(delay)).await;
            }
            result => return result,
        }
    }
    // Return last failure
}
```

- **Benefice**: Resilience aux erreurs temporaires
- **Risque regression**: Faible
- **Validation**: Tests avec mock qui echoue puis reussit

#### OPT-SA-11: Correlation ID pour Tracage Hierarchique

- **Fichiers**:
  - `src-tauri/src/models/sub_agent.rs`
  - `src-tauri/src/tools/sub_agent_executor.rs`
- **Changement**: Ajouter `parent_execution_id` dans les logs

```rust
// Dans SubAgentExecution
pub struct SubAgentExecution {
    // ... existing
    pub parent_execution_id: Option<String>, // NEW: For tracing hierarchy
}

// Dans logs
info!(
    parent_execution_id = %parent_exec_id,
    sub_agent_id = %agent_id,
    "Starting sub-agent execution"
);
```

- **Benefice**: Tracage complet parent -> children
- **Risque regression**: Quasi-nul (additive only)

### Differe (Impact faible, Effort eleve)

| Optimisation | Raison du Report |
|--------------|------------------|
| Migration native async traits | Incompatible avec Arc<dyn Tool>, attendu Rust 2026 |
| Tests d'integration complets | Necessiterait mock LLM/MCP, hors scope optimisation |
| OpenTelemetry exporter | Infrastructure externe requise |

## Dependencies

### Mises a Jour Recommandees

| Crate | Actuel | Recommande | Breaking Changes |
|-------|--------|------------|------------------|
| once_cell | 1.19 | 1.20.3 | Non |
| tokio | 1.48.0 | 1.48.0 | A jour |
| futures | 0.3.31 | 0.3.31 | A jour |
| async-trait | 0.1.89 | 0.1.89 | A jour |
| tokio-util | 0.7.17 | 0.7.17 | A jour |

### Nouvelles Dependencies

| Crate | Raison | Impact bundle |
|-------|--------|---------------|
| Aucune | Toutes fonctionnalites disponibles via tokio/tokio-util | N/A |

## Verification Non-Regression

### Tests Existants

- [x] `cargo test` - 34 tests couvrent sub-agent tools
  - `sub_agent_executor.rs`: 8 tests
  - `spawn_agent.rs`: 9 tests
  - `delegate_task.rs`: 9 tests
  - `parallel_tasks.rs`: 8 tests

### Tests a Ajouter

- [ ] Test inactivity timeout avec execution active (OPT-SA-1) - doit continuer
- [ ] Test inactivity timeout avec silence complet (OPT-SA-1) - doit timeout apres 300s
- [ ] Test activity callback propagation (OPT-SA-1) - callback appele a chaque token/tool
- [ ] Test cancellation token propagation (OPT-SA-7)
- [ ] Test circuit breaker state transitions (OPT-SA-8)
- [ ] Test JoinSet equivalence avec join_all (OPT-SA-6)
- [ ] Test retry avec backoff (OPT-SA-10)

### Commandes de Validation

```bash
# Backend validation complete
cd src-tauri
cargo fmt --check
cargo clippy -- -D warnings
cargo test
cargo build --release

# Verification specifique sub-agent tools
cargo test sub_agent -- --nocapture
cargo test spawn_agent -- --nocapture
cargo test delegate_task -- --nocapture
cargo test parallel_tasks -- --nocapture
```

## Estimation

| Optimisation | Effort | Impact | Priorite |
|--------------|--------|--------|----------|
| OPT-SA-1 (Inactivity Timeout Heartbeat) | 4h | Haut | P2 |
| OPT-SA-2 (once_cell) | 5min | Faible | P1 |
| OPT-SA-3 (Magic numbers) | 0.5h | Faible | P1 |
| OPT-SA-4 (Event emission) | 1h | Moyen | P1.5 |
| OPT-SA-5 (update_record) | 1h | Moyen | P1.5 |
| OPT-SA-6 (JoinSet) | 0.5h | Faible | P1.5 |
| OPT-SA-7 (CancellationToken) | 2h | Haut | P2 |
| OPT-SA-8 (Circuit breaker) | 3h | Haut | P2 |
| OPT-SA-9 (Reduire CC) | 2h | Moyen | P2 |
| OPT-SA-10 (Retry backoff) | 1.5h | Faible | P3 |
| OPT-SA-11 (Correlation ID) | 1h | Faible | P3 |

**Total P1**: ~0.6h (once_cell + magic numbers)
**Total P1.5**: ~2.5h (event emission + update_record + JoinSet)
**Total P2**: ~11h (Heartbeat Timeout + CancellationToken + Circuit breaker + Reduire CC)
**Total P3**: ~2.5h (Retry + Correlation ID)
**Grand Total**: ~16.6h

## Risques et Mitigations

| Risque | Probabilite | Impact | Mitigation |
|--------|-------------|--------|------------|
| Regression fonctionnelle | Moyenne | Eleve | Tests exhaustifs avant/apres |
| Heartbeat callback non propage | Moyenne | Haut | Tests integration avec mock LLM |
| Inactivity timeout trop court | Faible | Moyen | Constante configurable (300s) |
| Circuit breaker trop sensible | Faible | Moyen | Threshold configurable (3 echecs) |
| Refactoring CC casse tests | Moyenne | Moyen | Refactoring incremental |

## Prochaines Etapes

1. [x] Valider approche timeout heartbeat avec l'utilisateur
2. [x] **OPT-SA-1 (Heartbeat Timeout) - COMPLETE** (2025-12-09)
   - Added `ActivityMonitor` struct with thread-safe activity tracking
   - Implemented `execute_with_heartbeat_timeout()` in SubAgentExecutor
   - Added constants: `INACTIVITY_TIMEOUT_SECS=300`, `ACTIVITY_CHECK_INTERVAL_SECS=30`
   - Updated SpawnAgentTool to use heartbeat timeout
   - Added 6 new tests for ActivityMonitor
   - Note: DelegateTaskTool and ParallelTasksTool need deeper refactor (OPT-SA-4/5/6)
3. [x] **OPT-SA-2 (once_cell 1.19 -> 1.20) - COMPLETE** (2025-12-09)
   - Updated `src-tauri/Cargo.toml`: `once_cell = "1.20"`
   - Validation: `cargo check` passed
4. [x] **OPT-SA-3 (Magic Numbers) - COMPLETE** (2025-12-09)
   - Added to `constants.rs::sub_agent`: `VALIDATION_TIMEOUT_SECS`, `VALIDATION_POLL_MS`
   - Updated `spawn_agent.rs`: Uses `TASK_DESC_TRUNCATE_CHARS` instead of `100`
   - Updated `validation_helper.rs`: Imports from centralized `constants::sub_agent`
   - Removed local constants in validation_helper.rs
   - All tests passed (spawn_agent: 7, validation_helper: 6+7 integration)
5. [x] **OPT-SA-4 (Unifier Event Emission) - COMPLETE** (2025-12-09)
   - Removed duplicate `emit_event()` methods from `delegate_task.rs` and `parallel_tasks.rs`
   - Both tools now use `SubAgentExecutor::emit_start_event()` and `emit_complete_event()`
   - Updated imports: removed `StreamChunk`, `events`, `SubAgentStreamMetrics`; added `ExecutionResult`, `SubAgentExecutor`
   - Created `SubAgentExecutor` instance in both tools for event emission
   - All 33 unit tests + 17 integration tests passing
   - Reduced code duplication by ~40 lines
6. [x] **OPT-SA-5 (Unifier update_execution_record) - COMPLETE** (2025-12-09)
   - Removed duplicate `update_execution_record()` method from `parallel_tasks.rs`
   - ParallelTasksTool now uses `SubAgentExecutor::update_execution_record()`
   - Removed `SubAgentExecutionComplete` import (no longer needed)
   - Unified DB update logic: single source of truth in SubAgentExecutor
   - All 50 sub-agent tests passing (33 unit + 17 integration)
   - Reduced code duplication by ~40 lines
7. [x] **OPT-SA-6 (JoinSet Migration) - COMPLETE** (2025-12-09)
   - Migrated ParallelTasksTool from `orchestrator.execute_parallel()` to `tokio::task::JoinSet`
   - Added `use tokio::task::JoinSet;` import
   - Implemented index-preserving pattern: results include task index for order restoration
   - Used `join_next()` loop instead of `join_all()` for graceful error handling
   - Added `#[allow(dead_code)]` to `AgentOrchestrator::execute_parallel()` (kept for tests)
   - Updated docstrings: module header, struct docs, performance benefits
   - All 50 sub-agent tests passing (33 unit + 17 integration)
   - Benefits: per-task control, future cancellation support (OPT-SA-7), better memory management
8. [ ] Planifier P2 pour sprint suivant:
   - OPT-SA-7 (CancellationToken - 2h)
   - OPT-SA-8 (Circuit breaker - 3h)
   - OPT-SA-9 (Reduire CC - 2h)

## References

### Code Analyse
- `src-tauri/src/tools/spawn_agent.rs` (854 lignes)
- `src-tauri/src/tools/delegate_task.rs` (765 lignes - was 797, reduced by OPT-SA-4)
- `src-tauri/src/tools/parallel_tasks.rs` (854 lignes - was 880, modified by OPT-SA-4, OPT-SA-5, OPT-SA-6)
- `src-tauri/src/tools/sub_agent_executor.rs` (542 lignes)
- `src-tauri/src/tools/validation_helper.rs` (516 lignes)
- `src-tauri/src/tools/constants.rs`
- `src-tauri/src/tools/registry.rs`

### Documentation Consultee
- `CLAUDE.md` (Tool Development Patterns)
- `docs/MULTI_AGENT_ARCHITECTURE.md`
- `docs/AGENT_TOOLS_DOCUMENTATION.md`
- `docs/ARCHITECTURE_DECISIONS.md`

### Sources Externes
- [Tokio Timeout Patterns](https://www.slingacademy.com/article/handling-timeouts-in-rust-with-async-and-tokio-timers/)
- [Tokio CancellationToken](https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html)
- [Galileo Multi-Agent Failure Recovery](https://galileo.ai/blog/multi-agent-ai-system-failure-recovery)
- [Anthropic Building Effective Agents](https://www.anthropic.com/research/building-effective-agents)
- [OneSignal Rust Concurrency Patterns](https://onesignal.com/blog/rust-concurrency-patterns/)
- [developerlife.com Cancellation Safety](https://developerlife.com/2024/07/10/rust-async-cancellation-safety-tokio/)
- [Cognition: Don't Build Multi-Agents](https://cognition.ai/blog/dont-build-multi-agents)
- [Databricks Agent Design Patterns](https://docs.databricks.com/aws/en/generative-ai/guide/agent-system-design-patterns)

### Memories Serena
- `sub_agent_system_summary`
- `sub_agent_system_implementation_spec`
- `sub_agent_dependencies_analysis`
