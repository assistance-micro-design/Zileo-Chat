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

ConflictResolver - Resolve import conflicts with existing entities.
Each conflict is a compact entity row (type icon, name, "already exists" note)
with a resolution select (Skip / Overwrite / Rename). Group buttons apply one
resolution to every conflict at once. Selection stays unresolved until the user
picks an action, which keeps the parent's "all resolved" gate honest.
-->

<script lang="ts">
	import { Card, Badge, Button } from '$lib/components/ui';
	import { Bot, Server, Cpu, FileText, Sparkles, Cloud } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import type { ImportConflict, ConflictResolution } from '$types/import-export';

	/** Props */
	interface Props {
		/** List of detected conflicts */
		conflicts: ImportConflict[];
		/** Current resolution state */
		resolutions: Record<string, ConflictResolution>;
		/** Resolution change callback */
		onResolve: (resolutions: Record<string, ConflictResolution>) => void;
	}

	let { conflicts, resolutions, onResolve }: Props = $props();

	/** Lucide icon per entity type (falls back to a generic bot). */
	const iconMap: Record<string, typeof Bot> = {
		agent: Bot,
		mcp: Server,
		model: Cpu,
		prompt: FileText,
		skill: Sparkles,
		custom_provider: Cloud
	};

	function iconFor(type: string): typeof Bot {
		return iconMap[type] ?? Bot;
	}

	/**
	 * Get entity type label
	 */
	function getEntityTypeLabel(type: string): string {
		const keys: Record<string, string> = {
			agent: 'ie_entity_agent',
			mcp: 'ie_entity_mcp_server',
			model: 'ie_entity_model',
			prompt: 'ie_entity_prompt',
			skill: 'ie_entity_skill',
			custom_provider: 'ie_entity_custom_provider'
		};
		return $i18n(keys[type] || type);
	}

	/**
	 * Generate composite key for conflict resolution.
	 * Uses entityType:entityName to avoid collisions between different entity types.
	 * NOTE: entityName is the unique identifier (IDs are not exported).
	 */
	function getConflictKey(conflict: ImportConflict): string {
		return `${conflict.entityType}:${conflict.entityName}`;
	}

	/**
	 * Set (or clear) the resolution for a single conflict. An empty value leaves
	 * the conflict unresolved by removing its key.
	 */
	function setResolution(conflict: ImportConflict, value: string): void {
		const key = getConflictKey(conflict);
		const next = { ...resolutions };
		if (value === 'skip' || value === 'overwrite' || value === 'rename') {
			next[key] = value;
		} else {
			delete next[key];
		}
		onResolve(next);
	}

	/**
	 * Apply one resolution to every conflict at once.
	 */
	function applyAll(resolution: ConflictResolution): void {
		const next = { ...resolutions };
		for (const conflict of conflicts) {
			next[getConflictKey(conflict)] = resolution;
		}
		onResolve(next);
	}

	/** Whether every conflict has a resolution. */
	const allResolved = $derived(conflicts.every((c) => resolutions[getConflictKey(c)]));

	/** Count of unresolved conflicts. */
	const unresolvedCount = $derived(conflicts.filter((c) => !resolutions[getConflictKey(c)]).length);
</script>

<div class="conflict-resolver">
	<Card title={$i18n('ie_resolve_conflicts_title')}>
		{#snippet headerActions()}
			{#if allResolved}
				<Badge variant="success">{$i18n('ie_all_resolved')}</Badge>
			{:else}
				<Badge variant="warning">
					{$i18n('ie_x_unresolved').replace('{count}', String(unresolvedCount))}
				</Badge>
			{/if}
		{/snippet}
		{#snippet body()}
			<p class="subtitle">
				{$i18n('ie_conflicts_need_resolution')
					.replace('{unresolved}', String(unresolvedCount))
					.replace('{total}', String(conflicts.length))}
			</p>

			<div class="bulk-actions">
				<Button variant="ghost" size="sm" onclick={() => applyAll('skip')}>
					{$i18n('ie_skip_all')}
				</Button>
				<Button variant="ghost" size="sm" onclick={() => applyAll('overwrite')}>
					{$i18n('ie_overwrite_all')}
				</Button>
				<Button variant="ghost" size="sm" onclick={() => applyAll('rename')}>
					{$i18n('ie_rename_all')}
				</Button>
			</div>

			<div class="conflict-rows">
				{#each conflicts as conflict (getConflictKey(conflict))}
					{@const Icon = iconFor(conflict.entityType)}
					<div class="entity-row">
						<span class="entity-icon"><Icon size={18} aria-hidden="true" /></span>
						<div class="entity-main">
							<strong>
								{getEntityTypeLabel(conflict.entityType)} « {conflict.entityName} »
							</strong>
							<span>{$i18n('ie_conflict_already_exists')}</span>
						</div>
						<select
							class="form-select conflict-select"
							aria-label={conflict.entityName}
							value={resolutions[getConflictKey(conflict)] ?? ''}
							onchange={(e) => setResolution(conflict, e.currentTarget.value)}
						>
							<option value="">{$i18n('ie_select_resolution')}</option>
							<option value="skip">{$i18n('ie_resolution_skip')}</option>
							<option value="overwrite">{$i18n('ie_resolution_overwrite')}</option>
							<option value="rename">{$i18n('ie_resolution_rename')}</option>
						</select>
					</div>
				{/each}
			</div>
		{/snippet}
	</Card>
</div>

<style>
	.conflict-resolver {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.subtitle {
		margin: 0 0 var(--spacing-md);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.bulk-actions {
		display: flex;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-md);
		flex-wrap: wrap;
	}

	/* Compact rows: type icon, name + note, resolution select on the right. */
	.conflict-rows {
		display: flex;
		flex-direction: column;
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-md);
	}

	.entity-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light);
	}

	.entity-row:last-child {
		border-bottom: none;
	}

	.entity-icon {
		display: inline-flex;
		flex-shrink: 0;
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
	}

	.conflict-select {
		width: auto;
		min-width: 150px;
		flex-shrink: 0;
	}

	@media (max-width: 768px) {
		.entity-row {
			flex-wrap: wrap;
		}

		.conflict-select {
			width: 100%;
		}
	}
</style>
