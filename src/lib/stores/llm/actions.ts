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
 * LLM async actions - Tauri IPC calls with cache management.
 * @module stores/llm/actions
 */

import { invoke } from '@tauri-apps/api/core';
import type {
	LLMModel,
	CreateModelRequest,
	UpdateModelRequest,
	ProviderSettings,
	ConnectionTestResult,
	ProviderType
} from '$types/llm';
import type { ProviderInfo, CustomProviderResponse } from '$types/custom-provider';
import { clearFilteredModelsCache } from './selectors';

// ============================================================================
// Cache Management
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
 * Invalidates the LLM data cache.
 * Call this after any mutation (create/update/delete model, update provider settings).
 */
export function invalidateLLMCache(): void {
	llmCache = { data: null, timestamp: 0 };
	clearFilteredModelsCache();
}

// ============================================================================
// Model Actions
// ============================================================================

/**
 * Loads all LLM models from the backend.
 */
export async function loadModels(provider?: ProviderType): Promise<LLMModel[]> {
	return invoke<LLMModel[]>('list_models', { provider: provider ?? null });
}

/**
 * Gets a single model by ID.
 */
export async function fetchModel(id: string): Promise<LLMModel> {
	return invoke<LLMModel>('get_model', { id });
}

/**
 * Gets a model by API name and provider.
 */
export async function fetchModelByApiName(apiName: string, provider: ProviderType): Promise<LLMModel> {
	return invoke<LLMModel>('get_model_by_api_name', { apiName, provider });
}

/**
 * Creates a new custom model.
 */
export async function createModel(data: CreateModelRequest): Promise<LLMModel> {
	const model = await invoke<LLMModel>('create_model', { data });
	invalidateLLMCache();
	return model;
}

/**
 * Updates an existing model.
 */
export async function updateModel(id: string, data: UpdateModelRequest): Promise<LLMModel> {
	const model = await invoke<LLMModel>('update_model', { id, data });
	invalidateLLMCache();
	return model;
}

/**
 * Deletes a custom model.
 */
export async function deleteModel(id: string): Promise<boolean> {
	const result = await invoke<boolean>('delete_model', { id });
	invalidateLLMCache();
	return result;
}

// ============================================================================
// Provider Settings Actions
// ============================================================================

/**
 * Loads provider settings from the backend.
 */
export async function loadProviderSettings(provider: ProviderType): Promise<ProviderSettings> {
	return invoke<ProviderSettings>('get_provider_settings', { provider });
}

/**
 * Updates provider settings.
 */
export async function updateProviderSettings(
	provider: ProviderType,
	enabled?: boolean,
	defaultModelId?: string,
	baseUrl?: string
): Promise<ProviderSettings> {
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
 */
export async function testConnection(provider: ProviderType): Promise<ConnectionTestResult> {
	return invoke<ConnectionTestResult>('test_provider_connection', { provider });
}

/**
 * Seeds the database with builtin models.
 */
export async function seedBuiltinModels(): Promise<number> {
	return invoke<number>('seed_builtin_models');
}

// ============================================================================
// Custom Provider Actions
// ============================================================================

/**
 * Lists all providers (builtin + custom) from the backend.
 */
export async function listProviders(): Promise<ProviderInfo[]> {
	return invoke<ProviderInfo[]>('list_providers');
}

/**
 * Creates a new custom provider.
 */
export async function createCustomProvider(
	name: string,
	displayName: string,
	baseUrl: string,
	apiKey: string
): Promise<CustomProviderResponse> {
	const result = await invoke<CustomProviderResponse>('create_custom_provider', {
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
 */
export async function updateCustomProvider(
	name: string,
	displayName?: string,
	baseUrl?: string,
	apiKey?: string,
	enabled?: boolean
): Promise<CustomProviderResponse> {
	const result = await invoke<CustomProviderResponse>('update_custom_provider', {
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
 */
export async function deleteCustomProvider(name: string): Promise<void> {
	await invoke<void>('delete_custom_provider', { name });
	invalidateLLMCache();
}

// ============================================================================
// Bulk Loading
// ============================================================================

/**
 * Loads all provider settings and models.
 * Uses cache with 30s TTL to avoid duplicate API calls.
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
