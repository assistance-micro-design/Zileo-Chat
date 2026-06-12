/**
 * Copyright 2025 Assistance Micro Design
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

/**
 * Reasoning effort level for thinking models.
 *
 * `xhigh` ("Think Max") is only meaningful on providers that expose an
 * `xhigh` tier, currently DeepSeek V4 routed via OpenAI-compatible gateways.
 * On Mistral and any provider without an xhigh tier the backend collapses
 * it to `high` (cf `ReasoningEffort::to_mistral_str`); the UI hides the
 * option unless the selected model's `api_name` looks like DeepSeek.
 */
export type ReasoningEffort = 'low' | 'medium' | 'high' | 'xhigh';

/**
 * Agent lifecycle type
 */
export type Lifecycle = 'permanent' | 'temporary';

/**
 * Agent specialization (meta-role). `kind = undefined` = standard agent.
 */
export type AgentKind = 'kanban';

/**
 * One entry of an agent's MCP tool allowlist.
 *
 * Lists, per immutable `server_id` (never the display name, so it survives a
 * server rename), the exact MCP tool names the agent may call in an unattended
 * (detached) run: auto-analyze, compose-card, worker re-run. An empty/absent
 * allowlist means nothing is armed → every MCP tool is refused in detached.
 */
export interface McpToolAllowlistEntry {
	/** Immutable MCP server id (not the display name). */
	server_id: string;
	/** Exact tool names auto-approved for this server in detached runs. */
	tools: string[];
	/**
	 * Whether the armed tools are also authorized when this agent runs as a
	 * delegated/parallel sub-agent of a detached parent (not just in a direct
	 * detached run). Defaults to `false` (strict) — closes the delegation
	 * confused-deputy. Does not apply to spawned sub-agents (they clone the
	 * parent's allowlist). Matches Rust `#[serde(default)] bool`.
	 */
	allow_in_delegated_runs: boolean;
}

/**
 * Agent configuration
 */
export interface AgentConfig {
	/** Unique identifier */
	id: string;
	/** Agent name */
	name: string;
	/** Lifecycle type */
	lifecycle: Lifecycle;
	/** LLM configuration */
	llm: LLMConfig;
	/** List of available tools */
	tools: string[];
	/** MCP server NAMES (not IDs) that the agent can use */
	mcp_servers: string[];
	/** Skill names assigned to this agent */
	skills: string[];
	/** Authorized directory paths for FileManagerTool */
	folders: string[];
	/** Require user confirmation for destructive file operations (default: true) */
	require_file_confirmation: boolean;
	/** System prompt */
	system_prompt: string;
	/** Maximum number of tool execution iterations (1-200, default: 50) */
	max_tool_iterations: number;
	/** Reasoning effort for thinking models (absent = disabled) */
	reasoning_effort?: ReasoningEffort;
	/** Agent specialization. Absent = standard agent. */
	kind?: AgentKind;
	/** When true and kind === 'kanban', the agent auto-analyzes workflow reports on completion. */
	auto_analyze_reports?: boolean;
	/** MCP tools auto-approved for this agent in unattended (detached) runs. */
	mcp_tool_allowlist?: McpToolAllowlistEntry[];
}

/**
 * LLM provider configuration
 */
export interface LLMConfig {
	/** Provider name (e.g., "Mistral", "Ollama") */
	provider: string;
	/** Model name */
	model: string;
	/** Sampling temperature */
	temperature: number;
	/** Maximum tokens to generate */
	max_tokens: number;
	/** Whether the model is a reasoning/thinking model (from DB) */
	is_reasoning: boolean;
	/** Context window size in tokens (from model config, passed to providers like Ollama as num_ctx) */
	context_window?: number;
}

/**
 * Agent configuration for creation (without ID, timestamps)
 */
