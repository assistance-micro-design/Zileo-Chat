/**
 * Tests for panel-merge utility functions (SA-011 M-003, M-004)
 */
import { describe, it, expect } from 'vitest';
import type { ThinkingStep, ActiveThinkingStep } from '$types/thinking';
import type { ToolExecution, WorkflowToolExecution } from '$types/tool';
import type { ActiveTool } from '$lib/stores/streaming';
import { mergeAndSortReasoningSteps, mergeToolExecutions } from '../panel-merge';

describe('mergeAndSortReasoningSteps', () => {
	it('should return empty array when no steps provided', () => {
		const result = mergeAndSortReasoningSteps([], []);
		expect(result).toEqual([]);
	});

	it('should merge active steps only', () => {
		const active: ActiveThinkingStep[] = [
			{ content: 'Step 1', timestamp: 1000, stepNumber: 1 },
			{ content: 'Step 2', timestamp: 2000, stepNumber: 2 }
		];
		const result = mergeAndSortReasoningSteps(active, []);
		expect(result).toHaveLength(2);
		expect(result[0].isActive).toBe(true);
		expect(result[0].content).toBe('Step 1');
		expect(result[1].content).toBe('Step 2');
	});

	it('should merge persisted steps only', () => {
		const persisted: ThinkingStep[] = [
			{
				id: 'step-1',
				workflow_id: 'wf-1',
				message_id: 'msg-1',
				agent_id: 'agent-1',
				step_number: 0,
				content: 'Persisted step',
				duration_ms: 100,
				tokens: 50,
				created_at: '2025-01-01T00:00:00Z'
			}
		];
		const result = mergeAndSortReasoningSteps([], persisted);
		expect(result).toHaveLength(1);
		expect(result[0].isActive).toBe(false);
		expect(result[0].stepNumber).toBe(1); // 0-indexed to 1-indexed
		expect(result[0].tokens).toBe(50);
	});

	it('should merge and sort active + persisted by stepNumber', () => {
		const active: ActiveThinkingStep[] = [
			{ content: 'Active step 3', timestamp: 3000, stepNumber: 3 }
		];
		const persisted: ThinkingStep[] = [
			{
				id: 'step-1',
				workflow_id: 'wf-1',
				message_id: 'msg-1',
				agent_id: 'agent-1',
				step_number: 0,
				content: 'Persisted step 1',
				created_at: '2025-01-01T00:00:00Z'
			},
			{
				id: 'step-2',
				workflow_id: 'wf-1',
				message_id: 'msg-1',
				agent_id: 'agent-1',
				step_number: 1,
				content: 'Persisted step 2',
				created_at: '2025-01-01T00:00:01Z'
			}
		];
		const result = mergeAndSortReasoningSteps(active, persisted);
		expect(result).toHaveLength(3);
		expect(result[0].stepNumber).toBe(1);
		expect(result[1].stepNumber).toBe(2);
		expect(result[2].stepNumber).toBe(3);
		expect(result[2].isActive).toBe(true);
	});

	it('should generate stable IDs for active steps', () => {
		const active: ActiveThinkingStep[] = [
			{ content: 'Step', timestamp: 12345, stepNumber: 1 }
		];
		const result = mergeAndSortReasoningSteps(active, []);
		expect(result[0].id).toBe('active-1-12345');
	});
});

describe('mergeToolExecutions', () => {
	it('should return empty array when no executions provided', () => {
		const result = mergeToolExecutions([], [], []);
		expect(result).toEqual([]);
	});

	it('should merge active tools only', () => {
		const active: ActiveTool[] = [
			{ name: 'search', status: 'running', startedAt: 1000 },
			{ name: 'read', status: 'completed', startedAt: 2000, duration: 150 }
		];
		const result = mergeToolExecutions(active, [], []);
		expect(result).toHaveLength(2);
		expect(result[0].isActive).toBe(true);
		expect(result[0].name).toBe('search');
		expect(result[0].type).toBe('unknown');
		expect(result[1].duration).toBe(150);
	});

	it('should merge workflow executions', () => {
		const workflow: WorkflowToolExecution[] = [
			{
				tool_name: 'memory_search',
				tool_type: 'local',
				success: true,
				duration_ms: 200,
				iteration: 1,
				server_name: undefined,
				error_message: undefined,
				input_params: {},
				output_result: null
			}
		];
		const result = mergeToolExecutions([], workflow, []);
		expect(result).toHaveLength(1);
		expect(result[0].isActive).toBe(false);
		expect(result[0].status).toBe('completed');
		expect(result[0].iteration).toBe(1);
	});

	it('should merge persisted executions', () => {
		const persisted: ToolExecution[] = [
			{
				id: 'exec-1',
				workflow_id: 'wf-1',
				message_id: 'msg-1',
				agent_id: 'agent-1',
				tool_name: 'web_search',
				tool_type: 'mcp',
				server_name: 'brave',
				input_params: {},
				output_result: null,
				success: false,
				error_message: 'timeout',
				duration_ms: 5000,
				iteration: 2,
				created_at: '2025-01-01T00:00:00Z'
			}
		];
		const result = mergeToolExecutions([], [], persisted);
		expect(result).toHaveLength(1);
		expect(result[0].status).toBe('error');
		expect(result[0].error).toBe('timeout');
		expect(result[0].serverName).toBe('brave');
	});

	it('should merge all three sources in order', () => {
		const active: ActiveTool[] = [
			{ name: 'active_tool', status: 'running', startedAt: 1000 }
		];
		const workflow: WorkflowToolExecution[] = [
			{
				tool_name: 'workflow_tool',
				tool_type: 'local',
				success: true,
				duration_ms: 100,
				iteration: 1,
				server_name: undefined,
				error_message: undefined,
				input_params: {},
				output_result: null
			}
		];
		const persisted: ToolExecution[] = [
			{
				id: 'exec-1',
				workflow_id: 'wf-1',
				message_id: 'msg-1',
				agent_id: 'agent-1',
				tool_name: 'persisted_tool',
				tool_type: 'local',
				server_name: undefined,
				input_params: {},
				output_result: null,
				success: true,
				error_message: undefined,
				duration_ms: 50,
				iteration: 0,
				created_at: '2025-01-01T00:00:00Z'
			}
		];
		const result = mergeToolExecutions(active, workflow, persisted);
		expect(result).toHaveLength(3);
		// Active tools come first
		expect(result[0].isActive).toBe(true);
		expect(result[0].name).toBe('active_tool');
		// Then workflow executions
		expect(result[1].isActive).toBe(false);
		expect(result[1].id).toBe('workflow-0');
		// Then persisted executions
		expect(result[2].isActive).toBe(false);
		expect(result[2].id).toBe('exec-1');
	});
});
