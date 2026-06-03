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
  Configures the detached card-compose behaviour. For now the only setting is
  the wall-clock timeout (seconds) applied to a single compose run.
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { Card, Input, Button } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import { toastStore } from '$lib/stores/toast';
	import { getErrorMessage } from '$lib/utils/error';
	import { KanbanSquare } from '@lucide/svelte';
	import {
		COMPOSE_TIMEOUT_MIN_SECS,
		COMPOSE_TIMEOUT_MAX_SECS,
		COMPOSE_TIMEOUT_DEFAULT_SECS,
		type KanbanSettings
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

	onMount(async () => {
		try {
			const settings = await invoke<KanbanSettings>('get_kanban_settings');
			persistedTimeoutSecs = settings.composeTimeoutSecs;
			composeTimeoutSecs = String(settings.composeTimeoutSecs);
		} catch (err) {
			// On load failure the field keeps the backend default (600); surface
			// the error so the user knows the shown value is a fallback, not the
			// stored setting.
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

	async function handleTimeoutChange(
		event: Event & { currentTarget: HTMLInputElement }
	): Promise<void> {
		const next = clampInput(event.currentTarget.value);
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

	async function handleReset(): Promise<void> {
		// Re-load from backend to restore the persisted value.
		loading = true;
		try {
			const settings = await invoke<KanbanSettings>('get_kanban_settings');
			persistedTimeoutSecs = settings.composeTimeoutSecs;
			composeTimeoutSecs = String(settings.composeTimeoutSecs);
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
	<div class="section-title-row">
		<KanbanSquare size={24} />
		<h2 class="section-title">{$i18n('kanban_settings_title')}</h2>
	</div>
	<p class="section-description">{$i18n('kanban_settings_description')}</p>

	{#if loading}
		<div class="lazy-loading">{$i18n('kanban_settings_loading')}</div>
	{:else}
		<Card>
			{#snippet body()}
				<div class="form-grid">
					<Input
						type="number"
						label={$i18n('kanban_settings_compose_timeout_label')}
						value={composeTimeoutSecs}
						min={COMPOSE_TIMEOUT_MIN_SECS}
						max={COMPOSE_TIMEOUT_MAX_SECS}
						step={30}
						disabled={saving}
						onchange={handleTimeoutChange}
						help={$i18n('kanban_settings_compose_timeout_help')}
					/>
					<p class="form-hint">
						{$i18n('kanban_settings_compose_timeout_range')
							.replace('{min}', String(COMPOSE_TIMEOUT_MIN_SECS))
							.replace('{max}', String(COMPOSE_TIMEOUT_MAX_SECS))}
					</p>

					<div class="actions">
						<Button variant="secondary" onclick={handleReset} disabled={saving}>
							{$i18n('kanban_settings_reload')}
						</Button>
					</div>
				</div>
			{/snippet}
		</Card>
	{/if}
</section>

<style>
	.section-description {
		color: var(--color-text-secondary);
		margin-bottom: var(--spacing-lg);
	}

	.form-grid {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.form-hint {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.actions {
		display: flex;
		gap: var(--spacing-sm);
	}
</style>
