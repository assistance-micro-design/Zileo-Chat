# Security Policy

## Supported Versions

| Version | Supported |
|---------|-----------|
| 0.9.x   | ✅ |
| < 0.9   | ❌ |

## Reporting a Vulnerability

**Do NOT create public GitHub issues for security vulnerabilities.**

Please report vulnerabilities via:
- [GitHub Security Advisories](https://github.com/assistance-micro-design/zileo-chat/security/advisories/new)

We will respond within 7 days and work with you to understand and resolve the issue.

## Security Measures

Zileo Chat implements the following security measures:

- **API Key Storage**: Encrypted storage via OS keychain (keyring) + AES-256
- **Content Security Policy**: Strict CSP (`default-src 'self'`)
- **SQL Injection Prevention**: Parameterized queries for all database operations
- **Input Validation**: Server-side validation for all user inputs
- **No Telemetry**: No data is sent to external servers (except LLM API calls you configure)

## Scope

### In Scope

Security issues in Zileo Chat code:

| Area | Examples |
|------|----------|
| **API Key Handling** | Key leakage, insecure storage, transmission issues |
| **MCP Server Execution** | Command injection via env vars or args, malicious server configs |
| **Human-in-the-Loop Bypass** | Validation system bypasses, unauthorized tool execution |
| **Import/Export** | Malicious JSON payloads, path traversal, data injection |
| **Database Queries** | SQL injection in SurrealDB queries |
| **Tool Execution** | Memory, Todo, SubAgent tools - unauthorized actions |
| **IPC Commands** | Tauri command parameter manipulation |

### Out of Scope

Report these to the respective maintainers:

| Area | Report To |
|------|-----------|
| LLM Provider APIs (Mistral, Ollama) | Provider's security team |
| MCP Server code (third-party) | Server maintainer |
| Tauri framework | [Tauri Security](https://github.com/tauri-apps/tauri/security) |
| SurrealDB engine | [SurrealDB Security](https://github.com/surrealdb/surrealdb/security) |
| OS keychain | Operating system vendor |
| Rust/Node.js runtime | Respective security teams |

### Not Applicable

- **XSS**: Desktop app with strict CSP, no untrusted web content
- **CSRF**: No web sessions, local IPC only
- **Physical access attacks**: Local app assumes trusted user
- **Social engineering**: User education, not code issue

## Acknowledgments

We appreciate responsible disclosure and will acknowledge security researchers who report valid vulnerabilities (with your permission).
