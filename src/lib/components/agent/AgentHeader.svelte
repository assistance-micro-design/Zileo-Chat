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
Discreet workflow header (workflow title + agent selector + iterations popover).
-->

<script lang="ts">
	import { SlidersHorizontal } from '@lucide/svelte';
	import AgentSelector from '$lib/components/workflow/AgentSelector.svelte';
	import { HelpButton } from '$lib/components/ui';
	import { focusTrap } from '$lib/actions/focusTrap';
	import { i18n } from '$lib/i18n';
	import { ITERATIONS_LIMITS } from '$lib/utils/constants';
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

	let iterationsPopoverOpen = $state(false);
	let popoverEl: HTMLDivElement | null = $state(null);

	function handleIterationsInput(e: Event) {
		const target = e.target as HTMLInputElement;
		const value = Math.max(
			ITERATIONS_LIMITS.MIN,
			Math.min(ITERATIONS_LIMITS.MAX, parseInt(target.value) || ITERATIONS_LIMITS.DEFAULT)
		);
		oniterationschange(value);
	}

	function toggleIterationsPopover(): void {
		iterationsPopoverOpen = !iterationsPopoverOpen;
	}

	function closeIterationsPopover(): void {
		iterationsPopoverOpen = false;
	}

	function handlePopoverKeydown(event: KeyboardEvent): void {
		if (event.key === 'Escape') {
			event.preventDefault();
			closeIterationsPopover();
		}
	}

	function handleDocumentClick(event: MouseEvent): void {
		if (!iterationsPopoverOpen) return;
		const target = event.target as Node | null;
		if (popoverEl && target && !popoverEl.contains(target)) {
			closeIterationsPopover();
		}
	}

	$effect(() => {
		if (iterationsPopoverOpen) {
			document.addEventListener('mousedown', handleDocumentClick);
			return () => document.removeEventListener('mousedown', handleDocumentClick);
		}
	});
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
				<div class="iterations-popover-anchor" bind:this={popoverEl}>
					<button
						type="button"
						class="iterations-toggle"
						class:open={iterationsPopoverOpen}
						onclick={toggleIterationsPopover}
						aria-haspopup="dialog"
						aria-expanded={iterationsPopoverOpen}
						aria-label={$i18n('agent_header_iterations_popover_aria')}
						title={`${$i18n('agent_header_iterations_tooltip')} (${maxIterations})`}
					>
						<SlidersHorizontal size={14} />
					</button>

					{#if iterationsPopoverOpen}
						<div
							class="iterations-popover"
							role="dialog"
							tabindex="-1"
							aria-modal="true"
							aria-label={$i18n('agent_header_iterations_label')}
							onkeydown={handlePopoverKeydown}
							{@attach focusTrap}
						>
							<label for="max-iterations" class="iterations-label">
								{$i18n('agent_header_iterations_label')}
							</label>
							<input
								type="number"
								id="max-iterations"
								class="iterations-input"
								min={ITERATIONS_LIMITS.MIN}
								max={ITERATIONS_LIMITS.MAX}
								value={maxIterations}
								oninput={handleIterationsInput}
							/>
							<button
								type="button"
								class="popover-close"
								onclick={closeIterationsPopover}
							>
								{$i18n('agent_header_iterations_popover_close')}
							</button>
						</div>
					{/if}
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
		padding: var(--spacing-xs) var(--spacing-lg);
		border-bottom: 1px solid var(--color-border);
		background: var(--color-bg-secondary);
		display: flex;
		justify-content: center;
		align-items: center;
		min-height: 44px;
	}

	.header-content {
		display: flex;
		align-items: center;
		justify-content: center;
		gap: var(--spacing-sm);
		flex-wrap: wrap;
		max-width: 100%;
	}

	.agent-title {
		font-size: var(--font-size-base);
		font-weight: var(--font-weight-semibold);
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
		max-width: clamp(80px, 18vw, 200px);
	}

	.agent-controls {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		flex-shrink: 0;
	}

	.iterations-popover-anchor {
		position: relative;
		display: inline-flex;
	}

	.iterations-toggle {
		display: inline-flex;
		align-items: center;
		justify-content: center;
		width: 28px;
		height: 28px;
		padding: 0;
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		color: var(--color-text-secondary);
		cursor: pointer;
		transition:
			background-color 0.15s ease,
			color 0.15s ease,
			border-color 0.15s ease;
	}

	.iterations-toggle:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
		border-color: var(--color-text-tertiary);
	}

	.iterations-toggle.open {
		background: var(--color-bg-hover);
		color: var(--color-accent);
		border-color: var(--color-accent);
	}

	.iterations-toggle:focus-visible {
		outline: 2px solid var(--color-accent);
		outline-offset: 2px;
	}

	.iterations-popover {
		position: absolute;
		top: calc(100% + 8px);
		right: 0;
		z-index: 10;
		display: flex;
		flex-direction: column;
		gap: var(--spacing-xs);
		padding: var(--spacing-sm);
		min-width: 180px;
		background: var(--color-bg-primary);
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		box-shadow: 0 4px 12px rgba(0, 0, 0, 0.15);
	}

	.iterations-label {
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		white-space: nowrap;
	}

	.iterations-input {
		width: 100%;
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

	.popover-close {
		margin-top: var(--spacing-xs);
		padding: var(--spacing-xs) var(--spacing-sm);
		font-size: var(--font-size-xs);
		background: transparent;
		border: 1px solid var(--color-border);
		border-radius: var(--border-radius-md);
		color: var(--color-text-secondary);
		cursor: pointer;
	}

	.popover-close:hover {
		background: var(--color-bg-hover);
		color: var(--color-text-primary);
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
			padding: var(--spacing-xs) var(--spacing-md);
		}

		.header-content {
			gap: var(--spacing-xs);
		}

		.agent-title {
			max-width: clamp(60px, 12vw, 120px);
			font-size: var(--font-size-sm);
		}

		.agent-controls {
			gap: var(--spacing-xs);
		}
	}

	/* Responsive: Small screens - stack vertically */
	@media (max-width: 550px) {
		.agent-header {
			padding: var(--spacing-xs);
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

		.iterations-popover {
			right: auto;
			left: 50%;
			transform: translateX(-50%);
		}
	}
</style>
