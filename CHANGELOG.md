# Changelog

All notable changes to Zileo Chat will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.9.1-beta] - 2025-12-14

### Fixed

- **Error Handling**: Replaced `unwrap()` with proper pattern matching in production code (`models.rs`)
- **Clippy Warnings**: Fixed 13 clippy warnings in test code:
  - Unused variable prefixed with `_` (`sub_agent_executor.rs`)
  - Replaced `assert!(true)` with meaningful assertions (`sub_agent_execution.rs`)
  - Fixed constant assertions by using local variables (`pricing.rs`, `user_question/tests.rs`, `sub_agent_executor.rs`)
  - Removed `::default()` on unit struct (`calculator/tool.rs`)

### Documentation

- **ROADMAP_TO_1.0.md**: Updated with detailed analysis of `unwrap()`/`expect()` occurrences (68/69 in tests, 1 fixed in production)

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
- CI/CD pipeline with GitHub Actions

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

[0.9.0-beta]: https://github.com/assistance-micro-design/Zileo-Chat-3/releases/tag/v0.9.0-beta
[Unreleased]: https://github.com/assistance-micro-design/Zileo-Chat-3/compare/v0.9.0-beta...HEAD
