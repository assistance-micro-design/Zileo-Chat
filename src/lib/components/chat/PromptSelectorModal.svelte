<!--
  PromptSelectorModal Component
  Modal for browsing and selecting prompts from the prompt library.
  Supports search, category filtering, variable filling, and preview.

  @example
  <PromptSelectorModal
    open={showModal}
    onclose={() => showModal = false}
    onselect={(content) => handleInsert(content)}
  />
-->
<script lang="ts">
	import { onMount } from 'svelte';
	import Modal from '$lib/components/ui/Modal.svelte';
	import Button from '$lib/components/ui/Button.svelte';
	import Input from '$lib/components/ui/Input.svelte';
	import Select from '$lib/components/ui/Select.svelte';
	import Badge from '$lib/components/ui/Badge.svelte';
	import Spinner from '$lib/components/ui/Spinner.svelte';
	import {
		promptStore,
		prompts,
		promptLoading,
		extractVariables,
		interpolateVariables
	} from '$lib/stores/prompts';
	import { PROMPT_CATEGORY_LABELS } from '$types/prompt';
	import type { Prompt, PromptSummary, PromptCategory } from '$types/prompt';

	/**
	 * PromptSelectorModal props
	 */
	interface Props {
		/** Whether modal is open */
		open: boolean;
		/** Close handler */
		onclose?: () => void;
		/** Select handler (receives interpolated content) */
		onselect?: (content: string) => void;
	}

	let { open, onclose, onselect }: Props = $props();

	// State
	let searchQuery = $state('');
	let categoryFilter = $state<PromptCategory | ''>('');
	let selectedPrompt = $state<Prompt | null>(null);
	let variableValues = $state<Record<string, string>>({});
	let loadingPrompt = $state(false);

	// Category options for Select component
	const categoryOptions = [
		{ value: '', label: 'All Categories' },
		...Object.entries(PROMPT_CATEGORY_LABELS).map(([value, label]) => ({
			value: value as PromptCategory,
			label
		}))
	];

	// Filtered prompts based on search query and category filter
	let filteredPrompts = $derived.by(() => {
		let result = $prompts;

		if (searchQuery.trim()) {
			const query = searchQuery.toLowerCase();
			result = result.filter(
				(p) =>
					p.name.toLowerCase().includes(query) || p.description.toLowerCase().includes(query)
			);
		}

		if (categoryFilter) {
			result = result.filter((p) => p.category === categoryFilter);
		}

		return result;
	});

	// Detected variables from selected prompt
	let variables = $derived(selectedPrompt ? extractVariables(selectedPrompt.content) : []);

	// Interpolated preview
	let preview = $derived(
		selectedPrompt ? interpolateVariables(selectedPrompt.content, variableValues) : ''
	);

	// Check if all variables have values
	let allVariablesFilled = $derived(
		variables.length === 0 || variables.every((v: string) => variableValues[v]?.trim())
	);

	/**
	 * Load prompts on mount if not already loaded
	 */
	onMount(() => {
		if ($prompts.length === 0) {
			promptStore.loadPrompts();
		}
	});

	/**
	 * Select a prompt from the list (load full data)
	 */
	async function selectPromptFromList(summary: PromptSummary): Promise<void> {
		loadingPrompt = true;
		try {
			const prompt = await promptStore.getPrompt(summary.id);
			selectedPrompt = prompt;
			// Initialize variable values with defaults or empty
			variableValues = {};
			for (const v of prompt.variables) {
				variableValues[v.name] = v.defaultValue ?? '';
			}
		} catch (e) {
			console.error('Failed to load prompt:', e);
		} finally {
			loadingPrompt = false;
		}
	}

	/**
	 * Go back to prompt list
	 */
	function goBack(): void {
		selectedPrompt = null;
		variableValues = {};
	}

	/**
	 * Use the selected prompt (interpolate and send to parent)
	 */
	function handleUsePrompt(): void {
		if (!selectedPrompt) return;
		const finalContent = interpolateVariables(selectedPrompt.content, variableValues);
		onselect?.(finalContent);
		handleClose();
	}

	/**
	 * Close modal and reset state
	 */
	function handleClose(): void {
		selectedPrompt = null;
		variableValues = {};
		searchQuery = '';
		categoryFilter = '';
		onclose?.();
	}
</script>

