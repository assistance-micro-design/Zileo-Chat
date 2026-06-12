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
LLM Section - Extracted from Settings page
Manages LLM providers and models: list, create, edit, delete.
Builtin providers render as cards, custom providers as compact entity rows
(edit / test / delete on hover), models as a filterable card grid.
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
	import type { ProviderInfo } from '$types/custom-provider';
	import {
		Card,
		Button,
		StatusIndicator,
		Modal,
		Select,
		DeleteConfirmModal,
		ErrorBanner,
		HelpButton
	} from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { ProviderCard, ModelCard, ModelForm } from '$lib/components/llm';
	import SettingsSectionHeader from '../SettingsSectionHeader.svelte';
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
		hasApiKey as hasApiKeyInState,
		loadAllLLMData,
		createModel,
		updateModel,
		deleteModel,
		deleteCustomProvider,
		testConnection
	} from '$lib/stores/llm';
	import { Plus, Cpu, Server, Globe, Pencil, Zap, Trash2 } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import { createModalController } from '$lib/utils/modal.svelte';
	import type { ModalController } from '$lib/utils/modal.svelte';
	import { getErrorMessage } from '$lib/utils/error';
	import { dispatchSettingsRefresh } from '$lib/utils/settings-refresh';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';

	/**
	 * Emits a transient toast for a completed CRUD action. Centralised so every
	 * call site uses the same duration and shape.
	 */
	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	/** Props */
	interface Props {
		/** Callback when API key modal should be opened (cloud builtin providers) */
		onConfigureApiKey: (provider: ProviderType, hasApiKey: boolean, displayName?: string) => void;
	}

	let { onConfigureApiKey }: Props = $props();

	/** LLM state */
	let llmState = $state<LLMState>(createInitialLLMState());
	let providerList = $state<ProviderInfo[]>([]);
	const modelModal: ModalController<LLMModel> = createModalController<LLMModel>();
	let modelSaving = $state(false);
	let selectedModelsProvider = $state<ProviderType | 'all'>('all');
	let showCustomProviderForm = $state(false);
	/** Provider being edited (null = the form is in create mode). */
	let providerToEdit = $state<ProviderInfo | null>(null);

	/** Provider delete confirmation state */
	let showProviderDeleteConfirm = $state(false);
	let providerToDelete = $state<ProviderInfo | null>(null);
	let providerDeleting = $state(false);

	/** Model delete confirmation state */
	let showModelDeleteConfirm = $state(false);
	let modelToDelete = $state<LLMModel | null>(null);
	let modelDeleting = $state(false);

	/** Provider currently running an inline connection test (entity rows) */
	let testingProviderId = $state<string | null>(null);

	/** Provider filter options for models section (dynamic from providerList) */
	const modelsProviderOptions: SelectOption[] = $derived([
		{ value: 'all', label: $i18n('providers_all') },
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
			llmState = setLLMError(
				llmState,
				$i18n('settings_llm_load_failed', { error: getErrorMessage(err) })
			);
		}
	}

	/**
	 * Requests provider delete confirmation
	 */
	function handleDeleteProviderRequest(providerInfo: ProviderInfo): void {
		providerToDelete = providerInfo;
		showProviderDeleteConfirm = true;
	}

	/**
	 * Confirms and executes provider deletion
	 */
	async function confirmDeleteProvider(): Promise<void> {
		if (!providerToDelete) return;
		providerDeleting = true;
		try {
			const deletedName = providerToDelete.displayName;
			const deletedId = providerToDelete.id;
			await deleteCustomProvider(deletedId);
			// Mutate the local provider list + models list in-place so the UI does not
			// re-enter the loading state. The backend has already removed the rows,
			// so dropping them from memory keeps the two in sync.
			providerList = providerList.filter((p) => p.id !== deletedId);
			llmState = {
				...llmState,
				providers: Object.fromEntries(
					Object.entries(llmState.providers).filter(([id]) => id !== deletedId)
				),
				models: llmState.models.filter((m) => m.provider !== deletedId),
				error: null
			};
			notify('success', $i18n('settings_provider_deleted', { name: deletedName }));
			showProviderDeleteConfirm = false;
			providerToDelete = null;
			dispatchSettingsRefresh({ source: 'providers' });
		} catch (err) {
			notify('error', $i18n('settings_provider_delete_failed', { error: getErrorMessage(err) }));
		} finally {
			providerDeleting = false;
		}
	}

	/**
	 * Cancels provider delete confirmation
	 */
	function cancelDeleteProvider(): void {
		showProviderDeleteConfirm = false;
		providerToDelete = null;
	}

	/**
	 * Handles custom provider creation success.
	 * @param warning - Optional security warning from the backend
	 */
	async function handleCustomProviderCreated(
		newProvider: ProviderInfo,
		warning?: string
	): Promise<void> {
		closeCustomProviderForm();
		// Append the provider locally to avoid a reload flicker. Provider settings
		// for a freshly created entity are not yet in state; loadProviderSettings
		// would return defaults that the backend will generate on first use.
		if (!providerList.some((p) => p.id === newProvider.id)) {
			providerList = [...providerList, newProvider];
		}
		if (warning) {
			notify('warning', warning);
		} else {
			notify('success', $i18n('llm_custom_provider_created'));
		}
		dispatchSettingsRefresh({ source: 'providers' });
	}

	/**
	 * Opens the custom provider form in edit mode for the given provider.
	 */
	function handleEditProviderRequest(providerInfo: ProviderInfo): void {
		providerToEdit = providerInfo;
		showCustomProviderForm = true;
	}

	/**
	 * Closes the custom provider form and clears any edit target.
	 */
	function closeCustomProviderForm(): void {
		showCustomProviderForm = false;
		providerToEdit = null;
	}

	/**
	 * Handles custom provider update success.
	 * @param warning - Optional security warning from the backend
	 */
	function handleCustomProviderUpdated(updatedProvider: ProviderInfo, warning?: string): void {
		closeCustomProviderForm();
		// Replace the updated entity in place to refresh display name / flags
		// without a full reload.
		providerList = providerList.map((p) => (p.id === updatedProvider.id ? updatedProvider : p));
		if (warning) {
			notify('warning', warning);
		} else {
			notify('success', $i18n('llm_custom_provider_updated'));
		}
		dispatchSettingsRefresh({ source: 'providers' });
	}

	/**
	 * Runs a connection test for a custom provider row and reports the
	 * outcome as a toast (the row has no space for an inline result).
	 */
	async function handleTestProvider(providerInfo: ProviderInfo): Promise<void> {
		testingProviderId = providerInfo.id;
		try {
			const result = await testConnection(providerInfo.id);
			if (result.success) {
				notify(
					'success',
					$i18n('llm_connection_connected', { latency: `${result.latency_ms ?? 0}ms` })
				);
			} else {
				notify('error', result.error_message || $i18n('llm_connection_failed'));
			}
		} catch (err) {
			notify('error', getErrorMessage(err));
		} finally {
			testingProviderId = null;
		}
	}

	/**
	 * Builds the entity-row meta line for a custom provider
	 * (base URL + strict-mode toggles).
	 */
	function customProviderMeta(providerInfo: ProviderInfo): string {
		const yesNo = (value: boolean): string => $i18n(value ? 'common_yes' : 'common_no');
		return $i18n('llm_custom_provider_meta', {
			url: providerInfo.baseUrl ?? '',
			cache: yesNo(providerInfo.supportsCacheControl ?? true),
			reasoning: yesNo(providerInfo.supportsReasoningParam ?? true)
		});
	}

	/**
	 * Resolves the display name of a model's provider for the model cards.
	 */
	function providerLabelOf(providerId: string): string {
		return providerList.find((p) => p.id === providerId)?.displayName ?? providerId;
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
				notify('success', $i18n('settings_model_created', { name: model.name }));
			} else if (modelModal.editing) {
				const model = await updateModel(modelModal.editing.id, data as UpdateModelRequest);
				llmState = updateModelInState(llmState, modelModal.editing.id, model);
				notify('success', $i18n('settings_model_updated', { name: model.name }));
			}
			modelModal.close();
			dispatchSettingsRefresh({ source: 'providers' });
		} catch (err) {
			notify('error', $i18n('settings_model_save_failed', { error: getErrorMessage(err) }));
		} finally {
			modelSaving = false;
		}
	}

	/**
	 * Requests model delete confirmation
	 */
	function handleDeleteModelRequest(model: LLMModel): void {
		modelToDelete = model;
		showModelDeleteConfirm = true;
	}

	/**
	 * Confirms and executes model deletion
	 */
	async function confirmDeleteModel(): Promise<void> {
		if (!modelToDelete) return;
		modelDeleting = true;
		try {
			const deletedName = modelToDelete.name;
			await deleteModel(modelToDelete.id);
			llmState = removeModel(llmState, modelToDelete.id);
			notify('success', $i18n('settings_model_deleted', { name: deletedName }));
			showModelDeleteConfirm = false;
			modelToDelete = null;
			dispatchSettingsRefresh({ source: 'providers' });
		} catch (err) {
			notify('error', $i18n('settings_model_delete_failed', { error: getErrorMessage(err) }));
		} finally {
			modelDeleting = false;
		}
	}

	/**
	 * Cancels model delete confirmation
	 */
	function cancelDeleteModel(): void {
		showModelDeleteConfirm = false;
		modelToDelete = null;
	}

	/**
	 * Handles provider models filter change
	 */
	function handleModelsProviderChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		selectedModelsProvider = event.currentTarget.value as ProviderType | 'all';
	}

	/**
	 * Gets filtered models for the selected provider (or all if 'all' selected).
	 * Uses memoized selector to prevent recalculation during scroll.
	 */
	const filteredModels = $derived(getFilteredModelsMemoized(llmState, selectedModelsProvider));

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

	onMount(() => {
		loadLLMData();
	});
