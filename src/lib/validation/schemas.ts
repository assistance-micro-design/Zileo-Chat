/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * @fileoverview Zod schemas for runtime validation of IPC responses.
 *
 * These schemas validate data received from Tauri commands to catch
 * type mismatches at runtime, providing better error messages.
 */

import { z } from 'zod';

// ============================================================================
// LLM Schemas
// ============================================================================

export const ProviderTypeSchema = z.string().min(1).max(64);

export const LLMConfigSchema = z.object({
	provider: z.string(),
	model: z.string(),
	temperature: z.number().min(0).max(2),
	max_tokens: z.number().positive()
});

export const LLMModelSchema = z.object({
	id: z.string(),
	provider: ProviderTypeSchema,
	name: z.string(),
	api_name: z.string(),
	context_window: z.number().positive(),
	max_output_tokens: z.number().positive(),
	temperature_default: z.number().min(0).max(2),
	is_builtin: z.boolean(),
	is_reasoning: z.boolean(),
	input_price_per_mtok: z.number().nonnegative(),
	output_price_per_mtok: z.number().nonnegative(),
	created_at: z.string(),
	updated_at: z.string()
});

export const ProviderSettingsSchema = z.object({
	provider: ProviderTypeSchema,
	enabled: z.boolean(),
	default_model_id: z.string().nullable(),
	api_key_configured: z.boolean(),
	base_url: z.string().nullable(),
	updated_at: z.string()
});

// ============================================================================
// Agent Schemas
// ============================================================================

export const LifecycleSchema = z.enum(['permanent', 'temporary']);

export const AgentConfigSchema = z.object({
	id: z.string(),
	name: z.string().min(1).max(64),
	lifecycle: LifecycleSchema,
	llm: LLMConfigSchema,
	tools: z.array(z.string()),
	mcp_servers: z.array(z.string()),
	system_prompt: z.string(),
	max_tool_iterations: z.number().min(1).max(200),
	enable_thinking: z.boolean().optional()
});

export const AgentSummarySchema = z.object({
	id: z.string(),
	name: z.string(),
	lifecycle: LifecycleSchema,
	provider: z.string(),
	model: z.string(),
	tools_count: z.number().nonnegative(),
	mcp_servers_count: z.number().nonnegative()
});

// ============================================================================
// Memory Schemas
// ============================================================================

export const MemoryTypeSchema = z.enum(['user_pref', 'context', 'knowledge', 'decision']);

export const MemorySchema = z.object({
	id: z.string(),
	type: MemoryTypeSchema,
	content: z.string(),
	workflow_id: z.string().optional(),
	metadata: z.record(z.string(), z.unknown()),
	created_at: z.string()
});

// ============================================================================
// Type exports (infer types from schemas)
// ============================================================================

export type ValidatedLLMConfig = z.infer<typeof LLMConfigSchema>;
export type ValidatedAgentConfig = z.infer<typeof AgentConfigSchema>;
export type ValidatedAgentSummary = z.infer<typeof AgentSummarySchema>;
export type ValidatedMemory = z.infer<typeof MemorySchema>;
