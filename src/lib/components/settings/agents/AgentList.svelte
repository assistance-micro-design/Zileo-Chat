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

AgentList - Displays agents in a grid of cards.
Shows agent summary with actions for edit and delete.
-->

<script lang="ts">
	import type { AgentSummary } from '$types/agent';
	import { Card, Badge, Button, StatusIndicator } from '$lib/components/ui';
	import { Bot, Wrench, Plug, Edit, Trash2 } from 'lucide-svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * Component props
	 */
	interface Props {
		/** List of agents to display */
		agents: AgentSummary[];
		/** Loading state */
		loading: boolean;
		/** Edit callback */
		onedit: (agentId: string) => void;
		/** Delete callback */
		ondelete: (agentId: string) => void;
	}

	let { agents, loading, onedit, ondelete }: Props = $props();

	/**
	 * Gets badge variant for lifecycle type
	 */
	function getLifecycleVariant(lifecycle: string): 'primary' | 'warning' {
		return lifecycle === 'permanent' ? 'primary' : 'warning';
	}

	/**
	 * Gets localized lifecycle label
	 */
	function getLifecycleLabel(lifecycle: string): string {
		return lifecycle === 'permanent'
			? $i18n('agents_lifecycle_permanent')
			: $i18n('agents_lifecycle_temporary');
	}

	/**
	 * Formats provider name for display
	 */
	function formatProvider(provider: string): string {
		const providers: Record<string, string> = {
			Mistral: 'Mistral AI',
			Ollama: 'Ollama'
		};
		return providers[provider] || provider;
	}
</script>

<div class="agent-list">
	{#if loading}
		<Card>
			{#snippet body()}
				<div class="loading-state">
					<StatusIndicator status="running" />
					<span>{$i18n('agents_loading')}</span>
				</div>
			{/snippet}
		</Card>
	{:else if agents.length === 0}
		<Card>
			{#snippet body()}
				<div class="empty-state">
					<Bot size={48} class="empty-icon" />
					<h3 class="empty-title">{$i18n('agents_no_agents')}</h3>
					<p class="empty-description">
						{$i18n('agents_no_agents_description')}
					</p>
				</div>
			{/snippet}
		</Card>
	{:else}
		<div class="agent-grid">
			{#each agents as agent (agent.id)}
				<Card>
					{#snippet body()}
						<div class="agent-card">
							<div class="agent-header">
								<div class="agent-name-row">
									<Bot size={20} class="agent-icon" />
									<h4 class="agent-name">{agent.name}</h4>
								</div>
								<Badge variant={getLifecycleVariant(agent.lifecycle)}>
									{getLifecycleLabel(agent.lifecycle)}
								</Badge>
							</div>

							<div class="agent-details">
								<div class="detail-row">
									<span class="detail-label">{$i18n('agents_provider')}</span>
									<span class="detail-value">{formatProvider(agent.provider)}</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">{$i18n('agents_model')}</span>
									<span class="detail-value model-value">{agent.model}</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">
										<Wrench size={14} />
										{$i18n('agents_tools')}
									</span>
									<span class="detail-value">{$i18n('agents_tools_enabled', { count: agent.tools_count })}</span>
								</div>
								<div class="detail-row">
									<span class="detail-label">
										<Plug size={14} />
										{$i18n('agents_mcp_servers')}
									</span>
									<span class="detail-value">{$i18n('agents_mcp_configured', { count: agent.mcp_servers_count })}</span>
								</div>
							</div>

							<div class="agent-actions">
								<Button variant="ghost" size="sm" onclick={() => onedit(agent.id)}>
									<Edit size={16} />
									<span>{$i18n('common_edit')}</span>
								</Button>
								<Button variant="danger" size="sm" onclick={() => ondelete(agent.id)}>
									<Trash2 size={16} />
									<span>{$i18n('common_delete')}</span>
								</Button>
							</div>
						</div>
					{/snippet}
				</Card>
			{/each}
		</div>
	{/if}
</div>

<style>
	.agent-list {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-lg);
	}

	.loading-state {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-md);
		padding: var(--spacing-xl);
	}

	.empty-state {
		display: flex;
		flex-direction: column;
		align-items: center;
		text-align: center;
		padding: var(--spacing-2xl);
		gap: var(--spacing-md);
	}

	.empty-state :global(.empty-icon) {
		color: var(--color-text-secondary);
		opacity: 0.5;
	}

	.empty-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.empty-description {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		max-width: 400px;
		margin: 0;
		line-height: var(--line-height-relaxed);
	}

	.agent-grid {
		display: grid;
		grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
		gap: var(--spacing-lg);
	}

	.agent-card {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-md);
	}

	.agent-header {
		display: flex;
		justify-content: space-between;
		align-items: flex-start;
	}

	.agent-name-row {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.agent-name-row :global(.agent-icon) {
		color: var(--color-accent);
	}

	.agent-name {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
	}

	.agent-details {
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
	}

	.detail-row {
		display: flex;
		justify-content: space-between;
		align-items: center;
		font-size: var(--font-size-sm);
	}

	.detail-label {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		color: var(--color-text-secondary);
	}

	.detail-value {
		font-weight: var(--font-weight-medium);
	}

	.model-value {
		font-family: var(--font-family-mono);
		font-size: var(--font-size-xs);
	}

	.agent-actions {
		display: flex;
		gap: var(--spacing-sm);
		justify-content: flex-end;
		padding-top: var(--spacing-md);
		border-top: 1px solid var(--color-border);
	}

	.agent-actions :global(button) {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
	}

	@media (max-width: 768px) {
		.agent-grid {
			grid-template-columns: 1fr;
		}
	}
</style>
