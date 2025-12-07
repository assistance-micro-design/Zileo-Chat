/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * @fileoverview Validated invoke wrapper for Tauri IPC.
 *
 * Provides type-safe invoke calls with optional runtime validation.
 */

import { invoke as tauriInvoke } from '@tauri-apps/api/core';
import type { z } from 'zod';

/**
 * Invokes a Tauri command with optional Zod schema validation.
 *
 * @param cmd - Command name
 * @param args - Command arguments
 * @param schema - Optional Zod schema for response validation
 * @returns Validated response
 * @throws ZodError if validation fails
 *
 * @example
 * ```typescript
 * const agents = await invokeValidated('list_agents', {}, z.array(AgentSummarySchema));
 * ```
 */
export async function invokeValidated<T>(
	cmd: string,
	args: Record<string, unknown>,
	schema: z.ZodSchema<T>
): Promise<T> {
	const result = await tauriInvoke<unknown>(cmd, args);
	return schema.parse(result);
}

/**
 * Creates a typed invoke function for a specific command.
 *
 * @param cmd - Command name
 * @param schema - Zod schema for response validation
 * @returns Typed invoke function
 *
 * @example
 * ```typescript
 * const listAgents = createValidatedInvoke('list_agents', z.array(AgentSummarySchema));
 * const agents = await listAgents({});
 * ```
 */
export function createValidatedInvoke<TArgs extends Record<string, unknown>, TResult>(
	cmd: string,
	schema: z.ZodSchema<TResult>
): (args: TArgs) => Promise<TResult> {
	return async (args: TArgs) => {
		const result = await tauriInvoke<unknown>(cmd, args);
		return schema.parse(result);
	};
}
