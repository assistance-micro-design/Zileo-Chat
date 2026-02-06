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
  ReasoningPanel Component
  Displays agent thinking/reasoning steps for a workflow.
  Shows both real-time reasoning (during streaming) and persisted history.

  Phase 4: Thinking Steps Persistence

  @example
  <ReasoningPanel
    steps={thinkingSteps}
    isStreaming={isStreaming}
    activeSteps={activeReasoningSteps}
  />
-->
<script lang="ts">
	import type { ThinkingStep, ActiveThinkingStep } from '$types/thinking';
	import { formatThinkingDuration, truncateThinkingContent } from '$types/thinking';
	import { Brain, Clock, ChevronDown, ChevronUp, Loader2 } from '@lucide/svelte';
	import { i18n } from '$lib/i18n';

	/**
	 * ReasoningPanel props
	 */
	interface Props {
		/** Persisted thinking steps from database */
		steps?: ThinkingStep[];
		/** Active thinking steps during streaming */
		activeSteps?: ActiveThinkingStep[];
		/** Whether streaming is active */
		isStreaming?: boolean;
		/** Whether panel is collapsed by default */
		collapsed?: boolean;
		/** Maximum content length for preview (when collapsed) */
		previewLength?: number;
	}

	let {
		steps = [],
		activeSteps = [],
		isStreaming = false,
		collapsed = false,
		previewLength = 200
	}: Props = $props();

	/** Internal expanded state - syncs with collapsed prop */
	let expanded = $derived(!collapsed);

	/** Which step is fully expanded (null = none) */
	let expandedStepId = $state<string | null>(null);

	/**
	 * Toggle panel expansion
	 */
	function toggleExpanded(): void {
		expanded = !expanded;
	}

	/**
	 * Toggle individual step expansion
	 */
	function toggleStepExpanded(stepId: string): void {
		if (expandedStepId === stepId) {
			expandedStepId = null;
		} else {
			expandedStepId = stepId;
		}
	}

	/**
	 * All steps to display (combines active + historical)
	 */
	const displaySteps = $derived.by(() => {
		const items: Array<{
			id: string;
			stepNumber: number;
			content: string;
			durationMs?: number;
			tokens?: number;
			isActive: boolean;
			timestamp?: number;
		}> = [];

		// Add active steps first (during streaming)
		for (const step of activeSteps) {
			items.push({
				id: `active-${step.stepNumber}-${step.timestamp}`,
				stepNumber: step.stepNumber,
				content: step.content,
				durationMs: step.durationMs,
				isActive: true,
				timestamp: step.timestamp
			});
		}

		// Add persisted steps (from database)
		for (const step of steps) {
			items.push({
				id: step.id,
				stepNumber: step.step_number + 1, // Display as 1-indexed
				content: step.content,
				durationMs: step.duration_ms,
				tokens: step.tokens,
				isActive: false
			});
		}

		// Sort by step number
		items.sort((a, b) => a.stepNumber - b.stepNumber);

		return items;
	});

	/**
	 * Count of total steps
	 */
	const stepCount = $derived(displaySteps.length);

	/**
	 * Total duration across all steps
	 */
	const totalDuration = $derived(
		displaySteps.reduce((sum, step) => sum + (step.durationMs ?? 0), 0)
	);

	/**
	 * Total tokens across all steps
	 */
	const totalTokens = $derived(
		displaySteps.reduce((sum, step) => sum + (step.tokens ?? 0), 0)
	);

	/**
	 * Whether there are any steps to show
	 */
	const hasSteps = $derived(stepCount > 0 || isStreaming);

	/**
	 * Check if content needs truncation
	 */
	function needsTruncation(content: string): boolean {
		return content.length > previewLength;
	}

	/**
	 * Get display content (truncated or full)
	 */
	function getDisplayContent(stepId: string, content: string): string {
		if (expandedStepId === stepId || !needsTruncation(content)) {
			return content;
		}
		return truncateThinkingContent(content, previewLength);
	}
</script>

