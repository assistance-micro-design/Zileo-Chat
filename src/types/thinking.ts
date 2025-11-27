// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * @fileoverview Thinking step types for persistence and display.
 *
 * These types are synchronized with Rust backend types:
 * - src-tauri/src/models/thinking_step.rs (ThinkingStep, ThinkingStepCreate)
 *
 * Phase 4: Thinking Steps Persistence
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
	/** Timestamp when the step was recorded (ISO 8601 string) */
	created_at: string;
}

/**
 * Active thinking step for real-time display during streaming
 *
 * Lighter weight than ThinkingStep, used during streaming before persistence.
 */
export interface ActiveThinkingStep {
	/** Reasoning content */
	content: string;
	/** Timestamp when received (Unix ms) */
	timestamp: number;
	/** Step number in sequence (1-indexed for display) */
	stepNumber: number;
	/** Duration in milliseconds (when available) */
	durationMs?: number;
}

/**
 * Creates an ActiveThinkingStep from streaming data
 *
 * @param content - Reasoning content from stream
 * @param stepNumber - Step number (1-indexed)
 * @returns Active thinking step for display
 */
export function createActiveThinkingStep(
	content: string,
	stepNumber: number
): ActiveThinkingStep {
	return {
		content,
		timestamp: Date.now(),
		stepNumber
	};
}

/**
 * Formats thinking step duration for display
 *
 * @param durationMs - Duration in milliseconds
 * @returns Formatted duration string (e.g., "150ms", "1.5s")
 */
export function formatThinkingDuration(durationMs: number): string {
	if (durationMs < 1000) {
		return `${durationMs}ms`;
	}
	return `${(durationMs / 1000).toFixed(1)}s`;
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

/**
 * Groups thinking steps by message ID
 *
 * @param steps - Array of thinking steps
 * @returns Map of message_id to thinking steps
 */
export function groupThinkingStepsByMessage(
	steps: ThinkingStep[]
): Map<string, ThinkingStep[]> {
	const grouped = new Map<string, ThinkingStep[]>();

	for (const step of steps) {
		const existing = grouped.get(step.message_id) ?? [];
		existing.push(step);
		grouped.set(step.message_id, existing);
	}

	// Sort each group by step_number
	for (const [messageId, messageSteps] of grouped) {
		messageSteps.sort((a, b) => a.step_number - b.step_number);
		grouped.set(messageId, messageSteps);
	}

	return grouped;
}

/**
 * Calculates total tokens used across thinking steps
 *
 * @param steps - Array of thinking steps
 * @returns Total token count (only counts steps with token data)
 */
export function calculateTotalThinkingTokens(steps: ThinkingStep[]): number {
	return steps.reduce((total, step) => total + (step.tokens ?? 0), 0);
}

/**
 * Calculates total duration across thinking steps
 *
 * @param steps - Array of thinking steps
 * @returns Total duration in milliseconds (only counts steps with duration data)
 */
export function calculateTotalThinkingDuration(steps: ThinkingStep[]): number {
	return steps.reduce((total, step) => total + (step.duration_ms ?? 0), 0);
}
