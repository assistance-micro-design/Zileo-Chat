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
 * @fileoverview Thinking step types for persistence and display.
 *
 * These types are synchronized with Rust backend types:
 * - src-tauri/src/models/thinking_step.rs (ThinkingStep, ThinkingStepCreate)
 *
 * Thinking Steps Persistence
 *
 * @module types/thinking
 */

/**
 * Thinking step record from database (matches Rust ThinkingStep)
 *
 * Represents a single reasoning step captured during agent execution.
 * Useful for debugging, transparency, and understanding agent behavior.
 */
export interface ThinkingStep {
	/** Unique identifier (UUID) */
	id: string;
	/** Associated workflow ID */
	workflow_id: string;
	/** Associated message ID (the assistant message this thinking belongs to) */
	message_id: string;
	/** Agent ID that generated this thinking step */
	agent_id: string;
	/** Step number within the reasoning sequence (0-indexed) */
	step_number: number;
	/** Content of the thinking step (the actual reasoning text) */
	content: string;
	/** Duration to generate this step in milliseconds (optional) */
	duration_ms?: number;
	/** Number of tokens in this step (optional) */
	tokens?: number;
	/** Global ordering sequence within execution (for interleaving with tool executions) */
	sequence: number;
	/** Source of this reasoning step: "agent_flow" (synthetic) or "model_thinking" (real model output) */
	source: 'agent_flow' | 'model_thinking';
	/** Timestamp when the step was recorded (ISO 8601 string) */
	created_at: string;
}

/**
 * Truncates thinking content for preview display
 *
 * @param content - Full thinking content
 * @param maxLength - Maximum length for preview (default 150)
 * @returns Truncated content with ellipsis if needed
 */
export function truncateThinkingContent(content: string, maxLength: number = 150): string {
	if (content.length <= maxLength) {
		return content;
	}
	return content.slice(0, maxLength - 3) + '...';
}
