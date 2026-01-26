# Guide Configuration MCP Servers

> **Date**: 6 Décembre 2025
> **Spec**: JSON-RPC 2.0 • MCP 2025-06-18
> **Stack**: Rust + Tauri 2

## Protocole MCP

### Spécification
- **Standard**: JSON-RPC 2.0 (obligatoire)
- **Version actuelle**: 2025-06-18
- **Versions supportées**: 2025-03-26, 2024-11-05
- **Documentation**: https://modelcontextprotocol.io/specification/2025-06-18

### Nouveautés Version 2025-03-26
- OAuth 2.1 pour authentification
- Streamable HTTP (remplace HTTP+SSE)
- JSON-RPC batching
- Tool annotations enrichies

## Méthodes d'Installation

### NPX (JavaScript/TypeScript)
**Usage**: Serveurs Node.js
```json
{
  "mcpServers": {
    "memory": {
      "command": "npx",
      "args": ["-y", "@modelcontextprotocol/server-memory"]
    }
  }
}
```

### UVX (Python)
**Usage**: Serveurs Python (uv package manager)
```json
{
  "mcpServers": {
    "sqlite": {
      "command": "uvx",
      "args": ["mcp-server-sqlite", "--db-path", "test.db"]
    }
  }
}
```

### Docker (Recommandé Production)
**Usage**: Isolation et sécurité
```json
{
  "mcpServers": {
    "service": {
      "command": "docker",
      "args": ["run", "-i", "--rm", "mcp-server:latest"]
    }
  }
}
```

**Avantages Docker**:
- Isolation complète
- Pas d'accès host par défaut
- Credentials sécurisés
- Environnements reproductibles

## Formats Configuration

### Structure Base
```json
{
  "mcpServers": {
    "<server_name>": {
      "command": "<npx|uvx|docker>",
      "args": ["<arguments>"],
      "env": {
        "API_KEY": "${API_KEY}",
        "ENV": "production"
      }
    }
  }
}
```

### Variables Environnement
**Pattern**: `${VARIABLE_NAME}`
- Chargées depuis variables système (OS environment)
- Usage : MCP servers credentials uniquement (pas API keys LLM)
- Jamais en clair dans config
- Rotation régulière recommandée

### Configuration Multi-Environment

**Development**:
```json
{
  "mcpServers": {
    "local-db": {
      "command": "uvx",
      "args": ["mcp-server-sqlite", "--db-path", "dev.db"]
    }
  }
}
```

**Production**:
```json
{
  "mcpServers": {
    "prod-db": {
      "command": "docker",
      "args": [
        "run", "-i", "--rm",
        "-e", "DB_URL=${DATABASE_URL}",
        "mcp-db:latest"
      ]
    }
  }
}
```

## Distribution Servers

### 1. Développement Local
```bash
uv run mcp-server
```

### 2. Repository GitHub
```bash
uvx --from git+https://github.com/user/mcp-server mcp-server
```

### 3. Package Registry
```bash
# PyPI
uvx mcp-server-package

# npm
npx @scope/mcp-server-package
```

## Transports Supportés

### stdio (Local)
- Communication inter-process
- Performant pour Tauri
- Standard pour clients desktop

### Streamable HTTP (Cloud)
- Remplace HTTP+SSE depuis 2025-03-26
- APIs externes hébergées
- Streaming natif

### WebSocket (Temps Réel)
- Bidirectionnel
- Callbacks et notifications
- Interactions longues

## Sécurité

### Best Practices
- ✅ Docker pour isolation
- ✅ Variables env pour secrets
- ✅ Validation stricte inputs
- ✅ Audit logging
- ❌ Jamais credentials en clair
- ❌ Jamais npx/uvx en production sans audit

### Exposition Minimale
```json
{
  "command": "docker",
  "args": [
    "run", "-i", "--rm",
    "--network=none",
    "--read-only",
    "--cap-drop=ALL"
  ]
}
```

## Implémentation Zileo

### Architecture

