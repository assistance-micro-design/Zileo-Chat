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

AgentHeader Component - Phase C Component Extraction
Displays workflow title, agent selector, iteration controls, and context information.
-->

<script lang="ts">
	import { Bot } from '@lucide/svelte';
	import AgentSelector from '$lib/components/workflow/AgentSelector.svelte';
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
		onagentchange: (agentId: string) => void;
		oniterationschange: (value: number) => void;
	}

	let {
		workflow,
		agents,
		selectedAgentId,
		maxIterations,
		agentsLoading = false,
		messagesLoading = false,
		onagentchange,
		oniterationschange
	}: Props = $props();

	function handleIterationsInput(e: Event) {
		const target = e.target as HTMLInputElement;
		const value = Math.max(1, Math.min(200, parseInt(target.value) || 50));
		oniterationschange(value);
	}
</script>

<header class="agent-header">
	<div class="header-content">
		<Bot size={24} class="agent-icon" />
		<h2 class="agent-title">{workflow?.name ?? $i18n('agent_header_default_title')}</h2>
		<HelpButton
			titleKey="help_agent_header_title"
			descriptionKey="help_agent_header_description"
			tutorialKey="help_agent_header_tutorial"
		/>
		<span class="header-separator"></span>

		{#if agentsLoading}
			<span class="agents-loading">{$i18n('agent_header_loading')}</span>
		{:else if agents.length === 0}
			<span class="no-agents">
				<a href="/settings" class="settings-link">{$i18n('agent_header_add_agent')}</a>
			</span>
		{:else}
			<div class="agent-controls">
				<AgentSelector
					{agents}
					selected={selectedAgentId ?? agents[0]?.id ?? ''}
					onselect={onagentchange}
					label=""
				/>
				<div class="iterations-control">
					<label for="max-iterations" class="iterations-label">{$i18n('agent_header_iterations')}</label>
					<input
						type="number"
						id="max-iterations"
						class="iterations-input"
						min="1"
						max="200"
						value={maxIterations}
						oninput={handleIterationsInput}
						title={$i18n('agent_header_iterations_tooltip')}
					/>
				</div>
			</div>
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
		padding: var(--spacing-md) var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: linear-gradient(
			180deg,
			var(--color-bg-secondary) 0%,
			var(--color-bg-tertiary) 100%
		);
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 56px;
	}

	.header-content {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		max-width: 100%;
	}

	.header-content :global(.agent-icon) {
		color: var(--color-accent);
		flex-shrink: 0;
	}

	.agent-title {
		font-size: var(--font-size-lg);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: clamp(80px, 18vw, 200px);
	}

	.header-separator {
		width: 1px;
		height: 20px;
		background: var(--color-border);
		flex-shrink: 0;
	}

	.agent-controls {
		display: flex;
		align-items: center;
		gap: var(--spacing-md);
		flex-shrink: 0;
	}

	.iterations-control {
		display: flex;
		align-items: center;
		gap: var(--spacing-xs);
		padding-left: var(--spacing-sm);
		border-left: 1px solid var(--color-border);
	}

	.iterations-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.iterations-input {
		width: 52px;
		padding: var(--spacing-xs);
		font-size: var(--font-size-sm);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		background: var(--color-bg-primary);
		color: var(--color-text-primary);
		text-align: center;
	}

	.iterations-input:focus {
		outline: none;
		border-color: var(--color-accent);
		box-shadow: 0 0 0 2px var(--color-accent-light);
	}

	.iterations-input::-webkit-inner-spin-button,
	.iterations-input::-webkit-outer-spin-button {
		opacity: 1;
	}

	.loading-indicator {
		display: flex;
		align-items: center;
		margin-left: var(--spacing-sm);
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

	.agents-loading,
	.no-agents {
		font-size: var(--font-size-sm);
		color: var(--color-text-tertiary);
	}

	.settings-link {
		color: var(--color-accent);
		text-decoration: underline;
	}

	.settings-link:hover {
		color: var(--color-accent-hover);
	}

	/* Responsive: Medium screens - tighter spacing */
	@media (max-width: 900px) {
		.agent-header {
			padding: var(--spacing-sm) var(--spacing-md);
		}

		.header-content {
			gap: var(--spacing-xs);
		}

		.agent-title {
			max-width: clamp(60px, 12vw, 120px);
			font-size: var(--font-size-base);
		}

		.header-separator {
			display: none;
		}

		.iterations-label {
			display: none;
		}

		.iterations-control {
			border-left: none;
			padding-left: 0;
		}

		.agent-controls {
			gap: var(--spacing-sm);
		}
	}

	/* Responsive: Small screens - stack vertically */
	@media (max-width: 550px) {
		.agent-header {
			padding: var(--spacing-sm);
			min-height: auto;
		}

		.header-content {
			flex-direction: column;
			gap: var(--spacing-xs);
		}

		.agent-title {
			max-width: 180px;
		}

		.agent-controls {
			flex-wrap: wrap;
			justify-content: center;
		}
	}
</style>
