/**
 * Copyright 2025 Assistance Micro Design
 * SPDX-License-Identifier: Apache-2.0
 *
 * Shared frontend constants.
 */

/**
 * Bounds for an agent's max tool-call iterations.
 *
 * Synchronized with backend clamping in `src-tauri/src/main.rs`
 * (`max_tool_iterations.clamp(1, 200)`) and `agents/execution/tool_loop.rs`.
 */
export const ITERATIONS_LIMITS = {
	MIN: 1,
	MAX: 200,
	DEFAULT: 50
} as const;
