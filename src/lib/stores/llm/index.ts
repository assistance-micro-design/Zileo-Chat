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
 * LLM store - manages LLM provider and model state.
 * Re-exports all state, selectors, and actions from submodules.
 * @module stores/llm
 */

export {
	createInitialLLMState,
	setLLMLoading,
	setLLMError,
	setModels,
	addModel,
	updateModelInState,
	removeModel,
	setProviderSettings,
	setActiveProvider,
	setTestingProvider
} from './state';

export {
	getModelsByProvider,
	getAllModels,
	getBuiltinModels,
	getCustomModels,
	getBuiltinModelsByProvider,
	getCustomModelsByProvider,
	getModelById,
	getModelByApiName,
	getProviderSettingsFromState,
	isProviderEnabled,
	hasApiKey,
	getModelCount,
	getModelCountByProvider,
	getCustomModelCount,
	hasModel,
	isApiNameTaken,
	getFilteredModelsMemoized,
	clearFilteredModelsCache
} from './selectors';

export {
	invalidateLLMCache,
	loadModels,
	fetchModel,
	fetchModelByApiName,
	createModel,
	updateModel,
	deleteModel,
	loadProviderSettings,
	updateProviderSettings,
	testConnection,
	seedBuiltinModels,
	listProviders,
	createCustomProvider,
	updateCustomProvider,
	deleteCustomProvider,
	loadAllLLMData
} from './actions';
