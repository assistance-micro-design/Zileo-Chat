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
	import type { ProviderInfo } from '$types/customProvider';
	import { Card, Button, StatusIndicator, Modal, HelpButton, Select } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { ProviderCard, ModelCard, ModelForm } from '$lib/components/llm';
	import CustomProviderForm from './CustomProviderForm.svelte';
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
		updateProviderSettings,
		deleteCustomProvider
	} from '$lib/stores/llm';
	import { Plus, Cpu, Sparkles, Server, Globe } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { createModalController } from '$lib/utils/modal.svelte';
	import type { ModalController } from '$lib/utils/modal.svelte';

	/** Props */
	interface Props {
		/** Callback when API key modal should be opened */
		onConfigureApiKey: (provider: ProviderType, displayName?: string, isCustom?: boolean) => void;
	}

	let { onConfigureApiKey }: Props = $props();

	/** LLM state */
	let llmState = $state<LLMState>(createInitialLLMState());
	let providerList = $state<ProviderInfo[]>([]);
	const modelModal: ModalController<LLMModel> = createModalController<LLMModel>();
	let modelSaving = $state(false);
	let selectedModelsProvider = $state<ProviderType | 'all'>('all');
	let message = $state<{ type: 'success' | 'error'; text: string } | null>(null);
	let showCustomProviderForm = $state(false);

	/** Provider filter options for models section (dynamic from providerList) */
	const modelsProviderOptions: SelectOption[] = $derived([
		{ value: 'all', label: 'All Providers' },
		...providerList.map((p) => ({ value: p.id, label: p.displayName }))
	]);

	/**
	 * Loads all LLM data (providers and models) from the backend
	 */
	async function loadLLMData(): Promise<void> {
		llmState = setLLMLoading(llmState, true);
		try {
			const data = await loadAllLLMData();
			providerList = data.providerList;
			for (const [providerId, provSettings] of Object.entries(data.settings)) {
				llmState = setProviderSettings(llmState, providerId, provSettings);
			}
			llmState = setModels(llmState, data.models);
		} catch (err) {
			llmState = setLLMError(llmState, `Failed to load LLM data: ${err}`);
		}
	}

	/**
	 * Handles custom provider deletion
	 */
	async function handleDeleteCustomProvider(providerInfo: ProviderInfo): Promise<void> {
		if (!confirm($i18n('llm_custom_provider_delete_confirm'))) {
			return;
		}
		try {
			await deleteCustomProvider(providerInfo.id);
			message = { type: 'success', text: `Provider "${providerInfo.displayName}" deleted` };
			await loadLLMData();
		} catch (err) {
			message = { type: 'error', text: `Failed to delete provider: ${err}` };
		}
	}

	/**
	 * Handles custom provider creation success
	 */
	async function handleCustomProviderCreated(): Promise<void> {
		showCustomProviderForm = false;
		message = { type: 'success', text: $i18n('llm_custom_provider_created') };
		await loadLLMData();
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
			<!-- Builtin Provider Cards -->
			{#each providerList.filter((p) => p.isBuiltin) as provInfo (provInfo.id)}
				<ProviderCard
					provider={provInfo.id}
					settings={llmState.providers[provInfo.id] ?? null}
					hasApiKey={provInfo.id === 'ollama' ? true : providerHasApiKey(provInfo.id)}
					defaultModel={getProviderDefaultModel(provInfo.id)}
					onConfigure={() => onConfigureApiKey(provInfo.id)}
				>
					{#snippet icon()}
						{#if provInfo.id === 'mistral'}
							<Sparkles size={24} class="icon-accent" />
						{:else}
							<Server size={24} class="icon-success" />
						{/if}
					{/snippet}
				</ProviderCard>
			{/each}

			<!-- Custom Provider Cards -->
			{#each providerList.filter((p) => !p.isBuiltin) as provInfo (provInfo.id)}
				<ProviderCard
					provider={provInfo.id}
					displayName={provInfo.displayName}
					settings={llmState.providers[provInfo.id] ?? null}
					hasApiKey={providerHasApiKey(provInfo.id)}
					defaultModel={getProviderDefaultModel(provInfo.id)}
					isCustom={true}
					onConfigure={() => onConfigureApiKey(provInfo.id, provInfo.displayName, true)}
					onDelete={() => handleDeleteCustomProvider(provInfo)}
				>
					{#snippet icon()}
						<Globe size={24} class="icon-info" />
					{/snippet}
				</ProviderCard>
			{/each}
		</div>

		<!-- Add Custom Provider Button -->
		<div class="custom-provider-actions">
			<Button variant="secondary" size="sm" onclick={() => (showCustomProviderForm = true)}>
				<Plus size={16} />
				<span>{$i18n('llm_add_custom_provider')}</span>
			</Button>
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
						{:else}
							{$i18n('models_not_configured_provider')}
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
			{providerList}
			onsubmit={handleSaveModel}
			oncancel={() => modelModal.close()}
			saving={modelSaving}
		/>
	{/snippet}
</Modal>

<!-- Custom Provider Form Modal -->
<Modal
	open={showCustomProviderForm}
	title={$i18n('llm_add_custom_provider')}
	onclose={() => (showCustomProviderForm = false)}
>
	{#snippet body()}
		<CustomProviderForm
			oncreated={handleCustomProviderCreated}
			oncancel={() => (showCustomProviderForm = false)}
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

	.custom-provider-actions {
		display: flex;
		justify-content: flex-start;
		margin-top: var(--spacing-md);
	}

	.custom-provider-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
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
