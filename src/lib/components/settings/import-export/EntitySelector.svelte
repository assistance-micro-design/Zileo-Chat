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

EntitySelector - Reusable multi-select component for entities.
Displays a list of checkboxes with select all/deselect all functionality.
-->

<script lang="ts">
	import { i18n } from '$lib/i18n';

	/** Props */
	interface Props {
		/** Type of entity being selected */
		entityType: 'agent' | 'mcp' | 'model' | 'prompt';
		/** Array of items to select from */
		items: Array<{ id: string; name: string }>;
		/** Array of selected item IDs */
		selected: string[];
		/** Callback when selection changes */
		onchange: (selected: string[]) => void;
		/** Whether the selector is disabled */
		disabled?: boolean;
	}

	let { entityType, items, selected, onchange, disabled = false }: Props = $props();

	/** Entity type labels - mapped to i18n keys */
	const typeLabelsKeys: Record<string, string> = {
		agent: 'ie_entity_agents',
		mcp: 'ie_entity_mcp_servers',
		model: 'ie_entity_models',
		prompt: 'ie_entity_prompts'
	};

	/** Get translated label for entity type */
	function getTypeLabel(type: string): string {
		return $i18n(typeLabelsKeys[type] || type);
	}

	/** Check if all items are selected */
	const allSelected = $derived(items.length > 0 && selected.length === items.length);

	/**
	 * Toggles an item selection
	 */
	function toggleItem(itemId: string): void {
		const newSelected = selected.includes(itemId)
			? selected.filter((id) => id !== itemId)
			: [...selected, itemId];
		onchange(newSelected);
	}

	/**
	 * Selects all items
	 */
	function selectAll(): void {
		onchange(items.map((item) => item.id));
	}

	/**
	 * Deselects all items
	 */
	function deselectAll(): void {
		onchange([]);
	}
</script>

<div class="entity-selector">
	<div class="header">
		<h4 class="title">{getTypeLabel(entityType)}</h4>
		<div class="actions">
			<button
				type="button"
				class="action-link"
				onclick={selectAll}
				disabled={disabled || allSelected}
			>
				{$i18n('ie_select_all')}
			</button>
			<span class="separator">|</span>
			<button
				type="button"
				class="action-link"
				onclick={deselectAll}
				disabled={disabled || selected.length === 0}
			>
				{$i18n('ie_deselect_all')}
			</button>
		</div>
	</div>

	{#if items.length === 0}
		<div class="empty-state">
			<p>{$i18n('ie_no_available').replace('{type}', getTypeLabel(entityType).toLowerCase())}</p>
		</div>
	{:else}
		<div class="items-list">
			{#each items as item (item.id)}
				<label class="item-checkbox">
					<input
						type="checkbox"
						checked={selected.includes(item.id)}
						onchange={() => toggleItem(item.id)}
						{disabled}
					/>
					<span class="item-name">{item.name}</span>
				</label>
			{/each}
		</div>

		<div class="footer">
			<span class="count">
				{$i18n('ie_x_of_y_selected').replace('{selected}', String(selected.length)).replace('{total}', String(items.length))}
			</span>
		</div>
	{/if}
</div>

<style>
	.entity-selector {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		padding: var(--spacing-md);
		background: var(--color-bg-primary);
	}

	.header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
	}

	.title {
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.actions {
		display: flex;
		gap: var(--spacing-sm);
		align-items: center;
	}

	.action-link {
		background: none;
		border: none;
		color: var(--color-primary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: 0;
		text-decoration: none;
		transition: opacity 0.2s;
	}

	.action-link:hover:not(:disabled) {
		opacity: 0.8;
		text-decoration: underline;
	}

	.action-link:disabled {
		opacity: 0.4;
		cursor: not-allowed;
	}

	.separator {
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
	}

	.empty-state {
		padding: var(--spacing-lg);
		text-align: center;
	}

	.empty-state p {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
	}

	.items-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
		max-height: 300px;
		overflow-y: auto;
		padding: var(--spacing-xs);
	}

	.item-checkbox {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
		transition: background 0.2s;
		user-select: none;
	}

	.item-checkbox:hover {
		background: var(--color-bg-hover);
	}

	.item-checkbox input[type='checkbox'] {
		cursor: pointer;
		width: 16px;
		height: 16px;
		margin: 0;
	}

	.item-checkbox input[type='checkbox']:disabled {
		cursor: not-allowed;
	}

	.item-name {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.footer {
		display: flex;
		justify-content: flex-end;
		padding-top: var(--spacing-sm);
		border-top: 1px solid var(--color-border);
	}

	.count {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-weight: var(--font-weight-medium);
	}

	/* Scrollbar styling */
	.items-list::-webkit-scrollbar {
		width: 8px;
	}

	.items-list::-webkit-scrollbar-track {
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-sm);
	}

	.items-list::-webkit-scrollbar-thumb {
		background: var(--color-border);
		border-radius: var(--border-radius-sm);
	}

	.items-list::-webkit-scrollbar-thumb:hover {
		background: var(--color-text-tertiary);
	}
</style>
