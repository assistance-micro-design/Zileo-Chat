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

EmbeddingConfigCard - Inline editing of the embedding configuration.
Provider/model selects live in the card body; the footer carries the
delete (when a config exists) and save actions.
-->

<script lang="ts">
	import { Card, Button, Select, Badge } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import type { EmbeddingConfig } from '$types/embedding';
	import { Check } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	interface Props {
		/** Embedding configuration being edited */
		config: EmbeddingConfig;
		/** Whether a config has been saved */
		configExists: boolean;
		/** Whether a save is in flight */
		saving: boolean;
		/** Provider options */
		providerOptions: SelectOption[];
		/** Model options for the selected provider */
		modelOptions: SelectOption[];
		/** Provider select change handler */
		onProviderChange: (event: Event & { currentTarget: HTMLSelectElement }) => void;
		/** Model select change handler */
		onModelChange: (event: Event & { currentTarget: HTMLSelectElement }) => void;
		/** Callback to save the config */
		onSave: () => void;
		/** Callback to delete the config */
		onDelete: () => void;
	}

	let {
		config,
		configExists,
		saving,
		providerOptions,
		modelOptions,
		onProviderChange,
		onModelChange,
		onSave,
		onDelete
	}: Props = $props();
</script>

<Card title={$i18n('memory_embedding_config')} description={$i18n('memory_config_subtitle')}>
	{#snippet body()}
		<div class="config-form">
			<Select
				label={$i18n('memory_provider')}
				options={providerOptions}
				value={config.provider}
				onchange={onProviderChange}
				help={$i18n('memory_select_provider_help')}
			/>
			<div class="model-field">
				<Select
					label={$i18n('memory_model')}
					options={modelOptions}
					value={config.model}
					onchange={onModelChange}
					help={config.provider === 'mistral'
						? $i18n('memory_mistral_help')
						: $i18n('memory_ollama_help')}
				/>
				<div class="status-line">
					<Badge variant={configExists ? 'success' : 'warning'}>
						{#if configExists}
							<Check size={12} aria-hidden="true" />
						{/if}
						{configExists
							? $i18n('memory_status_configured')
							: $i18n('memory_status_not_configured')}
					</Badge>
				</div>
			</div>
		</div>
	{/snippet}
	{#snippet footer()}
		{#if configExists}
			<Button variant="danger-soft" size="sm" onclick={onDelete} disabled={saving}>
				{$i18n('common_delete')}
			</Button>
		{/if}
		<Button variant="primary" size="sm" onclick={onSave} disabled={saving}>
			{saving ? $i18n('common_saving') : $i18n('memory_save_config')}
		</Button>
	{/snippet}
</Card>

<style>
	.config-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.model-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.status-line {
		display: flex;
		align-items: center;
	}
</style>
