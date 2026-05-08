# Zileo Chat

[![Version](https://img.shields.io/badge/version-0.23.1-orange)](https://github.com/assistance-micro-design/zileo-chat)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue)](LICENSE)
[![Status](https://img.shields.io/badge/status-beta-yellow)](https://github.com/assistance-micro-design/zileo-chat)

> Application desktop multi-agent avec interface conversationnelle intelligente.

**Developed by** [Assistance Micro Design](https://www.assistancemicrodesign.net/)

**Built with** [Claude Code](https://claude.ai/code) by Anthropic

---

## Beta Warning

**This software is currently in beta (v0.23.1).**

Before using Zileo Chat, please be aware of the following risks:

| Risk | Description |
|------|-------------|
| **Data Loss** | Database schema may change between versions, potentially requiring data migration or reset |
| **API Costs** | LLM API calls incur costs based on token usage - monitor your provider usage |
| **Instability** | Features may be incomplete, contain bugs, or change without notice |
| **Security** | While security measures are implemented, the software has not undergone formal security audit |
| **Breaking Changes** | Updates may introduce breaking changes to configurations or workflows |

**Recommendation**: Back up your data regularly and avoid using for critical production tasks until v1.0 release.

---

## Description

Zileo Chat is a desktop application for orchestrating AI agents through a conversational interface. It supports multi-agent workflows with tool execution, memory persistence, and human-in-the-loop validation.

### Key Features

- **Multi-Agent System** - Create and orchestrate multiple AI agents with specialized tools and sub-agent delegation
- **Real-time Streaming** - Token-by-token response display with usage metrics and prompt caching
- **Tool Execution** - Memory, Todo, Calculator, FileManager, Skills, and sub-agent tools
- **Human-in-the-Loop** - Approve critical operations before execution with risk-based validation
- **Custom Providers** - Add any OpenAI-compatible LLM endpoint alongside built-in Mistral and Ollama
- **Reasoning Effort** - Granular thinking control (low/medium/high) for supported models
- **Background Workflows** - Run multiple workflows concurrently with progress tracking
- **MCP Protocol** - Extend agents with Model Context Protocol servers
- **Bilingual UI** - English and French interface

---

## Supported LLM Providers

Zileo Chat supports built-in and custom LLM providers:

| Provider | Type | Link |
|----------|------|------|
| **Mistral AI** | Cloud API (built-in) | [https://mistral.ai](https://mistral.ai) |
| **Ollama** | Local / Cloud (built-in) | [https://ollama.com](https://ollama.com) |
| **Custom Providers** | OpenAI-compatible API | Any OpenAI-compatible endpoint |

### Mistral AI (Cloud)

- Requires API key from [console.mistral.ai](https://console.mistral.ai)
- Supports all Mistral models (Mistral Large, Codestral, Magistral, etc.)
- Pay-per-use pricing based on token consumption

### Ollama (Local / Cloud)

- Free and runs locally on your machine or via cloud endpoints
- Requires [Ollama](https://ollama.com/download) installed and running
- Supports open-source models (Llama, Mistral, Qwen, Kimi, etc.)

### Custom Providers (OpenAI-compatible)

- Add any OpenAI-compatible API endpoint (Routerlab, OpenRouter, Together AI, vLLM, LM Studio, etc.)
- Full streaming, tool calling, and reasoning support
- Configure via Settings > Providers

---

## Prerequisites

### Required for First Launch

| Dependency | Purpose | Installation |
|------------|---------|--------------|
| **Docker Desktop** | MCP servers execution | [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop/) |
| **At least one LLM provider** | AI model access | See [Supported LLM Providers](#supported-llm-providers) |

> **Mistral API vs Le Chat Pro**: The [Le Chat subscription](https://mistral.ai/pricing) ($14.99/month) is for the web chat interface only. Zileo Chat requires a **separate API key** from [La Plateforme](https://docs.mistral.ai/deployment/laplateforme/pricing/) with pay-per-token billing.

### MCP Servers Configuration

Use [Docker MCP Toolkit](https://docs.docker.com/ai/mcp-catalog-and-toolkit/toolkit/) for MCP server management:

1. Enable MCP Toolkit: Docker Desktop > Settings > Beta features > **Enable Docker MCP Toolkit**
2. Open **MCP Toolkit** tab in Docker Desktop
3. Browse the [MCP Catalog](https://docs.docker.com/ai/mcp-catalog-and-toolkit/catalog/) (200+ servers available)
4. Select and install desired MCP servers
5. In server **Overview** > "Use this MCP Server" > copy the Docker configuration

> **Recommended**: Always prefer Docker configurations over NPX/UVX for better isolation and zero dependency management.

### Optional: Ollama (Local + Cloud Models)

| Step | Command |
|------|---------|
| Install Ollama | [ollama.com/download](https://ollama.com/download) |
| Run cloud model | `ollama run kimi-k2-thinking:cloud` |
| List available models | `ollama list` |

> **Ollama Cloud**: For large models like [Kimi K2 (1T params)](https://ollama.com/library/kimi-k2:1t-cloud), use `ollama run <model>:cloud`. No local GPU required.

---

## Build Requirements

- **Node.js** 20.19+ or 22.12+
- **Rust** 1.80.1+

```bash
node --version    # >= 20.19
rustc --version   # >= 1.80.1
```

---

## Installation

```bash
# Clone repository
git clone https://github.com/assistance-micro-design/zileo-chat.git
cd zileo-chat

# Install dependencies
npm install

# Development
npm run tauri:dev

# Production build
npm run tauri:build
```

---

## Tech Stack

| Layer | Technology |
|-------|------------|
| Frontend | SvelteKit 2.55 + Svelte 5.55 |
| Backend | Rust + Tauri 2.11 |
| Database | SurrealDB 2.6 (embedded) |
| LLM | Rig.rs 0.34 + direct HTTP |

---

## Documentation

Full documentation is available in the [`docs/`](docs/) directory:

- [Getting Started](docs/GETTING_STARTED.md)
- [Architecture Decisions](docs/ARCHITECTURE_DECISIONS.md)
- [Database Schema](docs/DATABASE_SCHEMA.md)
- [Agent Tools Documentation](docs/AGENT_TOOLS_DOCUMENTATION.md)
- [API Reference](docs/API_REFERENCE.md)

---

## Contributing

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/your-feature`)
3. Commit changes (`git commit -m 'Add feature'`)
4. Push to branch (`git push origin feature/your-feature`)
5. Open a Pull Request

---

## Security

To report a vulnerability, please open a private issue on [GitHub Security](https://github.com/assistance-micro-design/zileo-chat/security).

---

## License

This project is licensed under the **Apache License 2.0**. See [LICENSE](LICENSE) for details.

Third-party licenses are documented in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

```
Copyright 2025-2026 Assistance Micro Design
Licensed under the Apache License, Version 2.0
```

---

## Acknowledgments

- Built with [Claude Code](https://claude.ai/code) by [Anthropic](https://anthropic.com)
- Powered by [Tauri](https://tauri.app), [SvelteKit](https://kit.svelte.dev), [SurrealDB](https://surrealdb.com)
- LLM integration via [Rig.rs](https://github.com/0xPlaygrounds/rig) + direct HTTP

---

[Assistance Micro Design](https://www.assistancemicrodesign.net/) | [GitHub](https://github.com/assistance-micro-design)
