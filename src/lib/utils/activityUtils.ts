import type { WorkflowActivityEvent } from '$types/activity';

/**
 * Metadata computed for each round separator in the activity feed.
 */
export interface RoundMetadata {
	messageId: string;
	round: number;
	agentName?: string;
	count: number;
}

/**
 * Compute round metadata from a list of activities grouped by messageId.
 * Each unique messageId becomes a round, numbered sequentially.
 *
 * @param activities - Ordered list of workflow activity events
 * @returns Array of round metadata for each unique messageId group
 */
export function computeRoundMetadata(activities: WorkflowActivityEvent[]): RoundMetadata[] {
	const rounds: RoundMetadata[] = [];
	let currentMessageId: string | null = null;
	let currentRound: RoundMetadata | null = null;

	for (const activity of activities) {
		const msgId = activity.metadata?.messageId;
		if (!msgId) continue;

		if (msgId !== currentMessageId) {
			if (currentRound) {
				rounds.push(currentRound);
			}
			currentMessageId = msgId;
			currentRound = {
				messageId: msgId,
				round: rounds.length + 1,
				agentName: activity.metadata?.agentName,
				count: 1
			};
		} else if (currentRound) {
			currentRound.count++;
			if (!currentRound.agentName && activity.metadata?.agentName) {
				currentRound.agentName = activity.metadata.agentName;
			}
		}
	}

	if (currentRound) {
		rounds.push(currentRound);
	}

	return rounds;
}

/**
 * Format a human-readable round separator label.
 *
 * @param round - Round number (1-based)
 * @param agentName - Optional agent name
 * @param count - Number of activities in this round
 * @returns Formatted string like "Round 1 - Agent Name (3)"
 */
export function formatRoundSeparator(round: number, agentName: string | undefined, count: number): string {
	if (agentName) {
		return `Round ${round} - ${agentName} (${count})`;
	}
	return `Round ${round} (${count})`;
}