</script>

<!-- Providers Section -->
<section id="providers" class="settings-section">
	<SettingsSectionHeader
		titleKey="settings_providers"
		descriptionKey="settings_providers_description"
		helpTitleKey="help_providers_title"
		helpDescriptionKey="help_providers_description"
		helpTutorialKey="help_providers_tutorial"
	>
		{#snippet actions()}
			<Button variant="primary" size="sm" onclick={() => modelModal.openCreate()}>
				<Plus size={16} />
				<span>{$i18n('models_add')}</span>
			</Button>
		{/snippet}
	</SettingsSectionHeader>

	{#if llmState.error}
		<ErrorBanner
			message={llmState.error}
			onDismiss={() => (llmState = setLLMError(llmState, null))}
		/>
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
					onConfigure={provInfo.requiresApiKey
						? () =>
								onConfigureApiKey(provInfo.id, providerHasApiKey(provInfo.id), provInfo.displayName)
						: undefined}
				>
					{#snippet icon()}
						{#if provInfo.id === 'ollama'}
							<Server size={20} aria-hidden="true" />
						{:else}
							<Globe size={20} aria-hidden="true" />
						{/if}
					{/snippet}
				</ProviderCard>
			{/each}
		</div>

		<!-- Custom Providers -->
		<h3 class="subsection-title">{$i18n('llm_custom_providers')}</h3>
		<div class="card custom-providers-card">
			{#each providerList.filter((p) => !p.isBuiltin) as provInfo (provInfo.id)}
				<div class="entity-row">
					<span class="entity-icon">
						<Globe size={20} aria-hidden="true" />
					</span>
					<div class="entity-main">
						<strong>{provInfo.displayName}</strong>
						<span>{customProviderMeta(provInfo)}</span>
					</div>
					<div class="entity-actions">
						<Button
							variant="ghost"
							size="icon"
							ariaLabel={$i18n('common_edit')}
							onclick={() => handleEditProviderRequest(provInfo)}
						>
							<Pencil size={14} aria-hidden="true" />
						</Button>
						<Button
							variant="ghost"
							size="icon"
							ariaLabel={$i18n('llm_connection_test')}
							disabled={testingProviderId !== null}
							onclick={() => handleTestProvider(provInfo)}
						>
							<Zap size={14} aria-hidden="true" />
						</Button>
						<Button
							variant="ghost"
							size="icon"
							ariaLabel={$i18n('common_delete')}
							onclick={() => handleDeleteProviderRequest(provInfo)}
						>
							<Trash2 size={14} aria-hidden="true" />
						</Button>
					</div>
				</div>
			{/each}
			<div class="card-footer custom-providers-footer">
				<Button variant="outline" size="sm" onclick={() => (showCustomProviderForm = true)}>
					<Plus size={16} />
					<span>{$i18n('llm_add_custom_provider')}</span>
				</Button>
			</div>
		</div>
	{/if}
</section>

<!-- Models Section -->
<section id="models" class="settings-section">
	<div class="models-header">
		<div class="models-title-row">
			<h3 class="subsection-title">{$i18n('settings_models')}</h3>
			<HelpButton
				titleKey="help_models_title"
				descriptionKey="help_models_description"
				tutorialKey="help_models_tutorial"
			/>
		</div>
		<Select
			options={modelsProviderOptions}
			value={selectedModelsProvider}
			onchange={handleModelsProviderChange}
			ariaLabel={$i18n('models_filter_by_provider')}
		/>
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
					providerLabel={providerLabelOf(model.provider)}
					onEdit={() => modelModal.openEdit(model)}
					onDelete={() => handleDeleteModelRequest(model)}
				/>
			{/each}
		</div>
	{/if}
</section>

<!-- Model Modal (Create/Edit) -->
<Modal
	open={modelModal.show}
	title={modelModal.mode === 'create' ? $i18n('modal_add_custom_model') : $i18n('modal_edit_model')}
	wide
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
	title={providerToEdit ? $i18n('llm_edit_custom_provider') : $i18n('llm_add_custom_provider')}
	onclose={closeCustomProviderForm}
>
	{#snippet body()}
		<CustomProviderForm
			provider={providerToEdit ?? undefined}
			oncreated={handleCustomProviderCreated}
			onupdated={handleCustomProviderUpdated}
			oncancel={closeCustomProviderForm}
		/>
	{/snippet}
</Modal>

<!-- Provider Delete Confirmation Modal -->
<DeleteConfirmModal
	open={showProviderDeleteConfirm}
	titleKey="llm_provider_delete_title"
	confirmMessageKey="llm_provider_delete_confirm_msg"
	deleting={providerDeleting}
	itemName={providerToDelete?.displayName}
	warningMessageKey="llm_provider_delete_warning"
	onConfirm={confirmDeleteProvider}
	onCancel={cancelDeleteProvider}
/>

<!-- Model Delete Confirmation Modal -->
<DeleteConfirmModal
	open={showModelDeleteConfirm}
	titleKey="llm_model_delete_title"
	confirmMessageKey="llm_model_delete_confirm_msg"
	deleting={modelDeleting}
	itemName={modelToDelete?.name}
	onConfirm={confirmDeleteModel}
	onCancel={cancelDeleteModel}
/>

<style>
	/* Provider Cards */
	.provider-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: var(--spacing-lg);
		margin-bottom: var(--spacing-lg);
		contain: layout style; /* Isolate layout recalculations */
	}

	.subsection-title {
		margin: 0 0 var(--spacing-sm);
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	/* Custom provider entity rows */
	.custom-providers-card {
		margin-bottom: var(--spacing-lg);
	}

	.entity-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light);
	}

	.entity-icon {
		display: flex;
		align-items: center;
		color: var(--color-text-tertiary);
	}

	.entity-main {
		flex: 1;
		min-width: 0;
	}

	.entity-main strong {
		display: block;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.entity-main span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 1px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.entity-actions {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		opacity: 0.45;
		transition: opacity var(--transition-fast);
	}

	.entity-row:hover .entity-actions,
	.entity-row:focus-within .entity-actions {
		opacity: 1;
	}

	.custom-providers-footer {
		display: flex;
		justify-content: flex-start;
	}

	.custom-providers-footer:first-child {
		border-top: none;
	}

	.custom-providers-footer :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	/* LLM Section */
	.llm-loading {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	/* Models Section */
	.models-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: var(--spacing-md);
		margin-bottom: var(--spacing-sm);
	}

	.models-header .subsection-title {
		margin-bottom: 0;
	}

	.models-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.models-header :global(.form-group) {
		margin-bottom: 0;
	}

	.models-header :global(.form-select) {
		width: auto;
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-xs);
	}

	.models-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
		gap: var(--spacing-lg);
		contain: layout style; /* Isolate layout recalculations */
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
	}
</style>
