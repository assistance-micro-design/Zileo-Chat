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
import type { ProviderInfo } from '$types/customProvider';

// ============================================================================
// Cache Management (OPT-9)
// ============================================================================

interface LLMDataCache {
	data: {
		providerList: ProviderInfo[];
		settings: Record<string, ProviderSettings>;
		models: LLMModel[];
	} | null;
	timestamp: number;
}

let llmCache: LLMDataCache = { data: null, timestamp: 0 };
const LLM_CACHE_TTL = 30000; // 30 seconds

/**
 * Cache for filtered models to avoid recalculation during scroll (OPT-SCROLL-6).
 * Moved to top for access by invalidateLLMCache.
 */
interface FilteredModelsCache {
	key: string;
	result: LLMModel[];
}
let filteredModelsCache: FilteredModelsCache | null = null;

/**
 * Invalidates the LLM data cache.
 * Call this after any mutation (create/update/delete model, update provider settings).
 * Also clears the filtered models memoization cache (OPT-SCROLL-6).
 */
export function invalidateLLMCache(): void {
	llmCache = { data: null, timestamp: 0 };
	filteredModelsCache = null; // OPT-SCROLL-6: Clear memoized cache
}

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

// ============================================================================
// Memoized Selectors (OPT-SCROLL-6)
// ============================================================================

/**
 * Computes a simple hash from the models array for cache invalidation.
 * Uses model IDs and count to detect changes.
 */
function computeModelsHash(models: LLMModel[]): string {
	if (models.length === 0) return 'empty';
	return `${models.length}:${models[0]?.id ?? ''}:${models[models.length - 1]?.id ?? ''}`;
}

/**
 * Gets filtered models with memoization to prevent recalculation during scroll.
 * Returns cached result if state hasn't changed.
 * @param state - Current LLM state
 * @param provider - Provider filter ('all' for no filter, or specific provider)
 * @returns Array of models (possibly cached)
 */
export function getFilteredModelsMemoized(
	state: LLMState,
	provider: ProviderType | 'all'
): LLMModel[] {
	const modelsHash = computeModelsHash(state.models);
	const cacheKey = `${modelsHash}:${provider}`;

	if (filteredModelsCache?.key === cacheKey) {
		return filteredModelsCache.result;
	}

	const result = provider === 'all'
		? getAllModels(state)
		: getModelsByProvider(state, provider);

	filteredModelsCache = { key: cacheKey, result };
	return result;
}

/**
 * Clears the filtered models cache.
 * Called automatically when LLM cache is invalidated.
 */
export function clearFilteredModelsCache(): void {
	filteredModelsCache = null;
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
	return state.providers[provider] ?? null;
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
	const model = await invoke<LLMModel>('create_model', { data });
	invalidateLLMCache();
	return model;
}

/**
 * Updates an existing model.
 * @param id - Model ID to update
 * @param data - Update data
 * @returns Promise resolving to updated model
 */
export async function updateModel(id: string, data: UpdateModelRequest): Promise<LLMModel> {
	const model = await invoke<LLMModel>('update_model', { id, data });
	invalidateLLMCache();
	return model;
}

/**
 * Deletes a custom model.
 * @param id - Model ID to delete
 * @returns Promise resolving to true if deleted
 */
export async function deleteModel(id: string): Promise<boolean> {
	const result = await invoke<boolean>('delete_model', { id });
	invalidateLLMCache();
	return result;
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
	const settings = await invoke<ProviderSettings>('update_provider_settings', {
		provider,
		enabled: enabled ?? null,
		defaultModelId: defaultModelId ?? null,
		baseUrl: baseUrl ?? null
	});
	invalidateLLMCache();
	return settings;
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
 * Lists all providers (builtin + custom) from the backend.
 * @returns Promise resolving to array of provider info
 */
export async function listProviders(): Promise<ProviderInfo[]> {
	return invoke<ProviderInfo[]>('list_providers');
}

/**
 * Creates a new custom provider.
 * @param name - URL-safe identifier
 * @param displayName - Human-readable name
 * @param baseUrl - API base URL
 * @param apiKey - API key
 * @returns Promise resolving to created provider info
 */
export async function createCustomProvider(
	name: string,
	displayName: string,
	baseUrl: string,
	apiKey: string
): Promise<ProviderInfo> {
	const result = await invoke<ProviderInfo>('create_custom_provider', {
		name,
		displayName,
		baseUrl,
		apiKey
	});
	invalidateLLMCache();
	return result;
}

/**
 * Updates an existing custom provider.
 * @param name - Provider identifier
 * @param displayName - New display name
 * @param baseUrl - New base URL
 * @param apiKey - New API key
 * @param enabled - Enable/disable
 * @returns Promise resolving to updated provider info
 */
export async function updateCustomProvider(
	name: string,
	displayName?: string,
	baseUrl?: string,
	apiKey?: string,
	enabled?: boolean
): Promise<ProviderInfo> {
	const result = await invoke<ProviderInfo>('update_custom_provider', {
		name,
		displayName: displayName ?? null,
		baseUrl: baseUrl ?? null,
		apiKey: apiKey ?? null,
		enabled: enabled ?? null
	});
	invalidateLLMCache();
	return result;
}

/**
 * Deletes a custom provider.
 * @param name - Provider identifier
 */
export async function deleteCustomProvider(name: string): Promise<void> {
	await invoke<void>('delete_custom_provider', { name });
	invalidateLLMCache();
}

/**
 * Loads all provider settings and models.
 * Uses cache with 30s TTL to avoid duplicate API calls.
 * @param forceRefresh - Force reload ignoring cache
 * @returns Promise resolving to object with providers, provider list, and models
 */
export async function loadAllLLMData(forceRefresh = false): Promise<{
	providerList: ProviderInfo[];
	settings: Record<string, ProviderSettings>;
	models: LLMModel[];
}> {
	const now = Date.now();
	if (!forceRefresh && llmCache.data && (now - llmCache.timestamp) < LLM_CACHE_TTL) {
		return llmCache.data;
	}

	const providerList = await listProviders();

	const [settingsResults, models] = await Promise.all([
		Promise.all(providerList.map((p) => loadProviderSettings(p.id).catch(() => null))),
		loadModels()
	]);

	const settings: Record<string, ProviderSettings> = {};
	providerList.forEach((p, i) => {
		const s = settingsResults[i];
		if (s) {
			settings[p.id] = s;
		}
	});

	const data = { providerList, settings, models };

	llmCache = {
		data,
		timestamp: now
	};

	return data;
}
