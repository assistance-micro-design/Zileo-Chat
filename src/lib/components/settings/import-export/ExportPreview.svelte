<!--
Copyright 2025 Zileo-Chat-3 Contributors
SPDX-License-Identifier: Apache-2.0

ExportPreview - Preview what will be exported.
Shows summaries for each entity type and MCP sanitization options.
-->

<script lang="ts">
	import { Card, Badge } from '$lib/components/ui';
	import MCPFieldEditor from './MCPFieldEditor.svelte';
	import type { ExportPreviewData, MCPSanitizationConfig } from '$types';

	/** Props */
	interface Props {
		/** Preview data returned from backend */
		preview: ExportPreviewData;
		/** MCP sanitization configuration per server */
		mcpSanitization: Record<string, MCPSanitizationConfig>;
		/** Callback when MCP sanitization changes */
		onMcpSanitizationChange: (serverId: string, config: MCPSanitizationConfig) => void;
	}

	let { preview, mcpSanitization, onMcpSanitizationChange }: Props = $props();

	/** Total entity count */
	const totalCount = $derived(
		preview.agents.length +
			preview.mcpServers.length +
			preview.models.length +
			preview.prompts.length
	);

	/** Whether to show expanded sections */
	let expandedAgents = $state(false);
	let expandedMcp = $state(false);
	let expandedModels = $state(false);
	let expandedPrompts = $state(false);
</script>

