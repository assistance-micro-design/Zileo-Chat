<script lang="ts">
	// Copyright 2025 Zileo-Chat-3 Contributors
	// SPDX-License-Identifier: Apache-2.0

	/**
	 * ImportExportSettings - Main container for Import/Export functionality
	 *
	 * Provides tabbed interface for Export and Import operations.
	 */

	import { ExportPanel } from './index';
	import { ImportPanel } from './index';
	import { Download, Upload } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';
	import { HelpButton } from '$lib/components/ui';

	/** Props */
	interface Props {
		/** Callback when import completes - signals that data was imported and stores should refresh */
		onRefreshNeeded?: () => void | Promise<void>;
	}

	let { onRefreshNeeded }: Props = $props();

	// Tab state
	let activeTab: 'export' | 'import' = $state('export');

	// Message state for feedback
	let message: { type: 'success' | 'error'; text: string } | null = $state(null);

	function handleExportComplete(success: boolean) {
		if (success) {
			message = { type: 'success', text: $i18n('ie_export_success') };
		} else {
			message = { type: 'error', text: $i18n('ie_export_failed') };
		}
		// Clear message after 5 seconds
		setTimeout(() => {
			message = null;
		}, 5000);
	}

	async function handleImportComplete(success: boolean): Promise<void> {
		if (success) {
			message = { type: 'success', text: $i18n('ie_import_success') };
			// Signal that stores should be refreshed after successful import
			// CRITICAL: Await the refresh to ensure UI updates with new data
			try {
				await onRefreshNeeded?.();
			} catch (err) {
				console.error('Failed to refresh stores after import:', err);
			}
		} else {
			message = { type: 'error', text: $i18n('ie_import_failed') };
		}
		// Clear message after 5 seconds
		setTimeout(() => {
			message = null;
		}, 5000);
	}
</script>

<div class="import-export-settings">
	<div class="header">
		<div class="header-title-row">
			<h2>{$i18n('ie_title')}</h2>
			<HelpButton
				titleKey="help_import_export_title"
				descriptionKey="help_import_export_description"
				tutorialKey="help_import_export_tutorial"
			/>
		</div>
		<p class="description">
			{$i18n('ie_description')}
		</p>
	</div>

	<!-- Tab navigation -->
	<div class="tabs">
		<button
			class="tab"
			class:active={activeTab === 'export'}
			onclick={() => (activeTab = 'export')}
		>
			<Download size={16} />
			<span>{$i18n('ie_tab_export')}</span>
		</button>
		<button
			class="tab"
			class:active={activeTab === 'import'}
			onclick={() => (activeTab = 'import')}
		>
			<Upload size={16} />
			<span>{$i18n('ie_tab_import')}</span>
		</button>
	</div>

	<!-- Message banner -->
	{#if message}
		<div class="message" class:success={message.type === 'success'} class:error={message.type === 'error'}>
			{message.text}
		</div>
	{/if}

	<!-- Tab content -->
	<div class="tab-content">
		{#if activeTab === 'export'}
			<ExportPanel onexport={handleExportComplete} />
		{:else}
			<ImportPanel onimport={handleImportComplete} />
		{/if}
	</div>
</div>

<style>
	.import-export-settings {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg, 1.5rem);
		padding: var(--spacing-md, 1rem);
	}

	.header {
		margin-bottom: var(--spacing-sm, 0.5rem);
	}

	.header-title-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm, 0.5rem);
	}

	.header h2 {
		margin: 0;
		font-size: var(--font-size-xl, 1.25rem);
		font-weight: 600;
		color: var(--color-text-primary, #1a1a2e);
	}

	.header .description {
		margin: 0;
		font-size: var(--font-size-sm, 0.875rem);
		color: var(--color-text-secondary, #6b7280);
	}

	.tabs {
		display: flex;
		gap: var(--spacing-xs, 0.25rem);
		border-bottom: 1px solid var(--color-border, #e5e7eb);
		padding-bottom: var(--spacing-xs, 0.25rem);
	}

	.tab {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs, 0.25rem);
		padding: var(--spacing-sm, 0.5rem) var(--spacing-md, 1rem);
		border: none;
		background: transparent;
		color: var(--color-text-secondary, #6b7280);
		font-size: var(--font-size-sm, 0.875rem);
		font-weight: 500;
		cursor: pointer;
		border-radius: var(--border-radius-md, 0.375rem) var(--border-radius-md, 0.375rem) 0 0;
		transition:
			background-color 0.2s,
			color 0.2s;
	}

	.tab:hover {
		background: var(--color-bg-hover, #f3f4f6);
		color: var(--color-text-primary, #1a1a2e);
	}

	.tab.active {
		background: var(--color-bg-primary, #ffffff);
		color: var(--color-primary, #6366f1);
		border-bottom: 2px solid var(--color-primary, #6366f1);
		margin-bottom: -1px;
	}

	.message {
		padding: var(--spacing-sm, 0.5rem) var(--spacing-md, 1rem);
		border-radius: var(--border-radius-md, 0.375rem);
		font-size: var(--font-size-sm, 0.875rem);
		font-weight: 500;
	}

	.message.success {
		background: var(--color-success-bg, #dcfce7);
		color: var(--color-success, #16a34a);
		border: 1px solid var(--color-success-border, #86efac);
	}

	.message.error {
		background: var(--color-error-bg, #fee2e2);
		color: var(--color-error, #dc2626);
		border: 1px solid var(--color-error-border, #fca5a5);
	}

	.tab-content {
		min-height: 400px;
	}

	/* Dark mode */
	:global(.dark) .header h2 {
		color: var(--color-text-primary-dark, #f9fafb);
	}

	:global(.dark) .header .description {
		color: var(--color-text-secondary-dark, #9ca3af);
	}

	:global(.dark) .tabs {
		border-color: var(--color-border-dark, #374151);
	}

	:global(.dark) .tab {
		color: var(--color-text-secondary-dark, #9ca3af);
	}

	:global(.dark) .tab:hover {
		background: var(--color-bg-hover-dark, #374151);
		color: var(--color-text-primary-dark, #f9fafb);
	}

	:global(.dark) .tab.active {
		background: var(--color-bg-primary-dark, #1f2937);
		color: var(--color-primary, #818cf8);
		border-bottom-color: var(--color-primary, #818cf8);
	}
</style>
