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
  ModelCard Component
  Displays an LLM model: API name, provider, condensed specs and capability
  badges (vision / reasoning). Builtin models can be edited (the form locks
  the read-only fields) but not deleted.

  @example
  <ModelCard
    model={model}
    providerLabel="Mistral"
    onEdit={() => openEditModal(model)}
    onDelete={() => handleDelete(model.id)}
  />
-->
<script lang="ts">
	import { Card, Badge, Button } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { Eye, Brain, Pencil, Trash2 } from '@lucide/svelte';
	import type { LLMModel } from '$types/llm';

	/**
	 * ModelCard props
	 */
	interface Props {
		/** The LLM model to display */
		model: LLMModel;
		/** Display name of the model's provider (falls back to the raw id) */
		providerLabel?: string;
		/** Callback when edit button is clicked */
		onEdit?: () => void;
		/** Callback when delete button is clicked (custom models only) */
		onDelete?: () => void;
	}

	let { model, providerLabel, onEdit, onDelete }: Props = $props();

	/**
	 * Formats a number with locale-specific thousand separators
	 */
	function formatNumber(value: number): string {
		return value.toLocaleString();
	}
</script>

<Card hover>
	{#snippet header()}
		<span class="model-api-name">{model.api_name}</span>
		{#if model.is_builtin}
			<Badge variant="neutral">{$i18n('llm_model_builtin')}</Badge>
		{/if}
	{/snippet}

	{#snippet body()}
		<div class="model-specs">
			<span>
				{providerLabel ?? model.provider} · {$i18n('llm_model_context_window')} : {formatNumber(
					model.context_window
				)}
				{$i18n('llm_model_tokens')}
			</span>
			<span>
				{$i18n('llm_model_max_output')} : {formatNumber(model.max_output_tokens)} · {$i18n(
					'llm_form_temperature_label'
				)} : {model.temperature_default.toFixed(1)}
			</span>
			{#if model.supports_vision || model.is_reasoning}
				<span class="model-capabilities">
					{#if model.supports_vision}
						<Badge variant="primary">
							<Eye size={12} aria-hidden="true" />
							{$i18n('models_supports_vision')}
						</Badge>
					{/if}
					{#if model.is_reasoning}
						<span class="badge badge-reasoning">
							<Brain size={12} aria-hidden="true" />
							{$i18n('llm_model_reasoning_badge')}
						</span>
					{/if}
				</span>
			{/if}
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="model-actions">
			{#if onEdit}
				<Button variant="ghost" size="sm" onclick={onEdit}>
					<Pencil size={14} aria-hidden="true" />
					<span>{$i18n('llm_model_edit')}</span>
				</Button>
			{/if}
			{#if !model.is_builtin && onDelete}
				<Button
					variant="ghost"
					size="icon"
					ariaLabel={$i18n('llm_model_delete')}
					onclick={onDelete}
				>
					<Trash2 size={14} aria-hidden="true" />
				</Button>
			{/if}
		</div>
	{/snippet}
</Card>

<style>
	.model-api-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
		min-width: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.model-specs {
		display: flex;
		flex-direction: column;
		gap: 4px;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.model-capabilities {
		display: flex;
		flex-wrap: wrap;
		gap: var(--spacing-xs);
		margin-top: var(--spacing-xs);
	}

	.badge-reasoning {
		background: var(--channel-thinking-soft);
		color: var(--channel-thinking);
	}

	.model-actions {
		display: flex;
		justify-content: flex-end;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.model-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}
</style>
