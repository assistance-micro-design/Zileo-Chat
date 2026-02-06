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

MCPServerForm Component
Form for creating and editing MCP server configurations.
Includes validation and environment variable editor.

@example
<MCPServerForm
  mode="create"
  onsave={handleSave}
  oncancel={handleCancel}
/>

<MCPServerForm
  mode="edit"
  server={existingServer}
  onsave={handleSave}
  oncancel={handleCancel}
/>
-->
<script lang="ts">
	import type { MCPServerConfig, MCPDeploymentMethod } from '$types/mcp';
	import { Button, Input, Select, Textarea } from '$lib/components/ui';
	import type { SelectOption } from '$lib/components/ui/Select.svelte';
	import { Plus, X } from '@lucide/svelte';
	import { i18n, t } from '$lib/i18n';

	/**
	 * MCPServerForm props
	 */
	interface Props {
		/** Form mode: create or edit */
		mode: 'create' | 'edit';
		/** Existing server data for edit mode */
		server?: MCPServerConfig;
		/** Handler when form is saved */
		onsave: (config: MCPServerConfig) => void;
		/** Handler when form is cancelled */
		oncancel: () => void;
		/** Whether save is in progress */
		saving?: boolean;
	}

	let {
		mode,
		server,
		onsave,
		oncancel,
		saving = false
	}: Props = $props();

	/**
	 * Generates a unique ID for new servers
	 */
	function generateId(): string {
		return `mcp-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
	}

	/**
	 * Form state initialized from props
	 */
	let formData = $state<{
		id: string;
		name: string;
		enabled: boolean;
		command: MCPDeploymentMethod;
		args: string;
		env: Array<{ key: string; value: string }>;
		description: string;
	}>({
		id: generateId(),
		name: '',
		enabled: true,
		command: 'docker',
		args: '',
		env: [],
		description: ''
	});

	// Sync form data when server prop changes (e.g., switching between edit targets)
	$effect(() => {
		formData = {
			id: server?.id ?? generateId(),
			name: server?.name ?? '',
			enabled: server?.enabled ?? true,
			command: server?.command ?? 'docker',
			args: server?.args?.join('\n') ?? '',
			env: server?.env
				? Object.entries(server.env).map(([key, value]) => ({ key, value }))
				: [],
			description: server?.description ?? ''
		};
		// Reset validation state when server changes
		errors = {};
	});

	/**
	 * Validation errors state
	 */
	let errors = $state<{
		name?: string;
		args?: string;
		env?: string;
	}>({});

	/** Command options for select - reactive to locale changes */
	const commandOptions: SelectOption[] = $derived([
		{ value: 'docker', label: t('mcp_form_deployment_docker') },
		{ value: 'npx', label: t('mcp_form_deployment_npx') },
		{ value: 'uvx', label: t('mcp_form_deployment_uvx') },
		{ value: 'http', label: t('mcp_form_deployment_http') }
	]);

	/**
	 * Validates form data
	 * @returns True if valid
	 */
	function validate(): boolean {
		const newErrors: typeof errors = {};

		// Name validation
		if (!formData.name.trim()) {
			newErrors.name = t('mcp_form_name_required');
		} else if (!/^[a-zA-Z0-9_-]+$/.test(formData.name)) {
			newErrors.name = t('mcp_form_name_format');
		} else if (formData.name.length > 64) {
			newErrors.name = t('mcp_form_name_length');
		}

		// Args validation (must have at least one argument for most commands)
		// HTTP and Docker have different requirements
		if (!formData.args.trim()) {
			if (formData.command === 'http') {
				newErrors.args = t('mcp_form_args_url_required');
			} else if (formData.command !== 'docker') {
				newErrors.args = t('mcp_form_args_required');
			}
		} else if (formData.command === 'http') {
			// Validate HTTP URL format
			const url = formData.args.trim().split('\n')[0];
			if (!/^https?:\/\/.+/.test(url)) {
				newErrors.args = t('mcp_form_args_invalid_url');
			}
		}

		// Environment variables validation
		const envKeys = formData.env.map((e) => e.key).filter((k) => k.trim());
		const uniqueKeys = new Set(envKeys);
		if (envKeys.length !== uniqueKeys.size) {
			newErrors.env = t('mcp_form_env_duplicate');
		}

		errors = newErrors;
		return Object.keys(newErrors).length === 0;
	}

	/**
	 * Handles form submission
	 */
	function handleSubmit(event: Event): void {
		event.preventDefault();

		if (!validate()) {
			return;
		}

		// Parse args from newline-separated string
		const args = formData.args
			.split('\n')
			.map((arg) => arg.trim())
			.filter((arg) => arg.length > 0);

		// Convert env array to object
		const env: Record<string, string> = {};
		for (const item of formData.env) {
			if (item.key.trim()) {
				env[item.key.trim()] = item.value;
			}
		}

		const config: MCPServerConfig = {
			id: formData.id,
			name: formData.name.trim(),
			enabled: formData.enabled,
			command: formData.command,
			args,
			env,
			description: formData.description.trim() || undefined
		};

		onsave(config);
	}

	/**
	 * Adds a new environment variable row
	 */
	function addEnvVar(): void {
		formData.env = [...formData.env, { key: '', value: '' }];
	}

	/**
	 * Removes an environment variable row
	 */
	function removeEnvVar(index: number): void {
		formData.env = formData.env.filter((_, i) => i !== index);
	}

	/**
	 * Handle command change
	 */
	function handleCommandChange(event: Event & { currentTarget: HTMLSelectElement }): void {
		formData.command = event.currentTarget.value as MCPDeploymentMethod;
	}
</script>

<form class="mcp-form" onsubmit={handleSubmit}>
	<div class="form-section">
		<Input
			label={$i18n('mcp_form_name_label')}
			value={formData.name}
			oninput={(e) => { formData.name = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_name_placeholder')}
			required
			help={errors.name ?? $i18n('mcp_form_name_help')}
		/>
		{#if errors.name}
			<span class="error-text">{errors.name}</span>
		{/if}
	</div>

	<div class="form-section">
		<Select
			label={$i18n('mcp_form_deployment_label')}
			options={commandOptions}
			value={formData.command}
			onchange={handleCommandChange}
			required
			help={$i18n('mcp_form_deployment_help')}
		/>
	</div>

	<div class="form-section">
		<Textarea
			label={$i18n('mcp_form_args_label')}
			value={formData.args}
			oninput={(e) => { formData.args = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_args_placeholder')}
			rows={4}
			help={errors.args ?? $i18n('mcp_form_args_help')}
		/>
		{#if errors.args}
			<span class="error-text">{errors.args}</span>
		{/if}
	</div>

	<div class="form-section">
		<div class="env-header">
			<span class="form-label" id="env-vars-label">{$i18n('mcp_form_env_label')}</span>
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={addEnvVar}
				ariaLabel={$i18n('mcp_form_env_add')}
			>
				<Plus size={16} />
				<span>{$i18n('mcp_form_env_add')}</span>
			</Button>
		</div>

		{#if formData.env.length === 0}
			<p class="env-empty">{$i18n('mcp_form_env_empty')}</p>
		{:else}
			<div class="env-list">
				{#each formData.env as envVar, index (index)}
					<div class="env-row">
						<input
							type="text"
							class="form-input env-key"
							value={envVar.key}
							oninput={(e) => { formData.env[index].key = e.currentTarget.value; }}
							placeholder={$i18n('mcp_form_env_key_placeholder')}
							aria-label={$i18n('mcp_form_env_key_arialabel')}
						/>
						<span class="env-equals">=</span>
						<input
							type="text"
							class="form-input env-value"
							value={envVar.value}
							oninput={(e) => { formData.env[index].value = e.currentTarget.value; }}
							placeholder={$i18n('mcp_form_env_value_placeholder')}
							aria-label={$i18n('mcp_form_env_value_arialabel')}
						/>
						<button
							type="button"
							class="btn btn-ghost btn-icon env-remove"
							onclick={() => removeEnvVar(index)}
							aria-label={$i18n('mcp_form_env_remove_arialabel')}
						>
							<X size={16} />
						</button>
					</div>
				{/each}
			</div>
		{/if}
		{#if errors.env}
			<span class="error-text">{errors.env}</span>
		{/if}
	</div>

	<div class="form-section">
		<Textarea
			label={$i18n('mcp_form_description_label')}
			value={formData.description}
			oninput={(e) => { formData.description = e.currentTarget.value; }}
			placeholder={$i18n('mcp_form_description_placeholder')}
			rows={2}
			help={$i18n('mcp_form_description_help')}
		/>
	</div>

	<div class="form-section">
		<div class="checkbox-wrapper">
			<input
				type="checkbox"
				id="mcp-server-enabled"
				checked={formData.enabled}
				onchange={(e) => { formData.enabled = e.currentTarget.checked; }}
			/>
			<label for="mcp-server-enabled" class="checkbox-label">
				{$i18n('mcp_form_enabled_label')}
			</label>
		</div>
	</div>

	<div class="form-actions">
		<Button
			type="button"
			variant="ghost"
			onclick={oncancel}
			disabled={saving}
		>
			{$i18n('common_cancel')}
		</Button>
		<Button
			type="submit"
			variant="primary"
			disabled={saving}
		>
			{#if saving}
				{mode === 'create' ? $i18n('mcp_form_creating') : $i18n('mcp_form_saving')}
			{:else}
				{mode === 'create' ? $i18n('mcp_form_create_server') : $i18n('mcp_form_save_changes')}
			{/if}
		</Button>
	</div>
</form>

<style>
	.mcp-form {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.form-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.env-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.env-header :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.env-empty {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
		padding: var(--spacing-md);
		text-align: center;
		background: var(--color-bg-secondary);
		border-radius: var(--border-radius-md);
	}

	.env-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.env-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.env-key {
		flex: 1;
		max-width: 150px;
		font-family: var(--font-family-mono);
		text-transform: uppercase;
	}

	.env-equals {
		color: var(--color-text-secondary);
		font-family: var(--font-family-mono);
	}

	.env-value {
		flex: 2;
		font-family: var(--font-family-mono);
	}

	.env-remove {
		flex-shrink: 0;
	}

	.checkbox-wrapper {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.checkbox-wrapper input[type='checkbox'] {
		width: 18px;
		height: 18px;
		accent-color: var(--color-accent);
		cursor: pointer;
	}

	.checkbox-label {
		cursor: pointer;
		font-size: var(--font-size-sm);
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: var(--spacing-md);
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.error-text {
		font-size: var(--font-size-sm);
		color: var(--color-error);
	}
</style>
