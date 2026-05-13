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

AgentHeader Component
Read-only single-line header: workflow title + assigned agent + max iterations + link to Settings.
Agent and iterations are configured in Settings > Agents (source of truth).
-->

<script lang="ts">
	import { ExternalLink } from '@lucide/svelte';
	import { HelpButton } from '$lib/components/ui';
	import { i18n } from '$lib/i18n';
	import type { AgentSummary } from '$types/agent';
	import type { Workflow } from '$types/workflow';

	interface Props {
		workflow: Workflow | null;
		agents: AgentSummary[];
		selectedAgentId: string | null;
		maxIterations: number;
		agentsLoading?: boolean;
		messagesLoading?: boolean;
	}

	let {
		workflow,
		agents,
		selectedAgentId,
		maxIterations,
		agentsLoading = false,
		messagesLoading = false
	}: Props = $props();

	let selectedAgentName = $derived(agents.find((a) => a.id === selectedAgentId)?.name ?? null);
</script>

<header class="agent-header">
	<div class="header-content">
		<h2 class="agent-title">{workflow?.name ?? $i18n('agent_header_default_title')}</h2>
		<HelpButton
			titleKey="help_agent_header_title"
			descriptionKey="help_agent_header_description"
			tutorialKey="help_agent_header_tutorial"
		/>

		{#if agentsLoading}
			<span class="meta-text">{$i18n('agent_header_loading')}</span>
		{:else if agents.length === 0}
			<span class="meta-text">
				<a href="/settings/agents" class="settings-link">{$i18n('agent_header_add_agent')}</a>
			</span>
		{:else}
			<span class="separator" aria-hidden="true">·</span>
			<span class="meta-text">
				<span class="meta-label">{$i18n('agent_header_agent_label')}</span>
				<span class="meta-value">{selectedAgentName ?? $i18n('agent_header_unknown_agent')}</span>
			</span>
			<span class="separator" aria-hidden="true">·</span>
			<span class="meta-text" title={$i18n('agent_header_iterations_tooltip')}>
				<span class="meta-label">{$i18n('agent_header_iterations_label')}</span>
				<span class="meta-value">{maxIterations}</span>
			</span>
			<a
				href="/settings/agents"
				class="settings-link edit-link"
				aria-label={$i18n('agent_header_edit_aria')}
				title={$i18n('agent_header_edit_aria')}
			>
				{$i18n('agent_header_edit_link')}
				<ExternalLink size={12} aria-hidden="true" />
			</a>
		{/if}

		{#if messagesLoading}
			<div class="loading-indicator">
				<div class="loading-spinner"></div>
			</div>
		{/if}
	</div>
</header>

<style>
	.agent-header {
		padding: var(--spacing-xs) var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 36px;
	}

	.header-content {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		flex-wrap: nowrap;
		max-width: 100%;
		min-width: 0;
		overflow: hidden;
	}

	.agent-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: clamp(80px, 18vw, 200px);
		flex-shrink: 1;
	}

	.separator {
		color: var(--color-text-tertiary);
		font-size: var(--font-size-sm);
		flex-shrink: 0;
	}

	.meta-text {
		display: inline-flex;
		align-items: baseline;
		gap: 4px;
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		white-space: nowrap;
		min-width: 0;
		flex-shrink: 1;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.meta-label {
		color: var(--color-text-tertiary);
	}

	.meta-value {
		color: var(--color-text-primary);
		font-weight: var(--font-weight-medium);
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.settings-link {
		color: var(--color-accent);
		text-decoration: none;
		font-size: var(--font-size-sm);
	}

	.settings-link:hover {
		color: var(--color-accent-hover);
		text-decoration: underline;
	}

	.settings-link:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 2px;
		border-radius: var(--border-radius-sm);
	}

	.edit-link {
		display: inline-flex;
		align-items: center;
		gap: 3px;
		flex-shrink: 0;
	}

	.loading-indicator {
		display: flex;
		align-items: center;
		margin-left: var(--spacing-sm);
		flex-shrink: 0;
	}

	.loading-spinner {
		width: 14px;
		height: 14px;
		border: 2px solid var(--color-text-tertiary);
		border-top-color: transparent;
		border-radius: 50%;
		animation: spin 1s linear infinite;
	}

	@keyframes spin {
		from {
			transform: rotate(0deg);
		}
		to {
			transform: rotate(360deg);
		}
	}

	/* Responsive: Medium screens — tighter spacing, hide labels */
	@media (max-width: 900px) {
		.agent-header {
			padding: var(--spacing-xs) var(--spacing-md);
		}

		.header-content {
			gap: var(--spacing-xs);
		}

		.agent-title {
			max-width: clamp(60px, 12vw, 120px);
		}
	}

	/* Responsive: Small screens — drop iterations + label prefixes */
	@media (max-width: 550px) {
		.agent-header {
			padding: var(--spacing-xs);
		}

		.agent-title {
			max-width: 140px;
		}

		.meta-label {
			display: none;
		}
	}
</style>
