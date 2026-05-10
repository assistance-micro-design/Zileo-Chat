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
 * LLM selectors - pure functions for querying LLM state.
 * Includes memoized selectors for performance.
 * @module stores/llm/selectors
 */

import type { LLMState, LLMModel, ProviderSettings, ProviderType } from '$types/llm';

// ============================================================================
// Filtered Models Cache (memoization)
// ============================================================================

interface FilteredModelsCache {
	key: string;
	result: LLMModel[];
}
let filteredModelsCache: FilteredModelsCache | null = null;

/**
 * Clears the filtered models cache.
 * Called automatically when LLM cache is invalidated.
 */
export function clearFilteredModelsCache(): void {
	filteredModelsCache = null;
}

/**
 * Computes a simple hash from the models array for cache invalidation.
 */
function computeModelsHash(models: LLMModel[]): string {
	if (models.length === 0) return 'empty';
	return `${models.length}:${models[0]?.id ?? ''}:${models[models.length - 1]?.id ?? ''}`;
}

// ============================================================================
// Basic Selectors
// ============================================================================

/**
 * Gets models filtered by provider.
 */
export function getModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => m.provider === provider);
}

/**
 * Gets all models regardless of provider.
 */
export function getAllModels(state: LLMState): LLMModel[] {
	return state.models;
}

/**
 * Gets all builtin models.
 */
export function getBuiltinModels(state: LLMState): LLMModel[] {
	return state.models.filter((m) => m.is_builtin);
}

/**
 * Gets all custom (user-created) models.
 */
export function getCustomModels(state: LLMState): LLMModel[] {
	return state.models.filter((m) => !m.is_builtin);
}

/**
 * Gets builtin models for a specific provider.
 */
export function getBuiltinModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => m.is_builtin && m.provider === provider);
}

/**
 * Gets custom models for a specific provider.
 */
export function getCustomModelsByProvider(state: LLMState, provider: ProviderType): LLMModel[] {
	return state.models.filter((m) => !m.is_builtin && m.provider === provider);
}

/**
 * Gets a model by ID.
 */
export function getModelById(state: LLMState, id: string): LLMModel | undefined {
	return state.models.find((m) => m.id === id);
}

/**
 * Gets a model by API name and provider.
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
 */
export function getProviderSettingsFromState(
	state: LLMState,
	provider: ProviderType
): ProviderSettings | null {
	return state.providers[provider] ?? null;
}

/**
 * Checks if a provider is enabled.
 */
export function isProviderEnabled(state: LLMState, provider: ProviderType): boolean {
	return state.providers[provider]?.enabled ?? false;
}

/**
 * Checks if a provider has an API key configured.
 */
export function hasApiKey(state: LLMState, provider: ProviderType): boolean {
	return state.providers[provider]?.api_key_configured ?? false;
}

// ============================================================================
// Count Selectors
// ============================================================================

/**
 * Gets total model count.
 */
export function getModelCount(state: LLMState): number {
	return state.models.length;
}

/**
 * Gets model count for a specific provider.
 */
export function getModelCountByProvider(state: LLMState, provider: ProviderType): number {
	return getModelsByProvider(state, provider).length;
}

/**
 * Gets custom model count.
 */
export function getCustomModelCount(state: LLMState): number {
	return getCustomModels(state).length;
}

/**
 * Checks if a model exists.
 */
export function hasModel(state: LLMState, id: string): boolean {
	return state.models.some((m) => m.id === id);
}

/**
 * Checks if an API name is already taken for a provider.
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
// Memoized Selectors
// ============================================================================

/**
 * Gets filtered models with memoization to prevent recalculation during scroll.
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

	const result = provider === 'all' ? getAllModels(state) : getModelsByProvider(state, provider);

	filteredModelsCache = { key: cacheKey, result };
	return result;
}
