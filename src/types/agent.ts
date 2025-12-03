// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Agent lifecycle type
 */
export type Lifecycle = 'permanent' | 'temporary';

/**
 * Agent status
 */
export type AgentStatus = 'available' | 'busy';

/**
 * Agent entity
 */
export interface Agent {
  /** Unique identifier */
  id: string;
  /** Agent name */
  name: string;
  /** Lifecycle type */
  lifecycle: Lifecycle;
  /** Current status */
  status: AgentStatus;
  /** List of capabilities */
  capabilities: string[];
  /** List of available tools */
  tools: string[];
  /** List of MCP servers used */
  mcp_servers: string[];
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
  /** System prompt */
  system_prompt: string;
  /** Maximum number of tool execution iterations (1-200, default: 50) */
  max_tool_iterations: number;
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
  /** System prompt (1-10000 chars) */
  system_prompt: string;
  /** Maximum number of tool execution iterations (1-200, default: 50) */
  max_tool_iterations?: number;
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
  /** System prompt (1-10000 chars) */
  system_prompt?: string;
  /** Maximum number of tool execution iterations (1-200) */
  max_tool_iterations?: number;
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
  /** Number of enabled tools */
  tools_count: number;
  /** Number of configured MCP servers */
  mcp_servers_count: number;
}

/**
 * Available tools that agents can use.
 * These correspond to functional tools implemented in the Rust backend.
 */
export const AVAILABLE_TOOLS = [
	'MemoryTool',
	'TodoTool',
	'SpawnAgentTool',
	'DelegateTaskTool',
	'ParallelTasksTool'
] as const;

/**
 * Basic tools for memory and task management
 */
export const BASIC_TOOLS = ['MemoryTool', 'TodoTool'] as const;

/**
 * Sub-agent orchestration tools
 */
export const SUB_AGENT_TOOLS = ['SpawnAgentTool', 'DelegateTaskTool', 'ParallelTasksTool'] as const;

/**
 * Type for available tool names
 */
export type AvailableTool = (typeof AVAILABLE_TOOLS)[number];

/**
 * Type for basic tool names
 */
export type BasicToolName = (typeof BASIC_TOOLS)[number];

/**
 * Type for sub-agent tool names
 */
export type SubAgentToolName = (typeof SUB_AGENT_TOOLS)[number];
