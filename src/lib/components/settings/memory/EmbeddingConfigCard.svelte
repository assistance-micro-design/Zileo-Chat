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
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

EmbeddingConfigCard - Displays current embedding configuration with edit/delete actions.
Extracted from MemorySettings.svelte.
-->

<script lang="ts">
	import { Card, Button } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type { EmbeddingConfig } from '$types/embedding';
	import { Settings, Pencil, Trash2, Plus } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Current embedding configuration */
		config: EmbeddingConfig;
		/** Whether a config has been saved */
		configExists: boolean;
		/** Provider options for label lookup */
		providerOptions: SelectOption[];
		/** Strategy options for label lookup */
		strategyOptions: SelectOption[];
		/** Callback to open the config edit modal */
		onOpenConfigModal: () => void;
		/** Callback to delete the config */
		onDelete: () => void;
	}

	let {
		config,
		configExists,
		providerOptions,
		strategyOptions,
		onOpenConfigModal,
		onDelete
	}: Props = $props();

	/**
	 * Get provider display name
	 */
	function getProviderLabel(provider: string): string {
		return providerOptions.find((p) => p.value === provider)?.label || provider;
	}

	/**
	 * Get strategy display name
	 */
	function getStrategyLabel(strategy: string): string {
		return strategyOptions.find((s) => s.value === strategy)?.label || strategy;
	}
</script>

<Card>
	{#snippet header()}
		<div class="card-header-row">
			<h3 class="card-title">{$i18n('memory_embedding_config')}</h3>
			{#if configExists}
				<div class="header-actions">
					<button
						type="button"
						class="icon-btn"
						onclick={onOpenConfigModal}
						title={$i18n('common_edit')}
						aria-label={$i18n('common_edit')}
					>
						<Pencil size={16} />
					</button>
					<button
						type="button"
						class="icon-btn danger"
						onclick={onDelete}
						title={$i18n('common_delete')}
						aria-label={$i18n('common_delete')}
					>
						<Trash2 size={16} />
					</button>
				</div>
			{/if}
		</div>
	{/snippet}
	{#snippet body()}
		{#if configExists}
			<div class="config-display">
				<div class="config-grid">
					<div class="config-item">
						<span class="config-label">{$i18n('memory_provider')}</span>
						<span class="config-value">{getProviderLabel(config.provider)}</span>
					</div>
					<div class="config-item">
						<span class="config-label">{$i18n('memory_model')}</span>
						<span class="config-value">{config.model}</span>
					</div>
					<div class="config-item">
						<span class="config-label">{$i18n('memory_dimensions')}</span>
						<span class="config-value">{config.dimension}D</span>
					</div>
					<div class="config-item">
						<span class="config-label">{$i18n('memory_strategy')}</span>
						<span class="config-value">{getStrategyLabel(config.strategy || 'fixed')}</span>
					</div>
					<div class="config-item">
						<span class="config-label">{$i18n('memory_chunk_size')}</span>
						<span class="config-value">{config.chunk_size} {$i18n('memory_chars')}</span>
					</div>
					<div class="config-item">
						<span class="config-label">{$i18n('memory_overlap')}</span>
						<span class="config-value">{config.chunk_overlap} {$i18n('memory_chars')}</span>
					</div>
				</div>
			</div>
		{:else}
			<div class="empty-state">
				<Settings size={48} strokeWidth={1} />
				<h4>{$i18n('memory_no_config')}</h4>
				<p>{$i18n('memory_no_config_description')}</p>
				<Button variant="primary" onclick={onOpenConfigModal}>
					<Plus size={16} />
					{$i18n('memory_add_config')}
				</Button>
			</div>
		{/if}
	{/snippet}
</Card>

<style>
	.card-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.card-header-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
	}

	.header-actions {
		display: flex;
		gap: var(--spacing-xs);
	}

	.icon-btn {
		display: flex;
		align-items: center;
		justify-content: center;
		padding: var(--spacing-xs);
		background: transparent;
		border: none;
		border-radius: var(--border-radius-sm);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			color 0.2s,
			background 0.2s;
	}

	.icon-btn:hover {
		color: var(--color-text-primary);
		background: var(--color-bg-hover);
	}

	.icon-btn.danger:hover {
		color: var(--color-error);
	}

	.config-display {
		padding: var(--spacing-sm);
	}

	.config-grid {
		display: grid;
		grid-template-columns: repeat(3, 1fr);
		gap: var(--spacing-md);
	}

	.config-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-2xs);
	}

	.config-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.config-value {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-2xl);
		text-align: center;
		color: var(--color-text-secondary);
	}

	.empty-state h4 {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.empty-state p {
		font-size: var(--font-size-sm);
		margin: 0;
		max-width: 300px;
	}

	.empty-state :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	@media (max-width: 768px) {
		.config-grid {
			grid-template-columns: repeat(2, 1fr);
		}
	}

	@media (max-width: 480px) {
		.config-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
