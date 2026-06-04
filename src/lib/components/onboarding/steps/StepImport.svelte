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
	/**
	 * Onboarding step: import an existing configuration package.
	 *
	 * First-launch simplified import: the local database is assumed empty, so
	 * every entity is selected and no conflict resolution UI is shown. If the
	 * file is invalid or unexpectedly reports conflicts, the user is directed to
	 * the full Settings > Import/Export page and onboarding is never blocked.
	 *
	 * Reuses the exact backend contract of ImportPanel:
	 *   validate_import({ data })
	 *   execute_import({ data, selection, resolutions, mcpAdditions })
	 */
	import { i18n } from '$lib/i18n';
	import { scale } from 'svelte/transition';
	import { Button, Card, Badge } from '$lib/components/ui';
	import { tauriInvoke } from '$lib/tauri';
	import { onboardingStore } from '$lib/stores/onboarding';
	import { getErrorMessage } from '$lib/utils/error';
	import { motionDuration } from '$lib/utils/motion';
	import {
		createSelectionFromValidation,
		resolveWarningAction,
		resolveWarningDetail
	} from '$lib/components/settings/import-export/ImportPanel.helpers';
	import type {
		ConfigImportResult,
		ImportValidation,
		ImportWarning,
		ConflictResolution,
		MCPAdditions
	} from '$types/import-export';
	import { MAX_IMPORT_FILE_SIZE } from '$types/import-export';
	import { Upload, CircleCheckBig, TriangleAlert, CircleAlert } from '@lucide/svelte';

	interface Props {
		onNext: () => void;
	}

	let { onNext }: Props = $props();

	let loading = $state(false);
	let error = $state<string | null>(null);
	/** True when the file cannot be imported here and the user must use Settings. */
	let needsFullImporter = $state(false);
	let result = $state<ConfigImportResult | null>(null);
	/** Non-blocking warnings carried from validation, surfaced after import. */
	let warnings = $state<ImportWarning[]>([]);
	let fileInput: HTMLInputElement | null = $state(null);

	/** Total entities imported, used to gate the success summary. */
	const totalImported = $derived(
		result
			? result.imported.agents +
					result.imported.mcpServers +
					result.imported.models +
					result.imported.prompts +
					result.imported.skills +
					result.imported.customProviders
			: 0
	);

	/**
	 * Open the hidden file input.
	 */
	function handleBrowse(): void {
		if (!fileInput) return;
		fileInput.value = '';
		fileInput.click();
	}

	/**
	 * Validate and import the selected configuration file in one pass.
	 */
	async function handleFileSelected(event: Event): Promise<void> {
		const file = (event.target as HTMLInputElement).files?.[0];
		if (!file) return;

		error = null;
		needsFullImporter = false;
		result = null;
		warnings = [];

		if (file.size > MAX_IMPORT_FILE_SIZE) {
			error = $i18n('ie_file_too_large').replace(
				'{size}',
				String(MAX_IMPORT_FILE_SIZE / (1024 * 1024))
			);
			return;
		}

		loading = true;
		try {
			const text = await file.text();

			const validation = await tauriInvoke<ImportValidation>('validate_import', { data: text });

			if (!validation.valid) {
				needsFullImporter = true;
				return;
			}

			// First launch = empty database. Any reported conflict or missing MCP
			// secret means the simplified path is not appropriate; defer to Settings.
			if (validation.conflicts.length > 0 || Object.keys(validation.missingMcpEnv).length > 0) {
				needsFullImporter = true;
				return;
			}

			// Carry non-blocking warnings (e.g. MCP allowlist reset, missing
			// dependencies) so they can be surfaced after the import instead of
			// being silently swallowed.
			warnings = validation.warnings;

			const selection = createSelectionFromValidation(validation);
			const resolutions: Record<string, ConflictResolution> = {};
			const mcpAdditions: Record<string, MCPAdditions> = {};

			result = await tauriInvoke<ConfigImportResult>('execute_import', {
				data: text,
				selection,
				resolutions,
				mcpAdditions
			});

			// Flag the import so navigation skips the getting-started step (its
			// guidance assumes a from-scratch setup). A partial import (some
			// entities in, some per-entity errors) still counts as imported.
			if (totalImported > 0) {
				onboardingStore.setImported(true);
			}
		} catch (e) {
			error = getErrorMessage(e);
		} finally {
			loading = false;
		}
	}

	function handleSkip(): void {
		onNext();
	}
</script>

