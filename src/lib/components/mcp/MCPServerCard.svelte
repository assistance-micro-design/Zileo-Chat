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

MCPServerCard Component
Displays an MCP server with status, command info, and action buttons.

@example
<MCPServerCard
  server={mcpServer}
  testing={false}
  onEdit={() => handleEdit(server)}
  onTest={() => handleTest(server)}
  onToggle={() => handleToggle(server)}
  onDelete={() => handleDelete(server)}
/>
-->
<script lang="ts">
	import type { MCPServer, MCPServerStatus } from '$types/mcp';
	import { Card, Button, Badge } from '$lib/components/ui';
	import { Pencil, Play, Square, Trash2, TestTube2, Box, Terminal, Globe } from 'lucide-svelte';
	import { i18n, t } from '$lib/i18n';

	/**
	 * MCPServerCard props
	 */
	interface Props {
		/** MCP server data */
		server: MCPServer;
		/** Whether a test is in progress for this server */
		testing?: boolean;
		/** Handler for edit action */
		onEdit?: () => void;
		/** Handler for test connection action */
		onTest?: () => void;
		/** Handler for start/stop toggle action */
		onToggle?: () => void;
		/** Handler for delete action */
		onDelete?: () => void;
	}

	let {
		server,
		testing = false,
		onEdit,
		onTest,
		onToggle,
		onDelete
	}: Props = $props();

	/**
	 * Maps MCPServerStatus to StatusIndicator-compatible status
	 */
	function getStatusVariant(status: MCPServerStatus): 'success' | 'warning' | 'error' | 'primary' {
		switch (status) {
			case 'running':
				return 'success';
			case 'starting':
				return 'warning';
			case 'error':
			case 'disconnected':
				return 'error';
			case 'stopped':
			default:
				return 'primary';
		}
	}

	/**
	 * Gets human-readable status label
	 */
	function getStatusLabel(status: MCPServerStatus): string {
		switch (status) {
			case 'running':
				return t('mcp_card_status_running');
			case 'starting':
				return t('mcp_card_status_starting');
			case 'stopped':
				return t('mcp_card_status_stopped');
			case 'error':
				return t('mcp_card_status_error');
			case 'disconnected':
				return t('mcp_card_status_disconnected');
			default:
				return t('mcp_card_status_unknown');
		}
	}


	/**
	 * Formats the command display string
	 */
	function formatCommand(server: MCPServer): string {
		const args = server.args.slice(0, 3).join(' ');
		const truncated = server.args.length > 3 ? '...' : '';
		return `${server.command} ${args}${truncated}`;
	}

	/** Computed values */
	const statusVariant = $derived(getStatusVariant(server.status));
	const statusLabel = $derived(getStatusLabel(server.status));
	const commandDisplay = $derived(formatCommand(server));
	const isRunning = $derived(server.status === 'running');
	const isStarting = $derived(server.status === 'starting');
	const toolCount = $derived(server.tools?.length ?? 0);
	const resourceCount = $derived(server.resources?.length ?? 0);
</script>

<Card>
	{#snippet header()}
		<div class="server-header">
			<div class="server-info">
				{#if server.command === 'docker'}
					<Box size={20} class="server-icon" />
				{:else if server.command === 'http'}
					<Globe size={20} class="server-icon" />
				{:else}
					<Terminal size={20} class="server-icon" />
				{/if}
				<div class="server-details">
					<h3 class="server-name">{server.name}</h3>
					{#if server.description}
						<p class="server-description">{server.description}</p>
					{/if}
				</div>
			</div>
			<Badge variant={statusVariant}>{statusLabel}</Badge>
		</div>
	{/snippet}

	{#snippet body()}
		<div class="server-body">
			<div class="command-line">
				<code class="command-text">{commandDisplay}</code>
			</div>

			<div class="server-stats">
				<div class="stat-item">
					<span class="stat-label">{$i18n('mcp_card_tools')}</span>
					<span class="stat-value">{toolCount}</span>
				</div>
				<div class="stat-item">
					<span class="stat-label">{$i18n('mcp_card_resources')}</span>
					<span class="stat-value">{resourceCount}</span>
				</div>
				{#if !server.enabled}
					<div class="stat-item disabled-indicator">
						<span class="stat-value">{$i18n('mcp_card_disabled')}</span>
					</div>
				{/if}
			</div>
		</div>
	{/snippet}

	{#snippet footer()}
		<div class="server-actions">
			<Button
				variant="ghost"
				size="sm"
				onclick={onEdit}
				disabled={isStarting}
				ariaLabel={$i18n('mcp_card_edit_arialabel').replace('{name}', server.name)}
			>
				<Pencil size={16} />
				<span>{$i18n('mcp_card_edit')}</span>
			</Button>

			<Button
				variant="ghost"
				size="sm"
				onclick={onTest}
				disabled={testing || isStarting}
				ariaLabel={$i18n('mcp_card_test_arialabel').replace('{name}', server.name)}
			>
				<TestTube2 size={16} />
				<span>{testing ? $i18n('mcp_card_testing') : $i18n('mcp_card_test')}</span>
			</Button>

			<Button
				variant={isRunning ? 'secondary' : 'primary'}
				size="sm"
				onclick={onToggle}
				disabled={isStarting || !server.enabled}
				ariaLabel={isRunning ? $i18n('mcp_card_stop_arialabel').replace('{name}', server.name) : $i18n('mcp_card_start_arialabel').replace('{name}', server.name)}
			>
				{#if isRunning}
					<Square size={16} />
					<span>{$i18n('mcp_card_stop')}</span>
				{:else}
					<Play size={16} />
					<span>{$i18n('mcp_card_start')}</span>
				{/if}
			</Button>

			<Button
				variant="danger"
				size="sm"
				onclick={onDelete}
				disabled={isRunning || isStarting}
				ariaLabel={$i18n('mcp_card_delete_arialabel').replace('{name}', server.name)}
			>
				<Trash2 size={16} />
			</Button>
		</div>
	{/snippet}
</Card>

<style>
	.server-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		gap: var(--spacing-md);
	}

	.server-info {
		display: flex;
		align-items: flex-start;
		gap: var(--spacing-md);
	}

	.server-info :global(.server-icon) {
		color: var(--color-accent);
		flex-shrink: 0;
		margin-top: 2px;
	}

	.server-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.server-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.server-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		margin: 0;
	}

	.server-body {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.command-line {
		background: var(--color-bg-secondary);
		padding: var(--spacing-sm) var(--spacing-md);
		border-radius: var(--border-radius-md);
		overflow-x: auto;
	}

	.command-text {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.server-stats {
		display: flex;
		gap: var(--spacing-lg);
	}

	.stat-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.stat-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		text-transform: uppercase;
		letter-spacing: 0.05em;
	}

	.stat-value {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
	}

	.disabled-indicator .stat-value {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		font-style: italic;
	}

	.server-actions {
		display: flex;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.server-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}
</style>
