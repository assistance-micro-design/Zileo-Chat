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
-->

<script lang="ts">
	import { Card, Badge } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { ImportValidation, ImportSelection } from '$types/importExport';

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

	/**
	 * Toggle all entities of a specific type.
	 * Selection is now by NAME (not ID) since IDs are not exported.
	 */
	function toggleAll(type: 'agents' | 'mcpServers' | 'models' | 'prompts'): void {
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
	function toggleEntity(
		type: 'agents' | 'mcpServers' | 'models' | 'prompts',
		entityName: string
	): void {
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
	function allSelected(type: 'agents' | 'mcpServers' | 'models' | 'prompts'): boolean {
		const entities = validation.entities[type];
		const selectedNames = selection[type];
		return entities.length > 0 && entities.every((e) => selectedNames.includes(e.name));
	}

	/**
	 * Check if some entities of a type are selected (indeterminate state).
	 * Selection is by NAME.
	 */
	function someSelected(type: 'agents' | 'mcpServers' | 'models' | 'prompts'): boolean {
		const entities = validation.entities[type];
		const selectedNames = selection[type];
		return selectedNames.length > 0 && !entities.every((e) => selectedNames.includes(e.name));
	}
</script>

<div class="import-preview">
	<!-- Summary Cards -->
	<div class="summary-cards">
		<Card>
			{#snippet body()}
				<div class="summary-card">
					<div class="summary-header">
						<h4>{$i18n('ie_entity_agents')}</h4>
						<Badge variant="primary">{validation.entities.agents.length}</Badge>
					</div>
					<p class="summary-count">{$i18n('ie_x_selected').replace('{count}', String(selection.agents.length))}</p>
				</div>
			{/snippet}
		</Card>

		<Card>
			{#snippet body()}
				<div class="summary-card">
					<div class="summary-header">
						<h4>{$i18n('ie_entity_mcp_servers')}</h4>
						<Badge variant="primary">{validation.entities.mcpServers.length}</Badge>
					</div>
					<p class="summary-count">{$i18n('ie_x_selected').replace('{count}', String(selection.mcpServers.length))}</p>
				</div>
			{/snippet}
		</Card>

		<Card>
			{#snippet body()}
				<div class="summary-card">
					<div class="summary-header">
						<h4>{$i18n('ie_entity_models')}</h4>
						<Badge variant="primary">{validation.entities.models.length}</Badge>
					</div>
					<p class="summary-count">{$i18n('ie_x_selected').replace('{count}', String(selection.models.length))}</p>
				</div>
			{/snippet}
		</Card>

		<Card>
			{#snippet body()}
				<div class="summary-card">
					<div class="summary-header">
						<h4>{$i18n('ie_entity_prompts')}</h4>
						<Badge variant="primary">{validation.entities.prompts.length}</Badge>
					</div>
					<p class="summary-count">{$i18n('ie_x_selected').replace('{count}', String(selection.prompts.length))}</p>
				</div>
			{/snippet}
		</Card>
	</div>

	<!-- Entity Lists -->
	<div class="entity-lists">
		<!-- Agents -->
		{#if validation.entities.agents.length > 0}
			<Card title={$i18n('ie_entity_agents')}>
				{#snippet body()}
					<div class="entity-list">
						<label class="entity-item header-item">
							<input
								type="checkbox"
								checked={allSelected('agents')}
								indeterminate={someSelected('agents')}
								onchange={() => toggleAll('agents')}
							/>
							<span class="entity-name">{$i18n('ie_select_all')}</span>
						</label>
						{#each validation.entities.agents as agent (agent.name)}
							<label class="entity-item">
								<input
									type="checkbox"
									checked={selection.agents.includes(agent.name)}
									onchange={() => toggleEntity('agents', agent.name)}
								/>
								<span class="entity-name">{agent.name}</span>
								{#if hasConflict(agent.name)}
									<Badge variant="warning">{getConflictBadge(agent.name)}</Badge>
								{/if}
							</label>
						{/each}
					</div>
				{/snippet}
			</Card>
		{/if}

		<!-- MCP Servers -->
		{#if validation.entities.mcpServers.length > 0}
			<Card title={$i18n('ie_entity_mcp_servers')}>
				{#snippet body()}
					<div class="entity-list">
						<label class="entity-item header-item">
							<input
								type="checkbox"
								checked={allSelected('mcpServers')}
								indeterminate={someSelected('mcpServers')}
								onchange={() => toggleAll('mcpServers')}
							/>
							<span class="entity-name">{$i18n('ie_select_all')}</span>
						</label>
						{#each validation.entities.mcpServers as server (server.name)}
							<label class="entity-item">
								<input
									type="checkbox"
									checked={selection.mcpServers.includes(server.name)}
									onchange={() => toggleEntity('mcpServers', server.name)}
								/>
								<span class="entity-name">{server.name}</span>
								{#if hasConflict(server.name)}
									<Badge variant="warning">{getConflictBadge(server.name)}</Badge>
								{/if}
								{#if hasMissingEnv(server.name)}
									<Badge variant="error">{$i18n('ie_missing_env_vars')}</Badge>
								{/if}
							</label>
						{/each}
					</div>
				{/snippet}
			</Card>
		{/if}

		<!-- Models -->
		{#if validation.entities.models.length > 0}
			<Card title={$i18n('ie_entity_models')}>
				{#snippet body()}
					<div class="entity-list">
						<label class="entity-item header-item">
							<input
								type="checkbox"
								checked={allSelected('models')}
								indeterminate={someSelected('models')}
								onchange={() => toggleAll('models')}
							/>
							<span class="entity-name">{$i18n('ie_select_all')}</span>
						</label>
						{#each validation.entities.models as model (model.name)}
							<label class="entity-item">
								<input
									type="checkbox"
									checked={selection.models.includes(model.name)}
									onchange={() => toggleEntity('models', model.name)}
								/>
								<span class="entity-name">{model.name}</span>
								<span class="entity-meta">{model.provider} - {model.apiName}</span>
								{#if hasConflict(model.name)}
									<Badge variant="warning">{getConflictBadge(model.name)}</Badge>
								{/if}
							</label>
						{/each}
					</div>
				{/snippet}
			</Card>
		{/if}

		<!-- Prompts -->
		{#if validation.entities.prompts.length > 0}
			<Card title={$i18n('ie_entity_prompts')}>
				{#snippet body()}
					<div class="entity-list">
						<label class="entity-item header-item">
							<input
								type="checkbox"
								checked={allSelected('prompts')}
								indeterminate={someSelected('prompts')}
								onchange={() => toggleAll('prompts')}
							/>
							<span class="entity-name">{$i18n('ie_select_all')}</span>
						</label>
						{#each validation.entities.prompts as prompt (prompt.name)}
							<label class="entity-item">
								<input
									type="checkbox"
									checked={selection.prompts.includes(prompt.name)}
									onchange={() => toggleEntity('prompts', prompt.name)}
								/>
								<span class="entity-name">{prompt.name}</span>
								<span class="entity-meta">{prompt.category}</span>
								{#if hasConflict(prompt.name)}
									<Badge variant="warning">{getConflictBadge(prompt.name)}</Badge>
								{/if}
							</label>
						{/each}
					</div>
				{/snippet}
			</Card>
		{/if}
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
