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

ImportPreview - Preview what will be imported with selection controls.
Displays entity summaries with checkboxes for selection/deselection.
Shows warnings for conflicts and missing MCP env vars.
Uses data-driven loops for summary cards and entity lists.
-->

<script lang="ts">
	import { Card, Badge } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { ImportValidation, ImportSelection } from '$types/import-export';

	/** Props */
	interface Props {
		/** Validation result from backend */
		validation: ImportValidation;
		/** Current selection state */
		selection: ImportSelection;
		/** Selection change callback */
		onSelectionChange: (selection: ImportSelection) => void;
	}

	let { validation, selection, onSelectionChange }: Props = $props();

	/** Entity type definitions for data-driven rendering */
	type EntityType = 'agents' | 'mcpServers' | 'models' | 'prompts' | 'skills' | 'customProviders';

	const entityDefs: Array<{ type: EntityType; titleKey: string }> = [
		{ type: 'agents', titleKey: 'ie_entity_agents' },
		{ type: 'mcpServers', titleKey: 'ie_entity_mcp_servers' },
		{ type: 'models', titleKey: 'ie_entity_models' },
		{ type: 'prompts', titleKey: 'ie_entity_prompts' },
		{ type: 'skills', titleKey: 'ie_entity_skills' },
		{ type: 'customProviders', titleKey: 'ie_entity_custom_providers' }
	];

	/**
	 * Toggle all entities of a specific type.
	 * Selection is now by NAME (not ID) since IDs are not exported.
	 */
	function toggleAll(type: EntityType): void {
		const entities = validation.entities[type];
		const currentNames = selection[type];
		const allNames = entities.map((e) => e.name);

		// If all are selected, deselect all; otherwise, select all
		const allSelectedFlag = allNames.every((name) => currentNames.includes(name));
		const newNames = allSelectedFlag ? [] : allNames;

		onSelectionChange({
			...selection,
			[type]: newNames
		});
	}

	/**
	 * Toggle a single entity.
	 * Selection is now by NAME (not ID).
	 */
	function toggleEntity(type: EntityType, entityName: string): void {
		const currentNames = selection[type];
		const newNames = currentNames.includes(entityName)
			? currentNames.filter((name) => name !== entityName)
			: [...currentNames, entityName];

		onSelectionChange({
			...selection,
			[type]: newNames
		});
	}

	/**
	 * Check if entity has conflicts.
	 * Conflicts are now detected by NAME only.
	 */
	function hasConflict(entityName: string): boolean {
		return validation.conflicts.some((c) => c.entityName === entityName);
	}

	/**
	 * Check if MCP server has missing env vars.
	 * MissingMcpEnv is now keyed by server NAME.
	 */
	function hasMissingEnv(serverName: string): boolean {
		return validation.missingMcpEnv[serverName]?.length > 0;
	}

	/**
	 * Get conflict badge - always "Name Conflict" now since ID conflicts are not possible.
	 */
	function getConflictBadge(entityName: string): string {
		const conflict = validation.conflicts.find((c) => c.entityName === entityName);
		if (!conflict) return '';
		return $i18n('ie_name_conflict');
	}

	/**
	 * Check if all entities of a type are selected.
	 * Selection is by NAME.
	 */
	function allSelected(type: EntityType): boolean {
		const entities = validation.entities[type];
		const selectedNames = selection[type];
		return entities.length > 0 && entities.every((e) => selectedNames.includes(e.name));
	}

	/**
	 * Check if some entities of a type are selected (indeterminate state).
	 * Selection is by NAME.
	 */
	function someSelected(type: EntityType): boolean {
		const entities = validation.entities[type];
		const selectedNames = selection[type];
		return selectedNames.length > 0 && !entities.every((e) => selectedNames.includes(e.name));
	}

	/**
	 * Get type-specific metadata for an entity item.
	 * Returns null if no extra metadata for this entity type.
	 */
	function getEntityMeta(type: EntityType, item: { name: string }): string | null {
		if (type === 'models' && 'provider' in item && 'apiName' in item) {
			return `${String(item.provider)} - ${String(item.apiName)}`;
		}
		if (type === 'prompts' && 'category' in item) {
			return String(item.category);
		}
		if (type === 'skills' && 'category' in item) {
			return String(item.category);
		}
		if (type === 'customProviders' && 'displayName' in item) {
			return String(item.displayName);
		}
		return null;
	}
</script>

<div class="import-preview">
	<!-- Summary Cards -->
	<div class="summary-cards">
		{#each entityDefs as def (def.type)}
			<Card>
				{#snippet body()}
					<div class="summary-card">
						<div class="summary-header">
							<h4>{$i18n(def.titleKey)}</h4>
							<Badge variant="primary">{validation.entities[def.type].length}</Badge>
						</div>
						<p class="summary-count">{$i18n('ie_x_selected').replace('{count}', String(selection[def.type].length))}</p>
					</div>
				{/snippet}
			</Card>
		{/each}
	</div>

	<!-- Entity Lists -->
	<div class="entity-lists">
		{#each entityDefs as def (def.type)}
			{#if validation.entities[def.type].length > 0}
				<Card title={$i18n(def.titleKey)}>
					{#snippet body()}
						<div class="entity-list">
							<label class="entity-item header-item">
								<input
									type="checkbox"
									checked={allSelected(def.type)}
									indeterminate={someSelected(def.type)}
									onchange={() => toggleAll(def.type)}
								/>
								<span class="entity-name">{$i18n('ie_select_all')}</span>
							</label>
							{#each validation.entities[def.type] as item (item.name)}
								<label class="entity-item">
									<input
										type="checkbox"
										checked={selection[def.type].includes(item.name)}
										onchange={() => toggleEntity(def.type, item.name)}
									/>
									<span class="entity-name">{item.name}</span>
									{#if getEntityMeta(def.type, item)}
										<span class="entity-meta">{getEntityMeta(def.type, item)}</span>
									{/if}
									{#if hasConflict(item.name)}
										<Badge variant="warning">{getConflictBadge(item.name)}</Badge>
									{/if}
									{#if def.type === 'mcpServers' && hasMissingEnv(item.name)}
										<Badge variant="error">{$i18n('ie_missing_env_vars')}</Badge>
									{/if}
								</label>
							{/each}
						</div>
					{/snippet}
				</Card>
			{/if}
		{/each}
	</div>
</div>

<style>
	.import-preview {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.summary-cards {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
		gap: var(--spacing-md);
	}

	.summary-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.summary-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
	}

	.summary-header h4 {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
	}

	.summary-count {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.entity-lists {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.entity-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.entity-item {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
		transition: background-color 0.2s;
	}

	.entity-item:not(.header-item):hover {
		background: var(--color-bg-hover);
	}

	.header-item {
		font-weight: var(--font-weight-semibold);
		border-bottom: 1px solid var(--color-border);
		padding-bottom: var(--spacing-md);
		margin-bottom: var(--spacing-xs);
	}

	.entity-item input[type='checkbox'] {
		cursor: pointer;
	}

	.entity-name {
		flex: 1;
		font-size: var(--font-size-sm);
	}

	.entity-meta {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}


	@media (max-width: 768px) {
		.summary-cards {
			grid-template-columns: 1fr 1fr;
		}
	}

	@media (max-width: 480px) {
		.summary-cards {
			grid-template-columns: 1fr;
		}
	}
</style>