```
Frontend (SvelteKit)
    ↓ invoke() IPC
Tauri Backend (Rust)
    ↓
MCPManager (Registry)
├─ clients: HashMap<ServerName, MCPClient>  (keyed by NAME)
├─ Lifecycle: spawn_server, stop_server
├─ Tool Calling: call_tool → log_call
└─ Discovery: list_servers, list_server_tools
    ↓
MCPClient
├─ TransportHandle::Stdio (Docker/NPX/UVX)
└─ TransportHandle::Http (remote servers)
    ↓ JSON-RPC 2.0
MCP Server Process
```

**Design Decision**: Le `MCPManager` est indexé par **nom de serveur** (pas ID) car les agents référencent les serveurs par nom dans leur configuration (`mcp_servers: ["Serena", "Context7"]`).

### Commandes Tauri (11 commands)

#### Configuration

| Commande | Signature | Description |
|----------|-----------|-------------|
| `list_mcp_servers` | `() -> Result<Vec<MCPServer>>` | Liste tous les serveurs configurés |
| `get_mcp_server` | `(id: String) -> Result<MCPServer>` | Récupère un serveur par ID |
| `create_mcp_server` | `(config: MCPServerConfig) -> Result<MCPServer>` | Crée une nouvelle config serveur |
| `update_mcp_server` | `(id: String, config: MCPServerConfig) -> Result<MCPServer>` | Met à jour un serveur |
| `delete_mcp_server` | `(id: String) -> Result<()>` | Supprime un serveur |

#### Lifecycle

| Commande | Signature | Description |
|----------|-----------|-------------|
| `start_mcp_server` | `(id: String) -> Result<MCPServer>` | Démarre un serveur |
| `stop_mcp_server` | `(id: String) -> Result<MCPServer>` | Arrête un serveur |
| `test_mcp_server` | `(config: MCPServerConfig) -> Result<MCPTestResult>` | Teste une connexion (sans sauvegarder) |

#### Outils

| Commande | Signature | Description |
|----------|-----------|-------------|
| `list_mcp_tools` | `(serverName: String) -> Result<Vec<MCPTool>>` | Liste les outils d'un serveur |
| `call_mcp_tool` | `(request: MCPToolCallRequest) -> Result<MCPToolCallResult>` | Exécute un outil MCP |

#### Métriques

| Commande | Signature | Description |
|----------|-----------|-------------|
| `get_mcp_latency_metrics` | `(serverName?: String) -> Result<Vec<MCPLatencyMetrics>>` | Métriques de latence (p50, p95, p99) |

### Types TypeScript

```typescript
// Configuration serveur (création/mise à jour)
interface MCPServerConfig {
  id: string;            // Unique identifier
  name: string;          // Identifiant unique fonctionnel
  enabled: boolean;
  command: MCPDeploymentMethod;  // 'docker' | 'npx' | 'uvx' | 'http'
  args: string[];
  env: Record<string, string>;
  description?: string;
}

// Serveur complet (retourné par API)
interface MCPServer {
  config: MCPServerConfig;
  status: 'stopped' | 'starting' | 'running' | 'error' | 'disconnected';
  tools: MCPTool[];
  resources: MCPResource[];
  created_at: string;
  updated_at: string;
}

// Outil MCP
interface MCPTool {
  name: string;
  description: string;
  input_schema: object;
}

// Requête d'appel d'outil
interface MCPToolCallRequest {
  server_name: string;   // Nom du serveur (pas ID)
  tool_name: string;
  arguments: object;
}

// Résultat d'appel d'outil
interface MCPToolCallResult {
  success: boolean;
  content: unknown;       // Can be string, object, or array
  error?: string;         // Optional error message
  duration_ms: number;
}

// Résultat de test
interface MCPTestResult {
  success: boolean;
  message: string;
  tools: MCPTool[];
  resources: MCPResource[];
  latency_ms: number;
}

// Métriques de latence (percentiles)
interface MCPLatencyMetrics {
  server_name: string;
  p50_ms: number;      // Médiane
  p95_ms: number;
  p99_ms: number;
  total_calls: number;
}
```