{#if hasSteps}
	<div class="reasoning-panel" class:expanded>
		<!-- Header -->
		<button
			class="panel-header"
			onclick={toggleExpanded}
			aria-expanded={expanded}
			aria-controls="reasoning-step-list"
		>
			<div class="header-left">
				<Brain size={16} class="panel-icon" />
				<span class="panel-title">{$i18n('workflow_reasoning_title')}</span>
				<span class="step-count">
					{stepCount} {stepCount !== 1 ? $i18n('workflow_reasoning_steps') : $i18n('workflow_reasoning_step')}
				</span>
				{#if totalDuration > 0}
					<span class="duration-badge">
						<Clock size={10} />
						{formatThinkingDuration(totalDuration)}
					</span>
				{/if}
				{#if totalTokens > 0}
					<span class="token-badge">{totalTokens} {$i18n('workflow_reasoning_tokens')}</span>
				{/if}
			</div>
			<div class="header-right">
				{#if isStreaming}
					<Loader2 size={14} class="streaming-indicator" />
				{/if}
				{#if expanded}
					<ChevronUp size={16} />
				{:else}
					<ChevronDown size={16} />
				{/if}
			</div>
		</button>

		<!-- Step List -->
		{#if expanded}
			<div id="reasoning-step-list" class="step-list" role="list">
				{#each displaySteps as step (step.id)}
					<div
						class="step-item"
						class:active={step.isActive}
						class:expandable={needsTruncation(step.content)}
						role="listitem"
					>
						<div class="step-header">
							<span class="step-number">{step.stepNumber}.</span>
							{#if step.durationMs !== undefined}
								<span class="step-duration">
									{formatThinkingDuration(step.durationMs)}
								</span>
							{/if}
							{#if step.tokens !== undefined && step.tokens > 0}
								<span class="step-tokens">{step.tokens} tok</span>
							{/if}
							{#if step.isActive}
								<Loader2 size={12} class="active-indicator" />
							{/if}
						</div>
						<div class="step-content">
							{#if needsTruncation(step.content)}
								<button
									class="content-toggle"
									onclick={() => toggleStepExpanded(step.id)}
									aria-expanded={expandedStepId === step.id}
								>
									<p class="content-text">
										{getDisplayContent(step.id, step.content)}
									</p>
									<span class="toggle-hint">
										{expandedStepId === step.id ? $i18n('workflow_reasoning_show_less') : $i18n('workflow_reasoning_show_more')}
									</span>
								</button>
							{:else}
								<p class="content-text">{step.content}</p>
							{/if}
						</div>
					</div>
				{/each}
			</div>
		{/if}
	</div>
{/if}

<style>
	.reasoning-panel {
		background: var(--color-bg-secondary);
		border: 1px solid var(--color-border);
		border-radius: var(--radius-md);
		overflow: hidden;
		transition: all var(--transition-base, 200ms) ease-out;
	}

	.reasoning-panel.expanded {
		box-shadow: var(--shadow-sm);
	}

	.panel-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		width: 100%;
		padding: var(--spacing-sm) var(--spacing-md);
		background: transparent;
		border: none;
		cursor: pointer;
		font: inherit;
		text-align: left;
		color: var(--color-text-primary);
	}

	.panel-header:hover {
		background: var(--color-bg-tertiary);
	}

	.header-left {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
	}

	.header-left :global(.panel-icon) {
		color: var(--color-accent);
	}

	.panel-title {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-medium);
	}

	.step-count {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.duration-badge {
		display: flex;
		align-items: center;
		gap: 2px;
		font-size: var(--font-size-xs);
		color: var(--color-text-secondary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.token-badge {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		background: var(--color-bg-tertiary);
		padding: 0 var(--spacing-xs);
		border-radius: var(--radius-full);
	}

	.header-right {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		color: var(--color-text-tertiary);
	}

	.header-right :global(.streaming-indicator) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}

	@keyframes spin {
		from { transform: rotate(0deg); }
		to { transform: rotate(360deg); }
	}

	.step-list {
		max-height: 300px;
		overflow-y: auto;
		border-top: 1px solid var(--color-border);
		animation: slideDown 200ms ease-out;
	}

	@keyframes slideDown {
		from {
			opacity: 0;
			max-height: 0;
		}
		to {
			opacity: 1;
			max-height: 300px;
		}
	}

	.step-item {
		padding: var(--spacing-sm) var(--spacing-md);
		border-bottom: 1px solid var(--color-border-light, rgba(0, 0, 0, 0.05));
		animation: fadeInItem 150ms ease-out;
		transition: background-color var(--transition-fast, 150ms) ease;
	}

	@keyframes fadeInItem {
		from {
			opacity: 0;
			transform: translateX(-8px);
		}
		to {
			opacity: 1;
			transform: translateX(0);
		}
	}

	.step-item:last-child {
		border-bottom: none;
	}

	.step-item.active {
		background: var(--color-bg-tertiary);
	}

	.step-header {
		display: flex;
		align-items: center;
		gap: var(--spacing-sm);
		margin-bottom: var(--spacing-xs);
	}

	.step-number {
		font-size: var(--font-size-sm);
		font-weight: var(--font-weight-semibold);
		color: var(--color-accent);
		min-width: 1.5em;
	}

	.step-duration {
		font-size: var(--font-size-xs);
		font-family: var(--font-mono);
		color: var(--color-text-tertiary);
	}

	.step-tokens {
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
	}

	.step-item :global(.active-indicator) {
		animation: spin 1s linear infinite;
		color: var(--color-accent);
	}

	.step-content {
		margin-left: 1.5em;
	}

	.content-toggle {
		display: block;
		width: 100%;
		padding: 0;
		margin: 0;
		background: none;
		border: none;
		cursor: pointer;
		text-align: left;
		font: inherit;
		color: inherit;
	}

	.content-toggle:hover .toggle-hint {
		color: var(--color-accent);
	}

	.content-text {
		font-size: var(--font-size-sm);
		color: var(--color-text-secondary);
		line-height: 1.5;
		margin: 0;
		white-space: pre-wrap;
		word-break: break-word;
	}

	.toggle-hint {
		display: block;
		font-size: var(--font-size-xs);
		color: var(--color-text-tertiary);
		margin-top: var(--spacing-xs);
	}
</style>
