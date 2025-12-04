<!--
  ModelCard Component
  Displays an LLM model with its specifications and available actions.

  @example
  <ModelCard
    model={model}
    isDefault={model.id === defaultModelId}
    onEdit={() => openEditModal(model)}
    onDelete={() => handleDelete(model.id)}
    onSetDefault={() => setDefaultModel(model.id)}
  />
-->
<script lang="ts">
	import { Card, Badge, Button } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { LLMModel } from '$types/llm';

	/**
	 * ModelCard props
	 */
	interface Props {
		/** The LLM model to display */
		model: LLMModel;
		/** Whether this model is the default for its provider */
		isDefault: boolean;
		/** Callback when edit button is clicked (only for custom models) */
		onEdit?: () => void;
		/** Callback when delete button is clicked (only for custom models) */
		onDelete?: () => void;
		/** Callback when set default button is clicked */
		onSetDefault?: () => void;
	}

	let { model, isDefault, onEdit, onDelete, onSetDefault }: Props = $props();

	/**
	 * Formats a number with locale-specific thousand separators
	 */
	function formatNumber(value: number): string {
		return value.toLocaleString();
	}

	/**
	 * Formats context window for display (e.g., "128K" for 128000)
	 */
	function formatContextWindow(tokens: number): string {
		if (tokens >= 1_000_000) {
			return `${(tokens / 1_000_000).toFixed(1)}M`;
		}
		if (tokens >= 1_000) {
			return `${(tokens / 1_000).toFixed(0)}K`;
		}
		return formatNumber(tokens);
	}

	/**
	 * Formats provider name for display (capitalize first letter)
	 */
	function formatProvider(provider: string): string {
		return provider.charAt(0).toUpperCase() + provider.slice(1);
	}
</script>

<Card>
	{#snippet header()}
		<div class="model-header">
			<div class="model-info">
				<h4 class="model-name">{model.name}</h4>
				<code class="model-api-name">{model.api_name}</code>
			</div>
			<div class="model-badges">
				{#if model.is_builtin}
					<Badge variant="primary">{$i18n('llm_model_builtin')}</Badge>
				{/if}
				{#if isDefault}
					<Badge variant="warning">{$i18n('llm_model_default')}</Badge>
				{/if}
				<span class="provider-name">{formatProvider(model.provider)}</span>
			</div>
		</div>
	{/snippet}

	{#snippet body()}
		<div class="model-specs">
			<div class="spec-item">
				<span class="spec-label">{$i18n('llm_model_context_window')}</span>
				<span class="spec-value">{formatContextWindow(model.context_window)} {$i18n('llm_model_tokens')}</span>
			</div>
			<div class="spec-item">
				<span class="spec-label">{$i18n('llm_model_max_output')}</span>
				<span class="spec-value">{formatNumber(model.max_output_tokens)} {$i18n('llm_model_tokens')}</span>
			</div>
			<div class="spec-item">
				<span class="spec-label">{$i18n('llm_model_temperature')}</span>
				<span class="spec-value">{model.temperature_default.toFixed(1)}</span>
			</div>
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="model-actions">
			{#if !isDefault && onSetDefault}
				<Button variant="ghost" size="sm" onclick={onSetDefault}>
					{$i18n('llm_model_set_default')}
				</Button>
			{/if}
			{#if !model.is_builtin}
				{#if onEdit}
					<Button variant="ghost" size="sm" onclick={onEdit}>
						{$i18n('llm_model_edit')}
					</Button>
				{/if}
				{#if onDelete}
					<Button variant="danger" size="sm" onclick={onDelete}>
						{$i18n('llm_model_delete')}
					</Button>
				{/if}
			{/if}
		</div>
	{/snippet}
</Card>

<style>
	.model-header {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-md);
		width: 100%;
	}

	.model-info {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		min-width: 0;
	}

	.model-name {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.model-api-name {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
		background-color: var(--color-bg-secondary);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-radius: var(--radius-sm);
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.model-badges {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}

	.provider-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-secondary);
	}

	.model-specs {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-md);
	}

	.spec-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.spec-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.spec-value {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.model-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	/* Responsive: stack specs on small screens */
	@media (max-width: 480px) {
		.model-specs {
			grid-template-columns: 1fr 1fr;
		}
	}
</style>
