# Guide Configuration MCP Servers

> **Date**: 22 Novembre 2025
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

## Implémentations Rust

### SDK Officiel Anthropic
**Repository**: `modelcontextprotocol/rust-sdk`
- Maintenu par Anthropic
- Conformité spec 2025-06-18
- Runtime async Tokio

### MCP Framework (koki7o)
**Repository**: `koki7o/mcp-framework`
- Production-ready
- Web inspector (localhost:8123)
- Performance optimisée Rust
- Support multi-provider LLM
- **Requis**: Rust 1.70+

### Prism MCP RS
**Repository**: `prismworks-ai/prism-mcp-rs`
- Enterprise-grade
- Production features avancées

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

## Outils Utiles

### Inspector Web (koki7o/mcp-framework)
- URL: `localhost:8123`
- Debugging temps réel
- Inspection messages JSON-RPC
- Monitoring outils/resources

### CLI Discovery
- **mcp-discovery** (Rust)
- Découverte capabilities
- Test serveurs MCP

## Intégration Tauri 2

### Architecture Recommandée
```
Frontend (Svelte)
    ↓ IPC
Tauri Backend (Rust)
    ↓ stdio
MCP Server (Docker/Local)
    ↓ JSON-RPC 2.0
LLM Provider
```

### Configuration Tauri
```json
{
  "tauri": {
    "allowlist": {
      "shell": {
        "execute": true,
        "scope": [
          { "cmd": "docker" },
          { "cmd": "npx" },
          { "cmd": "uvx" }
        ]
      }
    }
  }
}
```

## Références

- **Spec MCP**: https://modelcontextprotocol.io
- **Rust SDK**: https://github.com/modelcontextprotocol/rust-sdk
- **MCP Framework**: https://github.com/koki7o/mcp-framework
- **Servers Collection**: https://github.com/modelcontextprotocol/servers
- **Docker MCP**: https://www.docker.com/blog/announcing-docker-mcp-catalog-and-toolkit-beta
