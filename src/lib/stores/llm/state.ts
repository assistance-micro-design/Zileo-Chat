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
 * LLM state management - initial state and pure state updaters.
 * All functions are pure: they take state in and return new state out.
 * @module stores/llm/state
 */

import type {
	LLMState,
	LLMModel,
	ProviderSettings,
	ProviderType
} from '$types/llm';

// ============================================================================
// Initial State
// ============================================================================

/**
 * Creates the initial LLM state.
 * @returns Initial LLM state with empty values
 */
export function createInitialLLMState(): LLMState {
	return {
		providers: {},
		models: [],
		activeProvider: null,
		loading: false,
		error: null,
		testingProvider: null
	};
}

// ============================================================================
// Pure State Updaters
// ============================================================================

/**
 * Sets the loading state.
 */
export function setLLMLoading(state: LLMState, loading: boolean): LLMState {
	return {
		...state,
		loading,
		error: loading ? null : state.error
	};
}

/**
 * Sets an error message.
 */
export function setLLMError(state: LLMState, error: string | null): LLMState {
	return {
		...state,
		error,
		loading: false
	};
}

/**
 * Sets the models list.
 */
export function setModels(state: LLMState, models: LLMModel[]): LLMState {
	return {
		...state,
		models,
		loading: false,
		error: null
	};
}

/**
 * Adds a new model to the state.
 */
export function addModel(state: LLMState, model: LLMModel): LLMState {
	const exists = state.models.some((m) => m.id === model.id);
	if (exists) {
		return updateModelInState(state, model.id, model);
	}

	return {
		...state,
		models: [...state.models, model],
		error: null
	};
}

/**
 * Updates an existing model in the state.
 */
export function updateModelInState(state: LLMState, id: string, model: LLMModel): LLMState {
	const models = state.models.map((m) => (m.id === id ? model : m));

	return {
		...state,
		models,
		error: null
	};
}

/**
 * Removes a model from the state.
 */
export function removeModel(state: LLMState, id: string): LLMState {
	return {
		...state,
		models: state.models.filter((m) => m.id !== id),
		error: null
	};
}

/**
 * Sets provider settings in the state.
 */
export function setProviderSettings(
	state: LLMState,
	provider: ProviderType,
	settings: ProviderSettings
): LLMState {
	return {
		...state,
		providers: {
			...state.providers,
			[provider]: settings
		},
		error: null
	};
}

/**
 * Sets the active provider.
 */
export function setActiveProvider(state: LLMState, provider: ProviderType | null): LLMState {
	return {
		...state,
		activeProvider: provider
	};
}

/**
 * Sets the testing provider indicator.
 */
export function setTestingProvider(state: LLMState, provider: ProviderType | null): LLMState {
	return {
		...state,
		testingProvider: provider
	};
}
