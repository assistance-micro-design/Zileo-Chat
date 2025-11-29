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
	import { Plus, X } from 'lucide-svelte';

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
		id: server?.id ?? generateId(),
		name: server?.name ?? '',
		enabled: server?.enabled ?? true,
		command: server?.command ?? 'docker',
		args: server?.args?.join('\n') ?? '',
		env: server?.env
			? Object.entries(server.env).map(([key, value]) => ({ key, value }))
			: [],
		description: server?.description ?? ''
	});

	/**
	 * Validation errors state
	 */
	let errors = $state<{
		name?: string;
		args?: string;
		env?: string;
	}>({});

	/** Command options for select */
	const commandOptions: SelectOption[] = [
		{ value: 'docker', label: 'Docker' },
		{ value: 'npx', label: 'NPX (Node.js)' },
		{ value: 'uvx', label: 'UVX (Python)' }
	];

	/**
	 * Validates form data
	 * @returns True if valid
	 */
	function validate(): boolean {
		const newErrors: typeof errors = {};

		// Name validation
		if (!formData.name.trim()) {
			newErrors.name = 'Server name is required';
		} else if (!/^[a-zA-Z0-9_-]+$/.test(formData.name)) {
			newErrors.name = 'Name must contain only letters, numbers, underscores, and hyphens';
		} else if (formData.name.length > 64) {
			newErrors.name = 'Name must be 64 characters or less';
		}

		// Args validation (must have at least one argument for most commands)
		if (!formData.args.trim() && formData.command !== 'docker') {
			newErrors.args = 'At least one argument is required';
		}

		// Environment variables validation
		const envKeys = formData.env.map((e) => e.key).filter((k) => k.trim());
		const uniqueKeys = new Set(envKeys);
		if (envKeys.length !== uniqueKeys.size) {
			newErrors.env = 'Duplicate environment variable keys are not allowed';
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
			label="Server Name"
			value={formData.name}
			oninput={(e) => { formData.name = e.currentTarget.value; }}
			placeholder="my-mcp-server"
			required
			help={errors.name ?? 'Unique identifier (letters, numbers, hyphens, underscores)'}
		/>
		{#if errors.name}
			<span class="error-text">{errors.name}</span>
		{/if}
	</div>

	<div class="form-section">
		<Select
			label="Deployment Method"
			options={commandOptions}
			value={formData.command}
			onchange={handleCommandChange}
			required
			help="How to run the MCP server"
		/>
	</div>

	<div class="form-section">
		<Textarea
			label="Command Arguments"
			value={formData.args}
			oninput={(e) => { formData.args = e.currentTarget.value; }}
			placeholder="run&#10;-i&#10;--rm&#10;my-image:latest"
			rows={4}
			help={errors.args ?? 'One argument per line'}
		/>
		{#if errors.args}
			<span class="error-text">{errors.args}</span>
		{/if}
	</div>

	<div class="form-section">
		<div class="env-header">
			<span class="form-label" id="env-vars-label">Environment Variables</span>
			<Button
				type="button"
				variant="ghost"
				size="sm"
				onclick={addEnvVar}
				ariaLabel="Add environment variable"
			>
				<Plus size={16} />
				<span>Add Variable</span>
			</Button>
		</div>

		{#if formData.env.length === 0}
			<p class="env-empty">No environment variables configured</p>
		{:else}
			<div class="env-list">
				{#each formData.env as envVar, index}
					<div class="env-row">
						<input
							type="text"
							class="form-input env-key"
							value={envVar.key}
							oninput={(e) => { formData.env[index].key = e.currentTarget.value; }}
							placeholder="KEY"
							aria-label="Environment variable key"
						/>
						<span class="env-equals">=</span>
						<input
							type="text"
							class="form-input env-value"
							value={envVar.value}
							oninput={(e) => { formData.env[index].value = e.currentTarget.value; }}
							placeholder="value"
							aria-label="Environment variable value"
						/>
						<button
							type="button"
							class="btn btn-ghost btn-icon env-remove"
							onclick={() => removeEnvVar(index)}
							aria-label="Remove environment variable"
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
			label="Description"
			value={formData.description}
			oninput={(e) => { formData.description = e.currentTarget.value; }}
			placeholder="What does this MCP server provide?"
			rows={2}
			help="Optional description of the server's purpose"
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
				Enable server (start automatically)
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
			Cancel
		</Button>
		<Button
			type="submit"
			variant="primary"
			disabled={saving}
		>
			{#if saving}
				{mode === 'create' ? 'Creating...' : 'Saving...'}
			{:else}
				{mode === 'create' ? 'Create Server' : 'Save Changes'}
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