### Règles de Validation

| Champ | Contrainte | Source |
|-------|------------|--------|
| Server ID/Name | Max 64 chars, `[a-zA-Z0-9_-]` | `commands/mcp.rs:57-82` |
| Description | Max 1024 chars, pas de caractères de contrôle | `commands/mcp.rs:117-140` |
| Args | Max 50 args, 512 chars chacun, pas de null bytes | `commands/mcp.rs:148-172` |
| Env Vars | Max 50 vars, noms `[A-Z0-9_]`, valeurs max 4096 chars | `commands/mcp.rs:180-229` |
| Tool Name | Max 128 chars, autorise `:` et `/` pour namespacing | `commands/mcp.rs:251-277` |

### Flux d'Appel d'Outil

```
Agent.execute(prompt)
    ↓
Agent détecte tool call: "Serena:find_symbol"
    ↓
Parse: server_name="Serena", tool_name="find_symbol"
    ↓
MCPManager.call_tool(server_name, tool_name, args)
    ↓
HashMap.get(server_name) → MCPClient
    ↓
MCPClient.call_tool(tool_name, args)
    ↓
TransportHandle (stdio ou HTTP)
    ↓
JSON-RPC Request → MCP Server → JSON-RPC Response
    ↓
MCPToolCallResult { success, content, error, duration_ms }
    ↓
Log vers table mcp_call_log (audit trail)
```

### Schéma Base de Données

#### Table `mcp_server`

```surql
DEFINE TABLE mcp_server SCHEMAFULL;
DEFINE FIELD name ON mcp_server TYPE string;
DEFINE FIELD enabled ON mcp_server TYPE bool;
DEFINE FIELD command ON mcp_server TYPE string;
DEFINE FIELD args ON mcp_server TYPE array<string>;
DEFINE FIELD env ON mcp_server TYPE string DEFAULT '{}';
DEFINE FIELD description ON mcp_server TYPE option<string>;
DEFINE FIELD created_at ON mcp_server TYPE datetime DEFAULT time::now();
DEFINE FIELD updated_at ON mcp_server TYPE datetime DEFAULT time::now();
```

**Note**: Le champ `env` est stocké comme string JSON (pas object) car SurrealDB SCHEMAFULL filtre les clés dynamiques non définies dans le schéma.

#### Table `mcp_call_log`

```surql
DEFINE TABLE mcp_call_log SCHEMAFULL;
DEFINE FIELD workflow_id ON mcp_call_log TYPE option<string>;
DEFINE FIELD server_name ON mcp_call_log TYPE string;
DEFINE FIELD tool_name ON mcp_call_log TYPE string;
DEFINE FIELD params ON mcp_call_log TYPE object;
DEFINE FIELD result ON mcp_call_log TYPE array | object; -- MCP tool results can be arrays or objects
DEFINE FIELD success ON mcp_call_log TYPE bool;
DEFINE FIELD duration_ms ON mcp_call_log TYPE int;
DEFINE FIELD timestamp ON mcp_call_log TYPE datetime DEFAULT time::now();
```

### Gestion des Erreurs

| Variante | Description |
|----------|-------------|
| `ProcessSpawnFailed` | Échec de démarrage du processus |
| `ConnectionFailed` | Connexion refusée ou timeout |
| `ProtocolError` | Erreur JSON-RPC retournée |
| `InitializationFailed` | Échec du handshake MCP |
| `ToolNotFound` | Outil inexistant sur le serveur |
| `ServerNotFound` | Serveur non enregistré dans le manager |
| `ServerNotRunning` | Opération sur serveur arrêté |
| `Timeout` | Opération trop longue |
| `IoError` | Erreur I/O |
| `SerializationError` | Erreur de parsing JSON |
| `DatabaseError` | Erreur d'opération DB |
| `ServerAlreadyExists` | Nom/ID de serveur dupliqué |
| `InvalidConfig` | Valeur de configuration invalide |

