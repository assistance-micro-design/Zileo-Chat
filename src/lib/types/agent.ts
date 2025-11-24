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
  /** List of MCP servers */
  mcp_servers: string[];
  /** System prompt */
  system_prompt: string;
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