<div class="step-import" data-step="import">
	<h1 class="step-title">{$i18n('onboarding_import_title')}</h1>
	<p class="step-description">{$i18n('onboarding_import_description')}</p>

	<input
		bind:this={fileInput}
		type="file"
		accept=".json"
		hidden
		onchange={handleFileSelected}
		aria-hidden="true"
	/>

	{#if needsFullImporter}
		<div class="import-notice import-notice-warning" role="status">
			{$i18n('onboarding_import_use_settings')}
		</div>
	{:else if error}
		<div class="import-notice import-notice-error" role="alert">
			{error}
		</div>
	{:else if result}
		{@const imported = result.imported}
		{@const hasErrors = result.errors.length > 0}
		{#if totalImported > 0}
			<Card>
				{#snippet body()}
					<div class="import-result">
						<span
							class="result-icon-wrapper"
							class:partial={hasErrors}
							transition:scale={{ duration: motionDuration(300), start: 0.6 }}
						>
							{#if hasErrors}
								<TriangleAlert size={40} class="result-icon" />
							{:else}
								<CircleCheckBig size={40} class="result-icon" />
							{/if}
						</span>
						<h2 class="result-title">
							{hasErrors
								? $i18n('onboarding_import_partial_title')
								: $i18n('onboarding_import_success_title')}
						</h2>
						<div class="result-counts">
							{#if imported.agents > 0}
								<Badge variant="success">{imported.agents} {$i18n('ie_entity_agents')}</Badge>
							{/if}
							{#if imported.models > 0}
								<Badge variant="success">{imported.models} {$i18n('ie_entity_models')}</Badge>
							{/if}
							{#if imported.mcpServers > 0}
								<Badge variant="success"
									>{imported.mcpServers} {$i18n('ie_entity_mcp_servers')}</Badge
								>
							{/if}
							{#if imported.prompts > 0}
								<Badge variant="success">{imported.prompts} {$i18n('ie_entity_prompts')}</Badge>
							{/if}
							{#if imported.skills > 0}
								<Badge variant="success">{imported.skills} {$i18n('ie_entity_skills')}</Badge>
							{/if}
							{#if imported.customProviders > 0}
								<Badge variant="success"
									>{imported.customProviders} {$i18n('ie_entity_custom_providers')}</Badge
								>
							{/if}
						</div>
						<p class="result-secrets">{$i18n('onboarding_import_secrets_notice')}</p>
					</div>
				{/snippet}
			</Card>
		{:else}
			<div class="import-notice import-notice-warning" role="status">
				{$i18n('onboarding_import_nothing')}
			</div>
		{/if}

		{#if warnings.length > 0}
			<div class="import-messages import-messages-warning" role="status">
				<p class="messages-title">{$i18n('onboarding_import_warnings_title')}</p>
				<ul class="messages-list">
					{#each warnings as warning, i (i)}
						<li>
							<TriangleAlert size={14} aria-hidden="true" />
							<span
								>{resolveWarningDetail(warning, $i18n)} — {resolveWarningAction(
									warning,
									$i18n
								)}</span
							>
						</li>
					{/each}
				</ul>
			</div>
		{/if}

		{#if hasErrors}
			<div class="import-messages import-messages-error" role="alert">
				<p class="messages-title">{$i18n('onboarding_import_errors_title')}</p>
				<ul class="messages-list">
					{#each result.errors as err, i (i)}
						<li>
							<CircleAlert size={14} aria-hidden="true" />
							<span>{err.entityId}: {err.error}</span>
						</li>
					{/each}
				</ul>
			</div>
		{/if}
	{/if}

	<div class="import-options">
		{#if !result || totalImported === 0}
			<Button variant="primary" onclick={handleBrowse} disabled={loading}>
				<Upload size={16} aria-hidden="true" />
				<span>{loading ? $i18n('common_loading') : $i18n('onboarding_import_choose_file')}</span>
			</Button>
		{/if}

		<Button variant={result && totalImported > 0 ? 'primary' : 'ghost'} onclick={handleSkip}>
			{result && totalImported > 0
				? $i18n('onboarding_import_continue')
				: $i18n('onboarding_import_skip')}
		</Button>
	</div>
</div>

<style>
	.step-import {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-lg);
		width: 100%;
	}

	.step-title {
		font-size: var(--font-size-2xl);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.step-description {
		font-size: var(--font-size-base);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 460px;
	}

	.import-result {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-lg);
		text-align: center;
	}

	.result-icon-wrapper {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		color: var(--color-success, #059669);
	}

	.result-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
		margin: 0;
	}

	.result-counts {
		display: flex;
		flex-wrap: wrap;
		justify-content: center;
		gap: var(--spacing-xs);
	}

	.result-secrets {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
		max-width: 460px;
		line-height: 1.5;
	}

	.import-notice {
		padding: var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
		max-width: 460px;
		text-align: center;
	}

	.import-notice-warning {
		background: var(--color-warning-bg, #fef3c7);
		color: var(--color-warning, #92400e);
	}

	.import-notice-error {
		background: var(--color-error-bg, #fee2e2);
		color: var(--color-error, #dc2626);
	}

	.result-icon-wrapper.partial {
		color: var(--color-warning, #92400e);
	}

	.import-messages {
		width: 100%;
		max-width: 460px;
		padding: var(--spacing-sm) var(--spacing-md);
		border-radius: var(--border-radius-md);
		font-size: var(--font-size-sm);
	}

	.import-messages-warning {
		background: var(--color-warning-bg, #fef3c7);
		color: var(--color-warning, #92400e);
	}

	.import-messages-error {
		background: var(--color-error-bg, #fee2e2);
		color: var(--color-error, #dc2626);
	}

	.messages-title {
		margin: 0 0 var(--spacing-xs);
		font-weight: var(--font-weight-semibold);
	}

	.messages-list {
		margin: 0;
		padding: 0;
		list-style: none;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.messages-list li {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-xs);
		line-height: 1.4;
	}

	.messages-list li :global(svg) {
		flex-shrink: 0;
		margin-top: 2px;
	}

	.import-options {
		display: flex;
		flex-direction: column;
		align-items: center;
		gap: var(--spacing-md);
		margin-top: var(--spacing-sm);
	}
</style>
