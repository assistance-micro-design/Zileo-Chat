# Changelog

All notable changes to Zileo Chat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.3] - 2026-01-30

### Fixed

- **SurrealDB Panic**: Prevent database panic on null characters in MCP responses
  - Created `sanitize_for_surrealdb()` utility to remove `\0` from JSON strings
  - Applied to MCP call logging, user questions, and embedding imports
- **Token Display**: Sync token counter with streaming in real-time
  - Cross-store synchronization between `streamingStore` and `tokenStore`
- **Agent Config**: Load agent configuration when creating workflow
- **Import/Export**: Add missing `enable_thinking` field for agents
- **Security**: Add native keyring features for API key persistence

### Changed

- **Tool Descriptions**: Improved sub-agent tool descriptions for LLM clarity
  - Added "DO NOT USE WHEN" sections for usage guidance
  - Added ⚠️ CONTEXT ISOLATION warnings
  - Improved examples with structured prompts (TASK/CONTEXT/FOCUS/REPORT)
  - Applied to SpawnAgentTool, DelegateTaskTool, ParallelTasksTool

---

## [0.9.2] - 2026-01-25

### Added

- **Human-in-the-Loop Validation**: Complete validation system for workflow operations
  - Three validation modes: Auto, Manual, Selective
  - Granular control per operation type (Tools, Sub-agents, MCP)
  - Risk threshold overrides (auto-approve-low, always-confirm-high)
  - Dynamic UI showing available tools and MCP servers with status badges
- **New Command**: `list_available_tools` for retrieving tools/MCP info
- **New Type**: `AvailableToolInfo` for tool metadata

### Changed

- **ToolFactory**: Now stores `app_handle` for sub-agent validation support
- **LLMAgent**: Integrated ValidationHelper before tool/MCP execution
- **ValidationSettings UI**: Enhanced with mode-specific displays and visual feedback

### Documentation

- **WORKFLOW_ORCHESTRATION.md**: Added comprehensive "Human-in-the-Loop Validation" section
- **FRONTEND_SPECIFICATIONS.md**: Updated validation settings description
- **API_REFERENCE.md**: Documented new validation commands

---

## [0.9.1] - 2026-01-23

### Added

- **Legal Notices**: GDPR-compliant privacy policy and legal notices accessible from Help menu
- **GitHub Actions**: CI/CD workflows for validation and release

### Changed

- **Dependencies (Rust)**:
  - `keyring` 2.3.3 → 3.6.3 (with API migration: `delete_password` → `delete_credential`)
  - `reqwest` 0.12.24 → 0.12.28
  - `tauri-plugin-opener` 2.5.2 → 2.5.3
  - `thiserror` 1.0.69 → 2.0.17
  - `tracing-subscriber` 0.3.20 → 0.3.22
- **Dependencies (NPM)**:
  - `typescript-eslint` 8.48.1 → 8.53.1
  - `@playwright/test` 1.57.0 → 1.58.0
  - `@tauri-apps/cli` 2.9.5 → 2.9.6
  - `@sveltejs/vite-plugin-svelte` 6.2.1 → 6.2.4
- **GitHub Actions**:
  - `actions/checkout` v4 → v6
  - `actions/setup-node` v4 → v6
  - `actions/download-artifact` v4 → v7
  - `softprops/action-gh-release` v1 → v2

### Fixed

- **CI/CD**: Added frontend dist placeholder for Tauri compile-time validation
- **CI/CD**: Added clang/llvm for RocksDB compilation in CI
- **CI/CD**: Added rustup targets for macOS universal binary builds
- **Security**: Updated keyring API for v3.x compatibility (`delete_credential`)
- **Error Handling**: Replaced `unwrap()` with proper pattern matching in production code (`models.rs`)
- **Clippy Warnings**: Fixed 13 clippy warnings in test code

### Documentation

- **ROADMAP_TO_1.0.md**: Updated with detailed analysis of `unwrap()`/`expect()` occurrences
- **DEPLOYMENT_GUIDE.md**: Added GitHub Actions configuration

---

## [0.9.0-beta] - 2025-12-14

