# Frontend Specifications

> **Stack**: SvelteKit 2 | Svelte 5 | Vite 7 | Tauri 2
> **Target**: Desktop/Laptop only | Fullscreen mode
> **Architecture**: Multi-workflow concurrent execution with real-time indicators

## Overview

The application has two main pages accessible via a floating top menu: **Settings** (configuration) and **Agent** (workflow execution). Both use a sidebar + content layout.

### Workflow Interaction Flow

1. User submits input
2. If a workflow is already running, the message is queued; otherwise it is processed immediately
3. Depending on validation mode (Auto / Manual / Selective), the operation either executes directly or pauses for user approval
4. Operations include: tool calls, sub-agent spawns, MCP server calls, file/DB operations
5. Results stream to the UI in real time (tokens, tool status, reasoning steps)
6. On workflow completion, the message queue is drained

## 1. Floating Menu

Fixed top navigation bar with backdrop blur, linking to `/settings` and `/agent`. Supports keyboard navigation and icon + label buttons.

See `src/lib/components/layout/FloatingMenu.svelte`

## 2. Settings Page

### Route Structure

Route-based architecture with code splitting per section. The settings layout provides a collapsible sidebar for navigation.

| Route | Section | Description |
|-------|---------|-------------|
| `/settings` | (redirect) | Redirects to `/settings/providers` |
| `/settings/providers` | Providers | LLM provider list (Mistral, Ollama, custom OpenAI-compatible), API keys, connection test, enable/disable toggle |
| `/settings/agents` | Agents | Agent CRUD (permanent + temporary), filtering by name/type/usage, search bar |
| `/settings/mcp` | MCP Servers | MCP server list, connection settings (stdio/docker/HTTP/SSE), status monitoring, capability/tool listing, logs |
| `/settings/memory` | Memory | Embedding model selection (per provider), chunking settings, memory CRUD table with semantic search, manual memory creation, bulk export/import/purge, statistics |
| `/settings/validation` | Validation | Validation mode (Auto/Manual/Selective), granular toggles per operation type (tools, sub-agents, MCP, file ops, DB ops), risk threshold overrides, audit settings (timeout, retention) |
| `/settings/audit-log` | Audit Log | Validation audit log: list with filters (tool, decision, risk, date), summary stats, manual purge, CSV export |
| `/settings/prompts` | Prompts | Prompt library with name/description/category/content/variables, duplicate/export/versioning |
| `/settings/skills` | Skills | Skill document CRUD |
| `/settings/import-export` | Import/Export | Data portability (schema v1.2, 6 entity types) |
| `/settings/theme` | Theme | Light/Dark/Auto theme, color scheme, font settings, live preview |

See `src/routes/settings/` for all section pages.

### Settings Components

Settings-specific components live under `src/lib/components/settings/`:

| Subdirectory | Components | Description |
|--------------|------------|-------------|
| `agents/` | AgentSettings, AgentList, AgentForm | Agent CRUD with list and form views |
| `memory/` | MemorySettings, MemoryList, MemoryForm | Memory management with embedding config and CRUD |
| `prompts/` | PromptSettings, PromptList, PromptForm | Prompt library management |
| `skills/` | SkillSettings, SkillList, SkillForm | Skill document management |
| `validation/` | ValidationSettings, ValidationInfoCard | Validation mode configuration with dynamic tool/server badges, timeout + audit retention controls |
| `audit-log/` | AuditLogFilters, AuditLogList, AuditLogRow, AuditLogStats | Validation audit log explorer with filters, paginated list, summary stats |
| `import-export/` | ImportExportSettings, ExportPanel, ImportPanel, EntitySelector, ExportPreview, ImportPreview, ConflictResolver, MCPFieldEditor, MCPEnvEditor | Full import/export workflow with conflict resolution |

## 3. Agent Page

### Layout

Two-column layout: workflow sidebar (left) + main content area (input, output stream, metrics/tools panel).

See `src/routes/agent/+page.svelte`

### Workflow Sidebar

