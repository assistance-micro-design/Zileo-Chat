<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ConflictResolver - Resolve import conflicts with existing entities.
Shows each conflict with resolution options: Skip, Overwrite, or Rename.
Supports bulk resolution with "Apply to all" option.
-->

<script lang="ts">
	import { Card, Badge, Button } from '$lib/components/ui';
	import type { ImportConflict, ConflictResolution } from '$types/importExport';

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

	/** Bulk resolution state */
	let bulkResolution = $state<ConflictResolution | ''>('');

	/**
	 * Get badge variant for entity type
	 */
	function getEntityTypeBadge(
		type: 'agent' | 'mcp' | 'model' | 'prompt'
	): 'primary' | 'success' | 'warning' | 'error' {
		const variants = {
			agent: 'primary' as const,
			mcp: 'success' as const,
			model: 'warning' as const,
			prompt: 'error' as const
		};
		return variants[type] || 'primary';
	}

	/**
	 * Get entity type label
	 */
	function getEntityTypeLabel(type: 'agent' | 'mcp' | 'model' | 'prompt'): string {
		const labels = {
			agent: 'Agent',
			mcp: 'MCP Server',
			model: 'Model',
			prompt: 'Prompt'
		};
		return labels[type] || type;
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
	 * Update resolution for a single conflict
	 */
	function updateResolution(conflict: ImportConflict, resolution: ConflictResolution): void {
		const key = getConflictKey(conflict);
		onResolve({
			...resolutions,
			[key]: resolution
		});
	}

	/**
	 * Apply bulk resolution to all unresolved conflicts
	 */
	function applyBulkResolution(): void {
		if (!bulkResolution) return;

		const newResolutions = { ...resolutions };
		for (const conflict of conflicts) {
			const key = getConflictKey(conflict);
			if (!resolutions[key]) {
				newResolutions[key] = bulkResolution;
			}
		}

		onResolve(newResolutions);
		bulkResolution = '';
	}

	/**
	 * Check if all conflicts are resolved
	 */
	const allResolved = $derived(conflicts.every((c) => resolutions[getConflictKey(c)]));

	/**
	 * Count unresolved conflicts
	 */
	const unresolvedCount = $derived(
		conflicts.filter((c) => !resolutions[getConflictKey(c)]).length
	);

	/**
	 * Key for forcing re-render when resolutions change.
	 * Svelte 5's fine-grained reactivity may not detect new keys added to the object,
	 * so we create a key based on resolved entity IDs to force UI updates.
	 */
	const resolutionsKey = $derived(
		Object.entries(resolutions)
			.map(([id, res]) => `${id}:${res}`)
			.sort()
			.join('|')
	);

	/**
	 * Whether bulk resolution can be applied.
	 * Explicitly derived for Svelte 5 reactivity - ensures button state updates
	 * when bulkResolution or unresolvedCount changes.
	 */
	const canApplyBulk = $derived(bulkResolution !== '' && unresolvedCount > 0);
</script>

<div class="conflict-resolver">
	<!-- Header -->
	<div class="resolver-header">
		<h3>Resolve Import Conflicts</h3>
		<p class="header-info">
			{unresolvedCount} of {conflicts.length} conflicts need resolution
		</p>
		{#if allResolved}
			<Badge variant="success">All Resolved</Badge>
		{:else}
			<Badge variant="warning">{unresolvedCount} Unresolved</Badge>
		{/if}
	</div>

	<!-- Bulk Resolution -->
	<Card title="Bulk Resolution">
		{#snippet body()}
			<div class="bulk-resolution">
				<p class="bulk-help">
					Apply the same resolution to all unresolved conflicts:
				</p>
				<div class="bulk-controls">
					<select
						bind:value={bulkResolution}
						class="bulk-select"
					>
						<option value="">Select resolution...</option>
						<option value="skip">Skip All</option>
						<option value="overwrite">Overwrite All</option>
						<option value="rename">Rename All</option>
					</select>
					<Button
						variant="primary"
						disabled={!canApplyBulk}
						onclick={applyBulkResolution}
					>
						Apply to All Unresolved ({unresolvedCount})
					</Button>
				</div>
			</div>
		{/snippet}
	</Card>

	<!-- Conflict List -->
	<!-- Use {#key} to force re-render when resolutions change (fixes bulk resolution UI update) -->
	{#key resolutionsKey}
	<div class="conflicts-list">
		{#each conflicts as conflict (conflict.entityName)}
			<Card>
				{#snippet body()}
					<div class="conflict-item">
						<div class="conflict-header">
							<Badge variant={getEntityTypeBadge(conflict.entityType)}>
								{getEntityTypeLabel(conflict.entityType)}
							</Badge>
							<Badge variant="warning">
								Name Conflict
							</Badge>
						</div>

						<div class="conflict-details">
							<div class="detail-row">
								<span class="detail-label">Import:</span>
								<span class="detail-value">{conflict.entityName}</span>
							</div>
							<div class="detail-row conflict-arrow">↓</div>
							<div class="detail-row">
								<span class="detail-label">Existing:</span>
								<span class="detail-value">{conflict.entityName}</span>
								<span class="detail-id">(ID: {conflict.existingId})</span>
							</div>
						</div>

						<div class="resolution-options">
							<label class="resolution-option">
								<input
									type="radio"
									name="resolution-{conflict.entityType}-{conflict.entityName}"
									value="skip"
									checked={resolutions[getConflictKey(conflict)] === 'skip'}
									onchange={() => updateResolution(conflict, 'skip')}
								/>
								<div class="option-content">
									<span class="option-label">Skip</span>
									<span class="option-description">
										Do not import this entity
									</span>
								</div>
							</label>

							<label class="resolution-option">
								<input
									type="radio"
									name="resolution-{conflict.entityType}-{conflict.entityName}"
									value="overwrite"
									checked={resolutions[getConflictKey(conflict)] === 'overwrite'}
									onchange={() => updateResolution(conflict, 'overwrite')}
								/>
								<div class="option-content">
									<span class="option-label">Overwrite</span>
									<span class="option-description">
										Replace existing entity with imported one
									</span>
								</div>
							</label>

							<label class="resolution-option">
								<input
									type="radio"
									name="resolution-{conflict.entityType}-{conflict.entityName}"
									value="rename"
									checked={resolutions[getConflictKey(conflict)] === 'rename'}
									onchange={() => updateResolution(conflict, 'rename')}
								/>
								<div class="option-content">
									<span class="option-label">Rename</span>
									<span class="option-description">
										Import as new entity with modified name
									</span>
								</div>
							</label>
						</div>
					</div>
				{/snippet}
			</Card>
		{/each}
	</div>
	{/key}
</div>

<style>
	.conflict-resolver {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.resolver-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		flex-wrap: wrap;
	}

	.resolver-header h3 {
		margin: 0;
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.header-info {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		flex: 1;
	}

	.bulk-resolution {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.bulk-help {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.bulk-controls {
		display: flex;
		gap: var(--spacing-md);
		align-items: center;
		flex-wrap: wrap;
	}

	.bulk-select {
		flex: 1;
		min-width: 200px;
		padding: var(--spacing-sm) var(--spacing-md);
		font-size: var(--font-size-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		cursor: pointer;
	}

	.conflicts-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.conflict-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.conflict-header {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.conflict-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-sm);
	}

	.detail-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		font-size: var(--font-size-sm);
	}

	.conflict-arrow {
		justify-content: center;
		color: var(--color-text-secondary);
		font-size: var(--font-size-lg);
	}

	.detail-label {
		font-weight: var(--font-weight-semibold);
		min-width: 80px;
	}

	.detail-value {
		flex: 1;
	}

	.detail-id {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.resolution-options {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.resolution-option {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-sm);
		padding: var(--spacing-md);
		border: 2px solid var(--color-border);
		border-radius: var(--border-radius-sm);
		cursor: pointer;
		transition: border-color 0.2s, background-color 0.2s;
	}

	.resolution-option:hover {
		background: var(--color-bg-hover);
	}

	.resolution-option:has(input:checked) {
		border-color: var(--color-primary);
		background: var(--color-primary-light);
	}

	.resolution-option input[type='radio'] {
		margin-top: 2px;
		cursor: pointer;
	}

	.option-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		flex: 1;
	}

	.option-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
	}

	.option-description {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	@media (max-width: 768px) {
		.bulk-controls {
			flex-direction: column;
			align-items: stretch;
		}

		.bulk-select {
			min-width: unset;
		}
	}
</style>
