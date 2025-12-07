/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 */

/**
 * Available tools that agents can use.
 * These correspond to functional tools implemented in the Rust backend.
 *
 * IMPORTANT: This is the single source of truth for tool names on the frontend.
 * The Rust backend has its own registry at src-tauri/src/tools/registry.rs
 */
export const AVAILABLE_TOOLS = [
  'MemoryTool',
  'TodoTool',
  'CalculatorTool',
  'UserQuestionTool',
  'SpawnAgentTool',
  'DelegateTaskTool',
  'ParallelTasksTool'
] as const;

/**
 * Basic tools for memory and task management.
 * These tools do not require AgentToolContext.
 */
export const BASIC_TOOLS = ['MemoryTool', 'TodoTool', 'CalculatorTool', 'UserQuestionTool'] as const;

/**
 * Sub-agent orchestration tools.
 * These tools require AgentToolContext and only available to primary agents.
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
