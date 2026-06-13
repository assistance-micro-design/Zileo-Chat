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
  Settings > Kanban page.
  Configures the detached card-compose behaviour: the wall-clock timeout, plus
  the two GLOBAL supervisor agents (one for compose, one for analyze). A
  read-only preview shows the effective prompt each role would run with.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Card, Input, Button, Select } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui';
	import { RefreshCw, Eye } from '@lucide/svelte';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { i18n } from '$lib/i18n';
	import { toastStore } from '$lib/stores/toast';
	import { getErrorMessage } from '$lib/utils/error';
	import { kanbanSupervisorStore } from '$lib/stores/kanban-settings';
	import type { AgentSummary } from '$types/agent';
	import {
		COMPOSE_TIMEOUT_MIN_SECS,
		COMPOSE_TIMEOUT_MAX_SECS,
		COMPOSE_TIMEOUT_DEFAULT_SECS,
		type KanbanSettings,
		type UpdateKanbanSettingsRequest
	} from '$types/kanban-settings';

	/**
	 * Compose timeout in seconds, as a string for the number input binding.
	 * Seeded with the backend default (not MAX) so a failed `get_kanban_settings`
	 * at mount still shows the true default value, never an out-of-context MAX.
	 */
	let composeTimeoutSecs = $state(String(COMPOSE_TIMEOUT_DEFAULT_SECS));
	/** The last value successfully persisted (for optimistic rollback). */
	let persistedTimeoutSecs = $state(COMPOSE_TIMEOUT_DEFAULT_SECS);
	let loading = $state(true);
	let saving = $state(false);

	/** Currently selected supervisor agents (empty string = none). */
	let composeAgentId = $state('');
	let analyzeAgentId = $state('');
	/** Persisted values for optimistic rollback. */
	let persistedComposeAgentId = $state('');
	let persistedAnalyzeAgentId = $state('');
	let savingSupervisors = $state(false);

	/** Kanban-kind agents available as supervisors. */
	let kanbanAgents = $state<AgentSummary[]>([]);

	/** Read-only prompt preview state. */
	let previewMode = $state<'compose' | 'analyze'>('compose');
	let previewText = $state('');
	let previewLoading = $state(false);
	let previewError = $state<string | null>(null);

	const agentOptions = $derived<SelectOption[]>([
		{ value: '', label: $i18n('kanban_settings_agent_none') },
		...kanbanAgents.map((a) => ({ value: a.id, label: a.name }))
	]);

	/** The agent id whose prompt the preview would render for the chosen mode. */
	const previewAgentId = $derived(previewMode === 'compose' ? composeAgentId : analyzeAgentId);

	// Invalidate a shown preview whenever the previewed agent/mode changes — the
	// preview is on-demand, so a stale prompt from a previous selection must not
	// linger. Reads previewAgentId; writes preview output it never reads (no loop).
	$effect(() => {
		void previewAgentId;
		previewText = '';
		previewError = null;
	});

	const supervisorsDirty = $derived(
		composeAgentId !== persistedComposeAgentId || analyzeAgentId !== persistedAnalyzeAgentId
	);

	function applyLoadedSettings(settings: KanbanSettings): void {
		persistedTimeoutSecs = settings.composeTimeoutSecs;
		composeTimeoutSecs = String(settings.composeTimeoutSecs);
		persistedComposeAgentId = settings.composeAgentId ?? '';
		persistedAnalyzeAgentId = settings.analyzeAgentId ?? '';
		composeAgentId = persistedComposeAgentId;
		analyzeAgentId = persistedAnalyzeAgentId;
		kanbanSupervisorStore.setFromSettings(settings);
	}

	onMount(async () => {
		try {
			const [settings, agents] = await Promise.all([
				invoke<KanbanSettings>('get_kanban_settings'),
				invoke<AgentSummary[]>('list_agents')
			]);
			kanbanAgents = agents.filter((a) => a.kind === 'kanban');
			applyLoadedSettings(settings);
		} catch (err) {
			// On load failure the fields keep the backend defaults; surface the
			// error so the user knows the shown values are a fallback.
			toastStore.add({
				type: 'error',
				title: $i18n('kanban_settings_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 5000
			});
		} finally {
			loading = false;
		}
	});

	/**
	 * Parses and clamps the entered value to the allowed bounds. Falls back to
	 * the last persisted value when the input is not a finite number.
	 */
	function clampInput(raw: string): number {
		const parsed = Number.parseInt(raw, 10);
		if (!Number.isFinite(parsed)) {
			return persistedTimeoutSecs;
		}
		return Math.min(Math.max(parsed, COMPOSE_TIMEOUT_MIN_SECS), COMPOSE_TIMEOUT_MAX_SECS);
	}

	async function handleSave(): Promise<void> {
		const next = clampInput(composeTimeoutSecs);
		const previous = persistedTimeoutSecs;
		// Reflect the clamped value immediately in the input.
		composeTimeoutSecs = String(next);
		if (next === previous) {
			return;
		}
		saving = true;
		try {
			const updated = await invoke<KanbanSettings>('update_kanban_settings', {
				request: { composeTimeoutSecs: next }
			});
			persistedTimeoutSecs = updated.composeTimeoutSecs;
			composeTimeoutSecs = String(updated.composeTimeoutSecs);
			toastStore.add({
				type: 'success',
				title: $i18n('kanban_settings_saved'),
				message: $i18n('kanban_settings_compose_timeout_saved'),
				persistent: false,
				duration: 2500
			});
		} catch (err) {
			// Roll back to the last persisted value on failure.
			persistedTimeoutSecs = previous;
			composeTimeoutSecs = String(previous);
			toastStore.add({
				type: 'error',
				title: $i18n('kanban_settings_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 6000
			});
		} finally {
			saving = false;
		}
	}

	/** Persists the two supervisor ids (tri-state: empty string clears the id). */
	async function handleSaveSupervisors(): Promise<void> {
		if (!supervisorsDirty) {
			return;
		}
		const previousCompose = persistedComposeAgentId;
		const previousAnalyze = persistedAnalyzeAgentId;
		savingSupervisors = true;
		try {
			const request: UpdateKanbanSettingsRequest = {
				composeAgentId: composeAgentId || null,
				analyzeAgentId: analyzeAgentId || null
			};
			const updated = await invoke<KanbanSettings>('update_kanban_settings', { request });
			persistedComposeAgentId = updated.composeAgentId ?? '';
			persistedAnalyzeAgentId = updated.analyzeAgentId ?? '';
			composeAgentId = persistedComposeAgentId;
			analyzeAgentId = persistedAnalyzeAgentId;
			kanbanSupervisorStore.setFromSettings(updated);
			toastStore.add({
				type: 'success',
				title: $i18n('kanban_settings_saved'),
				message: $i18n('kanban_settings_supervisors_saved'),
				persistent: false,
				duration: 2500
			});
		} catch (err) {
			// Roll back to the last persisted ids on failure.
			composeAgentId = previousCompose;
			analyzeAgentId = previousAnalyze;
			toastStore.add({
				type: 'error',
				title: $i18n('kanban_settings_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 6000
			});
		} finally {
			savingSupervisors = false;
		}
	}

	/** Loads the read-only effective prompt for the selected agent + mode. */
	async function loadPreview(): Promise<void> {
		previewError = null;
		previewText = '';
		if (!previewAgentId) {
			return;
		}
		previewLoading = true;
		try {
			previewText = await invoke<string>('preview_kanban_role_prompt', {
				agentId: previewAgentId,
				mode: previewMode
			});
		} catch (err) {
			previewError = `${$i18n('kanban_settings_prompt_preview_error')}: ${getErrorMessage(err)}`;
		} finally {
			previewLoading = false;
		}
	}

	async function handleReset(): Promise<void> {
		// Re-load from backend to restore the persisted values.
		loading = true;
		try {
			const settings = await invoke<KanbanSettings>('get_kanban_settings');
			applyLoadedSettings(settings);
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: $i18n('kanban_settings_save_error'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 5000
			});
		} finally {
			loading = false;
		}
	}
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="kanban_settings_title"
		descriptionKey="kanban_settings_description"
		helpTitleKey="help_kanban_settings_title"
		helpDescriptionKey="help_kanban_settings_description"
		helpTutorialKey="help_kanban_settings_tutorial"
	/>

	{#if loading}
		<div class="lazy-loading">{$i18n('kanban_settings_loading')}</div>
	{:else}
		<div class="settings-card">
			<Card>
				{#snippet body()}
					<div class="timeout-field">
						<Input
							type="number"
							label={$i18n('kanban_settings_compose_timeout_label')}
							value={composeTimeoutSecs}
							min={COMPOSE_TIMEOUT_MIN_SECS}
							max={COMPOSE_TIMEOUT_MAX_SECS}
							step={30}
							disabled={saving}
							oninput={(e) => {
								composeTimeoutSecs = e.currentTarget.value;
							}}
							help={`${$i18n('kanban_settings_compose_timeout_help')} ${$i18n(
								'kanban_settings_compose_timeout_range'
							)
								.replace('{min}', String(COMPOSE_TIMEOUT_MIN_SECS))
								.replace('{max}', String(COMPOSE_TIMEOUT_MAX_SECS))}`}
						/>
					</div>
				{/snippet}
				{#snippet footer()}
					<div class="card-actions">
						<Button variant="outline" size="sm" onclick={handleReset} disabled={saving}>
							<RefreshCw size={14} />
							<span>{$i18n('kanban_settings_reload')}</span>
						</Button>
						<Button variant="primary" size="sm" onclick={handleSave} disabled={saving}>
							{$i18n('common_save')}
						</Button>
					</div>
				{/snippet}
			</Card>
		</div>

		<div class="settings-card">
			<Card
				title={$i18n('kanban_settings_supervisors_title')}
				description={$i18n('kanban_settings_supervisors_description')}
			>
				{#snippet body()}
					<div class="supervisors-grid">
						<Select
							label={$i18n('kanban_settings_compose_agent_label')}
							options={agentOptions}
							value={composeAgentId}
							help={$i18n('kanban_settings_agent_help')}
							disabled={savingSupervisors}
							onchange={(e) => (composeAgentId = e.currentTarget.value)}
						/>
						<Select
							label={$i18n('kanban_settings_analyze_agent_label')}
							options={agentOptions}
							value={analyzeAgentId}
							help={$i18n('kanban_settings_agent_help')}
							disabled={savingSupervisors}
							onchange={(e) => (analyzeAgentId = e.currentTarget.value)}
						/>
					</div>

					<div class="preview">
						<div class="preview-head">
							<span class="preview-title">{$i18n('kanban_settings_prompt_preview_title')}</span>
							<div class="preview-modes" role="tablist">
								<button
									type="button"
									role="tab"
									class="mode"
									class:active={previewMode === 'compose'}
									aria-selected={previewMode === 'compose'}
									onclick={() => (previewMode = 'compose')}
								>
									{$i18n('kanban_settings_prompt_preview_compose')}
								</button>
								<button
									type="button"
									role="tab"
									class="mode"
									class:active={previewMode === 'analyze'}
									aria-selected={previewMode === 'analyze'}
									onclick={() => (previewMode = 'analyze')}
								>
									{$i18n('kanban_settings_prompt_preview_analyze')}
								</button>
							</div>
							<Button
								variant="outline"
								size="sm"
								onclick={loadPreview}
								disabled={!previewAgentId || previewLoading}
							>
								<Eye size={14} />
								<span>{$i18n('kanban_settings_prompt_preview_show')}</span>
							</Button>
						</div>
						{#if previewError}
							<p class="preview-error" role="alert">{previewError}</p>
						{:else if previewText}
							<pre class="preview-panel">{previewText}</pre>
						{:else}
							<p class="preview-empty">{$i18n('kanban_settings_prompt_preview_empty')}</p>
						{/if}
					</div>
				{/snippet}
				{#snippet footer()}
					<div class="card-actions">
						<Button
							variant="primary"
							size="sm"
							onclick={handleSaveSupervisors}
							disabled={savingSupervisors || !supervisorsDirty}
						>
							{$i18n('common_save')}
						</Button>
					</div>
				{/snippet}
			</Card>
		</div>
	{/if}
</section>

<style>
	.settings-section :global(.settings-header) {
		margin-bottom: var(--spacing-lg);
	}

	.settings-card {
		max-width: 640px;
		margin-bottom: var(--spacing-lg);
	}

	.timeout-field :global(input) {
		width: 180px;
	}

	.supervisors-grid {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.preview {
		margin-top: var(--spacing-lg);
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}
	.preview-head {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}
	.preview-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}
	.preview-modes {
		display: inline-flex;
		gap: 2px;
		padding: 3px;
		background: var(--color-bg-tertiary);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-full);
		margin-left: auto;
	}
	.mode {
		border: none;
		background: none;
		border-radius: var(--border-radius-full);
		padding: 0.2rem 0.7rem;
		cursor: pointer;
		font-size: var(--font-size-xs);
		font-family: var(--font-family);
		color: var(--color-text-secondary);
	}
	.mode.active {
		background: var(--surface-1);
		color: var(--color-text-primary);
		box-shadow: var(--shadow-xs), var(--glow-accent-soft);
	}
	.preview-panel {
		margin: 0;
		max-height: 320px;
		overflow: auto;
		padding: var(--spacing-md);
		font-family: var(--font-family-mono);
		font-size: var(--font-size-xs);
		line-height: 1.5;
		white-space: pre-wrap;
		word-break: break-word;
		color: var(--color-text-secondary);
		background: var(--color-bg-tertiary);
		border: 1px solid var(--color-border-light);
		border-radius: var(--border-radius-md);
	}
	.preview-empty {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
	}
	.preview-error {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-error);
	}

	.card-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
		align-items: center;
		flex: 1;
	}

	.card-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}
</style>
