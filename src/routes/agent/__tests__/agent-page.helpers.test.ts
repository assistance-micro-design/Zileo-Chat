import { describe, expect, it } from 'vitest';
import type { TodoTaskDisplay } from '$types/chat-block';
import type { PersistedTask } from '$types/workflow';
import {
	DEFAULT_FOLDER_COLORS,
	getDefaultFolderColor,
	getInitialWorkflowSelectionDecision,
	mapPersistedTasksToDisplay,
	resolveAgentDisplayName,
	resolveTaskAgentNames,
	selectDisplayTasksSource,
	shouldRestoreStatusFilter
} from '../agent-page.helpers';

const knownAgentId = '123e4567-e89b-12d3-a456-426614174000';
const unknownAgentId = '123e4567-e89b-12d3-a456-426614174999';

function persistedTask(overrides: Partial<PersistedTask> = {}): PersistedTask {
	return {
		id: 'task-1',
		workflow_id: 'workflow-1',
		name: 'Plan work',
		description: 'Plan the work',
		agent_assigned: 'Planner',
		priority: 2,
		status: 'completed',
		dependencies: [],
		duration_ms: 1234,
		created_at: '2025-01-01T00:00:00Z',
		completed_at: '2025-01-01T00:00:02Z',
		...overrides
	};
}

function displayTask(overrides: Partial<TodoTaskDisplay> = {}): TodoTaskDisplay {
	return {
		id: 'task-1',
		name: 'Plan work',
		description: 'Plan the work',
		status: 'pending',
		priority: 3,
		agent_name: 'Planner',
		duration_ms: 10,
		...overrides
	};
}

describe('agent page helpers', () => {
	it('maps persisted tasks to display tasks', () => {
		expect(mapPersistedTasksToDisplay([])).toEqual([]);
		expect(mapPersistedTasksToDisplay([persistedTask()])).toEqual([
			{
				id: 'task-1',
				name: 'Plan work',
				description: 'Plan the work',
				status: 'completed',
				priority: 2,
				agent_name: 'Planner',
				duration_ms: 1234
			}
		]);
	});

	it('resolves agent display names from live names or UUIDs', () => {
		expect(
			resolveAgentDisplayName({ rawName: undefined, agents: [], unknownAgentLabel: 'Unknown' })
		).toBeUndefined();
		expect(
			resolveAgentDisplayName({ rawName: 'Live Agent', agents: [], unknownAgentLabel: 'Unknown' })
		).toBe('Live Agent');
		expect(
			resolveAgentDisplayName({
				rawName: knownAgentId,
				agents: [{ id: knownAgentId, name: 'Known Agent' }],
				unknownAgentLabel: 'Unknown'
			})
		).toBe('Known Agent');
		expect(
			resolveAgentDisplayName({ rawName: unknownAgentId, agents: [], unknownAgentLabel: 'Unknown' })
		).toBe('Unknown');
	});

	it('selects execution tasks only for the currently executing workflow', () => {
		const executionTasks = [displayTask({ id: 'exec' })];
		const persistedTasks = [displayTask({ id: 'persisted' })];

		expect(
			selectDisplayTasksSource({
				isExecuting: true,
				executionWorkflowId: 'workflow-1',
				selectedWorkflowId: 'workflow-1',
				executionTasks,
				persistedTasks
			})
		).toBe(executionTasks);

		expect(
			selectDisplayTasksSource({
				isExecuting: true,
				executionWorkflowId: 'workflow-2',
				selectedWorkflowId: 'workflow-1',
				executionTasks,
				persistedTasks
			})
		).toBe(persistedTasks);

		expect(
			selectDisplayTasksSource({
				isExecuting: false,
				executionWorkflowId: 'workflow-1',
				selectedWorkflowId: 'workflow-1',
				executionTasks,
				persistedTasks
			})
		).toBe(persistedTasks);
	});

	it('resolves task agent names without mutating input tasks', () => {
		const tasks = [displayTask({ agent_name: knownAgentId })];
		const resolved = resolveTaskAgentNames(tasks, (rawName) =>
			rawName === knownAgentId ? 'Known Agent' : rawName
		);

		expect(resolved).toEqual([{ ...tasks[0], agent_name: 'Known Agent' }]);
		expect(tasks[0]!.agent_name).toBe(knownAgentId);
		expect(resolved[0]!).not.toBe(tasks[0]);
	});

	it('returns default folder colors with modulo cycling', () => {
		expect(getDefaultFolderColor(0)).toBe(DEFAULT_FOLDER_COLORS[0]);
		expect(getDefaultFolderColor(1)).toBe(DEFAULT_FOLDER_COLORS[1]);
		expect(getDefaultFolderColor(DEFAULT_FOLDER_COLORS.length - 1)).toBe(
			DEFAULT_FOLDER_COLORS[DEFAULT_FOLDER_COLORS.length - 1]
		);
		expect(getDefaultFolderColor(DEFAULT_FOLDER_COLORS.length)).toBe(DEFAULT_FOLDER_COLORS[0]);
	});

	it('decides whether to restore a non-all status filter', () => {
		expect(shouldRestoreStatusFilter('all')).toBe(false);
		expect(shouldRestoreStatusFilter('running')).toBe(true);
	});

	it('decides initial workflow selection and filter reset', () => {
		expect(
			getInitialWorkflowSelectionDecision({
				lastWorkflowId: null,
				workflows: [{ id: 'workflow-1' }],
				filteredWorkflows: [{ id: 'workflow-1' }]
			})
		).toEqual({ workflowIdToSelect: null, shouldResetStatusFilter: false });

		expect(
			getInitialWorkflowSelectionDecision({
				lastWorkflowId: 'missing',
				workflows: [{ id: 'workflow-1' }],
				filteredWorkflows: [{ id: 'workflow-1' }]
			})
		).toEqual({ workflowIdToSelect: null, shouldResetStatusFilter: false });

		expect(
			getInitialWorkflowSelectionDecision({
				lastWorkflowId: 'workflow-1',
				workflows: [{ id: 'workflow-1' }],
				filteredWorkflows: [{ id: 'workflow-1' }]
			})
		).toEqual({ workflowIdToSelect: 'workflow-1', shouldResetStatusFilter: false });

		expect(
			getInitialWorkflowSelectionDecision({
				lastWorkflowId: 'workflow-1',
				workflows: [{ id: 'workflow-1' }],
				filteredWorkflows: []
			})
		).toEqual({ workflowIdToSelect: 'workflow-1', shouldResetStatusFilter: true });
	});
});
