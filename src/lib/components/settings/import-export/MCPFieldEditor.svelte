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

MCPFieldEditor - Edit MCP server environment variables before export.
Allows clearing sensitive env vars and excluding servers from export.
-->

<script lang="ts">
	import { Badge } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { MCPSanitizationConfig } from '$types';
	import { SENSITIVE_ENV_PATTERNS } from '$types/import-export';

	/** Props */
	interface Props {
		/** MCP server name */
		serverName: string;
		/** Environment variable keys */
		envKeys: string[];
		/** Current sanitization configuration */
		sanitization: MCPSanitizationConfig;
		/** Callback when sanitization changes */
		onchange: (config: MCPSanitizationConfig) => void;
		/** Optional auth type (v1.2). Drives the auth-section visibility. */
		authType?: 'none' | 'bearer' | 'apikey' | 'basic';
		/** Names of extra HTTP headers attached to the server (v1.2). */
		extraHeaderKeys?: string[];
	}

	let {
		serverName,
		envKeys,
		sanitization,
		onchange,
		authType,
		extraHeaderKeys = []
	}: Props = $props();

	/** Whether the server has an active HTTP auth method. */
	const hasAuth = $derived(authType !== undefined && authType !== 'none');

	/** Whether the server declares any extra HTTP headers. */
	const hasExtraHeaders = $derived(extraHeaderKeys.length > 0);

	function toggleClearAuthMetadata(): void {
		onchange({
			...sanitization,
			clearAuthMetadata: !sanitization.clearAuthMetadata
		});
	}

	function toggleClearExtraHeaders(): void {
		onchange({
			...sanitization,
			clearExtraHeaders: !sanitization.clearExtraHeaders
		});
	}

	/**
	 * Checks if an env key is sensitive
	 */
	function isSensitiveKey(key: string): boolean {
		const upperKey = key.toUpperCase();
		return SENSITIVE_ENV_PATTERNS.some((pattern) => upperKey.includes(pattern));
	}

	/**
	 * Toggles whether to clear an env key
	 */
	function toggleClearKey(key: string): void {
		const newClearKeys = sanitization.clearEnvKeys.includes(key)
			? sanitization.clearEnvKeys.filter((k) => k !== key)
			: [...sanitization.clearEnvKeys, key];

		onchange({
			...sanitization,
			clearEnvKeys: newClearKeys
		});
	}

	/**
	 * Toggles whether to exclude server from export
	 */
	function toggleExclude(): void {
		onchange({
			...sanitization,
			excludeFromExport: !sanitization.excludeFromExport
		});
	}

	/**
	 * Clears all sensitive keys
	 */
	function clearAllSensitive(): void {
		const sensitiveKeys = envKeys.filter(isSensitiveKey);
		onchange({
			...sanitization,
			clearEnvKeys: [...new Set([...sanitization.clearEnvKeys, ...sensitiveKeys])]
		});
	}

	/** Count of sensitive keys */
	const sensitiveCount = $derived(envKeys.filter(isSensitiveKey).length);
</script>