Displays all workflows with status indicators (running/idle/completed/error), inline name editing, and CRUD operations. Sorting by status, date, or name. Collapsible.

See `src/lib/components/agent/WorkflowSidebar.svelte` and `src/lib/components/workflow/`

### Chat Input

Text input area with prompt selector modal. Users can attach saved prompts with auto-detected variable placeholders. Input remains active during workflow execution; messages are queued with a visible badge and reorderable queue.

See `src/lib/components/chat/ChatInput.svelte` and `src/lib/components/chat/PromptSelectorModal.svelte`

### Validation System (Human-in-the-Loop)

When validation is required, the workflow pauses and displays a modal showing the operation type, details, and risk level. Users can approve, reject, or approve all pending validations.

Keyboard shortcuts: `Ctrl+Enter` (approve), `Ctrl+Shift+Enter` (approve all), `Esc` (reject).

See `src/lib/components/workflow/ValidationModal.svelte`

### Token Display

Real-time token counter showing `current / max` with tokens/s during streaming. Warning states: green (0-75%), orange (75-90%), red (90-100%), error (100%+). Pulse animation only at critical level.

See `src/lib/components/workflow/TokenDisplay.svelte`

### Tools and MCP Panel

Lists active tools with execution status and duration, plus MCP server call counts and average latency. Updates in real time via Tauri event streaming.

See `src/lib/components/chat/ToolCallBlock.svelte`

### Sub-Agent Display

Cards for each running sub-agent showing name, status, current task, progress bar, and expandable tool list.

See `src/lib/components/chat/SubAgentBlock.svelte`

### Reasoning Display

Collapsible panel showing reasoning/thinking steps with token count and duration per step. Streams in real time with auto-scroll. Only visible when the model supports reasoning.

See `src/lib/components/chat/ThinkingBlock.svelte`

### Agent Configuration

Modal dialog for per-agent settings: model selection, temperature, max tokens, system prompt, tool toggles, and MCP server toggles. Also supports a multi-step wizard for creating custom agents (Basic Info, Model, Capabilities, Review).

See `src/lib/components/agent/AgentHeader.svelte`

## 4. Multi-Workflow Concurrent Execution

| Validation Mode | Max Concurrent | Behavior |
|----------------|----------------|----------|
| Auto | 3 | Multiple workflows run in background |
| Manual | 1 | Single workflow at a time |
| Selective | 1 | Single workflow at a time |

Enforcement is done in both frontend (`backgroundWorkflowsStore.canStart()`) and backend. Warning toast shown when limit is reached. Background workflows fire toast notifications for completion and user questions.

See `WORKFLOW_ORCHESTRATION.md` for full architecture.

### Keyboard Shortcuts

- `Ctrl+Tab` / `Ctrl+Shift+Tab`: Next/previous workflow
- `Ctrl+T`: New workflow
- `Ctrl+W`: Close current workflow
- `Ctrl+1-9`: Jump to workflow N

### Persistence

Workflows are auto-saved to SurrealDB. On startup, non-terminated workflows are restored and the user is prompted to resume any that were running (crash recovery).

## 5. Component Library

101 total components organized under `src/lib/components/`:

| Directory | Count | Description |
|-----------|-------|-------------|
| `ui/` | 20 | Atomic UI: Badge, Button, Card, ContextMenu, DeleteConfirmModal, ErrorBanner, HelpButton, Input, LanguageSelector, MarkdownRenderer, Modal, PasswordInput, ProgressBar, Select, Skeleton, Spinner, StatusIndicator, Textarea, ToastContainer, ToastItem |
| `layout/` | 3 | AppContainer, FloatingMenu, Sidebar |
| `agent/` | 3 | AgentHeader, ChatContainer, WorkflowSidebar |
| `chat/` | 11 | ChatInput, ExecutionSpinner, MessageBubble, MessageList, MessageListSkeleton, MessageMetrics, PromptSelectorModal, SubAgentBlock, ThinkingBlock, TodoTasksBlock, ToolCallBlock |
| `workflow/` | 10 | AgentSelector, FolderItem, NewWorkflowModal, StatusFilters, TokenDisplay, UserQuestionModal, ValidationModal, WorkflowItem, WorkflowItemCompact, WorkflowList |
| `legal/` | 1 | LegalModal |
| `mcp/` | 3 | MCPServerCard, MCPServerForm, MCPServerTester |
| `llm/` | 4 | ConnectionTester, ModelCard, ModelForm, ProviderCard |
| `settings/` | 37 | See Settings Components table above (incl. `audit-log/` and enriched `validation/`) |
| `onboarding/` | 9 | OnboardingModal, OnboardingProgress, steps: Welcome, Language, Theme, ApiKey, Values, Import, Complete |

