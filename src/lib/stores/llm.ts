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
 * LLM store for managing LLM provider and model state in the frontend.
 * Provides reactive state management using pure functions.
 * @module stores/llm
 */

import { invoke } from '@tauri-apps/api/core';
import type {
	LLMState,
	LLMModel,
	CreateModelRequest,
	UpdateModelRequest,
	ProviderSettings,
	ConnectionTestResult,
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
		providers: {
			mistral: null,
			ollama: null
		},
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
 * @param state - Current LLM state
 * @param loading - Loading state value
 * @returns Updated state with new loading value
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
 * @param state - Current LLM state
 * @param error - Error message (or null to clear)
 * @returns Updated state with error
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
 * @param state - Current LLM state
 * @param models - Array of LLM models
 * @returns Updated state with new models
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
 * @param state - Current LLM state
 * @param model - LLM model to add
 * @returns Updated state with new model
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
 * @param state - Current LLM state
 * @param id - Model ID to update
 * @param model - Updated model data
 * @returns Updated state with modified model
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
 * @param state - Current LLM state
 * @param id - Model ID to remove
 * @returns Updated state without the model
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
 * @param state - Current LLM state
 * @param provider - Provider type
 * @param settings - Provider settings
 * @returns Updated state with provider settings
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
 * @param state - Current LLM state
 * @param provider - Active provider (or null)
 * @returns Updated state with active provider
 */
export function setActiveProvider(state: LLMState, provider: ProviderType | null): LLMState {
	return {
		...state,
		activeProvider: provider
	};
}

/**
 * Sets the testing provider indicator.
 * @param state - Current LLM state
 * @param provider - Provider being tested (or null when done)
 * @returns Updated state with testing indicator
 */
export function setTestingProvider(state: LLMState, provider: ProviderType | null): LLMState {
	return {
		...state,
		testingProvider: provider
	};
}

// ============================================================================
// Selectors
// ============================================================================

/**
 * Gets models filtered by provider.
 * @param state - Current LLM state
 * @param provider - Provider to filter by
 * @returns Array of models for the specified provider
 */
export function getModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => m.provider === provider);
}

/**
 * Gets all models regardless of provider.
 * @param state - Current LLM state
 * @returns Array of all models
 */
export function getAllModels(state: LLMState): LLMModel[] {
	return state.models;
}

/**
 * Gets all builtin models.
 * @param state - Current LLM state
 * @returns Array of builtin models
 */
export function getBuiltinModels(state: LLMState): LLMModel[] {
	return state.models.filter((m) => m.is_builtin);
}

/**
 * Gets all custom (user-created) models.
 * @param state - Current LLM state
 * @returns Array of custom models
 */
export function getCustomModels(state: LLMState): LLMModel[] {
	return state.models.filter((m) => !m.is_builtin);
}

/**
 * Gets builtin models for a specific provider.
 * @param state - Current LLM state
 * @param provider - Provider to filter by
 * @returns Array of builtin models for the provider
 */
export function getBuiltinModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => m.is_builtin && m.provider === provider);
}

/**
 * Gets custom models for a specific provider.
 * @param state - Current LLM state
 * @param provider - Provider to filter by
 * @returns Array of custom models for the provider
 */
export function getCustomModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => !m.is_builtin && m.provider === provider);
}

/**
 * Gets a model by ID.
 * @param state - Current LLM state
 * @param id - Model ID
 * @returns Model or undefined if not found
 */
export function getModelById(state: LLMState, id: string): LLMModel | undefined {
	return state.models.find((m) => m.id === id);
}

/**
 * Gets a model by API name and provider.
 * @param state - Current LLM state
 * @param apiName - Model API name
 * @param provider - Provider type
 * @returns Model or undefined if not found
 */
export function getModelByApiName(
	state: LLMState,
	apiName: string,
	provider: ProviderType
): LLMModel | undefined {
	return state.models.find((m) => m.api_name === apiName && m.provider === provider);
}

/**
 * Gets the default model for a provider.
 * @param state - Current LLM state
 * @param provider - Provider type
 * @returns Default model or undefined
 */
export function getDefaultModel(state: LLMState, provider: ProviderType): LLMModel | undefined {
	const settings = state.providers[provider];
	if (!settings?.default_model_id) {
		return undefined;
	}
	return getModelById(state, settings.default_model_id);
}

/**
 * Gets settings for a specific provider.
 * @param state - Current LLM state
 * @param provider - Provider type
 * @returns Provider settings or null
 */
export function getProviderSettingsFromState(
	state: LLMState,
	provider: ProviderType
): ProviderSettings | null {
	return state.providers[provider];
}

/**
 * Checks if a provider is enabled.
 * @param state - Current LLM state
 * @param provider - Provider type
 * @returns True if provider is enabled
 */
export function isProviderEnabled(state: LLMState, provider: ProviderType): boolean {
	return state.providers[provider]?.enabled ?? false;
}

/**
 * Checks if a provider has an API key configured.
 * @param state - Current LLM state
 * @param provider - Provider type
 * @returns True if API key is configured
 */
export function hasApiKey(state: LLMState, provider: ProviderType): boolean {
	return state.providers[provider]?.api_key_configured ?? false;
}

