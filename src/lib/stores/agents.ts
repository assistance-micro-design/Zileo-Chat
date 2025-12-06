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

// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

/**
 * Agent store for managing agent state in the frontend.
 * Provides reactive state management with Svelte stores and Tauri IPC integration.
 * @module stores/agents
 */

import { writable, derived } from 'svelte/store';
import { invoke } from '@tauri-apps/api/core';
import type {
	AgentConfig,
	AgentSummary,
	AgentConfigCreate,
	AgentConfigUpdate
} from '$types/agent';

/**
 * State interface for the agent store
 */
export interface AgentStoreState {
	/** List of all agents (summary view) */
	agents: AgentSummary[];
	/** Currently selected agent ID */
	selectedId: string | null;
	/** Loading state indicator */
	loading: boolean;
	/** Error message if any */
	error: string | null;
	/** Current form mode (create, edit, or null when closed) */
	formMode: 'create' | 'edit' | null;
	/** Agent being edited (full config for edit mode) */
	editingAgent: AgentConfig | null;
}

/**
 * Initial state for the agent store
 */
const initialState: AgentStoreState = {
	agents: [],
	selectedId: null,
	loading: false,
	error: null,
	formMode: null,
	editingAgent: null
};

/**
 * Internal writable store
 */
const store = writable<AgentStoreState>(initialState);

/**
 * Agent store with actions for CRUD operations and UI state management.
 * Combines Svelte store subscription with async Tauri IPC calls.
 */
export const agentStore = {
	/**
	 * Subscribe to store changes
	 */
	subscribe: store.subscribe,

	/**
	 * Loads all agents from the backend.
	 * Updates the store with agent summaries.
	 */
	async loadAgents(): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const agents = await invoke<AgentSummary[]>('list_agents');
			store.update((s) => ({ ...s, agents, loading: false }));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
		}
	},

	/**
	 * Creates a new agent.
	 * @param config - Agent configuration for creation
	 * @returns The created agent's ID
	 * @throws Error if creation fails
	 */
	async createAgent(config: AgentConfigCreate): Promise<string> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			const id = await invoke<string>('create_agent', { config });
			await this.loadAgents();
			store.update((s) => ({ ...s, formMode: null }));
			return id;
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Updates an existing agent.
	 * @param agentId - ID of the agent to update
	 * @param config - Partial configuration with fields to update
	 * @throws Error if update fails
	 */
	async updateAgent(agentId: string, config: AgentConfigUpdate): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('update_agent', { agentId, config });
			await this.loadAgents();
			store.update((s) => ({ ...s, formMode: null, editingAgent: null }));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Deletes an agent.
	 * @param agentId - ID of the agent to delete
	 * @throws Error if deletion fails
	 */
	async deleteAgent(agentId: string): Promise<void> {
		store.update((s) => ({ ...s, loading: true, error: null }));
		try {
			await invoke('delete_agent', { agentId });
			await this.loadAgents();
			store.update((s) => ({
				...s,
				selectedId: s.selectedId === agentId ? null : s.selectedId
			}));
		} catch (e) {
			store.update((s) => ({ ...s, error: String(e), loading: false }));
			throw e;
		}
	},

	/**
	 * Gets the full configuration of an agent.
	 * @param agentId - ID of the agent
	 * @returns Full agent configuration
	 */
	async getAgentConfig(agentId: string): Promise<AgentConfig> {
		return await invoke<AgentConfig>('get_agent_config', { agentId });
	},

	/**
	 * Selects an agent by ID.
	 * @param agentId - ID to select (or null to deselect)
	 */
	select(agentId: string | null): void {
		store.update((s) => ({ ...s, selectedId: agentId }));
	},

	/**
	 * Opens the create agent form.
	 */
	openCreateForm(): void {
		store.update((s) => ({ ...s, formMode: 'create', editingAgent: null }));
	},

	/**
	 * Opens the edit form for a specific agent.
	 * Loads the full agent configuration.
	 * @param agentId - ID of the agent to edit
	 */
	async openEditForm(agentId: string): Promise<void> {
		const config = await this.getAgentConfig(agentId);
		store.update((s) => ({ ...s, formMode: 'edit', editingAgent: config }));
	},

	/**
	 * Closes the form (create or edit).
	 */
	closeForm(): void {
		store.update((s) => ({ ...s, formMode: null, editingAgent: null }));
	},

	/**
	 * Clears the current error message.
	 */
	clearError(): void {
		store.update((s) => ({ ...s, error: null }));
	},

	/**
	 * Resets the store to initial state.
	 */
	reset(): void {
		store.set(initialState);
	}
};

// ============================================================================
// Derived Stores
// ============================================================================

/**
 * Derived store: list of all agents
 */
export const agents = derived(store, ($s) => $s.agents);

/**
 * Derived store: currently selected agent summary
 */
export const selectedAgent = derived(store, ($s) =>
	$s.agents.find((a) => a.id === $s.selectedId) ?? null
);

/**
 * Derived store: loading state
 */
export const isLoading = derived(store, ($s) => $s.loading);

/**
 * Derived store: error message
 */
export const error = derived(store, ($s) => $s.error);

/**
 * Derived store: current form mode
 */
export const formMode = derived(store, ($s) => $s.formMode);

/**
 * Derived store: agent being edited
 */
export const editingAgent = derived(store, ($s) => $s.editingAgent);

/**
 * Derived store: number of agents
 */
export const agentCount = derived(store, ($s) => $s.agents.length);

/**
 * Derived store: whether agents are available (non-empty list)
 */
export const hasAgents = derived(store, ($s) => $s.agents.length > 0);

// ============================================================================
// Legacy Pure Functions (for backward compatibility)
// ============================================================================

/**
 * @deprecated Use agentStore instead
 * Creates the initial agent state
 */
export interface AgentState {
	agentIds: string[];
	configs: Map<string, AgentConfig>;
	selectedId: string | null;
	loading: boolean;
	error: string | null;
}

/**
 * @deprecated Use agentStore instead
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
