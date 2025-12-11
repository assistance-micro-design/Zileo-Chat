<!--
  Copyright 2025 Assistance Micro Design

  Licensed under the Apache License, Version 2.0 (the "License");
  you may not use this file except in compliance with the License.
  You may obtain a copy of the License at

      http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing, software
  distributed under the License is distributed on an "AS IS" BASIS,
  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
  See the License for the specific language governing permissions and
  limitations under the License.
-->

<!--
LLM Section - Extracted from Settings page (OPT-6b)
Manages LLM providers and models: list, create, edit, delete, set default.
Combines Providers and Models sections.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import type {
		LLMModel,
		ProviderType,
		CreateModelRequest,
		UpdateModelRequest,
		LLMState
	} from '$types/llm';
	import { Card, Button, StatusIndicator, Modal, HelpButton, Select } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { ProviderCard, ModelCard, ModelForm } from '$lib/components/llm';
	import {
		createInitialLLMState,
		setLLMLoading,
		setLLMError,
		setModels,
		setProviderSettings,
		addModel as addModelToState,
		updateModelInState,
		removeModel,
		getFilteredModelsMemoized,
		getDefaultModel,
		hasApiKey as hasApiKeyInState,
		loadAllLLMData,
		createModel,
		updateModel,
		deleteModel,
		updateProviderSettings
	} from '$lib/stores/llm';
	import { Plus, Cpu, Sparkles, Server } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { createModalController } from '$lib/utils/modal.svelte';
	import type { ModalController } from '$lib/utils/modal.svelte';

	/** Props */
	interface Props {
		/** Callback when API key modal should be opened */
		onConfigureApiKey: (provider: ProviderType) => void;
	}

	let { onConfigureApiKey }: Props = $props();

	/** LLM state */
	let llmState = $state<LLMState>(createInitialLLMState());
	const modelModal: ModalController<LLMModel> = createModalController<LLMModel>();
	let modelSaving = $state(false);
	let selectedModelsProvider = $state<ProviderType | 'all'>('all');
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);

	/** Provider filter options for models section */
	const modelsProviderOptions: SelectOption[] = [
		{ value: 'all', label: 'All Providers' },
		{ value: 'mistral', label: 'Mistral' },
		{ value: 'ollama', label: 'Ollama' }
	];

	/**
	 * Loads all LLM data (providers and models) from the backend
	 */
	async function loadLLMData(): Promise<void> {
		llmState = setLLMLoading(llmState, true);
		try {
			const data = await loadAllLLMData();
			llmState = setProviderSettings(llmState, 'mistral', data.mistral);
			llmState = setProviderSettings(llmState, 'ollama', data.ollama);
			llmState = setModels(llmState, data.models);
		} catch (err) {
			llmState = setLLMError(llmState, `Failed to load LLM data: ${err}`);
		}
	}

	/**
	 * Handles model form submission (create or update)
	 */
	async function handleSaveModel(data: CreateModelRequest | UpdateModelRequest): Promise<void> {
		modelSaving = true;
		try {
			if (modelModal.mode === 'create') {
				const model = await createModel(data as CreateModelRequest);
				llmState = addModelToState(llmState, model);
				message = { type: 'success', text: `Model "${model.name}" created successfully` };
			} else if (modelModal.editing) {
				const model = await updateModel(modelModal.editing.id, data as UpdateModelRequest);
				llmState = updateModelInState(llmState, modelModal.editing.id, model);
				message = { type: 'success', text: `Model "${model.name}" updated successfully` };
			}
			modelModal.close();
		} catch (err) {
			message = { type: 'error', text: `Failed to save model: ${err}` };
		} finally {
			modelSaving = false;
		}
	}

	/**
	 * Handles model deletion
	 */
	async function handleDeleteModel(model: LLMModel): Promise<void> {
		if (!confirm(`Are you sure you want to delete "${model.name}"?`)) {
			return;
		}

		try {
			await deleteModel(model.id);
			llmState = removeModel(llmState, model.id);
			message = { type: 'success', text: `Model "${model.name}" deleted successfully` };
		} catch (err) {
			message = { type: 'error', text: `Failed to delete model: ${err}` };
		}
	}

	/**
	 * Handles setting a model as the default for its provider
	 */
	async function handleSetDefaultModel(model: LLMModel): Promise<void> {
		try {
			const updatedSettings = await updateProviderSettings(
				model.provider,
				undefined,
				model.id,
				undefined
			);
			llmState = setProviderSettings(llmState, model.provider, updatedSettings);
			message = { type: 'success', text: `"${model.name}" set as default` };
		} catch (err) {
			message = { type: 'error', text: `Failed to set default model: ${err}` };
		}
	}

	/**
	 * Handles provider models filter change
	 */
	function handleModelsProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		selectedModelsProvider = event.currentTarget.value as ProviderType | 'all';
	}

	/**
	 * Gets filtered models for the selected provider (or all if 'all' selected).
	 * Uses memoized selector to prevent recalculation during scroll (OPT-SCROLL-6).
	 */
	const filteredModels = $derived(
		getFilteredModelsMemoized(llmState, selectedModelsProvider)
	);

	/**
	 * Gets the default model for a specific provider
	 */
	function getProviderDefaultModel(provider: ProviderType): LLMModel | undefined {
		return getDefaultModel(llmState, provider);
	}

	/**
	 * Checks if a provider has an API key configured
	 */
	function providerHasApiKey(provider: ProviderType): boolean {
		return hasApiKeyInState(llmState, provider);
	}

	/**
	 * Reloads LLM data (exposed for parent component)
	 */
	export function reload(): void {
		loadLLMData();
	}

	/**
	 * Clears current message
	 */
	export function clearMessage(): void {
		message = null;
	}

	onMount(() => {
		loadLLMData();
	});
