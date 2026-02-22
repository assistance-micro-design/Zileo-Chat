import { describe, it, expect } from 'vitest';
import { computeRoundMetadata, formatRoundSeparator } from '../activityUtils';
import type { WorkflowActivityEvent } from '$types/activity';

function makeActivity(
	messageId: string,
	agentName?: string,
	id?: string
): WorkflowActivityEvent {
	return {
		id: id ?? `act-${Math.random().toString(36).slice(2)}`,
		timestamp: Date.now(),
		type: 'tool_start',
		title: 'Test activity',
		status: 'completed',
		metadata: { messageId, ...(agentName ? { agentName } : {}) }
	};
}

describe('computeRoundMetadata', () => {
	it('should return empty array for empty input', () => {
		expect(computeRoundMetadata([])).toEqual([]);
	});

	it('should compute round number, agent name, and count from activities', () => {
		const activities = [
			makeActivity('msg-1', 'Research Agent'),
			makeActivity('msg-1'),
			makeActivity('msg-2', 'Writer Agent')
		];
		const rounds = computeRoundMetadata(activities);
		expect(rounds).toHaveLength(2);
		expect(rounds[0]).toEqual({
			messageId: 'msg-1',
			round: 1,
			agentName: 'Research Agent',
			count: 2
		});
		expect(rounds[1]).toEqual({
			messageId: 'msg-2',
			round: 2,
			agentName: 'Writer Agent',
			count: 1
		});
	});

	it('should handle activities without agent name', () => {
		const activities = [makeActivity('msg-1')];
		const rounds = computeRoundMetadata(activities);
		expect(rounds[0].agentName).toBeUndefined();
	});

	it('should handle activities without messageId', () => {
		const activity: WorkflowActivityEvent = {
			id: 'act-1',
			timestamp: Date.now(),
			type: 'tool_start',
			title: 'No messageId',
			status: 'completed'
		};
		const rounds = computeRoundMetadata([activity]);
		expect(rounds).toEqual([]);
	});

	it('should use first agentName found in a round', () => {
		const activities = [
			makeActivity('msg-1', 'Agent A'),
			makeActivity('msg-1', 'Agent B')
		];
		const rounds = computeRoundMetadata(activities);
		expect(rounds[0].agentName).toBe('Agent A');
	});
});

describe('formatRoundSeparator', () => {
	it('should format round with number and agent name', () => {
		const result = formatRoundSeparator(1, 'Research Agent', 3);
		expect(result).toBe('Round 1 - Research Agent (3)');
	});

	it('should format round without agent name', () => {
		const result = formatRoundSeparator(2, undefined, 5);
		expect(result).toBe('Round 2 (5)');
	});

	it('should format round with count of 1', () => {
		const result = formatRoundSeparator(1, undefined, 1);
		expect(result).toBe('Round 1 (1)');
	});
});
