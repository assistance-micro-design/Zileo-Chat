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
	import { Download, Upload } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { getErrorMessage } from '$lib/utils/error';
	import { toastStore } from '$lib/stores/toast';
	import type { ToastType } from '$types/background-workflow';

	/** Props */
	interface Props {
		/** Callback when import completes - signals that data was imported and stores should refresh */
		onRefreshNeeded?: () => void | Promise<void>;
	}

	let { onRefreshNeeded }: Props = $props();

	// Tab state
	let activeTab: 'export' | 'import' = $state('export');

	function notify(type: ToastType, text: string): void {
		toastStore.add({ type, title: text, message: '', persistent: false, duration: 5000 });
	}

	function handleExportComplete(success: boolean): void {
		notify(
			success ? 'success' : 'error',
			$i18n(success ? 'ie_export_success' : 'ie_export_failed')
		);
	}

	async function handleImportComplete(success: boolean): Promise<void> {
		if (success) {
			notify('success', $i18n('ie_import_success'));
			// Await the refresh to ensure UI updates with new data before further interactions.
			try {
				await onRefreshNeeded?.();
			} catch (err) {
				notify('error', $i18n('ie_refresh_failed').replace('{error}', getErrorMessage(err)));
			}
		} else {
			notify('error', $i18n('ie_import_failed'));
		}
	}
</script>

<div class="import-export-settings">
	<SettingsSectionHeader
		titleKey="ie_title"
		descriptionKey="ie_description"
		helpTitleKey="help_import_export_title"
		helpDescriptionKey="help_import_export_description"
		helpTutorialKey="help_import_export_tutorial"
	/>

	<!-- Tab navigation: brand segmented control (mirrors the dock's nav-seg) -->
	<div class="seg-tabs" role="tablist" aria-label={$i18n('ie_title')}>
		<button
			type="button"
			class="seg-tab"
			role="tab"
			aria-selected={activeTab === 'export'}
			class:active={activeTab === 'export'}
			onclick={() => (activeTab = 'export')}
		>
			<Download size={16} aria-hidden="true" />
			<span>{$i18n('ie_tab_export')}</span>
		</button>
		<button
			type="button"
			class="seg-tab"
			role="tab"
			aria-selected={activeTab === 'import'}
			class:active={activeTab === 'import'}
			onclick={() => (activeTab = 'import')}
		>
			<Upload size={16} aria-hidden="true" />
			<span>{$i18n('ie_tab_import')}</span>
		</button>
	</div>

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
		gap: var(--spacing-lg);
		padding: var(--spacing-md);
	}

	/* Segmented control: a translucent pill rail; the active segment lifts onto
	   a surface with a soft brand glow and tints its icon (mirrors .nav-seg). */
	.seg-tabs {
		display: inline-flex;
		align-items: center;
		gap: 2px;
		padding: 3px;
		width: fit-content;
		background: var(--color-bg-tertiary);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-full);
	}

	.seg-tab {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-sm);
		padding: 0.4rem 0.9rem;
		border: none;
		background: transparent;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		cursor: pointer;
		border-radius: var(--border-radius-full);
		transition:
			background-color var(--transition-fast),
			color var(--transition-fast);
	}

	.seg-tab:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
	}

	.seg-tab.active {
		background: var(--surface-1);
		color: var(--color-text-primary);
		box-shadow: var(--shadow-xs), var(--glow-accent-soft);
	}

	.seg-tab.active :global(svg) {
		color: var(--color-accent-deep);
	}

	.seg-tab:focus-visible {
		outline: none;
		box-shadow:
			0 0 0 3px var(--color-accent-light),
			0 0 0 1px var(--color-accent-deep);
	}

	.tab-content {
		min-height: 400px;
	}
</style>