## 6. Stores

| Store | Purpose | File |
|-------|---------|------|
| `agents` | Agent CRUD with reactive state (agents, selectedAgent, isLoading, hasAgents) | `src/lib/stores/agents.ts` |
| `audit-log` | Validation audit log entries, filters, stats, pagination | `src/lib/stores/audit-log.ts` |
| `background-workflows` | Concurrent background workflow dispatch (canStartNew, runningWorkflowIds, questionPendingIds) | `src/lib/stores/background-workflows.ts` |
| `execution-blocks` | Execution block state for chat display | `src/lib/stores/execution-blocks.ts` |
| `folders` | Workflow folder management | `src/lib/stores/folders.ts` |
| `llm` | LLM provider/model state + custom provider CRUD (pure functions) | `src/lib/stores/llm/` |
| `locale` | i18n language management | `src/lib/stores/locale.ts` |
| `mcp` | MCP server state (pure functions) | `src/lib/stores/mcp/` |
| `onboarding` | First-launch wizard state | `src/lib/stores/onboarding.ts` |
| `prompts` | Prompt library management | `src/lib/stores/prompts.ts` |
| `skills` | Skill CRUD via createCRUDStore factory | `src/lib/stores/skills.ts` |
| `streaming` | Real-time workflow execution (isStreaming, streamContent, activeTools, reasoningSteps) | `src/lib/stores/streaming.ts` |
| `theme` | Light/dark mode with localStorage persistence | `src/lib/stores/theme.ts` |
| `toast` | Toast notifications for background workflow events | `src/lib/stores/toast.ts` |
| `tokens` | Token usage/cost tracking (streaming + cumulative) | `src/lib/stores/tokens.ts` |
| `user-question` | User question modal state | `src/lib/stores/user-question.ts` |
| `validation` | Human-in-the-loop validation requests | `src/lib/stores/validation.ts` |
| `validation-settings` | Validation configuration persistence | `src/lib/stores/validation-settings.ts` |
| `workflows` | Workflow management with pure functions + reactive store | `src/lib/stores/workflows.ts` |

## 7. Types

25 type files (including `index.ts`) in `src/types/`. Always import via `$types/module`.