/**
 * Gets total model count.
 * @param state - Current LLM state
 * @returns Number of models
 */
export function getModelCount(state: LLMState): number {
	return state.models.length;
}

/**
 * Gets model count for a specific provider.
 * @param state - Current LLM state
 * @param provider - Provider type
 * @returns Number of models for the provider
 */
export function getModelCountByProvider(state: LLMState, provider: ProviderType): number {
	return getModelsByProvider(state, provider).length;
}

/**
 * Gets custom model count.
 * @param state - Current LLM state
 * @returns Number of custom models
 */
export function getCustomModelCount(state: LLMState): number {
	return getCustomModels(state).length;
}

/**
 * Checks if a model exists.
 * @param state - Current LLM state
 * @param id - Model ID to check
 * @returns True if model exists
 */
export function hasModel(state: LLMState, id: string): boolean {
	return state.models.some((m) => m.id === id);
}

/**
 * Checks if an API name is already taken for a provider.
 * @param state - Current LLM state
 * @param apiName - API name to check
 * @param provider - Provider type
 * @param excludeId - Model ID to exclude from check (for updates)
 * @returns True if API name is taken
 */
export function isApiNameTaken(
	state: LLMState,
	apiName: string,
	provider: ProviderType,
	excludeId?: string
): boolean {
	return state.models.some(
		(m) => m.api_name === apiName && m.provider === provider && m.id !== excludeId
	);
}

// ============================================================================
// Async Actions (Tauri IPC)
// ============================================================================

/**
 * Loads all LLM models from the backend.
 * @param provider - Optional provider filter
 * @returns Promise resolving to array of models
 */
export async function loadModels(provider?: ProviderType): Promise<LLMModel[]> {
	return invoke<LLMModel[]>('list_models', { provider: provider ?? null });
}

/**
 * Gets a single model by ID.
 * @param id - Model ID
 * @returns Promise resolving to the model
 */
export async function fetchModel(id: string): Promise<LLMModel> {
	return invoke<LLMModel>('get_model', { id });
}

/**
 * Gets a model by API name and provider.
 * Used to retrieve model details (context_window, pricing) from agent config.
 * @param apiName - Model API name (e.g., "mistral-large-latest")
 * @param provider - Provider type (e.g., "mistral", "ollama")
 * @returns Promise resolving to the model
 */
export async function fetchModelByApiName(apiName: string, provider: ProviderType): Promise<LLMModel> {
	return invoke<LLMModel>('get_model_by_api_name', { apiName, provider });
}

/**
 * Creates a new custom model.
 * @param data - Model creation data
 * @returns Promise resolving to created model
 */
export async function createModel(data: CreateModelRequest): Promise<LLMModel> {
	return invoke<LLMModel>('create_model', { data });
}

/**
 * Updates an existing model.
 * @param id - Model ID to update
 * @param data - Update data
 * @returns Promise resolving to updated model
 */
export async function updateModel(id: string, data: UpdateModelRequest): Promise<LLMModel> {
	return invoke<LLMModel>('update_model', { id, data });
}

/**
 * Deletes a custom model.
 * @param id - Model ID to delete
 * @returns Promise resolving to true if deleted
 */
export async function deleteModel(id: string): Promise<boolean> {
	return invoke<boolean>('delete_model', { id });
}

/**
 * Loads provider settings from the backend.
 * @param provider - Provider type
 * @returns Promise resolving to provider settings
 */
export async function loadProviderSettings(provider: ProviderType): Promise<ProviderSettings> {
	return invoke<ProviderSettings>('get_provider_settings', { provider });
}

/**
 * Updates provider settings.
 * @param provider - Provider type
 * @param enabled - Whether to enable/disable the provider
 * @param defaultModelId - Default model ID
 * @param baseUrl - Custom base URL (for Ollama)
 * @returns Promise resolving to updated settings
 */
export async function updateProviderSettings(
	provider: ProviderType,
	enabled?: boolean,
	defaultModelId?: string,
	baseUrl?: string
): Promise<ProviderSettings> {
	// Tauri converts snake_case Rust params to camelCase in JS
	return invoke<ProviderSettings>('update_provider_settings', {
		provider,
		enabled: enabled ?? null,
		defaultModelId: defaultModelId ?? null,
		baseUrl: baseUrl ?? null
	});
}

/**
 * Tests connection to a provider.
 * @param provider - Provider to test
 * @returns Promise resolving to connection test result
 */
export async function testConnection(provider: ProviderType): Promise<ConnectionTestResult> {
	return invoke<ConnectionTestResult>('test_provider_connection', { provider });
}

/**
 * Seeds the database with builtin models.
 * @returns Promise resolving to number of models inserted
 */
export async function seedBuiltinModels(): Promise<number> {
	return invoke<number>('seed_builtin_models');
}

/**
 * Loads all provider settings and models.
 * Convenience function to initialize the LLM state.
 * @returns Promise resolving to object with providers and models
 */
export async function loadAllLLMData(): Promise<{
	mistral: ProviderSettings;
	ollama: ProviderSettings;
	models: LLMModel[];
}> {
	const [mistral, ollama, models] = await Promise.all([
		loadProviderSettings('mistral'),
		loadProviderSettings('ollama'),
		loadModels()
	]);

	return { mistral, ollama, models };
}