export interface AgentConfigCreate {
	/** Agent name (1-64 chars) */
	name: string;
	/** Lifecycle type */
	lifecycle: Lifecycle;
	/** LLM configuration */
	llm: LLMConfig;
	/** List of available tools */
	tools: string[];
	/** List of MCP servers */
	mcp_servers: string[];
	/** Skill names assigned to this agent */
	skills: string[];
	/** Authorized directory paths for FileManagerTool */
	folders: string[];
	/**
	 * Require user confirmation for destructive file operations.
	 * Optional on create: omitted by AgentForm so a new agent takes the
	 * fail-safe backend default (true). Edited on Settings > Validation.
	 */
	require_file_confirmation?: boolean;
	/** System prompt (1-10000 chars) */
	system_prompt: string;
	/** Maximum number of tool execution iterations (1-200, default: 50) */
	max_tool_iterations: number;
	/** Reasoning effort for thinking models (absent = disabled) */
	reasoning_effort?: ReasoningEffort;
	/** Agent specialization. Absent = standard agent. */
	kind?: AgentKind;
	/** When true and kind === 'kanban', the agent auto-analyzes workflow reports on completion. */
	auto_analyze_reports?: boolean;
	/** MCP tools auto-approved for this agent in unattended (detached) runs. */
	mcp_tool_allowlist?: McpToolAllowlistEntry[];
}

/**
 * Agent configuration for updates (all fields optional except lifecycle which cannot change)
 */
export interface AgentConfigUpdate {
	/** Agent name (1-64 chars) */
	name?: string;
	/** LLM configuration */
	llm?: LLMConfig;
	/** List of available tools */
	tools?: string[];
	/** List of MCP servers */
	mcp_servers?: string[];
	/** Skill names assigned to this agent */
	skills?: string[];
	/** Authorized directory paths for FileManagerTool */
	folders?: string[];
	/** Require user confirmation for destructive file operations */
	require_file_confirmation?: boolean;
	/** System prompt (1-10000 chars) */
	system_prompt?: string;
	/** Maximum number of tool execution iterations (1-200) */
	max_tool_iterations?: number;
	/** Reasoning effort for thinking models */
	reasoning_effort?: ReasoningEffort | null;
	/** Agent specialization (tri-state: absent = keep, null = clear, value = set). */
	kind?: AgentKind | null;
	/** Auto-analyze workflow reports flag (absent = keep, value = set). */
	auto_analyze_reports?: boolean;
	/** MCP tool allowlist (absent = keep existing, value = replace). */
	mcp_tool_allowlist?: McpToolAllowlistEntry[];
}

/**
 * Agent summary for listing (lightweight representation)
 */
export interface AgentSummary {
	/** Unique identifier */
	id: string;
	/** Agent name */
	name: string;
	/** Lifecycle type */
	lifecycle: Lifecycle;
	/** LLM provider name */
	provider: string;
	/** LLM model name */
	model: string;
	/**
	 * Reasoning effort configured for the agent, if any. Mirrors Rust
	 * `Option<ReasoningEffort>` with skip-when-none, hence optional. Surfaced
	 * so the conversation header can show the reasoning badge without loading
	 * the full agent config.
	 */
	reasoning_effort?: ReasoningEffort;
	/** Number of enabled tools */
	tools_count: number;
	/** Number of configured MCP servers */
	mcp_servers_count: number;
	/** Number of assigned skills */
	skills_count: number;
	/** Number of authorized folders */
	folders_count: number;
	/** Specialization, if any (e.g. 'kanban'). */
	kind?: AgentKind;
	/**
	 * True when the agent has at least one MCP tool auto-approved for unattended
	 * (detached) runs (a non-empty allowlist entry). Used by the per-agent
	 * authorizations UI to flag agents whose MCP auto-approval has been
	 * configured. `require_file_confirmation` is deliberately NOT part of this
	 * (it is near-universally off and would defeat differentiation). Matches Rust
	 * `#[serde(default)] bool`.
	 */
	has_mcp_auto_approval: boolean;
}

// Re-export tool constants from centralized location
export {
	AVAILABLE_TOOLS,
	BASIC_TOOLS,
	SUB_AGENT_TOOLS,
	type AvailableTool,
	type BasicToolName,
	type SubAgentToolName
} from '$lib/constants/tools';