| Module | Key Types | Description |
|--------|-----------|-------------|
| `agent.ts` | Agent, AgentConfig, AgentConfigCreate, AgentSummary, LLMConfig | Agent configuration |
| `background-workflow.ts` | BackgroundWorkflowStatus, WorkflowStreamState, Toast, ToastType | Background workflows and toasts |
| `chat-block.ts` | Chat message block types | Chat message block parsing |
| `custom-provider.ts` | ProviderInfo, CreateCustomProviderRequest | Custom OpenAI-compatible providers |
| `embedding.ts` | Embedding config types | Vector embeddings |
| `i18n.ts` | Locale, LocaleInfo, LOCALES | Internationalization |
| `import-export.ts` | Import/export structures | Backup/restore |
| `llm.ts` | LLMModel, ProviderSettings, ConnectionTestResult, LLMState, ProviderType | LLM providers |
| `mcp.ts` | MCPServer, MCPServerConfig, MCPServerConfigWithSecret, MCPAuthType, MCPAuthMetadata, MCPAuthSecret, LegacyHttpAuthWarning, MCPTool, MCPTestResult | MCP servers (HTTP auth: Bearer/API Key/Basic, secrets in keychain) |
| `memory.ts` | Memory, MemoryType | Memory/RAG |
| `message.ts` | Message | Chat messages |
| `onboarding.ts` | Onboarding state types | First-launch wizard |
| `prompt.ts` | Prompt, PromptCreate, PromptSummary, PromptCategory | Prompt library |
| `services.ts` | ModalState | Service layer |
| `sidebar.ts` | Sidebar state types | Sidebar state management |
| `skill.ts` | Skill, SkillCreate, SkillUpdate, SkillSummary, SkillCategory | Skill documents |
| `streaming.ts` | StreamChunk, WorkflowComplete, ChunkType | Streaming events |
| `sub-agent.ts` | SubAgentExecution, ValidationRequiredEvent | Sub-agent execution |
| `thinking.ts` | ThinkingStep | Reasoning steps |
| `tool.ts` | ToolExecution, WorkflowToolExecution | Tool execution |
| `user-question.ts` | User question types | User question events |
| `validation.ts` | ValidationRequest, ValidationType, RiskLevel | Validation requests |
| `workflow.ts` | Workflow, WorkflowResult, WorkflowMetrics, WorkflowFullState | Workflow execution |

## 8. Utilities and Services

### Utilities (`src/lib/utils/`, 17 modules)

| Module | Key Exports | Description |
|--------|-------------|-------------|
| `modal.svelte.ts` | `createModalController<T>()` | Factory for modal state management (show/mode/editing) using Svelte 5 runes |
| `async.ts` | `createAsyncHandler()`, `withLoadingState()` | Async operation wrappers with loading/error handling |
| `error.ts` | `getErrorMessage()`, `formatErrorForDisplay()` | Error extraction and formatting |
| `activity.ts` | `combineActivities()`, `filterActivities()` | Activity feed helpers |
| `contextMenu.ts` | Context menu helpers | Context menu positioning and state |
| `dateGrouping.ts` | Date grouping helpers | Group items by date (today, yesterday, older) |
| `dragDrop.ts` | Drag and drop helpers | Drag and drop event handling |
| `url.ts` | `isAllowedScheme()` | URL scheme validation for safe external links |
| `duration.ts` | `formatDuration()` | Duration formatting (ms / s / m,s) |
| `debounce.ts` | `debounce()` | Debounce wrapper |
| `uuid.ts` | `isUuid()` | Canonical 8-4-4-4-12 hex UUID validation |
| `constants.ts` | `ITERATIONS_LIMITS` | Shared frontend constants (synchronized with backend clamping) |
| `settings-refresh.ts` | `onSettingsRefresh()`, `attachSettingsRefreshListener()`, `SETTINGS_REFRESH_EVENT` | Subscribe to the global `settings:refresh` event after import/export |
| `mcp-auth-validation.ts` | MCP HTTP auth validators | Validates `MCPAuthMetadata`/`MCPAuthSecret` symmetrically with the Rust backend |
| `agent-reasoning.ts` | `getReasoningOptions()`, `getReasoningHelp()`, `normalizeReasoningEffortForProvider()` | Provider-aware reasoning_effort selector helpers (Mistral exposes Off/High only; other providers expose Off/Low/Medium/High) |
| `index.ts` | Re-exports | Barrel file |

### Actions (`src/lib/actions/`, 1 module)

| Module | Key Exports | Description |
|--------|-------------|-------------|
| `focusTrap.ts` | `focusTrap` (Svelte 5 attachment) | WCAG 2.1 modal keyboard focus trap with Tab cycling and focus restoration on teardown |

### Services (`src/lib/services/`, 7 modules including index)

