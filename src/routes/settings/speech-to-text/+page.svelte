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
  Settings > Speech-to-text page.
  Configures the Voxtral push-to-talk pipeline (toggle, model ID, optional
  context bias and language override).
-->

<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Input, Textarea, Select, type SelectOption } from '$lib/components/ui';
	import {
		sttSettingsStore,
		sttSettings,
		sttSettingsLoading,
		sttSettingsSaving
	} from '$lib/stores/sttSettings';
	import { STT_SUPPORTED_LANGUAGES, DEFAULT_VOXTRAL_MODEL_ID } from '$types/stt';
	import { validateVoxtralModelId } from '$lib/utils/stt-validation';
	import { openExternalUrl } from '$lib/tauri/opener';
	import { toastStore } from '$lib/stores/toast';
	import SettingsSectionHeader from '$lib/components/settings/SettingsSectionHeader.svelte';
	import { i18n } from '$lib/i18n';
	import { ExternalLink } from '@lucide/svelte';
	import { getErrorMessage } from '$lib/utils/error';

	let enabled = $state(false);
	let modelId = $state(DEFAULT_VOXTRAL_MODEL_ID);
	let contextBiasRaw = $state('');
	let language = $state<'auto' | (typeof STT_SUPPORTED_LANGUAGES)[number]>('auto');
	let modelError = $state<string | null>(null);

	const languageOptions: SelectOption[] = [
		{ value: 'auto', label: 'stt_language_auto' },
		...STT_SUPPORTED_LANGUAGES.map((code) => ({ value: code, label: `stt_language_${code}` }))
	];

	function hydrate(settings: typeof $sttSettings): void {
		if (!settings) return;
		enabled = settings.enabled;
		modelId = settings.modelId;
		contextBiasRaw = settings.contextBias.join('\n');
		language = (settings.language ?? 'auto') as typeof language;
	}

	onMount(async () => {
		try {
			await sttSettingsStore.loadSettings();
			hydrate(sttSettingsStore.getState().settings);
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: $i18n('stt_toast_error_title'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 5000
			});
		}
	});

	function parseContextBias(raw: string): string[] {
		return raw
			.split('\n')
			.map((line) => line.trim())
			.filter((line) => line.length > 0);
	}

	async function handleSave(): Promise<void> {
		const validation = validateVoxtralModelId(modelId);
		modelError = validation;
		if (validation) return;

		const bias = parseContextBias(contextBiasRaw);
		try {
			await sttSettingsStore.updateSettings({
				enabled,
				modelId: modelId.trim(),
				contextBias: bias,
				language: language === 'auto' ? null : language
			});
			toastStore.add({
				type: 'success',
				title: $i18n('stt_toast_saved_title'),
				message: $i18n('stt_toast_saved_message'),
				persistent: false,
				duration: 3000
			});
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: $i18n('stt_toast_error_title'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 6000
			});
		}
	}

	async function handleReset(): Promise<void> {
		try {
			await sttSettingsStore.resetToDefaults();
			hydrate(sttSettingsStore.getState().settings);
			modelError = null;
			toastStore.add({
				type: 'success',
				title: $i18n('stt_toast_saved_title'),
				message: $i18n('stt_toast_reset_message'),
				persistent: false,
				duration: 3000
			});
		} catch (err) {
			toastStore.add({
				type: 'error',
				title: $i18n('stt_toast_error_title'),
				message: getErrorMessage(err),
				persistent: false,
				duration: 6000
			});
		}
	}

	async function openModelsDoc(): Promise<void> {
		try {
			await openExternalUrl('https://docs.mistral.ai/models/overview');
		} catch {
			/* ignored — browsers without the opener plugin */
		}
	}

	function handleModelInput(ev: Event & { currentTarget: HTMLInputElement }): void {
		modelId = ev.currentTarget.value;
		modelError = validateVoxtralModelId(modelId);
	}
</script>

<section class="settings-section">
	<SettingsSectionHeader
		titleKey="stt_title"
		descriptionKey="stt_description"
		helpTitleKey="help_stt_title"
		helpDescriptionKey="help_stt_description"
		helpTutorialKey="help_stt_tutorial"
	/>
	<p class="shortcut-hint">{$i18n('stt_shortcut_help')}</p>

	{#if $sttSettingsLoading}
		<div class="lazy-loading">{$i18n('stt_loading')}</div>
	{:else}
		<Card>
			{#snippet body()}
				<div class="form-grid">
					<label class="toggle-row">
						<input type="checkbox" bind:checked={enabled} />
						<span>{$i18n('stt_enabled_label')}</span>
					</label>
					<p class="form-hint">{$i18n('stt_enabled_help')}</p>

					<Input
						label={$i18n('stt_model_id_label')}
						value={modelId}
						placeholder={DEFAULT_VOXTRAL_MODEL_ID}
						oninput={handleModelInput}
						help={$i18n('stt_model_id_help')}
					/>
					{#if modelError}
						<p class="form-error" role="alert">{$i18n(modelError)}</p>
					{/if}

					<button type="button" class="link-button" onclick={openModelsDoc}>
						<ExternalLink size={14} />
						<span>{$i18n('stt_see_models_link')}</span>
					</button>

					<Select
						label={$i18n('stt_language_label')}
						value={language}
						options={languageOptions.map((opt) => ({ ...opt, label: $i18n(opt.label) }))}
						onchange={(ev) => (language = ev.currentTarget.value as typeof language)}
						help={$i18n('stt_language_help')}
					/>

					<Textarea
						label={$i18n('stt_context_bias_label')}
						value={contextBiasRaw}
						placeholder={$i18n('stt_context_bias_placeholder')}
						rows={5}
						oninput={(e) => (contextBiasRaw = e.currentTarget.value)}
						help={$i18n('stt_context_bias_help')}
					/>

					<div class="actions">
						<Button
							variant="primary"
							onclick={handleSave}
							disabled={$sttSettingsSaving || modelError !== null}
						>
							{$sttSettingsSaving ? $i18n('stt_saving') : $i18n('stt_save')}
						</Button>
						<Button variant="secondary" onclick={handleReset} disabled={$sttSettingsSaving}>
							{$i18n('stt_reset')}
						</Button>
					</div>
				</div>
			{/snippet}
		</Card>
	{/if}
</section>

<style>
	.settings-section :global(.settings-header) {
		margin-bottom: var(--spacing-sm);
	}

	.shortcut-hint {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0 0 var(--spacing-lg);
	}

	.form-grid {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.toggle-row {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-sm);
		font-weight: var(--font-weight-medium);
		cursor: pointer;
	}

	.form-hint {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.form-error {
		color: var(--color-error, #dc2626);
		font-size: var(--font-size-sm);
		margin: 0;
	}

	.link-button {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		background: transparent;
		border: none;
		color: var(--color-accent);
		cursor: pointer;
		padding: 0;
		font-size: var(--font-size-sm);
		text-decoration: underline;
		align-self: flex-start;
	}

	.actions {
		display: flex;
		gap: var(--spacing-md);
		margin-top: var(--spacing-md);
	}
</style>
