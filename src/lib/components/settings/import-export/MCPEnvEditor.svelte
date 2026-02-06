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

MCPEnvEditor - Add missing environment variables for MCP server import.
Displays missing env var keys and provides input fields for values.
Marks sensitive keys as required.
-->

<script lang="ts">
	import { Card, Input, Badge } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { MCPAdditions } from '$types/importExport';
	import { SENSITIVE_ENV_PATTERNS } from '$types/importExport';

	/** Props */
	interface Props {
		/** MCP server ID */
		serverId: string;
		/** MCP server name */
		serverName: string;
		/** List of missing env var keys */
		missingKeys: string[];
		/** Current additions state */
		additions: MCPAdditions;
		/** Change callback */
		onchange: (additions: MCPAdditions) => void;
	}

	let { serverId, serverName, missingKeys, additions, onchange }: Props = $props();

	/**
	 * Check if env key matches sensitive pattern
	 */
	function isSensitiveKey(key: string): boolean {
		return SENSITIVE_ENV_PATTERNS.some((pattern) => key.toUpperCase().includes(pattern));
	}

	/**
	 * Update env var value
	 */
	function updateEnvValue(key: string, value: string): void {
		const newAddEnv = { ...additions.addEnv, [key]: value };
		onchange({
			...additions,
			addEnv: newAddEnv
		});
	}

	/**
	 * Get env value for key
	 */
	function getEnvValue(key: string): string {
		return additions.addEnv[key] || '';
	}

	/**
	 * Check if all required sensitive keys are filled
	 */
	const allRequiredFilled = $derived(
		missingKeys
			.filter(isSensitiveKey)
			.every((key) => additions.addEnv[key]?.trim())
	);

	/**
	 * Count filled vs total keys
	 */
	const filledCount = $derived(
		missingKeys.filter((key) => additions.addEnv[key]?.trim()).length
	);
</script>

<div class="mcp-env-editor">
	<Card>
		{#snippet body()}
			<div class="editor-content">
				<div class="editor-header">
					<div class="server-info">
						<h4>{serverName}</h4>
						<Badge variant="primary">{serverId}</Badge>
					</div>
					<div class="progress-info">
						<span class="progress-text">
							{$i18n('ie_x_of_y_filled').replace('{filled}', String(filledCount)).replace('{total}', String(missingKeys.length))}
						</span>
						{#if allRequiredFilled}
							<Badge variant="success">{$i18n('ie_required_filled')}</Badge>
						{:else}
							<Badge variant="warning">{$i18n('ie_required_missing')}</Badge>
						{/if}
					</div>
				</div>

				<p class="help-text">
					{$i18n('ie_mcp_env_help')}
				</p>

				<div class="env-fields">
					{#each missingKeys as key (key)}
						{@const sensitive = isSensitiveKey(key)}
						<div class="env-field">
							<div class="field-header">
								<label class="field-label" for="env-{serverId}-{key}">
									{key}
									{#if sensitive}
										<span class="required-mark">*</span>
									{/if}
								</label>
								<div class="field-badges">
									{#if sensitive}
										<Badge variant="error">{$i18n('ie_sensitive')}</Badge>
									{:else}
										<Badge variant="primary">{$i18n('ie_optional')}</Badge>
									{/if}
								</div>
							</div>
							<Input
								id="env-{serverId}-{key}"
								type={sensitive ? 'password' : 'text'}
								value={getEnvValue(key)}
								placeholder={$i18n('ie_enter_value_for').replace('{key}', key)}
								required={sensitive}
								oninput={(e) => updateEnvValue(key, e.currentTarget.value)}
							/>
						</div>
					{/each}
				</div>

				{#if !allRequiredFilled}
					<div class="warning-box">
						<Badge variant="warning">{$i18n('ie_warnings')}</Badge>
						<p class="warning-text">
							{$i18n('ie_env_warning')}
						</p>
					</div>
				{/if}
			</div>
		{/snippet}
	</Card>
</div>

<style>
	.mcp-env-editor {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.editor-content {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.editor-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
		gap: var(--spacing-md);
		flex-wrap: wrap;
	}

	.server-info {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.server-info h4 {
		margin: 0;
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
	}

	.progress-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.progress-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.help-text {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
	}

	.env-fields {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.env-field {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.field-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
	}

	.field-label {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.required-mark {
		color: var(--color-error);
		margin-left: var(--spacing-xs);
	}

	.field-badges {
		display: flex;
		gap: var(--spacing-xs);
	}

	.warning-box {
		display: flex;
		gap: var(--spacing-md);
		padding: var(--spacing-md);
		background: var(--color-warning-light);
		border: 1px solid var(--color-warning);
		border-radius: var(--border-radius-md);
	}

	.warning-text {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-warning-dark);
	}

	@media (max-width: 768px) {
		.editor-header {
			flex-direction: column;
			align-items: stretch;
		}

		.progress-info {
			flex-direction: column;
			align-items: flex-start;
		}
	}
</style>
