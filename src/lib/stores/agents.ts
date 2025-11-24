// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Agent store for managing agent state in the frontend.
 * Provides reactive state management for agents using pure functions.
 * @module stores/agents
 */

import type { AgentConfig, Lifecycle } from '$lib/types/agent';

/**
 * State interface for the agent store
 */
export interface AgentState {
	/** List of all agent IDs */
	agentIds: string[];
	/** Map of agent configurations by ID */
	configs: Map<string, AgentConfig>;
	/** Currently selected agent ID */
	selectedId: string | null;
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
}

/**
 * Creates the initial agent state
 * @returns Initial agent state with empty values
 */
export function createInitialAgentState(): AgentState {
	return {
		agentIds: [],
		configs: new Map(),
		selectedId: null,
		loading: false,
		error: null
	};
}

/**
 * Sets the agent IDs list
 * @param state - Current agent state
 * @param ids - Array of agent IDs
 * @returns Updated state with new agent IDs
 */
export function setAgentIds(state: AgentState, ids: string[]): AgentState {
	return {
		...state,
		agentIds: ids,
		loading: false,
		error: null
	};
}

/**
 * Adds an agent configuration to the state
 * @param state - Current agent state
 * @param config - Agent configuration to add
 * @returns Updated state with new config
 */
export function addAgentConfig(state: AgentState, config: AgentConfig): AgentState {
	const newConfigs = new Map(state.configs);
	newConfigs.set(config.id, config);

	const newIds = state.agentIds.includes(config.id)
		? state.agentIds
		: [...state.agentIds, config.id];

	return {
		...state,
		agentIds: newIds,
		configs: newConfigs,
		error: null
	};
}

/**
 * Removes an agent from the state
 * @param state - Current agent state
 * @param id - Agent ID to remove
 * @returns Updated state without the agent
 */
export function removeAgent(state: AgentState, id: string): AgentState {
	const newConfigs = new Map(state.configs);
	newConfigs.delete(id);

	const newIds = state.agentIds.filter((agentId) => agentId !== id);
	const newSelectedId = state.selectedId === id ? null : state.selectedId;

	return {
		...state,
		agentIds: newIds,
		configs: newConfigs,
		selectedId: newSelectedId,
		error: null
	};
}

/**
 * Selects an agent by ID
 * @param state - Current agent state
 * @param id - Agent ID to select (or null to deselect)
 * @returns Updated state with new selection
 */
export function selectAgent(state: AgentState, id: string | null): AgentState {
	return {
		...state,
		selectedId: id,
		error: null
	};
}

/**
 * Sets the loading state
 * @param state - Current agent state
 * @param loading - Loading state value
 * @returns Updated state with new loading value
 */
export function setAgentLoading(state: AgentState, loading: boolean): AgentState {
	return {
		...state,
		loading,
		error: loading ? null : state.error
	};
}

/**
 * Sets an error message
 * @param state - Current agent state
 * @param error - Error message (or null to clear)
 * @returns Updated state with error
 */
export function setAgentError(state: AgentState, error: string | null): AgentState {
	return {
		...state,
		error,
		loading: false
	};
}

/**
 * Gets the currently selected agent config
 * @param state - Current agent state
 * @returns Selected agent config or undefined
 */
export function getSelectedAgentConfig(state: AgentState): AgentConfig | undefined {
	if (!state.selectedId) return undefined;
	return state.configs.get(state.selectedId);
}

/**
 * Gets agent config by ID
 * @param state - Current agent state
 * @param id - Agent ID
 * @returns Agent config or undefined
 */
export function getAgentConfig(state: AgentState, id: string): AgentConfig | undefined {
	return state.configs.get(id);
}

/**
 * Gets agents filtered by lifecycle
 * @param state - Current agent state
 * @param lifecycle - Lifecycle to filter by
 * @returns Array of agent IDs with matching lifecycle
 */
export function getAgentsByLifecycle(state: AgentState, lifecycle: Lifecycle): string[] {
	return state.agentIds.filter((id) => {
		const config = state.configs.get(id);
		return config?.lifecycle === lifecycle;
	});
}

/**
 * Checks if an agent exists
 * @param state - Current agent state
 * @param id - Agent ID to check
 * @returns True if agent exists
 */
export function hasAgent(state: AgentState, id: string): boolean {
	return state.agentIds.includes(id);
}

/**
 * Gets total agent count
 * @param state - Current agent state
 * @returns Number of agents
 */
export function getAgentCount(state: AgentState): number {
	return state.agentIds.length;
}

/**
 * Gets permanent agent count
 * @param state - Current agent state
 * @returns Number of permanent agents
 */
export function getPermanentAgentCount(state: AgentState): number {
	return getAgentsByLifecycle(state, 'permanent').length;
}

/**
 * Gets temporary agent count
 * @param state - Current agent state
 * @returns Number of temporary agents
 */
export function getTemporaryAgentCount(state: AgentState): number {
	return getAgentsByLifecycle(state, 'temporary').length;
}

/**
 * Gets all agent configs as an array
 * @param state - Current agent state
 * @returns Array of all agent configs
 */
export function getAllAgentConfigs(state: AgentState): AgentConfig[] {
	return Array.from(state.configs.values());
}
