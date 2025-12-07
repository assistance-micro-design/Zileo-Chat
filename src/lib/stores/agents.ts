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
 * Agent store for managing agent state in the frontend.
 * Uses the CRUD store factory for standardized state management.
 * @module stores/agents
 */

import {
	createCRUDStore,
	createDerivedStores
} from './factory/createCRUDStore';
import type {
	AgentConfig,
	AgentSummary,
	AgentConfigCreate,
	AgentConfigUpdate
} from '$types/agent';

// ============================================================================
// Store State Type (for backward compatibility)
// ============================================================================

/**
 * State interface for the agent store
 * Maps to CRUDStoreState with agent-specific field names
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

// ============================================================================
// Base CRUD Store
// ============================================================================

const baseCrudStore = createCRUDStore<
	AgentConfig,
	AgentConfigCreate,
	AgentConfigUpdate,
	AgentSummary
>({
	name: 'agent',
	idParamName: 'agentId',
	commands: {
		list: 'list_agents',
		get: 'get_agent_config',
		create: 'create_agent',
		update: 'update_agent',
		delete: 'delete_agent'
	}
});

// ============================================================================
// Agent Store (with backward-compatible API)
// ============================================================================

/**
 * Agent store with actions for CRUD operations and UI state management.
 * Combines Svelte store subscription with async Tauri IPC calls.
 */
export const agentStore = {
	/**
	 * Subscribe to store changes
	 * Maps internal state to AgentStoreState interface
	 */
	subscribe: (run: (value: AgentStoreState) => void) => {
		return baseCrudStore.subscribe((state) => {
			run({
				agents: state.items,
				selectedId: state.selectedId,
				loading: state.loading,
				error: state.error,
				formMode: state.formMode,
				editingAgent: state.editing
			});
		});
	},

	/**
	 * Loads all agents from the backend.
	 * Updates the store with agent summaries.
	 */
	loadAgents: () => baseCrudStore.loadItems(),

	/**
	 * Creates a new agent.
	 * @param config - Agent configuration for creation
	 * @returns The created agent's ID
	 * @throws Error if creation fails
	 */
	createAgent: (config: AgentConfigCreate) => baseCrudStore.createItem(config),

	/**
	 * Updates an existing agent.
	 * @param agentId - ID of the agent to update
	 * @param config - Partial configuration with fields to update
	 * @throws Error if update fails
	 */
	updateAgent: (agentId: string, config: AgentConfigUpdate) =>
		baseCrudStore.updateItem(agentId, config),

	/**
	 * Deletes an agent.
	 * @param agentId - ID of the agent to delete
	 * @throws Error if deletion fails
	 */
	deleteAgent: (agentId: string) => baseCrudStore.deleteItem(agentId),

	/**
	 * Gets the full configuration of an agent.
	 * @param agentId - ID of the agent
	 * @returns Full agent configuration
	 */
	getAgentConfig: (agentId: string) => baseCrudStore.getItem(agentId),

	/**
	 * Selects an agent by ID.
	 * @param agentId - ID to select (or null to deselect)
	 */
	select: (agentId: string | null) => baseCrudStore.select(agentId),

	/**
	 * Opens the create agent form.
	 */
	openCreateForm: () => baseCrudStore.openCreateForm(),

	/**
	 * Opens the edit form for a specific agent.
	 * Loads the full agent configuration.
	 * @param agentId - ID of the agent to edit
	 */
	openEditForm: (agentId: string) => baseCrudStore.openEditForm(agentId),

	/**
	 * Closes the form (create or edit).
	 */
	closeForm: () => baseCrudStore.closeForm(),

	/**
	 * Clears the current error message.
	 */
	clearError: () => baseCrudStore.clearError(),

	/**
	 * Resets the store to initial state.
	 */
	reset: () => baseCrudStore.reset()
};

// ============================================================================
// Derived Stores
// ============================================================================

// Get base derived stores
const derivedStores = createDerivedStores(baseCrudStore);

/**
 * Derived store: list of all agents
 */
export const agents = derivedStores.items;

/**
 * Derived store: currently selected agent summary
 */
export const selectedAgent = derivedStores.selected;

/**
 * Derived store: loading state
 */
export const isLoading = derivedStores.isLoading;

/**
 * Derived store: error message
 */
export const error = derivedStores.error;

/**
 * Derived store: current form mode
 */
export const formMode = derivedStores.formMode;

/**
 * Derived store: agent being edited
 */
export const editingAgent = derivedStores.editing;

/**
 * Derived store: number of agents
 * @deprecated Use `agents.length` instead
 */
export const agentCount = derivedStores.count;

/**
 * Derived store: whether agents are available (non-empty list)
 */
export const hasAgents = derivedStores.hasItems;

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