</script>

<!-- Providers Section -->
<section id="providers" class="settings-section">
	<div class="section-title-row">
		<h2 class="section-title">{$i18n('settings_providers')}</h2>
		<HelpButton
			titleKey="help_providers_title"
			descriptionKey="help_providers_description"
			tutorialKey="help_providers_tutorial"
		/>
	</div>

	{#if llmState.error}
		<div class="llm-error">
			{llmState.error}
		</div>
	{/if}

	{#if llmState.loading}
		<Card>
			{#snippet body()}
				<div class="llm-loading">
					<StatusIndicator status="running" />
					<span>{$i18n('providers_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="provider-grid">
			<!-- Mistral Provider Card -->
			<ProviderCard
				provider="mistral"
				settings={llmState.providers.mistral}
				hasApiKey={providerHasApiKey('mistral')}
				defaultModel={getProviderDefaultModel('mistral')}
				onConfigure={() => onConfigureApiKey('mistral')}
			>
				{#snippet icon()}
					<Sparkles size={24} class="icon-accent" />
				{/snippet}
			</ProviderCard>

			<!-- Ollama Provider Card -->
			<ProviderCard
				provider="ollama"
				settings={llmState.providers.ollama}
				hasApiKey={true}
				defaultModel={getProviderDefaultModel('ollama')}
				onConfigure={() => onConfigureApiKey('ollama')}
			>
				{#snippet icon()}
					<Server size={24} class="icon-success" />
				{/snippet}
			</ProviderCard>
		</div>
	{/if}

	{#if message}
		<div class="message-toast" class:success={message.type === 'success'} class:error={message.type === 'error'}>
			{message.text}
		</div>
	{/if}
</section>

<!-- Models Section -->
<section id="models" class="settings-section">
	<div class="section-header-row">
		<div class="section-title-row">
			<h2 class="section-title">{$i18n('settings_models')}</h2>
			<HelpButton
				titleKey="help_models_title"
				descriptionKey="help_models_description"
				tutorialKey="help_models_tutorial"
			/>
		</div>
		<div class="models-header-actions">
			<Select
				options={modelsProviderOptions}
				value={selectedModelsProvider}
				onchange={handleModelsProviderChange}
			/>
			<Button variant="primary" size="sm" onclick={() => modelModal.openCreate()}>
				<Plus size={16} />
				<span>{$i18n('models_add')}</span>
			</Button>
		</div>
	</div>

	{#if llmState.loading}
		<Card>
			{#snippet body()}
				<div class="llm-loading">
					<StatusIndicator status="running" />
					<span>{$i18n('models_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if filteredModels.length === 0}
		<Card>
			{#snippet body()}
				<div class="models-empty">
					<Cpu size={48} class="empty-icon" />
					<h3 class="empty-title">{$i18n('models_not_found')}</h3>
					<p class="empty-description">
						{#if selectedModelsProvider === 'all'}
							{$i18n('models_not_configured_all')}
						{:else if selectedModelsProvider === 'mistral'}
							{$i18n('models_not_configured_mistral')}
						{:else}
							{$i18n('models_not_configured_ollama')}
						{/if}
						{$i18n('models_add_custom')}
					</p>
					<Button variant="primary" onclick={() => modelModal.openCreate()}>
						<Plus size={16} />
						<span>{$i18n('models_add_first')}</span>
					</Button>
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="models-grid">
			{#each filteredModels as model (model.id)}
				<ModelCard
					{model}
					isDefault={llmState.providers[model.provider]?.default_model_id === model.id}
					onEdit={() => modelModal.openEdit(model)}
					onDelete={() => handleDeleteModel(model)}
					onSetDefault={() => handleSetDefaultModel(model)}
				/>
			{/each}
		</div>
	{/if}
</section>

<!-- Model Modal (Create/Edit) -->
<Modal
	open={modelModal.show}
	title={modelModal.mode === 'create' ? $i18n('modal_add_custom_model') : $i18n('modal_edit_model')}
	onclose={() => modelModal.close()}
>
	{#snippet body()}
		<ModelForm
			mode={modelModal.mode}
			model={modelModal.editing}
			provider={selectedModelsProvider === 'all' ? 'mistral' : selectedModelsProvider}
			onsubmit={handleSaveModel}
			oncancel={() => modelModal.close()}
			saving={modelSaving}
		/>
	{/snippet}
</Modal>

<style>
	.settings-section {
		margin-bottom: var(--spacing-2xl);
		padding-bottom: var(--spacing-xl);
	}

	.section-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-lg);
	}

	.section-title-row .section-title {
		margin-bottom: 0;
	}

	.section-header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: var(--spacing-lg);
	}

	.section-header-row .section-title {
		margin-bottom: 0;
	}

	.section-header-row :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* Provider Cards */
	.provider-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
		margin-bottom: var(--spacing-lg);
		contain: layout style; /* OPT-SCROLL-5: Isolate layout recalculations */
	}

	.message-toast {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		margin-top: var(--spacing-md);
	}

	.message-toast.success {
		background: var(--color-success-light);
		color: var(--color-success);
	}

	.message-toast.error {
		background: var(--color-error-light);
		color: var(--color-error);
	}

	/* LLM Section */
	.llm-error {
		padding: var(--spacing-md);
		background: var(--color-error-light);
		color: var(--color-error);
		border-radius: var(--border-radius-md);
		margin-bottom: var(--spacing-lg);
	}

	.llm-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	/* Models Section */
	.models-header-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
	}

	.models-header-actions :global(.form-group) {
		margin-bottom: 0;
	}

	.models-header-actions :global(.form-select) {
		width: auto;
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-xs);
	}

	.models-header-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.models-grid {
		display: grid;
		grid-template-columns: repeat(2, 1fr);
		gap: var(--spacing-lg);
		contain: layout style; /* OPT-SCROLL-5: Isolate layout recalculations */
	}

	.models-empty {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: var(--spacing-2xl);
		gap: var(--spacing-md);
	}

	.models-empty :global(.empty-icon) {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.empty-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.empty-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		max-width: 400px;
	}

	.models-empty :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* Responsive */
	@media (max-width: 768px) {
		.provider-grid,
		.models-grid {
			grid-template-columns: 1fr;
		}

		.models-header-actions {
			flex-direction: column;
			align-items: stretch;
		}
	}
</style>
