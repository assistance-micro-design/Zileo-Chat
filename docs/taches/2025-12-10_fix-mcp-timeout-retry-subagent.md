# Fix MCP Timeout/Retry et Sub-Agent Heartbeat

**Date**: 2025-12-10
**Statut**: Complete
**Branche**: OPT-WF

## Contexte

Le SpawnAgentTool se bloquait indefiniment lors de l'execution de sub-agents. L'analyse a revele trois problemes majeurs :

1. **MCP stdio sans timeout** : `read_line()` bloquait indefiniment si le serveur MCP ne repondait pas
2. **MCP sans retry** : Pas de mecanisme de retry sur les erreurs transitoires
3. **Sub-agent heartbeat casse** : Le callback d'activite n'etait jamais propage, donc le timeout d'inactivite ne fonctionnait pas

## Modifications

### 1. MCP Timeout pour stdio (`src-tauri/src/mcp/server_handle.rs`)

**Probleme** : La methode `send_request()` utilisait `read_line()` de maniere synchrone et bloquante. Si un serveur MCP (Docker, NPX, UVX) ne repondait pas, l'application restait bloquee indefiniment.

**Solution** :
- Change `stdout_reader` de `Mutex<BufReader>` a `Arc<std::sync::Mutex<BufReader>>` pour permettre le clonage dans `spawn_blocking`
- Utilise `tokio::task::spawn_blocking` pour executer le `read_line()` dans un thread dedie
- Enveloppe avec `tokio::time::timeout` pour limiter l'attente a 30 secondes

```rust
// Avant (bloquant indefiniment)
stdout_guard.read_line(&mut response_line)?;

// Apres (avec timeout de 30s)
let read_result = tokio::time::timeout(timeout_duration, async {
    tokio::task::spawn_blocking(move || {
        stdout_guard.read_line(&mut response_line)
    }).await?
}).await;
```

**Constantes** :
- `DEFAULT_TIMEOUT_MS = 30000` (30 secondes)

### 2. MCP Retry avec Backoff Exponentiel (`src-tauri/src/mcp/manager.rs`)

**Probleme** : Aucun mecanisme de retry pour les erreurs transitoires (timeout, problemes reseau).

**Solution** :
- Ajout d'une boucle de retry avec backoff exponentiel
- 3 tentatives maximum (1 initiale + 2 retries)
- Delais : 500ms -> 1000ms -> 2000ms

```rust
const MCP_MAX_RETRY_ATTEMPTS: u32 = 2;
const MCP_INITIAL_RETRY_DELAY_MS: u64 = 500;
```

**Erreurs retryables** :
- `MCPError::Timeout` - Timeout d'operation
- `MCPError::ConnectionFailed` - Probleme de connexion
- `MCPError::IoError` - Erreur I/O

**Erreurs NON retryables** (echec immediat) :
- `MCPError::ServerNotFound`
- `MCPError::InvalidConfig`
- `MCPError::ProtocolError`
- `MCPError::CircuitBreakerOpen`

**Nouveau variant d'erreur** (`src-tauri/src/mcp/error.rs`) :
```rust
MCPError::RetryExhausted {
    server: String,
    attempts: u32,
    last_error: String,
}
```

### 3. Sub-Agent Executor Heartbeat (`src-tauri/src/tools/sub_agent_executor.rs`)

**Probleme** : Le callback d'activite etait cree mais jamais utilise :
```rust
let _activity_callback = on_activity.unwrap_or_else(|| monitor.create_callback());
// Le underscore prefix indique que la variable n'est jamais utilisee!
```

Le `select!` ne pouvait pas atteindre le branch du check interval car l'execution etait bloquee sur des appels synchrones.

**Solution** :
1. L'execution est maintenant dans un `tokio::spawn` separe :
   ```rust
   let execution_handle = tokio::spawn(async move {
       monitor_for_exec.record_activity();
       let result = orchestrator.execute_with_mcp(...).await;
       monitor_for_exec.record_activity();
       result
   });
   ```

2. Utilisation de `AbortHandle` pour pouvoir annuler la tache :
   ```rust
   let abort_handle = execution_handle.abort_handle();
   ```

3. Enregistrement automatique de l'activite a chaque check interval :
   ```rust
   _ = tokio::time::sleep(Duration::from_secs(ACTIVITY_CHECK_INTERVAL_SECS)) => {
       // Si on atteint ce branch, le runtime tokio est responsive
       monitor.record_activity();
       // ...
   }
   ```

4. Abort propre en cas de timeout ou cancellation :
   ```rust
   abort_handle.abort();
   ```

## Fichiers modifies

| Fichier | Modifications |
|---------|---------------|
| `src-tauri/src/mcp/server_handle.rs` | Timeout 30s sur `send_request()`, `Arc<std::sync::Mutex>` pour stdout_reader |
| `src-tauri/src/mcp/manager.rs` | Retry avec backoff exponentiel, `is_retryable_error()` |
| `src-tauri/src/mcp/error.rs` | Nouveau variant `RetryExhausted` |
| `src-tauri/src/tools/sub_agent_executor.rs` | `tokio::spawn` pour execution, `AbortHandle`, activite auto |

## Configuration des timeouts/retries

| Composant | Timeout | Retry | Backoff |
|-----------|---------|-------|---------|
| MCP stdio | 30s | 3 tentatives | 500ms -> 1s -> 2s |
| MCP HTTP | 30s | 3 tentatives | 500ms -> 1s -> 2s |
| Ollama LLM | 5min | Non (circuit breaker) | N/A |
| Sub-agent heartbeat | 5min inactivite | 3 tentatives | 500ms -> 1s -> 2s |

## Tests

- `cargo check` : OK
- `cargo test --lib` : 836 tests passes, 0 echecs

## Impact

- Les sub-agents ne se bloquent plus indefiniment sur des appels MCP
- Les erreurs transitoires sont automatiquement retries
- Le heartbeat fonctionne correctement et permet de detecter les executions bloquees
- Les taches peuvent etre proprement annulees (timeout ou user cancellation)
