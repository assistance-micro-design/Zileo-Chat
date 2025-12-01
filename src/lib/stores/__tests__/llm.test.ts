// Copyright 2025 Zileo-Chat-3 Contributors
// SPDX-License-Identifier: Apache-2.0

import { describe, it, expect, beforeEach, vi } from 'vitest';
import {
	createInitialLLMState,
	setLLMLoading,
	setLLMError,
	setModels,
	addModel,
	updateModelInState,
	removeModel,
	setProviderSettings,
	setActiveProvider,
	setTestingProvider,
	getModelsByProvider,
	getBuiltinModels,
	getCustomModels,
	getBuiltinModelsByProvider,
	getCustomModelsByProvider,
	getModelById,
	getModelByApiName,
	getDefaultModel,
	getProviderSettingsFromState,
	isProviderEnabled,
	hasApiKey,
	getModelCount,
	getModelCountByProvider,
	getCustomModelCount,
	hasModel,
	isApiNameTaken
} from '../llm';
import type { LLMModel, LLMState, ProviderSettings, ProviderType } from '$types/llm';

// Mock @tauri-apps/api/core
vi.mock('@tauri-apps/api/core', () => ({
	invoke: vi.fn()
}));

describe('LLM Store', () => {
	let initialState: LLMState;

	const createMockModel = (
		id: string,
		provider: ProviderType,
		isBuiltin: boolean = false
	): LLMModel => ({
		id,
		provider,
		name: `Model ${id}`,
		api_name: `model-${id}`,
		context_window: 32000,
		max_output_tokens: 4096,
		temperature_default: 0.7,
		is_builtin: isBuiltin,
		is_reasoning: false,
		input_price_per_mtok: 0,
		output_price_per_mtok: 0,
		created_at: new Date().toISOString(),
		updated_at: new Date().toISOString()
	});

	const createMockProviderSettings = (
		provider: ProviderType,
		options: Partial<ProviderSettings> = {}
	): ProviderSettings => ({
		provider,
		enabled: true,
		default_model_id: null,
		api_key_configured: false,
		base_url: provider === 'ollama' ? 'http://localhost:11434' : null,
		updated_at: new Date().toISOString(),
		...options
	});

	beforeEach(() => {
		initialState = createInitialLLMState();
	});

	// =========================================================================
	// Initial State Tests
	// =========================================================================

	describe('createInitialLLMState', () => {
		it('should create empty initial state', () => {
			const state = createInitialLLMState();

			expect(state.providers.mistral).toBeNull();
			expect(state.providers.ollama).toBeNull();
			expect(state.models).toEqual([]);
			expect(state.activeProvider).toBeNull();
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
			expect(state.testingProvider).toBeNull();
		});
	});

	// =========================================================================
	// Pure State Updater Tests
	// =========================================================================

	describe('setLLMLoading', () => {
		it('should set loading to true', () => {
			const state = setLLMLoading(initialState, true);
			expect(state.loading).toBe(true);
		});

		it('should set loading to false', () => {
			let state = setLLMLoading(initialState, true);
			state = setLLMLoading(state, false);
			expect(state.loading).toBe(false);
		});

		it('should clear error when setting loading to true', () => {
			let state = setLLMError(initialState, 'Test error');
			state = setLLMLoading(state, true);
			expect(state.error).toBeNull();
		});
	});

	describe('setLLMError', () => {
		it('should set error message', () => {
			const state = setLLMError(initialState, 'Test error');
			expect(state.error).toBe('Test error');
			expect(state.loading).toBe(false);
		});

		it('should clear error with null', () => {
			let state = setLLMError(initialState, 'Test error');
			state = setLLMError(state, null);
			expect(state.error).toBeNull();
		});
	});

	describe('setModels', () => {
		it('should replace all models', () => {
			const model1 = createMockModel('m1', 'mistral');
			const model2 = createMockModel('m2', 'ollama');

			const state = setModels(initialState, [model1, model2]);

			expect(state.models).toHaveLength(2);
			expect(state.loading).toBe(false);
			expect(state.error).toBeNull();
		});

		it('should handle empty array', () => {
			const model = createMockModel('m1', 'mistral');
			let state = addModel(initialState, model);

			state = setModels(state, []);

			expect(state.models).toHaveLength(0);
		});
	});

	describe('addModel', () => {
		it('should add a model to empty state', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			expect(state.models).toHaveLength(1);
			expect(state.models[0].id).toBe('m1');
			expect(state.error).toBeNull();
		});

		it('should add model to existing models', () => {
			const model1 = createMockModel('m1', 'mistral');
			const model2 = createMockModel('m2', 'ollama');

			let state = addModel(initialState, model1);
			state = addModel(state, model2);

			expect(state.models).toHaveLength(2);
			expect(state.models[1].id).toBe('m2');
		});

		it('should update model if ID already exists', () => {
			const model = createMockModel('m1', 'mistral');
			const updatedModel = { ...model, name: 'Updated Name' };

			let state = addModel(initialState, model);
			state = addModel(state, updatedModel);

			expect(state.models).toHaveLength(1);
			expect(state.models[0].name).toBe('Updated Name');
		});
	});

	describe('updateModelInState', () => {
		it('should update existing model', () => {
			const model = createMockModel('m1', 'mistral');
			const updatedModel = { ...model, name: 'Updated Name' };

			let state = addModel(initialState, model);
			state = updateModelInState(state, 'm1', updatedModel);

			expect(state.models[0].name).toBe('Updated Name');
			expect(state.error).toBeNull();
		});

		it('should not modify other models', () => {
			const model1 = createMockModel('m1', 'mistral');
			const model2 = createMockModel('m2', 'ollama');

			let state = addModel(initialState, model1);
			state = addModel(state, model2);

			const updated1 = { ...model1, name: 'Updated' };
			state = updateModelInState(state, 'm1', updated1);

			expect(state.models[0].name).toBe('Updated');
			expect(state.models[1].name).toBe('Model m2');
		});

		it('should not crash when updating non-existent model', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const nonExistent = createMockModel('m2', 'ollama');
			const updated = updateModelInState(state, 'm2', nonExistent);

			expect(updated.models).toHaveLength(1);
			expect(updated.models[0].id).toBe('m1');
		});
	});

	describe('removeModel', () => {
		it('should remove existing model', () => {
			const model = createMockModel('m1', 'mistral');
			let state = addModel(initialState, model);

			state = removeModel(state, 'm1');

			expect(state.models).toHaveLength(0);
			expect(state.error).toBeNull();
		});

		it('should not affect other models', () => {
			const model1 = createMockModel('m1', 'mistral');
			const model2 = createMockModel('m2', 'ollama');

			let state = addModel(initialState, model1);
			state = addModel(state, model2);
			state = removeModel(state, 'm1');

			expect(state.models).toHaveLength(1);
			expect(state.models[0].id).toBe('m2');
		});

		it('should not crash when removing non-existent model', () => {
			const model = createMockModel('m1', 'mistral');
			let state = addModel(initialState, model);

			state = removeModel(state, 'nonexistent');

			expect(state.models).toHaveLength(1);
		});
	});

	describe('setProviderSettings', () => {
		it('should set mistral settings', () => {
			const settings = createMockProviderSettings('mistral', { api_key_configured: true });

			const state = setProviderSettings(initialState, 'mistral', settings);

			expect(state.providers.mistral).toBeDefined();
			expect(state.providers.mistral?.api_key_configured).toBe(true);
			expect(state.providers.ollama).toBeNull();
		});

		it('should set ollama settings', () => {
			const settings = createMockProviderSettings('ollama', { default_model_id: 'llama3' });

			const state = setProviderSettings(initialState, 'ollama', settings);

			expect(state.providers.ollama).toBeDefined();
			expect(state.providers.ollama?.default_model_id).toBe('llama3');
			expect(state.providers.mistral).toBeNull();
		});
	});

	describe('setActiveProvider', () => {
		it('should set active provider', () => {
			const state = setActiveProvider(initialState, 'mistral');
			expect(state.activeProvider).toBe('mistral');
		});

		it('should allow clearing active provider', () => {
			let state = setActiveProvider(initialState, 'mistral');
			state = setActiveProvider(state, null);
			expect(state.activeProvider).toBeNull();
		});
	});

	describe('setTestingProvider', () => {
		it('should set testing provider', () => {
			const state = setTestingProvider(initialState, 'ollama');
			expect(state.testingProvider).toBe('ollama');
		});

		it('should allow clearing testing provider', () => {
			let state = setTestingProvider(initialState, 'ollama');
			state = setTestingProvider(state, null);
			expect(state.testingProvider).toBeNull();
		});
	});

	// =========================================================================
	// Selector Tests
	// =========================================================================

	describe('getModelsByProvider', () => {
		it('should filter models by provider', () => {
			const m1 = createMockModel('m1', 'mistral');
			const m2 = createMockModel('m2', 'ollama');
			const m3 = createMockModel('m3', 'mistral');

			let state = addModel(initialState, m1);
			state = addModel(state, m2);
			state = addModel(state, m3);

			const mistralModels = getModelsByProvider(state, 'mistral');
			const ollamaModels = getModelsByProvider(state, 'ollama');

			expect(mistralModels).toHaveLength(2);
			expect(ollamaModels).toHaveLength(1);
		});

		it('should return empty array for no matches', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const ollamaModels = getModelsByProvider(state, 'ollama');
			expect(ollamaModels).toHaveLength(0);
		});
	});

	describe('getBuiltinModels', () => {
		it('should return only builtin models', () => {
			const builtin = createMockModel('b1', 'mistral', true);
			const custom = createMockModel('c1', 'mistral', false);

			let state = addModel(initialState, builtin);
			state = addModel(state, custom);

			const builtins = getBuiltinModels(state);
			expect(builtins).toHaveLength(1);
			expect(builtins[0].id).toBe('b1');
		});
	});

	describe('getCustomModels', () => {
		it('should return only custom models', () => {
			const builtin = createMockModel('b1', 'mistral', true);
			const custom = createMockModel('c1', 'mistral', false);

			let state = addModel(initialState, builtin);
			state = addModel(state, custom);

			const customs = getCustomModels(state);
			expect(customs).toHaveLength(1);
			expect(customs[0].id).toBe('c1');
		});
	});

	describe('getBuiltinModelsByProvider', () => {
		it('should return builtin models for specific provider', () => {
			const b1 = createMockModel('b1', 'mistral', true);
			const b2 = createMockModel('b2', 'ollama', true);
			const c1 = createMockModel('c1', 'mistral', false);

			let state = addModel(initialState, b1);
			state = addModel(state, b2);
			state = addModel(state, c1);

			const mistralBuiltins = getBuiltinModelsByProvider(state, 'mistral');
			expect(mistralBuiltins).toHaveLength(1);
			expect(mistralBuiltins[0].id).toBe('b1');
		});
	});

	describe('getCustomModelsByProvider', () => {
		it('should return custom models for specific provider', () => {
			const b1 = createMockModel('b1', 'mistral', true);
			const c1 = createMockModel('c1', 'mistral', false);
			const c2 = createMockModel('c2', 'ollama', false);

			let state = addModel(initialState, b1);
			state = addModel(state, c1);
			state = addModel(state, c2);

			const ollamaCustoms = getCustomModelsByProvider(state, 'ollama');
			expect(ollamaCustoms).toHaveLength(1);
			expect(ollamaCustoms[0].id).toBe('c2');
		});
	});

	describe('getModelById', () => {
		it('should return model by ID', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const found = getModelById(state, 'm1');
			expect(found?.id).toBe('m1');
		});

		it('should return undefined for non-existent ID', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const found = getModelById(state, 'nonexistent');
			expect(found).toBeUndefined();
		});
	});

	describe('getModelByApiName', () => {
		it('should return model by API name and provider', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const found = getModelByApiName(state, 'model-m1', 'mistral');
			expect(found?.id).toBe('m1');
		});

		it('should not match different provider', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const found = getModelByApiName(state, 'model-m1', 'ollama');
			expect(found).toBeUndefined();
		});
	});

	describe('getDefaultModel', () => {
		it('should return default model when set', () => {
			const model = createMockModel('m1', 'mistral');
			const settings = createMockProviderSettings('mistral', { default_model_id: 'm1' });

			let state = addModel(initialState, model);
			state = setProviderSettings(state, 'mistral', settings);

			const defaultModel = getDefaultModel(state, 'mistral');
			expect(defaultModel?.id).toBe('m1');
		});

		it('should return undefined when no default set', () => {
			const settings = createMockProviderSettings('mistral');
			const state = setProviderSettings(initialState, 'mistral', settings);

			const defaultModel = getDefaultModel(state, 'mistral');
			expect(defaultModel).toBeUndefined();
		});
	});

	describe('getProviderSettingsFromState', () => {
		it('should return settings for provider', () => {
			const settings = createMockProviderSettings('mistral', { enabled: false });
			const state = setProviderSettings(initialState, 'mistral', settings);

			const retrieved = getProviderSettingsFromState(state, 'mistral');
			expect(retrieved?.enabled).toBe(false);
		});

		it('should return null when no settings', () => {
			const retrieved = getProviderSettingsFromState(initialState, 'mistral');
			expect(retrieved).toBeNull();
		});
	});

	describe('isProviderEnabled', () => {
		it('should return true when provider is enabled', () => {
			const settings = createMockProviderSettings('mistral', { enabled: true });
			const state = setProviderSettings(initialState, 'mistral', settings);

			expect(isProviderEnabled(state, 'mistral')).toBe(true);
		});

		it('should return false when provider is disabled', () => {
			const settings = createMockProviderSettings('mistral', { enabled: false });
			const state = setProviderSettings(initialState, 'mistral', settings);

			expect(isProviderEnabled(state, 'mistral')).toBe(false);
		});

		it('should return false when no settings', () => {
			expect(isProviderEnabled(initialState, 'mistral')).toBe(false);
		});
	});

	describe('hasApiKey', () => {
		it('should return true when API key is configured', () => {
			const settings = createMockProviderSettings('mistral', { api_key_configured: true });
			const state = setProviderSettings(initialState, 'mistral', settings);

			expect(hasApiKey(state, 'mistral')).toBe(true);
		});

		it('should return false when API key is not configured', () => {
			const settings = createMockProviderSettings('mistral', { api_key_configured: false });
			const state = setProviderSettings(initialState, 'mistral', settings);

			expect(hasApiKey(state, 'mistral')).toBe(false);
		});
	});

	describe('getModelCount', () => {
		it('should return 0 for empty state', () => {
			expect(getModelCount(initialState)).toBe(0);
		});

		it('should return correct count', () => {
			const m1 = createMockModel('m1', 'mistral');
			const m2 = createMockModel('m2', 'ollama');

			let state = addModel(initialState, m1);
			state = addModel(state, m2);

			expect(getModelCount(state)).toBe(2);
		});
	});

	describe('getModelCountByProvider', () => {
		it('should return count for specific provider', () => {
			const m1 = createMockModel('m1', 'mistral');
			const m2 = createMockModel('m2', 'mistral');
			const m3 = createMockModel('m3', 'ollama');

			let state = addModel(initialState, m1);
			state = addModel(state, m2);
			state = addModel(state, m3);

			expect(getModelCountByProvider(state, 'mistral')).toBe(2);
			expect(getModelCountByProvider(state, 'ollama')).toBe(1);
		});
	});

	describe('getCustomModelCount', () => {
		it('should return count of custom models only', () => {
			const builtin = createMockModel('b1', 'mistral', true);
			const custom1 = createMockModel('c1', 'mistral', false);
			const custom2 = createMockModel('c2', 'ollama', false);

			let state = addModel(initialState, builtin);
			state = addModel(state, custom1);
			state = addModel(state, custom2);

			expect(getCustomModelCount(state)).toBe(2);
		});
	});

	describe('hasModel', () => {
		it('should return true for existing model', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			expect(hasModel(state, 'm1')).toBe(true);
		});

		it('should return false for non-existing model', () => {
			expect(hasModel(initialState, 'nonexistent')).toBe(false);
		});
	});

	describe('isApiNameTaken', () => {
		it('should return true when API name is taken', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			expect(isApiNameTaken(state, 'model-m1', 'mistral')).toBe(true);
		});

		it('should return false when API name is not taken', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			expect(isApiNameTaken(state, 'new-api-name', 'mistral')).toBe(false);
		});

		it('should not match different provider', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			expect(isApiNameTaken(state, 'model-m1', 'ollama')).toBe(false);
		});

		it('should exclude specified ID', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			// Excluding m1 should make its api_name not "taken"
			expect(isApiNameTaken(state, 'model-m1', 'mistral', 'm1')).toBe(false);
		});
	});

	// =========================================================================
	// State Immutability Tests
	// =========================================================================

	describe('State Immutability', () => {
		it('should not mutate original state when adding model', () => {
			const model = createMockModel('m1', 'mistral');
			const newState = addModel(initialState, model);

			expect(initialState.models).toHaveLength(0);
			expect(newState.models).toHaveLength(1);
		});

		it('should not mutate original state when updating model', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);

			const updatedModel = { ...model, name: 'Updated' };
			const newState = updateModelInState(state, 'm1', updatedModel);

			expect(state.models[0].name).toBe('Model m1');
			expect(newState.models[0].name).toBe('Updated');
		});

		it('should not mutate original state when removing model', () => {
			const model = createMockModel('m1', 'mistral');
			const state = addModel(initialState, model);
			const newState = removeModel(state, 'm1');

			expect(state.models).toHaveLength(1);
			expect(newState.models).toHaveLength(0);
		});

		it('should not mutate original state when setting provider settings', () => {
			const settings = createMockProviderSettings('mistral');
			const newState = setProviderSettings(initialState, 'mistral', settings);

			expect(initialState.providers.mistral).toBeNull();
			expect(newState.providers.mistral).toBeDefined();
		});
	});
});
