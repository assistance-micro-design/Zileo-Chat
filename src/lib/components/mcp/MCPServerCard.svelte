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
MCPServerCard Component
Displays an MCP server with status, deployment info, and action buttons.

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
	import { Server, Wrench, Pencil, Zap, Play, Square, Trash2 } from '@lucide/svelte';
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

	let { server, testing = false, onEdit, onTest, onToggle, onDelete }: Props = $props();

	/**
	 * Maps MCPServerStatus to a Badge variant
	 */
	function getStatusVariant(status: MCPServerStatus): 'success' | 'warning' | 'error' | 'neutral' {
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
				return 'neutral';
		}
	}

	/**
	 * Maps MCPServerStatus to the glowing status-dot recipe (global.css)
	 */
	function getStatusDotClass(status: MCPServerStatus): string {
		switch (status) {
			case 'running':
				return 'status-completed';
			case 'starting':
				return 'status-running';
			case 'error':
			case 'disconnected':
				return 'status-error';
			case 'stopped':
			default:
				return 'status-idle';
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

	/** Human-readable deployment method (mirrors the form's select labels). */
	const deploymentLabel = $derived.by(() => {
		switch (server.command) {
			case 'docker':
				return $i18n('mcp_form_deployment_docker');
			case 'npx':
				return $i18n('mcp_form_deployment_npx');
			case 'uvx':
				return $i18n('mcp_form_deployment_uvx');
			case 'http':
				return $i18n('mcp_form_deployment_http');
			default:
				return server.command;
		}
	});

	/** Auth method label appended to the deployment line for HTTP servers. */
	const authLabel = $derived.by(() => {
		if (server.command !== 'http' || !server.authType || server.authType === 'none') {
			return null;
		}
		switch (server.authType) {
			case 'bearer':
				return $i18n('mcp_auth_method_bearer');
			case 'apikey':
				return $i18n('mcp_auth_method_apikey');
			case 'basic':
				return $i18n('mcp_auth_method_basic');
			default:
				return null;
		}
	});

	/** Computed values */
	const statusVariant = $derived(getStatusVariant(server.status));
	const statusDotClass = $derived(getStatusDotClass(server.status));
	const statusLabel = $derived(getStatusLabel(server.status));
	const isRunning = $derived(server.status === 'running');
	const isStarting = $derived(server.status === 'starting');

	/**
	 * Tool/resource counts. An em dash stands in while nothing has been
	 * discovered on a non-running server (counts unknown until it starts).
	 */
	function formatCount(count: number): string {
		return !isRunning && count === 0 ? '—' : String(count);
	}

	const toolCount = $derived(formatCount(server.tools?.length ?? 0));
	const resourceCount = $derived(formatCount(server.resources?.length ?? 0));
</script>

<div class="server-card" class:is-disabled={!server.enabled}>
	<Card hover>
		{#snippet header()}
			<div class="server-name-row">
				<Server size={20} class="server-icon" />
				<span class="server-name">{server.name}</span>
			</div>
			<div class="server-badges">
				<Badge variant={statusVariant}>
					<span class="status-indicator {statusDotClass}"></span>
					{statusLabel}
				</Badge>
				{#if !server.enabled}
					<Badge variant="neutral">{$i18n('mcp_card_disabled')}</Badge>
				{/if}
			</div>
		{/snippet}

		{#snippet body()}
			<div class="server-details">
				<span class="detail-line">
					{deploymentLabel}{#if authLabel}&nbsp;· {$i18n('mcp_card_auth', {
							method: authLabel
						})}{/if}
				</span>
				<span class="detail-line">
					<Wrench size={14} />
					{$i18n('mcp_card_counts', { tools: toolCount, resources: resourceCount })}
				</span>
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
					<Pencil size={14} />
					<span>{$i18n('mcp_card_edit')}</span>
				</Button>

				<Button
					variant="ghost"
					size="sm"
					onclick={onTest}
					disabled={testing || isStarting}
					ariaLabel={$i18n('mcp_card_test_arialabel').replace('{name}', server.name)}
				>
					<Zap size={14} />
					<span>{testing ? $i18n('mcp_card_testing') : $i18n('mcp_card_test')}</span>
				</Button>

				<Button
					variant="ghost"
					size="sm"
					onclick={onToggle}
					disabled={isStarting || !server.enabled}
					ariaLabel={isRunning
						? $i18n('mcp_card_stop_arialabel').replace('{name}', server.name)
						: $i18n('mcp_card_start_arialabel').replace('{name}', server.name)}
				>
					{#if isRunning}
						<Square size={14} />
						<span>{$i18n('mcp_card_stop')}</span>
					{:else}
						<Play size={14} />
						<span>{$i18n('mcp_card_start')}</span>
					{/if}
				</Button>

				<Button
					variant="ghost"
					size="sm"
					onclick={onDelete}
					disabled={isRunning || isStarting}
					ariaLabel={$i18n('mcp_card_delete_arialabel').replace('{name}', server.name)}
				>
					<Trash2 size={14} />
				</Button>
			</div>
		{/snippet}
	</Card>
</div>

<style>
	.server-card.is-disabled {
		opacity: 0.75;
	}

	.server-name-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		min-width: 0;
	}

	.server-name-row :global(.server-icon) {
		color: var(--channel-mcp);
		flex-shrink: 0;
	}

	.server-name {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.server-badges {
		display: flex;
		gap: var(--spacing-xs);
		flex-shrink: 0;
	}

	.server-details {
		display: flex;
		flex-direction: column;
		gap: 5px;
	}

	.detail-line {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.server-actions {
		display: flex;
		gap: var(--spacing-sm);
		align-items: center;
		flex-wrap: wrap;
		flex: 1;
	}

	.server-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	.server-actions :global(button:last-child) {
		margin-left: auto;
	}
</style>