### Added

- **Multi-Agent System**: Full CRUD operations for agents via Settings UI
- **Tool System**: 7 integrated tools (Memory, Todo, Calculator, UserQuestion, InternalReport, SubAgent, WebSearch)
- **MCP Integration**: Support for Docker, NPX, and UVX MCP servers
- **Sub-Agent System**: Agent delegation with parent-child relationships
- **i18n Support**: English and French translations
- **Settings Navigation**: Route-based settings with deep linking
- **Circuit Breaker**: Resilience pattern for UserQuestionTool
- **Virtual Scrolling**: Performance optimization for large lists

### Changed

- **Icon Library**: Migrated from `lucide-svelte` to `@lucide/svelte` (OPT-FA-12)
- **Workflow Executor**: Extracted as dedicated service (OPT-FA-8)
- **PageState Interface**: Aggregated for cleaner component architecture (OPT-FA-9)
- **Tool Descriptions**: Optimized for token efficiency (OPT-TD-1 to OPT-TD-8)

### Performance

- **Scroll Optimization**: WebKit2GTK scroll performance improvements (OPT-SCROLL)
- **Messages Area**: Virtual scroll and derived store consolidation (OPT-MSG-1 to OPT-MSG-6)
- **Activity Feed**: Memoized filtering and lazy-loaded modals (OPT-FA-7 to OPT-FA-13)
- **Workflow Engine**: Reduced N+1 queries, optimized streaming (OPT-WF-1 to OPT-WF-9)
- **TodoTool**: Parameterized queries, reduced N+1 patterns (OPT-TODO-1 to OPT-TODO-12)
- **MemoryTool**: Query consolidation and input validation (OPT-MEM-1 to OPT-MEM-8)
- **UserQuestionTool**: Strategic optimizations with circuit breaker (OPT-UQ-1 to OPT-UQ-12)

### Fixed

- **LLM Provider**: Removed erroneous `#[allow(dead_code)]` attributes
- **Virtual Scroll**: Fixed overflow issues in ActivityFeed and MemoryList
- **MCP Resilience**: Added timeouts, retry logic, and sub-agent heartbeat fixes
- **Integration Tests**: Updated for new ToolFactory API

### Security

- **SQL Injection Prevention**: Parameterized queries across all tools
- **API Key Storage**: Tauri secure storage with AES-256 encryption
- **CSP Policy**: Strict Content Security Policy (`default-src 'self'`)

### Documentation

- Comprehensive documentation in `docs/` directory
- API Reference with all Tauri command signatures
- MCP Configuration Guide
- Multi-Agent Architecture documentation
- Tool development patterns and examples

---

## [Unreleased]

### Planned for 1.0.0

- Replace 69 `unwrap()`/`expect()` calls with proper error handling
- Integration tests with ephemeral SurrealDB
- E2E tests with Playwright
- macOS and Windows distribution packages

---

## Project History

### Phase 0 - Project Setup
- Initial Tauri + SvelteKit + Rust configuration
- SurrealDB embedded integration
- TypeScript/Rust type synchronization

### Phase 1-2 - Database Foundation
- SurrealDB schema design (SCHEMAFULL tables)
- Agent, Workflow, Memory persistence
- Query patterns and utilities

### Phase 3 - Multi-Agent Infrastructure
- Agent lifecycle management
- Tool registry and factory patterns
- MCP client/server architecture

### Phase 4 - Command Layer
- Tauri IPC commands
- Frontend-backend communication
- Error handling patterns

### Phase 5 - Frontend Implementation
- SvelteKit routing and stores
- Component library (atomic design)
- Theme system and i18n

### Phase 6-9 - Optimization Sprints
- Performance profiling and fixes
- Security hardening
- Documentation sync

---

[0.9.3]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.3
[0.9.2]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.2
[0.9.1]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.1
[0.9.0-beta]: https://github.com/assistance-micro-design/Zileo-Chat/releases/tag/v0.9.0-beta
[Unreleased]: https://github.com/assistance-micro-design/Zileo-Chat/compare/v0.9.3...HEAD