<Modal {open} onclose={handleClose} title={selectedPrompt ? 'Fill Variables' : 'Select Prompt'}>
	{#snippet body()}
		{#if selectedPrompt}
			<!-- Variable Input View -->
			<div class="variable-view">
				<button type="button" class="back-button" onclick={goBack}> &larr; Back to list </button>

				<div class="prompt-info">
					<h4>{selectedPrompt.name}</h4>
					<Badge variant="primary">{PROMPT_CATEGORY_LABELS[selectedPrompt.category]}</Badge>
				</div>

				{#if variables.length > 0}
					<div class="variables-form">
						{#each variables as variable}
							<div class="variable-field">
								<Input
									label={variable}
									bind:value={variableValues[variable]}
									placeholder={`Enter value for ${variable}`}
								/>
							</div>
						{/each}
					</div>
				{:else}
					<p class="no-variables">This prompt has no variables.</p>
				{/if}

				<div class="preview-section">
					<h5>Preview</h5>
					<div class="preview-content">{preview}</div>
				</div>
			</div>
		{:else}
			<!-- Prompt List View -->
			<div class="list-view">
				<div class="filters">
					<Input placeholder="Search prompts..." bind:value={searchQuery} />
					<Select
						value={categoryFilter}
						options={categoryOptions}
						onchange={(e) => (categoryFilter = e.currentTarget.value as PromptCategory | '')}
					/>
				</div>

				{#if $promptLoading || loadingPrompt}
					<div class="loading-state">
						<Spinner />
						<span>Loading...</span>
					</div>
				{:else if filteredPrompts.length === 0}
					<div class="empty-state">
						{#if $prompts.length === 0}
							<p>No prompts available.</p>
							<p class="hint">Create prompts in Settings to use them here.</p>
						{:else}
							<p>No matching prompts found.</p>
						{/if}
					</div>
				{:else}
					<div class="prompt-list">
						{#each filteredPrompts as prompt (prompt.id)}
							<button
								type="button"
								class="prompt-item"
								onclick={() => selectPromptFromList(prompt)}
							>
								<div class="prompt-header">
									<span class="prompt-name">{prompt.name}</span>
									<Badge variant="primary">
										{PROMPT_CATEGORY_LABELS[prompt.category]}
									</Badge>
								</div>
								<p class="prompt-description">{prompt.description || 'No description'}</p>
								<span class="prompt-vars">
									{prompt.variables_count} variable{prompt.variables_count !== 1 ? 's' : ''}
								</span>
							</button>
						{/each}
					</div>
				{/if}
			</div>
		{/if}
	{/snippet}

	{#snippet footer()}
		<div class="modal-footer">
			<Button variant="ghost" onclick={handleClose}>Cancel</Button>
			{#if selectedPrompt}
				<Button variant="primary" onclick={handleUsePrompt} disabled={!allVariablesFilled}>
					Use Prompt
				</Button>
			{/if}
		</div>
	{/snippet}
</Modal>

<style>
	.variable-view {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.back-button {
		align-self: flex-start;
		background: none;
		border: none;
		color: var(--color-accent);
		cursor: pointer;
		font-size: var(--font-size-sm);
		padding: 0;
	}

	.back-button:hover {
		text-decoration: underline;
	}

	.prompt-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.prompt-info h4 {
		margin: 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.variables-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.variable-field {
		display: flex;
		flex-direction: column;
	}

	.no-variables {
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
	}

	.preview-section {
		border-top: 1px solid var(--color-border);
		padding-top: var(--spacing-md);
	}

	.preview-section h5 {
		margin: 0 0 var(--spacing-sm) 0;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.preview-content {
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		white-space: pre-wrap;
		max-height: 200px;
		overflow-y: auto;
	}

	.list-view {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.filters {
		display: flex;
		gap: var(--spacing-md);
	}

	.filters :global(> *) {
		flex: 1;
	}

	.loading-state,
	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-xl);
		text-align: center;
		color: var(--color-text-secondary);
	}

	.hint {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.prompt-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		max-height: 400px;
		overflow-y: auto;
	}

	.prompt-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		cursor: pointer;
		text-align: left;
		transition:
			border-color var(--transition-fast),
			background var(--transition-fast);
	}

	.prompt-item:hover {
		border-color: var(--color-accent);
		background: var(--color-bg-secondary);
	}

	.prompt-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.prompt-name {
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.prompt-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		display: -webkit-box;
		-webkit-line-clamp: 2;
		line-clamp: 2;
		-webkit-box-orient: vertical;
		overflow: hidden;
	}

	.prompt-vars {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.modal-footer {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
	}
</style>