<div class="export-preview">
	<div class="preview-header">
		<h3 class="preview-title">Export Preview</h3>
		<Badge variant="primary">{totalCount} total items</Badge>
	</div>

	<!-- Agents Section -->
	{#if preview.agents.length > 0}
		<Card>
			{#snippet header()}
				<button
					type="button"
					class="section-header"
					onclick={() => (expandedAgents = !expandedAgents)}
				>
					<div class="section-title">
						<span class="title-text">Agents</span>
						<Badge variant="primary">{preview.agents.length}</Badge>
					</div>
					<span class="expand-icon" class:expanded={expandedAgents}>▼</span>
				</button>
			{/snippet}
			{#snippet body()}
				{#if expandedAgents}
					<div class="items-list">
						{#each preview.agents as agent (agent.id)}
							<div class="item">
								<span class="item-name">{agent.name}</span>
								<div class="item-meta">
									<span class="meta-text">{agent.provider} / {agent.model}</span>
									<Badge variant="success">{agent.lifecycle}</Badge>
								</div>
							</div>
						{/each}
					</div>
				{/if}
			{/snippet}
		</Card>
	{/if}

	<!-- MCP Servers Section -->
	{#if preview.mcpServers.length > 0}
		<Card>
			{#snippet header()}
				<button
					type="button"
					class="section-header"
					onclick={() => (expandedMcp = !expandedMcp)}
				>
					<div class="section-title">
						<span class="title-text">MCP Servers</span>
						<Badge variant="primary">{preview.mcpServers.length}</Badge>
					</div>
					<span class="expand-icon" class:expanded={expandedMcp}>▼</span>
				</button>
			{/snippet}
			{#snippet body()}
				{#if expandedMcp}
					<div class="mcp-list">
						{#each preview.mcpServers as server (server.id ?? server.name)}
							{@const serverId = server.id ?? server.name}
							{@const sanitization = mcpSanitization[serverId]}
							{@const envKeys = preview.mcpEnvKeys[serverId] || []}

							{#if !sanitization?.excludeFromExport}
								<div class="mcp-item">
									<div class="item">
										<span class="item-name">{server.name}</span>
										<div class="item-meta">
											<Badge variant={server.enabled ? 'success' : 'error'}>
												{server.enabled ? 'Enabled' : 'Disabled'}
											</Badge>
											<span class="meta-text">{server.command}</span>
											{#if server.toolsCount > 0}
												<span class="meta-text">{server.toolsCount} tools</span>
											{/if}
										</div>
									</div>

									{#if envKeys.length > 0 && sanitization}
										<MCPFieldEditor
											serverId={serverId}
											serverName={server.name}
											{envKeys}
											{sanitization}
											onchange={(config) => onMcpSanitizationChange(serverId, config)}
										/>
									{/if}
								</div>
							{:else}
								<div class="excluded-item">
									<span class="item-name">{server.name}</span>
									<Badge variant="error">Excluded from export</Badge>
								</div>
							{/if}
						{/each}
					</div>
				{/if}
			{/snippet}
		</Card>
	{/if}

	<!-- Models Section -->
	{#if preview.models.length > 0}
		<Card>
			{#snippet header()}
				<button
					type="button"
					class="section-header"
					onclick={() => (expandedModels = !expandedModels)}
				>
					<div class="section-title">
						<span class="title-text">Models</span>
						<Badge variant="primary">{preview.models.length}</Badge>
					</div>
					<span class="expand-icon" class:expanded={expandedModels}>▼</span>
				</button>
			{/snippet}
			{#snippet body()}
				{#if expandedModels}
					<div class="items-list">
						{#each preview.models as model (model.id)}
							<div class="item">
								<span class="item-name">{model.name}</span>
								<div class="item-meta">
									<span class="meta-text">{model.provider}</span>
									<span class="meta-text">{model.apiName}</span>
									{#if model.isBuiltin}
										<Badge variant="success">Built-in</Badge>
									{:else}
										<Badge variant="warning">Custom</Badge>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				{/if}
			{/snippet}
		</Card>
	{/if}

	<!-- Prompts Section -->
	{#if preview.prompts.length > 0}
		<Card>
			{#snippet header()}
				<button
					type="button"
					class="section-header"
					onclick={() => (expandedPrompts = !expandedPrompts)}
				>
					<div class="section-title">
						<span class="title-text">Prompts</span>
						<Badge variant="primary">{preview.prompts.length}</Badge>
					</div>
					<span class="expand-icon" class:expanded={expandedPrompts}>▼</span>
				</button>
			{/snippet}
			{#snippet body()}
				{#if expandedPrompts}
					<div class="items-list">
						{#each preview.prompts as prompt (prompt.id)}
							<div class="item">
								<span class="item-name">{prompt.name}</span>
								<div class="item-meta">
									<Badge variant="primary">{prompt.category}</Badge>
									{#if prompt.description}
										<span class="meta-text">{prompt.description}</span>
									{/if}
								</div>
							</div>
						{/each}
					</div>
				{/if}
			{/snippet}
		</Card>
	{/if}

	{#if totalCount === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<p>No items selected for export</p>
				</div>
			{/snippet}
		</Card>
	{/if}
</div>

<style>
	.export-preview {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.preview-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
		padding-bottom: var(--spacing-sm);
		border-bottom: 2px solid var(--color-border);
	}

	.preview-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		color: var(--color-text-primary);
	}

	.section-header {
		display: flex;
		justify-content: space-between;
		align-items: center;
		width: 100%;
		padding: 0;
		background: none;
		border: none;
		cursor: pointer;
		gap: var(--spacing-md);
		transition: opacity 0.2s;
	}

	.section-header:hover {
		opacity: 0.8;
	}

	.section-title {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.title-text {
		font-size: var(--font-size-md);
		font-weight: var(--font-weight-semibold);
		color: var(--color-text-primary);
	}

	.expand-icon {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		transition: transform 0.2s;
	}

	.expand-icon.expanded {
		transform: rotate(180deg);
	}

	.items-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-sm);
	}

	.item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-secondary);
	}

	.item-name {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
		color: var(--color-text-primary);
	}

	.item-meta {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
	}

	.meta-text {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
	}

	.mcp-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.mcp-item {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.excluded-item {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: var(--spacing-md);
		padding: var(--spacing-sm);
		border-radius: var(--border-radius-sm);
		background: var(--color-bg-secondary);
		opacity: 0.6;
	}

	.empty-state {
		padding: var(--spacing-lg);
		text-align: center;
	}

	.empty-state p {
		margin: 0;
		color: var(--color-text-secondary);
		font-size: var(--font-size-sm);
	}

	@media (max-width: 768px) {
		.item {
			flex-direction: column;
			align-items: flex-start;
		}

		.item-meta {
			flex-direction: column;
			align-items: flex-start;
		}
	}
</style>