<div class="mcp-field-editor">
	<div class="server-header">
		<h4 class="server-name">{serverName}</h4>
		{#if sensitiveCount > 0}
			<Badge variant="warning">
				{sensitiveCount}
				{sensitiveCount === 1
					? $i18n('ie_sensitive').toLowerCase()
					: $i18n('ie_sensitive').toLowerCase() + 's'}
			</Badge>
		{/if}
	</div>

	<div class="actions">
		<label class="exclude-checkbox">
			<input type="checkbox" checked={sanitization.excludeFromExport} onchange={toggleExclude} />
			<span>{$i18n('ie_exclude_from_export')}</span>
		</label>

		{#if sensitiveCount > 0 && !sanitization.excludeFromExport}
			<button type="button" class="action-link" onclick={clearAllSensitive}>
				{$i18n('ie_clear_all_sensitive')}
			</button>
		{/if}
	</div>

	{#if !sanitization.excludeFromExport}
		{#if envKeys.length === 0}
			<div class="empty-state">
				<p>{$i18n('ie_no_env_vars')}</p>
			</div>
		{:else}
			<div class="env-keys-list">
				<div class="list-header">
					<span class="header-label">{$i18n('ie_env_vars')}</span>
					<span class="header-label">{$i18n('ie_clear_on_export')}</span>
				</div>
				{#each envKeys as key (key)}
					<div class="env-key-item">
						<div class="key-info">
							<span class="key-name">{key}</span>
							{#if isSensitiveKey(key)}
								<Badge variant="warning">{$i18n('ie_sensitive')}</Badge>
							{/if}
						</div>
						<label class="clear-checkbox">
							<input
								type="checkbox"
								checked={sanitization.clearEnvKeys.includes(key)}
								onchange={() => toggleClearKey(key)}
							/>
						</label>
					</div>
				{/each}
			</div>

			{#if sanitization.clearEnvKeys.length > 0}
				<div class="summary">
					<span class="summary-text">
						{$i18n('ie_x_vars_cleared').replace(
							'{count}',
							String(sanitization.clearEnvKeys.length)
						)}
					</span>
				</div>
			{/if}
		{/if}

		{#if hasAuth}
			<div class="auth-section">
				<div class="list-header">
					<span class="header-label">{$i18n('ie_mcp_auth_section')}</span>
				</div>
				<p class="auth-info">
					<strong>{$i18n('ie_mcp_auth_method')}:</strong>
					<span>{authType}</span>
				</p>
				<p class="auth-hint">{$i18n('ie_mcp_auth_secret_never_exported')}</p>
				<label class="exclude-checkbox">
					<input
						type="checkbox"
						checked={sanitization.clearAuthMetadata ?? false}
						onchange={toggleClearAuthMetadata}
					/>
					<span>{$i18n('ie_mcp_clear_auth_metadata')}</span>
				</label>
			</div>
		{/if}

		{#if hasExtraHeaders}
			<div class="auth-section">
				<div class="list-header">
					<span class="header-label">{$i18n('ie_mcp_extra_headers_section')}</span>
				</div>
				<ul class="header-list">
					{#each extraHeaderKeys as key (key)}
						<li>{key}</li>
					{/each}
				</ul>
				<label class="exclude-checkbox">
					<input
						type="checkbox"
						checked={sanitization.clearExtraHeaders ?? false}
						onchange={toggleClearExtraHeaders}
					/>
					<span>{$i18n('ie_mcp_clear_extra_headers')}</span>
				</label>
			</div>
		{/if}
	{/if}
</div>

<style>
	.mcp-field-editor {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		padding: var(--spacing-md);
		background: var(--color-bg-secondary);
	}

	.server-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
	}

	.server-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.actions {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.exclude-checkbox {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		cursor: pointer;
		user-select: none;
	}

	.exclude-checkbox input[type='checkbox'] {
		cursor: pointer;
		width: 16px;
		height: 16px;
		margin: 0;
	}

	.exclude-checkbox span {
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
		font-weight: var(--font-weight-medium);
	}

	.action-link {
		background: none;
		border: none;
		color: var(--color-primary);
		font-size: var(--font-size-sm);
		cursor: pointer;
		padding: 0;
		text-decoration: none;
		text-align: left;
		transition: opacity 0.2s;
	}

	.action-link:hover {
		opacity: 0.8;
		text-decoration: underline;
	}

	.empty-state {
		padding: var(--spacing-md);
		text-align: center;
	}

	.empty-state p {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
	}

	.env-keys-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-sm);
		padding: var(--spacing-sm);
	}

	.list-header {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: var(--spacing-md);
		padding: var(--spacing-xs) var(--spacing-sm);
		border-bottom: 1px solid var(--color-border);
		margin-bottom: var(--spacing-xs);
	}

	.header-label {
		font-size: var(--font-size-xs);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.5px;
	}

	.env-key-item {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: var(--spacing-md);
		align-items: center;
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		transition: background 0.2s;
	}

	.env-key-item:hover {
		background: var(--color-bg-hover);
	}

	.key-info {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.key-name {
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}

	.clear-checkbox {
		display: flex;
		align-items: center;
		cursor: pointer;
	}

	.clear-checkbox input[type='checkbox'] {
		cursor: pointer;
		width: 16px;
		height: 16px;
		margin: 0;
	}

	.summary {
		display: flex;
		justify-content: flex-end;
		padding-top: var(--spacing-sm);
		border-top: 1px solid var(--color-border);
	}

	.summary-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-weight: var(--font-weight-medium);
	}

	.auth-section {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		background: var(--color-bg-primary);
		border-radius: var(--border-radius-sm);
		margin-top: var(--spacing-sm);
	}

	.auth-info {
		margin: 0;
		font-size: var(--font-size-sm);
		color: var(--color-text-primary);
	}

	.auth-hint {
		margin: 0;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.header-list {
		margin: 0;
		padding-left: var(--spacing-lg);
		font-size: var(--font-size-sm);
		font-family: var(--font-mono);
		color: var(--color-text-primary);
	}
</style>
