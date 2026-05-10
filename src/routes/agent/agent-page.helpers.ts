import type { TodoTaskDisplay } from '$types/chat-block';
import type { PersistedTask } from '$types/workflow';
import { isUuid } from '$lib/utils/uuid';

export interface AgentNameResolutionInput {
	rawName: string | undefined;
	agents: Array<{ id: string; name: string }>;
	unknownAgentLabel: string;
}

export interface SelectDisplayTasksInput {
	isExecuting: boolean;
	executionWorkflowId: string | null | undefined;
	selectedWorkflowId: string | null;
	executionTasks: TodoTaskDisplay[];
	persistedTasks: TodoTaskDisplay[];
}

export interface InitialWorkflowSelectionDecision {
	workflowIdToSelect: string | null;
	shouldResetStatusFilter: boolean;
}

export const DEFAULT_FOLDER_COLORS = [
	'#3b82f6',
	'#ef4444',
	'#10b981',
	'#f59e0b',
	'#8b5cf6',
	'#ec4899'
] as const;

export function mapPersistedTasksToDisplay(tasks: PersistedTask[]): TodoTaskDisplay[] {
	return tasks.map((task) => ({
		id: task.id,
		name: task.name,
		description: task.description,
		status: task.status,
		priority: task.priority,
		agent_name: task.agent_assigned,
		duration_ms: task.duration_ms
	}));
}

export function resolveAgentDisplayName(input: AgentNameResolutionInput): string | undefined {
	if (!input.rawName) return undefined;
	if (!isUuid(input.rawName)) return input.rawName;

	const found = input.agents.find((agent) => agent.id === input.rawName);
	if (found) return found.name;

	return input.unknownAgentLabel;
}

export function selectDisplayTasksSource(input: SelectDisplayTasksInput): TodoTaskDisplay[] {
	if (input.isExecuting && input.executionWorkflowId === input.selectedWorkflowId) {
		return input.executionTasks;
	}

	return input.persistedTasks;
}

export function resolveTaskAgentNames(
	tasks: TodoTaskDisplay[],
	resolveAgentName: (rawName: string | undefined) => string | undefined
): TodoTaskDisplay[] {
	return tasks.map((task) => ({
		...task,
		agent_name: resolveAgentName(task.agent_name)
	}));
}

export function getDefaultFolderColor(folderCount: number): string {
	return DEFAULT_FOLDER_COLORS[folderCount % DEFAULT_FOLDER_COLORS.length]!;
}

export function shouldRestoreStatusFilter(savedFilter: string): boolean {
	return savedFilter !== 'all';
}

export function getInitialWorkflowSelectionDecision(input: {
	lastWorkflowId: string | null;
	workflows: Array<{ id: string }>;
	filteredWorkflows: Array<{ id: string }>;
}): InitialWorkflowSelectionDecision {
	if (!input.lastWorkflowId) {
		return { workflowIdToSelect: null, shouldResetStatusFilter: false };
	}

	const workflowExists = input.workflows.some((workflow) => workflow.id === input.lastWorkflowId);
	if (!workflowExists) {
		return { workflowIdToSelect: null, shouldResetStatusFilter: false };
	}

	const workflowIsVisible = input.filteredWorkflows.some(
		(workflow) => workflow.id === input.lastWorkflowId
	);

	return {
		workflowIdToSelect: input.lastWorkflowId,
		shouldResetStatusFilter: !workflowIsVisible
	};
}