| Module | Key Exports | Description |
|--------|-------------|-------------|
| `block.service.ts` | Block parsing helpers | Chat block parsing (tool calls, thinking, sub-agents) |
| `message.service.ts` | `MessageService.load()`, `.save()` | Message CRUD with error handling |
| `sub-agent-execution.service.ts` | Sub-agent execution helpers | Sub-agent invocation and lifecycle |
| `workflow.service.ts` | `WorkflowService.execute()`, `.cancel()` | Workflow execution management |
| `workflowExecutor.service.ts` | `WorkflowExecutorService.execute()` | 8-step workflow orchestration with concurrency check |
| `localStorage.service.ts` | `LocalStorage.get()`, `.set()`, `STORAGE_KEYS` | Typed localStorage access |

## 9. Frontend-Backend Communication

### Tauri Commands

Frontend calls backend via `invoke()` from `@tauri-apps/api/core`. Parameter names are automatically converted from camelCase (TypeScript) to snake_case (Rust).

### Streaming (Tauri Events)

Real-time updates use Tauri's event system via `listen()`. The backend emits `workflow_stream` events with typed `StreamChunk` payloads containing token content, tool start/end, reasoning steps, and sub-agent updates.

### Key Patterns

- **PageState**: Aggregate page state into a single `$state<PageState>()` reactive object instead of many individual state variables
- **Streaming store**: 14 derived stores (consolidated from 28) for filtering streaming data
- **Props**: Use `$props()` with typed `Props` interface (Svelte 5 pattern)

## 10. Accessibility (WCAG AA)

- Full keyboard navigation: Tab, Shift+Tab, Enter/Space, Esc, Arrow keys
- ARIA labels on all interactive elements and status indicators
- Focus management for modals (auto-focus first element on open)
- Color contrast minimum 4.5:1 (normal text), 3:1 (large text)

## 11. Performance Optimizations

### Settings Page

| Optimization | Impact | Location |
|-------------|--------|----------|
| Route-based navigation | Code splitting, lazy loading | `src/routes/settings/*` |
| Modal backdrop (no blur) | 15-30% GPU improvement | `global.css` |
| GPU scroll acceleration | GPU acceleration | `+layout.svelte` |
| CSS containment on grids | ~10% layout time reduction | Grid section CSS |
| Memoized selectors | ~5-10% JS execution reduction | `llm.ts` |
| Virtual scrolling (MemoryList) | ~20 DOM nodes vs 20000 | `MemoryList.svelte` |
| Animation pause on scroll | ~5% GPU during scroll | `global.css` |

### Agent Page

| Optimization | Impact | Location |
|-------------|--------|----------|
| Conditional animations | 60% GPU reduction (green/warning) | `TokenDisplay.svelte` |
| Virtual scroll ActivityFeed | 90% DOM reduction for 100+ items | `ActivityFeed.svelte` |
| CSS containment on message lists | Layout isolation for long conversations | `MessageList.svelte` |
| `content-visibility: auto` | Off-screen messages skip rendering | Message wrappers |
| Debounced token counting | Reduces IPC calls | ChatInput |

### General Strategies

- **Lazy loading**: Heavy settings components loaded on demand via SvelteKit routes
- **Virtual scrolling**: `@humanspeak/svelte-virtual-list` for lists exceeding 20 items
- **Memoization**: Svelte 5 `$derived` for computed values
- **CSS containment**: `contain: layout style` on grid containers (avoid `contain: content` which breaks fixed modals)

## 12. Styling Architecture

Theme system using CSS custom properties with light/dark mode support via `[data-theme="dark"]`. Variables cover colors, spacing, typography, shadows, and transitions. All component styles are scoped via Svelte's `<style>` blocks.

See `src/app.css` and `src/lib/styles/`

## 13. Testing Strategy

- **Unit tests**: Vitest + `@testing-library/svelte` for component and store tests (280+ tests; run `npm run test` for the current count)
- **E2E tests**: Playwright for workflow persistence, keyboard navigation, streaming indicators, responsive layout
- **Backend tests**: Rust unit tests for all Tauri commands (1000+ tests; run `cargo test --lib` for the current count)

## References

- [SvelteKit Docs](https://kit.svelte.dev/docs)
- [Svelte 5 Runes](https://svelte.dev/docs/svelte/what-are-runes)
- [Tauri IPC](https://v2.tauri.app/develop/calling-rust/)