### Utilisation Frontend

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { MCPServer, MCPServerConfig, MCPTestResult } from '$types/mcp';

// Lister les serveurs
const servers = await invoke<MCPServer[]>('list_mcp_servers');

// Créer un serveur
const newServer = await invoke<MCPServer>('create_mcp_server', {
  config: {
    name: 'my-server',
    enabled: true,
    command: 'docker',
    args: ['run', '-i', '--rm', 'mcp-server:latest'],
    env: { API_KEY: 'secret' }
  }
});

// Tester avant de sauvegarder
const testResult = await invoke<MCPTestResult>('test_mcp_server', {
  config: { name: 'test', command: 'npx', args: ['-y', '@mcp/server'], env: {} }
});
if (testResult.success) {
  console.log(`Latency: ${testResult.latency_ms}ms, Tools: ${testResult.tools.length}`);
}

// Démarrer/arrêter
await invoke('start_mcp_server', { id: 'server-id' });
await invoke('stop_mcp_server', { id: 'server-id' });

// Appeler un outil
const result = await invoke('call_mcp_tool', {
  request: {
    server_name: 'Serena',  // Par NOM, pas ID
    tool_name: 'find_symbol',
    arguments: { symbol: 'MyClass' }
  }
});
```

## Timeout, Retry & Resilience (Phase 6)

### Request Timeout

All MCP tool calls have a **30 second timeout** (`DEFAULT_HTTP_TIMEOUT_MS: 30_000`). If a server doesn't respond within this window, the request fails with a timeout error.

**Configuration** (not user-configurable in v1):
- HTTP requests: 30s timeout via `tokio::time::timeout`
- Pool idle timeout: 90s

### Retry Logic

Failed requests are automatically retried with exponential backoff:

| Attempt | Delay Before Retry |
|---------|-------------------|
| 1st retry | 500ms |
| 2nd retry | 1000ms |

**Retryable Errors**:
- Network timeouts
- Connection refused
- HTTP 502, 503, 429 (rate limit)

**Non-Retryable Errors** (fail immediately):
- HTTP 400 (bad request)
- HTTP 401/403 (auth errors)
- JSON-RPC protocol errors

### Health Checks

The MCPManager performs periodic health checks on all running servers:

**Default Interval**: 5 minutes (`DEFAULT_HEALTH_CHECK_INTERVAL: 300s`)

**Health Probe**: Calls `refresh_tools()` which makes a real `tools/list` JSON-RPC request to verify server responsiveness.

**Behavior**:
- Success: Circuit breaker reset to closed state
- Failure: Failure count incremented, may trigger circuit open

**Startup**:
```rust
let manager = Arc::new(MCPManager::new(db).await?);
let _health_task = MCPManager::start_health_checks(manager.clone(), None);
```

### Circuit Breaker Updates

When combined with the existing circuit breaker (Section "Gestion des Erreurs"):

| Config | Value | Description |
|--------|-------|-------------|
| Failure Threshold | 3 | Failures before circuit opens |
| Cooldown Period | 60s | Time in open state before testing recovery |
| Health Check Recovery | Auto | Health checks can reset circuit to closed |

**Recovery Flow**:
```
Closed → 3 failures → Open → 60s cooldown → HalfOpen
                                              ↓
                              Health check success → Closed
                              Health check fail → Open (restart cooldown)
```

## Références

- **Spec MCP**: https://modelcontextprotocol.io
- **Servers Collection**: https://github.com/modelcontextprotocol/servers
- **Docker MCP**: https://www.docker.com/blog/announcing-docker-mcp-catalog-and-toolkit-beta
- **Source Zileo MCP**: `src-tauri/src/mcp/` (manager, client, protocol, error)
- **Commandes Tauri**: `src-tauri/src/commands/mcp.rs`
- **Types Rust**: `src-tauri/src/models/mcp.rs`
