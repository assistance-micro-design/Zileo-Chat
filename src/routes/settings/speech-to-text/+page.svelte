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
	import {
		Card,
		Button,
		Input,
		Textarea,
		Select,
		Switch,
		type SelectOption
	} from '$lib/components/ui';
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
	import { ExternalLink, Info } from '@lucide/svelte';
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
	<p class="shortcut-hint">
		<Info size={14} aria-hidden="true" />
		<span>{$i18n('stt_shortcut_help')}</span>
	</p>

	<div class="stt-card">
		{#if $sttSettingsLoading}
			<div class="lazy-loading">{$i18n('stt_loading')}</div>
		{:else}
			<Card>
				{#snippet body()}
					<div class="stt-form">
						<div class="toggle-row">
							<span class="toggle-text">
								<strong id="stt-enabled-label">{$i18n('stt_enabled_label')}</strong>
								<span>{$i18n('stt_enabled_help')}</span>
							</span>
							<Switch
								checked={enabled}
								onchange={(value) => (enabled = value)}
								labelledBy="stt-enabled-label"
							/>
						</div>

						<div class="model-field">
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
							<Button variant="ghost" size="sm" onclick={openModelsDoc}>
								<ExternalLink size={14} aria-hidden="true" />
								<span>{$i18n('stt_see_models_link')}</span>
							</Button>
						</div>

						<div class="narrow">
							<Select
								label={$i18n('stt_language_label')}
								value={language}
								options={languageOptions.map((opt) => ({ ...opt, label: $i18n(opt.label) }))}
								onchange={(ev) => (language = ev.currentTarget.value as typeof language)}
								help={$i18n('stt_language_help')}
							/>
						</div>

						<Textarea
							label={$i18n('stt_context_bias_label')}
							value={contextBiasRaw}
							placeholder={$i18n('stt_context_bias_placeholder')}
							rows={5}
							oninput={(e) => (contextBiasRaw = e.currentTarget.value)}
							help={$i18n('stt_context_bias_help')}
						/>
					</div>
				{/snippet}
				{#snippet footer()}
					<Button variant="ghost" onclick={handleReset} disabled={$sttSettingsSaving}>
						{$i18n('stt_reset')}
					</Button>
					<Button
						variant="primary"
						onclick={handleSave}
						disabled={$sttSettingsSaving || modelError !== null}
					>
						{$sttSettingsSaving ? $i18n('stt_saving') : $i18n('stt_save')}
					</Button>
				{/snippet}
			</Card>
		{/if}
	</div>
</section>

<style>
	.settings-section :global(.settings-header) {
		margin-bottom: var(--spacing-xs);
	}

	/* Keyboard-shortcut hint: small tertiary line with an info glyph, sitting
	   directly under the section description like the mockup. */
	.shortcut-hint {
		display: inline-flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin: 0 0 var(--spacing-lg);
	}

	.shortcut-hint :global(svg) {
		flex-shrink: 0;
	}

	/* Mockup caps the dictation card at 720px so the form reads as one focused
	   column rather than stretching across the full settings pane. */
	.stt-card {
		max-width: 720px;
	}

	/* The shared .card-footer only carries padding/border/background; the mockup
	   right-aligns its Reset/Save actions, so opt this card's footer into the
	   flex layout locally (scoped to this page, no global side effects). */
	.stt-card :global(.card-footer) {
		display: flex;
		align-items: center;
		justify-content: flex-end;
		gap: var(--spacing-sm);
	}

	.stt-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	/* Field components own a .form-group with a bottom margin; spacing between
	   blocks here is driven by the flex gap, so neutralise the per-field margin. */
	.stt-form :global(.form-group) {
		margin-bottom: 0;
	}

	/* Toggle row: descriptive text block on the left, switch on the right. */
	.toggle-row {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: var(--spacing-lg);
	}

	.toggle-text strong {
		display: block;
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.toggle-text span {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: 2px;
		max-width: 56ch;
	}

	/* Model id cluster: input, optional validation error, and the ghost button
	   linking to the Voxtral model list. */
	.model-field {
		display: flex;
		flex-direction: column;
		align-items: flex-start;
		gap: var(--spacing-xs);
	}

	.model-field :global(.form-group) {
		width: 100%;
	}

	/* The model id is an exact identifier: monospaced and width-capped per the mockup. */
	.model-field :global(.form-input) {
		font-family: var(--font-mono);
		max-width: 320px;
	}

	.narrow :global(.form-select) {
		max-width: 320px;
	}

	.form-error {
		color: var(--color-error);
		font-size: var(--font-size-xs);
		margin: 0;
	}
</style>
