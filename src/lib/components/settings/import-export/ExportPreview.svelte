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

ExportPreview - Preview what will be exported.
Shows summaries for each entity type and MCP sanitization options.
Uses ExportEntitySection for collapsible entity sections.
-->

<script lang="ts">
	import { Card, Badge } from '$lib/components/ui';
	import MCPFieldEditor from './MCPFieldEditor.svelte';
	import ExportEntitySection from './ExportEntitySection.svelte';
	import { i18n } from '$lib/i18n';
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
			preview.prompts.length +
			(preview.skills?.length || 0) +
			(preview.customProviders?.length || 0)
	);

	/** Expanded state for all sections */
	let expanded = $state<Record<string, boolean>>({
		agents: false,
		mcp: false,
		models: false,
		prompts: false,
		skills: false,
		customProviders: false
	});
</script>

<div class="export-preview">
	<div class="preview-header">
		<h3 class="preview-title">{$i18n('ie_preview_title')}</h3>
		<Badge variant="primary">{$i18n('ie_total_items').replace('{count}', String(totalCount))}</Badge>
	</div>

	<!-- Agents Section -->
	{#if preview.agents.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_agents')}
			count={preview.agents.length}
			expanded={expanded.agents}
			onToggle={() => (expanded.agents = !expanded.agents)}
		>
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
		</ExportEntitySection>
	{/if}

	<!-- MCP Servers Section (kept inline - has MCPFieldEditor, sanitization, excluded items) -->
	{#if preview.mcpServers.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_mcp_servers')}
			count={preview.mcpServers.length}
			expanded={expanded.mcp}
			onToggle={() => (expanded.mcp = !expanded.mcp)}
		>
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
										{server.enabled ? $i18n('ie_enabled') : $i18n('ie_disabled')}
									</Badge>
									<span class="meta-text">{server.command}</span>
									{#if server.toolsCount > 0}
										<span class="meta-text">{$i18n('ie_x_tools').replace('{count}', String(server.toolsCount))}</span>
									{/if}
								</div>
							</div>

							{#if sanitization && (envKeys.length > 0 || server.authType !== undefined || (server.extraHeaderKeys && server.extraHeaderKeys.length > 0))}
								<MCPFieldEditor
									serverName={server.name}
									{envKeys}
									{sanitization}
									authType={server.authType}
									extraHeaderKeys={server.extraHeaderKeys ?? []}
									onchange={(config) => onMcpSanitizationChange(serverId, config)}
								/>
							{/if}
						</div>
					{:else}
						<div class="excluded-item">
							<span class="item-name">{server.name}</span>
							<Badge variant="error">{$i18n('ie_excluded_from_export')}</Badge>
						</div>
					{/if}
				{/each}
			</div>
		</ExportEntitySection>
	{/if}

	<!-- Models Section -->
	{#if preview.models.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_models')}
			count={preview.models.length}
			expanded={expanded.models}
			onToggle={() => (expanded.models = !expanded.models)}
		>
			<div class="items-list">
				{#each preview.models as model (model.id)}
					<div class="item">
						<span class="item-name">{model.name}</span>
						<div class="item-meta">
							<span class="meta-text">{model.provider}</span>
							<span class="meta-text">{model.apiName}</span>
							{#if model.isBuiltin}
								<Badge variant="success">{$i18n('ie_builtin')}</Badge>
							{:else}
								<Badge variant="warning">{$i18n('ie_custom')}</Badge>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		</ExportEntitySection>
	{/if}

	<!-- Prompts Section -->
	{#if preview.prompts.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_prompts')}
			count={preview.prompts.length}
			expanded={expanded.prompts}
			onToggle={() => (expanded.prompts = !expanded.prompts)}
		>
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
		</ExportEntitySection>
	{/if}

	<!-- Skills Section (v1.1) -->
	{#if preview.skills?.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_skills')}
			count={preview.skills.length}
			expanded={expanded.skills}
			onToggle={() => (expanded.skills = !expanded.skills)}
		>
			<div class="items-list">
				{#each preview.skills as skill (skill.id ?? skill.name)}
					<div class="item">
						<span class="item-name">{skill.name}</span>
						<div class="item-meta">
							<Badge variant="primary">{skill.category}</Badge>
							<Badge variant={skill.enabled ? 'success' : 'error'}>
								{skill.enabled ? $i18n('ie_enabled') : $i18n('ie_disabled')}
							</Badge>
						</div>
					</div>
				{/each}
			</div>
		</ExportEntitySection>
	{/if}

	<!-- Custom Providers Section (v1.1) -->
	{#if preview.customProviders?.length > 0}
		<ExportEntitySection
			title={$i18n('ie_entity_custom_providers')}
			count={preview.customProviders.length}
			expanded={expanded.customProviders}
			onToggle={() => (expanded.customProviders = !expanded.customProviders)}
		>
			<div class="items-list">
				{#each preview.customProviders as provider (provider.id ?? provider.name)}
					<div class="item">
						<span class="item-name">{provider.displayName}</span>
						<div class="item-meta">
							<span class="meta-text">{provider.name}</span>
							<span class="meta-text">{provider.baseUrl}</span>
						</div>
					</div>
				{/each}
			</div>
		</ExportEntitySection>
	{/if}

	{#if totalCount === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<p>{$i18n('ie_no_items_selected')}</p>
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
